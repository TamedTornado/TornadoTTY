//! Process ownership and durable evidence for real-product journeys.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use rustix::process::{Pid, Signal, kill_process_group};
use serde::{Deserialize, Serialize};

/// Stable error categories reported by journey session commands.
#[derive(Debug)]
pub enum SessionError {
    Arguments(String),
    Supervision(String),
    Deadline(String),
    Identity(String),
    Cleanup(String),
    Evidence(String),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (category, detail) = match self {
            Self::Arguments(detail) => ("arguments", detail),
            Self::Supervision(detail) => ("supervision", detail),
            Self::Deadline(detail) => ("deadline", detail),
            Self::Identity(detail) => ("identity", detail),
            Self::Cleanup(detail) => ("cleanup", detail),
            Self::Evidence(detail) => ("evidence", detail),
        };
        write!(formatter, "session-{category}: {detail}")
    }
}

impl std::error::Error for SessionError {}

const SESSION_SCHEMA_VERSION: u8 = 1;
const MAX_STATE_BYTES: u64 = 16 * 1024;
const MAX_JOURNAL_BYTES: usize = 1024 * 1024;
const MAX_JOURNAL_RECORD_BYTES: usize = 2 * 1024;
const MAX_RESOURCES: usize = 32;
const MAX_RESOURCE_BYTES: usize = 128;
const POLL_INTERVAL: Duration = Duration::from_millis(20);
const DESCENDANT_GRACE: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SessionPhase {
    Starting,
    Running,
    Exited,
    Failed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ProcessIdentity {
    pid: u32,
    start_ticks: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ExitIdentity {
    code: Option<i32>,
    signal: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SessionState {
    schema_version: u8,
    phase: SessionPhase,
    supervisor: ProcessIdentity,
    product: Option<ProcessIdentity>,
    exit: Option<ExitIdentity>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum StopSignal {
    Term,
    Kill,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum JourneyEvent {
    SessionStarted,
    ResourceAcquired {
        resource: String,
    },
    ProductStarted {
        pid: u32,
    },
    StopRequested {
        signal: StopSignal,
    },
    DescendantsReaped {
        signal: StopSignal,
        count: usize,
    },
    ProductExited {
        code: Option<i32>,
        signal: Option<i32>,
    },
    SessionCompleted,
    Failure {
        code: FailureCode,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum FailureCode {
    ResourceConflict,
    SpawnFailed,
    ProductFailed,
    DescendantLeak,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct JourneyRecord {
    schema_version: u8,
    sequence: u64,
    event: JourneyEvent,
}

struct Journal {
    file: File,
    sequence: u64,
    bytes_written: usize,
}

struct ResourceLease {
    #[allow(dead_code)]
    file: File,
}

#[derive(Debug)]
struct ProcessGroupMember {
    pid: u32,
    command: String,
}

struct SupervisorPaths {
    root: PathBuf,
    state: PathBuf,
    journal: PathBuf,
    product_log: PathBuf,
    stop_request: PathBuf,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct StopRequest {
    schema_version: u8,
    signal: StopSignal,
}

/// Runs a `session` subcommand for the journey-driver binary.
///
/// # Errors
///
/// Returns an error for invalid arguments, unsafe session state, ownership
/// violations, process failures, deadline expiry, or cleanup failures.
pub fn run(arguments: &[String]) -> Result<(), SessionError> {
    match arguments {
        [command, session, resource_root, rest @ ..] if command == "supervise" => {
            let (resources, product) =
                parse_supervise_arguments(rest).map_err(SessionError::Arguments)?;
            supervise(
                Path::new(session),
                Path::new(resource_root),
                &resources,
                product,
            )
            .map_err(SessionError::Supervision)
        }
        [command, session, timeout, phase] if command == "wait" => {
            let timeout = parse_positive_timeout(timeout).map_err(SessionError::Arguments)?;
            let phase = parse_phase(phase).map_err(SessionError::Arguments)?;
            wait_for_phase(Path::new(session), timeout, phase).map_err(SessionError::Deadline)
        }
        [command, session] if command == "product-pid" => {
            let paths =
                SupervisorPaths::existing(Path::new(session)).map_err(SessionError::Identity)?;
            let state = read_state(&paths.state).map_err(SessionError::Identity)?;
            let product = require_live_product(&state).map_err(SessionError::Identity)?;
            println!("{}", product.pid);
            Ok(())
        }
        [command, session, timeout] if command == "stop" => {
            let timeout = parse_positive_timeout(timeout).map_err(SessionError::Arguments)?;
            stop(Path::new(session), timeout).map_err(SessionError::Cleanup)
        }
        [command, session] if command == "inspect" => {
            let state_path = SupervisorPaths::existing(Path::new(session))
                .map_err(SessionError::Evidence)?
                .state;
            let state = read_state(&state_path).map_err(SessionError::Evidence)?;
            println!(
                "{}",
                serde_json::to_string(&state).map_err(|error| {
                    SessionError::Evidence(format!("could not encode session state: {error}"))
                })?
            );
            Ok(())
        }
        [command, session] if command == "validate-journal" => {
            let paths =
                SupervisorPaths::existing(Path::new(session)).map_err(SessionError::Evidence)?;
            let records = read_journal(&paths.journal).map_err(SessionError::Evidence)?;
            println!(
                "journey-journal-valid path={} records={}",
                paths.journal.display(),
                records.len()
            );
            Ok(())
        }
        _ => Err(SessionError::Arguments(usage().to_owned())),
    }
}

pub(crate) fn live_product_pid(session: &Path) -> Result<u32, String> {
    let state = read_state(&SupervisorPaths::existing(session)?.state)?;
    require_live_product(&state).map(|identity| identity.pid)
}

fn parse_supervise_arguments(arguments: &[String]) -> Result<(Vec<String>, &[String]), String> {
    let separator = arguments
        .iter()
        .position(|argument| argument == "--")
        .ok_or_else(|| format!("supervise command requires --\n{}", usage()))?;
    let options = &arguments[..separator];
    let product = &arguments[separator + 1..];
    if product.is_empty() {
        return Err("supervise command is missing the product executable".to_owned());
    }
    let mut resources = Vec::new();
    let mut index = 0;
    while index < options.len() {
        if options[index] != "--resource" || index + 1 >= options.len() {
            return Err(format!("invalid supervise option\n{}", usage()));
        }
        resources.push(options[index + 1].clone());
        index += 2;
    }
    if resources.len() > MAX_RESOURCES {
        return Err(format!(
            "at most {MAX_RESOURCES} exclusive resources may be claimed"
        ));
    }
    resources.sort();
    if resources.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("exclusive resource claims must be unique".to_owned());
    }
    for resource in &resources {
        validate_resource(resource)?;
    }
    Ok((resources, product))
}

fn supervise(
    session: &Path,
    resource_root: &Path,
    resources: &[String],
    product: &[String],
) -> Result<(), String> {
    let paths = SupervisorPaths::create(session)?;
    validate_owner_directory(resource_root, "resource root")?;
    let supervisor = current_identity()?;
    let mut journal = Journal::create(&paths.journal)?;
    journal.write(JourneyEvent::SessionStarted)?;
    write_state(
        &paths,
        &SessionState {
            schema_version: SESSION_SCHEMA_VERSION,
            phase: SessionPhase::Starting,
            supervisor,
            product: None,
            exit: None,
        },
    )?;
    eprintln!(
        "journey-phase=session-started supervisor-pid={}",
        supervisor.pid
    );

    let leases = match acquire_resources(resource_root, resources, &mut journal) {
        Ok(leases) => leases,
        Err(error) => {
            journal.write(JourneyEvent::Failure {
                code: FailureCode::ResourceConflict,
            })?;
            write_failed_state(&paths, supervisor, None)?;
            journal.write(JourneyEvent::SessionCompleted)?;
            return Err(error);
        }
    };
    let mut child = match spawn_product(product, &paths.product_log) {
        Ok(child) => child,
        Err(error) => {
            journal.write(JourneyEvent::Failure {
                code: FailureCode::SpawnFailed,
            })?;
            write_failed_state(&paths, supervisor, None)?;
            journal.write(JourneyEvent::SessionCompleted)?;
            return Err(error);
        }
    };
    let product_identity = child_identity(&child)?;
    journal.write(JourneyEvent::ProductStarted {
        pid: product_identity.pid,
    })?;
    write_state(
        &paths,
        &SessionState {
            schema_version: SESSION_SCHEMA_VERSION,
            phase: SessionPhase::Running,
            supervisor,
            product: Some(product_identity),
            exit: None,
        },
    )?;
    eprintln!(
        "journey-phase=product-running product-pid={}",
        product_identity.pid
    );

    let (status, requested_stop) =
        wait_for_child(&mut child, &paths, product_identity, &mut journal)?;
    finish_supervision(
        &paths,
        supervisor,
        product_identity,
        status,
        requested_stop,
        journal,
        leases,
    )
}

fn finish_supervision(
    paths: &SupervisorPaths,
    supervisor: ProcessIdentity,
    product: ProcessIdentity,
    status: ExitStatus,
    requested_stop: Option<StopSignal>,
    mut journal: Journal,
    leases: Vec<ResourceLease>,
) -> Result<(), String> {
    let descendants_reaped = reap_remaining_process_group(product, &mut journal)?;
    let exit = exit_identity(status);
    if descendants_reaped {
        journal.write(JourneyEvent::Failure {
            code: FailureCode::DescendantLeak,
        })?;
    }
    if !status.success() && !requested_stop_matches(requested_stop, status) {
        journal.write(JourneyEvent::Failure {
            code: FailureCode::ProductFailed,
        })?;
    }
    journal.write(JourneyEvent::ProductExited {
        code: exit.code,
        signal: exit.signal,
    })?;
    write_state(
        paths,
        &SessionState {
            schema_version: SESSION_SCHEMA_VERSION,
            phase: SessionPhase::Exited,
            supervisor,
            product: Some(product),
            exit: Some(exit),
        },
    )?;
    journal.write(JourneyEvent::SessionCompleted)?;
    drop(leases);
    eprintln!(
        "journey-phase=product-exited code={:?} signal={:?}",
        exit.code, exit.signal
    );
    if descendants_reaped {
        return Err(
            "product exited while descendants were still running; they were reaped".to_owned(),
        );
    }
    if status.success() || requested_stop_matches(requested_stop, status) {
        Ok(())
    } else {
        Err(format!(
            "product exited unsuccessfully: code={:?} signal={:?}",
            exit.code, exit.signal
        ))
    }
}

fn stop(session: &Path, timeout: Duration) -> Result<(), String> {
    let paths = SupervisorPaths::existing(session)?;
    let state = read_state(&paths.state)?;
    if state.phase == SessionPhase::Exited {
        return Ok(());
    }
    let product = require_live_product(&state)?;
    eprintln!("journey-phase=stop-requested product-pid={}", product.pid);
    write_stop_request(&paths, StopSignal::Term)?;
    if wait_until_exited(&paths.state, timeout)? {
        return Ok(());
    }
    eprintln!("journey-phase=stop-escalated product-pid={}", product.pid);
    write_stop_request(&paths, StopSignal::Kill)?;
    if wait_until_exited(&paths.state, Duration::from_secs(2))? {
        Ok(())
    } else {
        Err("supervisor did not publish exit after SIGKILL".to_owned())
    }
}

fn wait_for_child(
    child: &mut Child,
    paths: &SupervisorPaths,
    product: ProcessIdentity,
    journal: &mut Journal,
) -> Result<(ExitStatus, Option<StopSignal>), String> {
    let pid = checked_pid(product.pid)?;
    let mut delivered = None;
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("could not inspect product status: {error}"))?
        {
            return Ok((status, delivered));
        }
        if let Some(request) = read_stop_request(&paths.stop_request)?
            && delivered != Some(request.signal)
        {
            journal.write(JourneyEvent::StopRequested {
                signal: request.signal,
            })?;
            let signal = match request.signal {
                StopSignal::Term => Signal::TERM,
                StopSignal::Kill => Signal::KILL,
            };
            if let Err(error) = kill_process_group(pid, signal)
                && child
                    .try_wait()
                    .map_err(|cause| {
                        format!("could not inspect product after signal error: {cause}")
                    })?
                    .is_none()
            {
                return Err(format!("could not signal product process group: {error}"));
            }
            delivered = Some(request.signal);
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn requested_stop_matches(requested: Option<StopSignal>, status: ExitStatus) -> bool {
    matches!(
        (requested, status.signal()),
        (Some(StopSignal::Term), Some(15)) | (Some(StopSignal::Kill), Some(9))
    )
}

fn wait_for_phase(session: &Path, timeout: Duration, expected: SessionPhase) -> Result<(), String> {
    let paths = SupervisorPaths::existing(session)?;
    let deadline = Instant::now() + timeout;
    eprintln!(
        "journey-phase=session-wait expected={expected:?} timeout-ms={}",
        timeout.as_millis()
    );
    loop {
        match read_state(&paths.state) {
            Ok(state) if state.phase == expected => {
                println!(
                    "{}",
                    serde_json::to_string(&state)
                        .map_err(|error| format!("could not encode session state: {error}"))?
                );
                return Ok(());
            }
            Ok(state) if state.phase == SessionPhase::Failed => {
                return Err("session entered failed phase".to_owned());
            }
            Ok(state)
                if state.phase == SessionPhase::Exited && expected != SessionPhase::Exited =>
            {
                return Err(format!("product exited before phase {expected:?}"));
            }
            Ok(_) => {}
            Err(_error) if !paths.state.exists() => {}
            Err(error) => return Err(error),
        }
        if Instant::now() >= deadline {
            return Err(format!("session deadline expired waiting for {expected:?}"));
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn wait_until_exited(state_path: &Path, timeout: Duration) -> Result<bool, String> {
    let deadline = Instant::now() + timeout;
    loop {
        match read_state(state_path) {
            Ok(state) if matches!(state.phase, SessionPhase::Exited | SessionPhase::Failed) => {
                return Ok(true);
            }
            Ok(_) => {}
            Err(_error) if !state_path.exists() => {}
            Err(error) => return Err(error),
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn spawn_product(product: &[String], log_path: &Path) -> Result<Child, String> {
    let log = new_owner_file(log_path)?;
    let stderr = log
        .try_clone()
        .map_err(|error| format!("could not clone product log: {error}"))?;
    let mut command = Command::new(&product[0]);
    command
        .args(&product[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(stderr))
        .process_group(0);
    command
        .spawn()
        .map_err(|error| format!("could not start product: {error}"))
}

fn reap_remaining_process_group(
    product: ProcessIdentity,
    journal: &mut Journal,
) -> Result<bool, String> {
    let pid = checked_pid(product.pid)?;
    let mut members = live_process_group_members(product.pid)?;
    if members.is_empty() {
        return Ok(false);
    }
    let member_summary = members
        .iter()
        .map(|member| format!("{}:{}", member.pid, member.command))
        .collect::<Vec<_>>()
        .join(",");
    eprintln!(
        "journey-phase=descendant-leak process-group={} members={member_summary}",
        product.pid
    );
    journal.write(JourneyEvent::DescendantsReaped {
        signal: StopSignal::Term,
        count: members.len(),
    })?;
    kill_process_group(pid, Signal::TERM)
        .map_err(|error| format!("could not terminate remaining descendants: {error}"))?;
    let deadline = Instant::now() + DESCENDANT_GRACE;
    while Instant::now() < deadline {
        members = live_process_group_members(product.pid)?;
        if members.is_empty() {
            return Ok(true);
        }
        thread::sleep(POLL_INTERVAL);
    }
    journal.write(JourneyEvent::DescendantsReaped {
        signal: StopSignal::Kill,
        count: members.len(),
    })?;
    kill_process_group(pid, Signal::KILL)
        .map_err(|error| format!("could not kill remaining descendants: {error}"))?;
    Ok(true)
}

fn live_process_group_members(process_group: u32) -> Result<Vec<ProcessGroupMember>, String> {
    let mut members = Vec::new();
    for entry in
        fs::read_dir("/proc").map_err(|error| format!("could not enumerate /proc: {error}"))?
    {
        let entry = entry.map_err(|error| format!("could not inspect /proc entry: {error}"))?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(stat) = fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        let Some(close) = stat.rfind(')') else {
            continue;
        };
        let Some(fields) = stat.get(close + 1..) else {
            continue;
        };
        let fields = fields.split_ascii_whitespace().collect::<Vec<_>>();
        let Some(state) = fields.first() else {
            continue;
        };
        let Some(group) = fields.get(2).and_then(|value| value.parse::<u32>().ok()) else {
            continue;
        };
        if group == process_group && *state != "Z" {
            let command = stat
                .get(stat.find('(').unwrap_or_default() + 1..close)
                .unwrap_or("unknown")
                .to_owned();
            members.push(ProcessGroupMember { pid, command });
        }
    }
    members.sort_unstable_by_key(|member| member.pid);
    Ok(members)
}

fn acquire_resources(
    root: &Path,
    resources: &[String],
    journal: &mut Journal,
) -> Result<Vec<ResourceLease>, String> {
    let mut leases = Vec::with_capacity(resources.len());
    for resource in resources {
        let path = root.join(format!("{resource}.lock"));
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .mode(0o600)
            .open(&path)
            .map_err(|error| format!("could not open resource lease {resource}: {error}"))?;
        let metadata = file
            .metadata()
            .map_err(|error| format!("could not inspect resource lease {resource}: {error}"))?;
        if !metadata.is_file()
            || metadata.uid() != rustix::process::getuid().as_raw()
            || metadata.permissions().mode() & 0o777 != 0o600
        {
            return Err(format!("resource lease {resource} is not owner-only"));
        }
        file.try_lock().map_err(|error| {
            format!("exclusive resource is already claimed: {resource}: {error}")
        })?;
        journal.write(JourneyEvent::ResourceAcquired {
            resource: resource.clone(),
        })?;
        leases.push(ResourceLease { file });
    }
    Ok(leases)
}

fn validate_resource(resource: &str) -> Result<(), String> {
    if resource.is_empty()
        || resource.len() > MAX_RESOURCE_BYTES
        || !resource.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':' | b'=' | b'+')
        })
    {
        return Err("resource names must be bounded safe ASCII identifiers".to_owned());
    }
    Ok(())
}

fn require_live_product(state: &SessionState) -> Result<ProcessIdentity, String> {
    if state.schema_version != SESSION_SCHEMA_VERSION || state.phase != SessionPhase::Running {
        return Err("session does not identify a running product".to_owned());
    }
    let product = state
        .product
        .ok_or_else(|| "running session has no product identity".to_owned())?;
    let current = process_identity(product.pid)?;
    if current != product {
        return Err(format!("stale product PID identity: {}", product.pid));
    }
    Ok(product)
}

fn current_identity() -> Result<ProcessIdentity, String> {
    process_identity(std::process::id())
}

fn child_identity(child: &Child) -> Result<ProcessIdentity, String> {
    process_identity(child.id())
}

fn process_identity(pid: u32) -> Result<ProcessIdentity, String> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat"))
        .map_err(|error| format!("could not inspect process {pid}: {error}"))?;
    let close = stat
        .rfind(')')
        .ok_or_else(|| format!("process {pid} has malformed proc stat"))?;
    let fields = stat
        .get(close + 1..)
        .ok_or_else(|| format!("process {pid} has malformed proc stat"))?
        .split_ascii_whitespace()
        .collect::<Vec<_>>();
    let start_ticks = fields
        .get(19)
        .ok_or_else(|| format!("process {pid} proc stat lacks start time"))?
        .parse::<u64>()
        .map_err(|_| format!("process {pid} has invalid start time"))?;
    Ok(ProcessIdentity { pid, start_ticks })
}

fn checked_pid(raw: u32) -> Result<Pid, String> {
    let raw = i32::try_from(raw).map_err(|_| format!("PID {raw} cannot be represented"))?;
    Pid::from_raw(raw).ok_or_else(|| "PID may not be zero".to_owned())
}

fn exit_identity(status: ExitStatus) -> ExitIdentity {
    ExitIdentity {
        code: status.code(),
        signal: status.signal(),
    }
}

fn parse_positive_timeout(value: &str) -> Result<Duration, String> {
    let milliseconds = value
        .parse::<u64>()
        .map_err(|_| "session timeout must be a positive integer".to_owned())?;
    if milliseconds == 0 {
        return Err("session timeout must be positive".to_owned());
    }
    Ok(Duration::from_millis(milliseconds))
}

fn parse_phase(value: &str) -> Result<SessionPhase, String> {
    match value {
        "starting" => Ok(SessionPhase::Starting),
        "running" => Ok(SessionPhase::Running),
        "exited" => Ok(SessionPhase::Exited),
        "failed" => Ok(SessionPhase::Failed),
        _ => Err(format!("unknown session phase: {value}")),
    }
}

fn validate_owner_directory(path: &Path, label: &str) -> Result<PathBuf, String> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
    {
        return Err(format!("{label} must be an absolute normalized path"));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("could not inspect {label}: {error}"))?;
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("could not resolve {label}: {error}"))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || canonical != path
        || metadata.uid() != rustix::process::getuid().as_raw()
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(format!(
            "{label} must be canonical, owner-controlled, and mode 0700"
        ));
    }
    Ok(canonical)
}

fn new_owner_file(path: &Path) -> Result<File, String> {
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| format!("could not create {}: {error}", path.display()))
}

fn write_state(paths: &SupervisorPaths, state: &SessionState) -> Result<(), String> {
    let temporary = paths.root.join("state.next.json");
    let encoded = serde_json::to_vec(state)
        .map_err(|error| format!("could not encode session state: {error}"))?;
    if u64::try_from(encoded.len()).unwrap_or(u64::MAX) > MAX_STATE_BYTES {
        return Err("encoded session state exceeds its bound".to_owned());
    }
    let mut file = new_owner_file(&temporary)?;
    file.write_all(&encoded)
        .and_then(|()| file.write_all(b"\n"))
        .and_then(|()| file.flush())
        .map_err(|error| format!("could not write session state: {error}"))?;
    fs::rename(&temporary, &paths.state)
        .map_err(|error| format!("could not publish session state: {error}"))
}

fn write_stop_request(paths: &SupervisorPaths, signal: StopSignal) -> Result<(), String> {
    let request = StopRequest {
        schema_version: SESSION_SCHEMA_VERSION,
        signal,
    };
    let encoded = serde_json::to_vec(&request)
        .map_err(|error| format!("could not encode stop request: {error}"))?;
    let temporary = paths.root.join("stop.next.json");
    if fs::symlink_metadata(&temporary).is_ok() {
        fs::remove_file(&temporary)
            .map_err(|error| format!("could not replace stale stop request: {error}"))?;
    }
    let mut file = new_owner_file(&temporary)?;
    file.write_all(&encoded)
        .and_then(|()| file.write_all(b"\n"))
        .and_then(|()| file.flush())
        .map_err(|error| format!("could not write stop request: {error}"))?;
    fs::rename(&temporary, &paths.stop_request)
        .map_err(|error| format!("could not publish stop request: {error}"))
}

fn read_stop_request(path: &Path) -> Result<Option<StopRequest>, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("could not inspect stop request: {error}")),
    };
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != rustix::process::getuid().as_raw()
        || metadata.permissions().mode() & 0o777 != 0o600
        || metadata.len() > MAX_STATE_BYTES
    {
        return Err("stop request is not a bounded owner-only regular file".to_owned());
    }
    let request: StopRequest = serde_json::from_reader(
        File::open(path).map_err(|error| format!("could not open stop request: {error}"))?,
    )
    .map_err(|error| format!("stop request is malformed: {error}"))?;
    if request.schema_version != SESSION_SCHEMA_VERSION {
        return Err(format!(
            "unsupported stop request version: {}",
            request.schema_version
        ));
    }
    Ok(Some(request))
}

fn write_failed_state(
    paths: &SupervisorPaths,
    supervisor: ProcessIdentity,
    product: Option<ProcessIdentity>,
) -> Result<(), String> {
    write_state(
        paths,
        &SessionState {
            schema_version: SESSION_SCHEMA_VERSION,
            phase: SessionPhase::Failed,
            supervisor,
            product,
            exit: None,
        },
    )
}

fn read_state(path: &Path) -> Result<SessionState, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("could not inspect session state: {error}"))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != rustix::process::getuid().as_raw()
        || metadata.permissions().mode() & 0o777 != 0o600
        || metadata.len() > MAX_STATE_BYTES
    {
        return Err("session state is not a bounded owner-only regular file".to_owned());
    }
    let mut encoded = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or_default());
    File::open(path)
        .map_err(|error| format!("could not open session state: {error}"))?
        .take(MAX_STATE_BYTES + 1)
        .read_to_end(&mut encoded)
        .map_err(|error| format!("could not read session state: {error}"))?;
    let state: SessionState = serde_json::from_slice(&encoded)
        .map_err(|error| format!("session state is malformed: {error}"))?;
    if state.schema_version != SESSION_SCHEMA_VERSION {
        return Err(format!(
            "unsupported session state version: {}",
            state.schema_version
        ));
    }
    Ok(state)
}

fn read_journal(path: &Path) -> Result<Vec<JourneyRecord>, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("could not inspect journey evidence: {error}"))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != rustix::process::getuid().as_raw()
        || metadata.permissions().mode() & 0o777 != 0o600
        || metadata.len() > u64::try_from(MAX_JOURNAL_BYTES).unwrap_or(u64::MAX)
    {
        return Err("journey evidence is not a bounded owner-only regular file".to_owned());
    }
    let mut encoded = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or_default());
    File::open(path)
        .map_err(|error| format!("could not open journey evidence: {error}"))?
        .take(u64::try_from(MAX_JOURNAL_BYTES).unwrap_or(u64::MAX) + 1)
        .read_to_end(&mut encoded)
        .map_err(|error| format!("could not read journey evidence: {error}"))?;
    if encoded.is_empty() || !encoded.ends_with(b"\n") {
        return Err("journey evidence ends with a partial record".to_owned());
    }
    let mut records = Vec::new();
    for (index, line) in encoded.split(|byte| *byte == b'\n').enumerate() {
        if line.is_empty() {
            continue;
        }
        if line.len() > MAX_JOURNAL_RECORD_BYTES {
            return Err(format!(
                "journey evidence record {} exceeds its bound",
                index + 1
            ));
        }
        let record: JourneyRecord = serde_json::from_slice(line).map_err(|error| {
            format!(
                "journey evidence record {} is malformed: {error}",
                index + 1
            )
        })?;
        let expected = u64::try_from(index + 1)
            .map_err(|_| "journey evidence sequence cannot be represented".to_owned())?;
        if record.schema_version != SESSION_SCHEMA_VERSION {
            return Err(format!(
                "unsupported journey evidence version: {}",
                record.schema_version
            ));
        }
        if record.sequence != expected {
            return Err(format!(
                "journey evidence sequence mismatch: expected {expected}, found {}",
                record.sequence
            ));
        }
        records.push(record);
    }
    validate_journal_order(&records)?;
    Ok(records)
}

fn validate_journal_order(records: &[JourneyRecord]) -> Result<(), String> {
    if !matches!(
        records.first().map(|record| &record.event),
        Some(JourneyEvent::SessionStarted)
    ) {
        return Err("journey evidence must start with session_started".to_owned());
    }
    let started = records
        .iter()
        .filter(|record| matches!(record.event, JourneyEvent::ProductStarted { .. }))
        .count();
    let exited = records
        .iter()
        .filter(|record| matches!(record.event, JourneyEvent::ProductExited { .. }))
        .count();
    let completed = records
        .iter()
        .filter(|record| matches!(record.event, JourneyEvent::SessionCompleted))
        .count();
    if completed != 1 {
        return Err(format!(
            "journey evidence requires one session_completed event; found {completed}"
        ));
    }
    let session_completed = records
        .iter()
        .position(|record| matches!(record.event, JourneyEvent::SessionCompleted))
        .expect("count checked");
    if session_completed + 1 != records.len() {
        return Err("session_completed must be the final journey event".to_owned());
    }
    if started == 0 && exited == 0 {
        if !records.iter().any(|record| {
            matches!(
                record.event,
                JourneyEvent::Failure {
                    code: FailureCode::ResourceConflict | FailureCode::SpawnFailed
                }
            )
        }) {
            return Err("journey evidence without a product lacks a pre-spawn failure".to_owned());
        }
        return Ok(());
    }
    if started != 1 || exited != 1 {
        return Err(format!(
            "journey evidence requires matching product_started/product_exited events; found {started}/{exited}"
        ));
    }
    let product_started = records
        .iter()
        .position(|record| matches!(record.event, JourneyEvent::ProductStarted { .. }))
        .expect("count checked");
    let product_exited = records
        .iter()
        .position(|record| matches!(record.event, JourneyEvent::ProductExited { .. }))
        .expect("count checked");
    if !(product_started < product_exited && product_exited < session_completed) {
        return Err("journey evidence lifecycle order is invalid".to_owned());
    }
    Ok(())
}

impl SupervisorPaths {
    fn create(root: &Path) -> Result<Self, String> {
        validate_owner_directory(root, "session directory")?;
        let paths = Self::from_root(root);
        for path in [
            &paths.state,
            &paths.journal,
            &paths.product_log,
            &paths.stop_request,
        ] {
            if fs::symlink_metadata(path).is_ok() {
                return Err(format!(
                    "session artifact already exists: {}",
                    path.display()
                ));
            }
        }
        Ok(paths)
    }

    fn existing(root: &Path) -> Result<Self, String> {
        validate_owner_directory(root, "session directory")?;
        Ok(Self::from_root(root))
    }

    fn from_root(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            state: root.join("state.json"),
            journal: root.join("journey.ndjson"),
            product_log: root.join("product.log"),
            stop_request: root.join("stop.json"),
        }
    }
}

impl Journal {
    fn create(path: &Path) -> Result<Self, String> {
        Ok(Self {
            file: new_owner_file(path)?,
            sequence: 1,
            bytes_written: 0,
        })
    }

    fn write(&mut self, event: JourneyEvent) -> Result<(), String> {
        let record = JourneyRecord {
            schema_version: SESSION_SCHEMA_VERSION,
            sequence: self.sequence,
            event,
        };
        let mut encoded = serde_json::to_vec(&record)
            .map_err(|error| format!("could not encode journey evidence: {error}"))?;
        if encoded.len() > MAX_JOURNAL_RECORD_BYTES {
            return Err("journey evidence record exceeds its bound".to_owned());
        }
        encoded.push(b'\n');
        self.bytes_written = self
            .bytes_written
            .checked_add(encoded.len())
            .filter(|total| *total <= MAX_JOURNAL_BYTES)
            .ok_or_else(|| "journey evidence exceeds its bound".to_owned())?;
        self.file
            .write_all(&encoded)
            .and_then(|()| self.file.flush())
            .map_err(|error| format!("could not write journey evidence: {error}"))?;
        self.sequence += 1;
        Ok(())
    }
}

fn usage() -> &'static str {
    "session usage:\n  tornadotty-journey-driver session supervise SESSION_DIR RESOURCE_ROOT [--resource NAME]... -- PRODUCT [ARG]...\n  tornadotty-journey-driver session wait SESSION_DIR TIMEOUT_MS starting|running|exited|failed\n  tornadotty-journey-driver session product-pid SESSION_DIR\n  tornadotty-journey-driver session inspect SESSION_DIR\n  tornadotty-journey-driver session validate-journal SESSION_DIR\n  tornadotty-journey-driver session stop SESSION_DIR TIMEOUT_MS"
}
