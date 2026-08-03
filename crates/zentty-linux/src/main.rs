#![forbid(unsafe_code)]

mod application_shell;
mod sidebar;

use application_shell::ApplicationShell;
use gtk::glib;
use gtk::prelude::*;
use std::cell::Cell;
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use zentty_core::{
    SaveReason, SessionRestoreEnvelope, SessionRestoreStore, WindowRecipe, WorkspaceRecipe,
};
use zentty_ghostty::{AsyncBackend, GhosttyRuntime};

#[derive(Debug)]
struct Options {
    command: Option<String>,
    exit_policy: ExitPolicy,
    terminal_count: usize,
    lifecycle_cycles: usize,
    async_backend: AsyncBackend,
    exercise_workspace_actions: bool,
    state_directory: Option<PathBuf>,
    restore_enabled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExitPolicy {
    Manual,
    LastTerminal,
    WorkspaceActions,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            command: None,
            exit_policy: ExitPolicy::Manual,
            terminal_count: 1,
            lifecycle_cycles: 1,
            async_backend: AsyncBackend::Default,
            exercise_workspace_actions: false,
            state_directory: None,
            restore_enabled: true,
        }
    }
}

fn parse_positive_count(name: &str, value: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .ok()
        .filter(|count| *count > 0 && *count <= 16)
        .ok_or_else(|| format!("{name} must be an integer from 1 through 16"))
}

fn parse_options() -> Result<Options, String> {
    let mut arguments = std::env::args().skip(1);
    let mut options = Options::default();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--command" => {
                options.command = Some(
                    arguments
                        .next()
                        .ok_or_else(|| "--command requires a value".to_owned())?,
                );
            }
            "--quit-after-last-terminal-exit" => {
                options.exit_policy = ExitPolicy::LastTerminal;
            }
            "--exercise-workspace-actions" => {
                options.exercise_workspace_actions = true;
            }
            "--quit-after-workspace-actions" => {
                options.exit_policy = ExitPolicy::WorkspaceActions;
            }
            "--state-directory" => {
                options.state_directory =
                    Some(PathBuf::from(arguments.next().ok_or_else(|| {
                        "--state-directory requires a value".to_owned()
                    })?));
            }
            "--no-session-restore" => {
                options.restore_enabled = false;
            }
            "--terminal-count" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| "--terminal-count requires a value".to_owned())?;
                options.terminal_count = parse_positive_count("--terminal-count", &value)?;
            }
            "--lifecycle-cycles" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| "--lifecycle-cycles requires a value".to_owned())?;
                options.lifecycle_cycles = parse_positive_count("--lifecycle-cycles", &value)?;
            }
            "--async-backend" => {
                options.async_backend = match arguments
                    .next()
                    .ok_or_else(|| "--async-backend requires a value".to_owned())?
                    .as_str()
                {
                    "default" => AsyncBackend::Default,
                    "epoll" => AsyncBackend::Epoll,
                    "io_uring" => AsyncBackend::IoUring,
                    value => {
                        return Err(format!(
                            "--async-backend must be default, epoll, or io_uring; got {value}"
                        ));
                    }
                };
            }
            _ => return Err(format!("unknown argument: {argument}")),
        }
    }
    if options.exit_policy == ExitPolicy::WorkspaceActions && !options.exercise_workspace_actions {
        return Err(
            "--quit-after-workspace-actions requires --exercise-workspace-actions".to_owned(),
        );
    }
    Ok(options)
}

fn run_lifecycle_cycle(
    runtime: &GhosttyRuntime,
    options: &Options,
    cycle: usize,
    restored_window: Option<WindowRecipe>,
) -> Result<WindowRecipe, String> {
    let main_loop = glib::MainLoop::new(None, false);
    let shell = ApplicationShell::new(
        runtime,
        options.command.clone(),
        options.terminal_count,
        options.exit_policy == ExitPolicy::LastTerminal,
        &main_loop,
        restored_window,
    )?;
    let close_loop = main_loop.clone();
    shell.borrow().window().connect_close_request(move |_| {
        close_loop.quit();
        glib::Propagation::Proceed
    });

    let ticking_runtime = runtime.clone();
    let tick_loop = main_loop.clone();
    let observed_window = shell.borrow().window().clone();
    let last_window_size = Rc::new(Cell::new((0, 0)));
    let ticking_window_size = Rc::clone(&last_window_size);
    let tick_source = glib::timeout_add_local(Duration::from_millis(10), move || {
        let window_size = (observed_window.width(), observed_window.height());
        if window_size != ticking_window_size.get() && window_size.0 > 0 && window_size.1 > 0 {
            eprintln!(
                "zentty-linux: window-size={}x{}",
                window_size.0, window_size.1
            );
            ticking_window_size.set(window_size);
        }
        if let Err(error) = ticking_runtime.tick() {
            eprintln!("zentty-linux: {error}");
            tick_loop.quit();
            // Keep ownership of the source until the composition root removes
            // it after `MainLoop::run`; this preserves one teardown path for
            // both ordinary exit and tick failure.
            glib::ControlFlow::Continue
        } else {
            glib::ControlFlow::Continue
        }
    });

    shell.borrow().present();
    if options.exercise_workspace_actions {
        ApplicationShell::schedule_workspace_actions(
            &shell,
            options.exit_policy == ExitPolicy::WorkspaceActions,
        );
    }
    main_loop.run();

    tick_source.remove();
    let window_recipe = shell.borrow().window_recipe();
    shell.borrow_mut().detach_and_close();
    settle_gtk_teardown();
    shell.borrow_mut().release_surfaces()?;
    settle_gtk_teardown();
    while glib::MainContext::default().pending() {
        glib::MainContext::default().iteration(false);
    }
    if options.exit_policy == ExitPolicy::LastTerminal && shell.borrow().live_children() != 0 {
        return Err(format!(
            "lifecycle cycle {cycle} ended with {} live children",
            shell.borrow().live_children()
        ));
    }
    eprintln!("zentty-linux: lifecycle-cycle={cycle} complete");
    Ok(window_recipe)
}

fn settle_gtk_teardown() {
    // GSK can retain GL-area widgets until the next frame after their window
    // is unmapped. Keep Ghostty's surface wrappers alive across that frame,
    // then give finalizers the same bounded opportunity after releasing the
    // wrappers. No ApplicationShell borrow is held while callbacks run.
    let settle_loop = glib::MainLoop::new(None, false);
    let quit_loop = settle_loop.clone();
    glib::timeout_add_local_once(Duration::from_millis(50), move || quit_loop.quit());
    settle_loop.run();
}

fn run() -> Result<(), String> {
    let options = parse_options()?;

    let state_directory = match &options.state_directory {
        Some(path) => path.clone(),
        None => default_state_directory()?,
    };
    let store = SessionRestoreStore::new(
        state_directory.join("restore-snapshot.json"),
        state_directory.join("restore-lifecycle.json"),
    );
    let launch_decision = store
        .prepare_for_launch(options.restore_enabled)
        .map_err(|error| error.to_string())?;
    let restored_drafts = launch_decision.as_ref().map_or_else(Vec::new, |decision| {
        decision.envelope.restore_draft_windows.clone()
    });
    let mut restored_window = launch_decision
        .as_ref()
        .map(|decision| select_restored_window(&decision.envelope.workspace))
        .transpose()?;
    store
        .mark_launch_started(reference_timestamp())
        .map_err(|error| error.to_string())?;

    // Ghostty owns process-global initialization that must precede GTK.
    let runtime = GhosttyRuntime::new(options.async_backend).map_err(|error| error.to_string())?;
    gtk::init().map_err(|error| format!("GTK initialization failed: {error}"))?;

    for cycle in 1..=options.lifecycle_cycles {
        restored_window = Some(run_lifecycle_cycle(
            &runtime,
            &options,
            cycle,
            restored_window,
        )?);
    }
    drop(runtime);
    let window = restored_window.expect("positive lifecycle cycle count");
    let workspace = WorkspaceRecipe {
        schema_version: Some(WorkspaceRecipe::CURRENT_SCHEMA_VERSION),
        active_window_id: Some(window.id.clone()),
        windows: vec![window],
    };
    store
        .save_snapshot(&SessionRestoreEnvelope {
            schema_version: 1,
            saved_at: reference_timestamp(),
            reason: SaveReason::CleanExit,
            workspace,
            restore_draft_windows: restored_drafts,
        })
        .map_err(|error| error.to_string())?;
    store
        .mark_clean_exit(reference_timestamp())
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn select_restored_window(workspace: &WorkspaceRecipe) -> Result<WindowRecipe, String> {
    if workspace.windows.len() != 1 {
        return Err(format!(
            "workspace restore has {} windows; Linux currently requires exactly one",
            workspace.windows.len()
        ));
    }
    let window = &workspace.windows[0];
    if workspace
        .active_window_id
        .as_deref()
        .is_some_and(|id| id != window.id)
    {
        return Err("workspace active window does not exist".to_owned());
    }
    Ok(window.clone())
}

fn default_state_directory() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("XDG_STATE_HOME") {
        return Ok(PathBuf::from(path).join("zentty"));
    }
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".local/state/zentty"))
        .ok_or_else(|| "neither XDG_STATE_HOME nor HOME is set".to_owned())
}

fn reference_timestamp() -> f64 {
    const APPLE_REFERENCE_EPOCH: f64 = 978_307_200.0;
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |duration| {
            duration.as_secs_f64() - APPLE_REFERENCE_EPOCH
        })
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("zentty-linux: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_positive_count;

    #[test]
    fn positive_counts_enforce_product_bounds() {
        assert_eq!(parse_positive_count("count", "1"), Ok(1));
        assert_eq!(parse_positive_count("count", "16"), Ok(16));
        assert!(parse_positive_count("count", "0").is_err());
        assert!(parse_positive_count("count", "17").is_err());
        assert!(parse_positive_count("count", "not-a-number").is_err());
    }
}
