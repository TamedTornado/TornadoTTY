#![forbid(unsafe_code)]

use std::fmt;
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant};

use tornadotty_test_receipts::{
    ActionName, ActionOutcome, FailureCode, FocusTarget, GeometrySnapshot, LifecycleState,
    MAX_FILE_BYTES, PaneColumn, ReceiptError, ReceiptErrorKind, ReceiptEvent, ReceiptId,
    ReceiptStream, WidgetName,
};

const POLL_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Debug)]
enum Selector {
    ProcessStarted,
    ProcessStopped,
    TerminalReady(ReceiptId),
    ChildExited(ReceiptId),
    FocusPane(ReceiptId),
    FocusWidget(WidgetName),
    WindowGeometry {
        window_id: ReceiptId,
        width: u32,
        height: u32,
    },
    PaneLayout {
        window_id: ReceiptId,
        worklane_id: ReceiptId,
        columns: Vec<PaneColumn>,
    },
    Action {
        action: ActionName,
        outcome: ActionOutcome,
        target_id: Option<ReceiptId>,
    },
    Failure {
        code: FailureCode,
        target_id: Option<ReceiptId>,
    },
}

#[derive(Debug)]
enum ReadStreamError {
    NotCreated,
    PartialRecord(ReceiptError),
    Invalid(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransientState {
    NotCreated,
    PartialRecord,
}

impl ReadStreamError {
    const fn transient_state(&self) -> Option<TransientState> {
        match self {
            Self::NotCreated => Some(TransientState::NotCreated),
            Self::PartialRecord(_) => Some(TransientState::PartialRecord),
            Self::Invalid(_) => None,
        }
    }
}

impl fmt::Display for ReadStreamError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotCreated => formatter.write_str("receipt has not been created"),
            Self::PartialRecord(error) => write!(formatter, "{error}"),
            Self::Invalid(error) => formatter.write_str(error),
        }
    }
}

fn main() -> ExitCode {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    match run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tornadotty-journey-driver: error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: &[String]) -> Result<(), String> {
    match arguments {
        [command, session_arguments @ ..] if command == "session" => {
            tornadotty_test_receipts::session::run(session_arguments)
                .map_err(|error| error.to_string())
        }
        [command, input_arguments @ ..] if command == "input" => {
            tornadotty_test_receipts::input::run(input_arguments).map_err(|error| error.to_string())
        }
        [command, scenario_arguments @ ..] if command == "scenario" => {
            tornadotty_test_receipts::scenario::run(scenario_arguments)
        }
        [command, path] if command == "validate" => {
            let stream = read_stream(Path::new(path)).map_err(|error| error.to_string())?;
            report_validation(path, &stream, false)
        }
        [command, path, flag] if command == "validate" && flag == "--complete" => {
            let stream = read_stream(Path::new(path)).map_err(|error| error.to_string())?;
            report_validation(path, &stream, true)
        }
        [command, path, timeout, minimum, selector @ ..] if command == "wait" => {
            let timeout = parse_positive_u64(timeout, "timeout milliseconds")?;
            let minimum = parse_positive_usize(minimum, "minimum match count")?;
            let selector = parse_selector(selector)?;
            wait_for(
                Path::new(path),
                Duration::from_millis(timeout),
                minimum,
                &selector,
            )
        }
        _ => Err(usage().to_owned()),
    }
}

fn report_validation(
    path: &str,
    stream: &ReceiptStream,
    require_complete: bool,
) -> Result<(), String> {
    if require_complete && !stream.is_complete() {
        return Err("receipt stream has no process_stopped event".to_owned());
    }
    println!(
        "receipt-valid path={path} records={} complete={}",
        stream.records().len(),
        stream.is_complete()
    );
    Ok(())
}

fn wait_for(
    path: &Path,
    timeout: Duration,
    minimum: usize,
    selector: &Selector,
) -> Result<(), String> {
    eprintln!(
        "journey-phase=receipt-wait path={} timeout-ms={} minimum={} selector={selector:?}",
        path.display(),
        timeout.as_millis(),
        minimum
    );
    let deadline = Instant::now() + timeout;
    let mut last_transient = None;
    let mut reported_transient = None;
    loop {
        match read_stream(path) {
            Ok(stream) => {
                let matches = stream
                    .records()
                    .iter()
                    .filter(|record| selector.matches(&record.event))
                    .collect::<Vec<_>>();
                if matches.len() >= minimum {
                    let Some(record) = matches.last() else {
                        return Err("positive receipt count had no final record".to_owned());
                    };
                    println!(
                        "{}",
                        serde_json::to_string(record)
                            .map_err(|error| format!("could not report matched record: {error}"))?
                    );
                    return Ok(());
                }
                if stream.is_complete() {
                    return Err(format!(
                        "product stopped after {} matching records; required {minimum}",
                        matches.len()
                    ));
                }
            }
            Err(error) => {
                let Some(transient_state) = error.transient_state() else {
                    return Err(error.to_string());
                };
                if reported_transient != Some(transient_state) {
                    eprintln!("journey-phase=receipt-transient state={transient_state:?}");
                    reported_transient = Some(transient_state);
                }
                last_transient = Some(error);
            }
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "receipt deadline expired{}",
                last_transient.map_or_else(String::new, |error| format!("; last state: {error}"))
            ));
        }
        thread::sleep(POLL_INTERVAL);
    }
}

impl Selector {
    fn matches(&self, event: &ReceiptEvent) -> bool {
        match (self, event) {
            (
                Self::ProcessStarted,
                ReceiptEvent::Lifecycle {
                    state: LifecycleState::ProcessStarted,
                    pane_id: None,
                },
            )
            | (
                Self::ProcessStopped,
                ReceiptEvent::Lifecycle {
                    state: LifecycleState::ProcessStopped,
                    pane_id: None,
                },
            ) => true,
            (
                Self::TerminalReady(expected),
                ReceiptEvent::Lifecycle {
                    state: LifecycleState::TerminalReady,
                    pane_id: Some(actual),
                },
            )
            | (
                Self::ChildExited(expected),
                ReceiptEvent::Lifecycle {
                    state: LifecycleState::ChildExited,
                    pane_id: Some(actual),
                },
            )
            | (
                Self::FocusPane(expected),
                ReceiptEvent::Focus {
                    focus: FocusTarget::Pane { pane_id: actual },
                },
            ) => expected == actual,
            (
                Self::FocusWidget(expected),
                ReceiptEvent::Focus {
                    focus: FocusTarget::Widget { widget: actual },
                },
            ) => expected == actual,
            (
                Self::WindowGeometry {
                    window_id,
                    width,
                    height,
                },
                ReceiptEvent::Geometry {
                    geometry:
                        GeometrySnapshot::Window {
                            window_id: actual_window,
                            width: actual_width,
                            height: actual_height,
                        },
                },
            ) => window_id == actual_window && width == actual_width && height == actual_height,
            (
                Self::PaneLayout {
                    window_id,
                    worklane_id,
                    columns,
                },
                ReceiptEvent::Geometry {
                    geometry:
                        GeometrySnapshot::PaneLayout {
                            window_id: actual_window,
                            worklane_id: actual_worklane,
                            columns: actual_columns,
                        },
                },
            ) => {
                window_id == actual_window
                    && worklane_id == actual_worklane
                    && columns == actual_columns
            }
            (
                Self::Action {
                    action,
                    outcome,
                    target_id,
                },
                ReceiptEvent::ActionCompletion {
                    action: actual_action,
                    outcome: actual_outcome,
                    target_id: actual_target,
                },
            ) => action == actual_action && outcome == actual_outcome && target_id == actual_target,
            (
                Self::Failure { code, target_id },
                ReceiptEvent::Failure {
                    code: actual_code,
                    target_id: actual_target,
                },
            ) => code == actual_code && target_id == actual_target,
            _ => false,
        }
    }
}

fn parse_selector(arguments: &[String]) -> Result<Selector, String> {
    match arguments {
        [name] if name == "process-started" => Ok(Selector::ProcessStarted),
        [name] if name == "process-stopped" => Ok(Selector::ProcessStopped),
        [name, pane_id] if name == "terminal-ready" => {
            Ok(Selector::TerminalReady(parse_id(pane_id)?))
        }
        [name, pane_id] if name == "child-exited" => Ok(Selector::ChildExited(parse_id(pane_id)?)),
        [name, pane_id] if name == "focus-pane" => Ok(Selector::FocusPane(parse_id(pane_id)?)),
        [name, widget] if name == "focus-widget" => {
            Ok(Selector::FocusWidget(parse_widget(widget)?))
        }
        [name, window_id, width, height] if name == "window-geometry" => {
            Ok(Selector::WindowGeometry {
                window_id: parse_id(window_id)?,
                width: parse_positive_u32(width, "window width")?,
                height: parse_positive_u32(height, "window height")?,
            })
        }
        [name, window_id, worklane_id, columns] if name == "pane-layout" => {
            Ok(Selector::PaneLayout {
                window_id: parse_id(window_id)?,
                worklane_id: parse_id(worklane_id)?,
                columns: parse_columns(columns)?,
            })
        }
        [name, action, outcome, target] if name == "action" => Ok(Selector::Action {
            action: parse_action(action)?,
            outcome: parse_outcome(outcome)?,
            target_id: parse_optional_id(target)?,
        }),
        [name, code, target] if name == "failure" => Ok(Selector::Failure {
            code: parse_failure(code)?,
            target_id: parse_optional_id(target)?,
        }),
        _ => Err(format!("invalid selector\n{}", usage())),
    }
}

fn parse_columns(value: &str) -> Result<Vec<PaneColumn>, String> {
    if value.is_empty() {
        return Err("pane layout columns may not be empty".to_owned());
    }
    value
        .split(';')
        .map(|column| {
            let (column_id, panes) = column
                .split_once('=')
                .ok_or_else(|| format!("invalid pane column: {column}"))?;
            let pane_ids = panes
                .split(',')
                .map(parse_id)
                .collect::<Result<Vec<_>, _>>()?;
            if pane_ids.is_empty() {
                return Err(format!("pane column has no panes: {column}"));
            }
            Ok(PaneColumn {
                column_id: parse_id(column_id)?,
                pane_ids,
            })
        })
        .collect()
}

fn parse_id(value: &str) -> Result<ReceiptId, String> {
    ReceiptId::new(value).map_err(|error| error.to_string())
}

fn parse_optional_id(value: &str) -> Result<Option<ReceiptId>, String> {
    if value == "-" {
        Ok(None)
    } else {
        parse_id(value).map(Some)
    }
}

fn parse_widget(value: &str) -> Result<WidgetName, String> {
    match value {
        "main-window" => Ok(WidgetName::MainWindow),
        "settings-window" => Ok(WidgetName::SettingsWindow),
        "notifications-section" => Ok(WidgetName::NotificationsSection),
        "notification-sound-import" => Ok(WidgetName::NotificationSoundImport),
        _ => Err(format!("unknown widget selector: {value}")),
    }
}

fn parse_action(value: &str) -> Result<ActionName, String> {
    match value {
        "open-settings" => Ok(ActionName::OpenSettings),
        "select-notifications-settings" => Ok(ActionName::SelectNotificationsSettings),
        "send-test-notification" => Ok(ActionName::SendTestNotification),
        "import-notification-sound" => Ok(ActionName::ImportNotificationSound),
        "preview-notification-sound" => Ok(ActionName::PreviewNotificationSound),
        "remove-notification-sound" => Ok(ActionName::RemoveNotificationSound),
        "split-pane-right" => Ok(ActionName::SplitPaneRight),
        "split-pane-below" => Ok(ActionName::SplitPaneBelow),
        "restore-workspace" => Ok(ActionName::RestoreWorkspace),
        _ => Err(format!("unknown action selector: {value}")),
    }
}

fn parse_outcome(value: &str) -> Result<ActionOutcome, String> {
    match value {
        "completed" => Ok(ActionOutcome::Completed),
        "unavailable" => Ok(ActionOutcome::Unavailable),
        _ => Err(format!("unknown action outcome: {value}")),
    }
}

fn parse_failure(value: &str) -> Result<FailureCode, String> {
    match value {
        "application-tick" => Ok(FailureCode::ApplicationTick),
        "receipt-write" => Ok(FailureCode::ReceiptWrite),
        "restore-workspace" => Ok(FailureCode::RestoreWorkspace),
        "settings-action" => Ok(FailureCode::SettingsAction),
        _ => Err(format!("unknown failure code: {value}")),
    }
}

fn read_stream(path: &Path) -> Result<ReceiptStream, ReadStreamError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(ReadStreamError::NotCreated);
        }
        Err(error) => {
            return Err(ReadStreamError::Invalid(format!(
                "could not inspect {}: {error}",
                path.display()
            )));
        }
    };
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != rustix::process::getuid().as_raw()
        || metadata.permissions().mode() & 0o777 != 0o600
    {
        return Err(ReadStreamError::Invalid(format!(
            "{} is not an owner-only regular receipt",
            path.display()
        )));
    }
    let maximum_file_bytes = u64::try_from(MAX_FILE_BYTES).map_err(|_| {
        ReadStreamError::Invalid("receipt size limit cannot be represented".to_owned())
    })?;
    if metadata.len() > maximum_file_bytes {
        return Err(ReadStreamError::Invalid(format!(
            "{} exceeds the receipt size limit",
            path.display()
        )));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(MAX_FILE_BYTES));
    File::open(path)
        .map_err(|error| {
            ReadStreamError::Invalid(format!("could not open {}: {error}", path.display()))
        })?
        .take(maximum_file_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            ReadStreamError::Invalid(format!("could not read {}: {error}", path.display()))
        })?;
    ReceiptStream::parse(&bytes).map_err(|error| {
        if error.kind() == ReceiptErrorKind::Truncated {
            ReadStreamError::PartialRecord(error)
        } else {
            ReadStreamError::Invalid(error.to_string())
        }
    })
}

fn parse_positive_u64(value: &str, name: &str) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("{name} must be a positive integer"))?;
    if parsed == 0 {
        return Err(format!("{name} must be positive"));
    }
    Ok(parsed)
}

fn parse_positive_u32(value: &str, name: &str) -> Result<u32, String> {
    let parsed = value
        .parse::<u32>()
        .map_err(|_| format!("{name} must be a positive integer"))?;
    if parsed == 0 {
        return Err(format!("{name} must be positive"));
    }
    Ok(parsed)
}

fn parse_positive_usize(value: &str, name: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("{name} must be a positive integer"))?;
    if parsed == 0 {
        return Err(format!("{name} must be positive"));
    }
    Ok(parsed)
}

fn usage() -> &'static str {
    "usage:\n  tornadotty-journey-driver validate FILE [--complete]\n  tornadotty-journey-driver wait FILE TIMEOUT_MS MINIMUM SELECTOR [SELECTOR-ARGS]\n  tornadotty-journey-driver session COMMAND ...\n  tornadotty-journey-driver input COMMAND ...\n  tornadotty-journey-driver scenario COMMAND ...\nselectors: process-started | process-stopped | terminal-ready PANE | child-exited PANE | focus-pane PANE | focus-widget WIDGET | window-geometry WINDOW WIDTH HEIGHT | pane-layout WINDOW WORKLANE COLUMN=PANE[,PANE][;...] | action ACTION OUTCOME TARGET-OR-- | failure CODE TARGET-OR--"
}
