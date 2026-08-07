#![forbid(unsafe_code)]

mod agent_runtime;
mod agent_status_view;
mod application_shell;
mod codex_enrichment;
mod command_palette;
mod pane_controls;
mod pane_scroll_switch;
mod pane_search;
mod peek_scroll_navigation;
mod sidebar;
mod sidebar_visibility;
mod source_ui;
mod tmux_compat;
mod window_chrome;
mod worklane_peek;

use application_shell::ApplicationShell;
use gtk::glib;
use gtk::prelude::*;
use std::cell::Cell;
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use zentty_core::{
    PaneRestoreDraft, SaveReason, SessionRestoreDraftWindow, SessionRestoreEnvelope,
    SessionRestoreStore, WindowRecipe, WorkspaceRecipe,
};
use zentty_ghostty::{AsyncBackend, GhosttyRuntime};

#[derive(Debug)]
struct Options {
    command: Option<String>,
    async_backend: AsyncBackend,
    state_directory: Option<PathBuf>,
    restore_enabled: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            command: None,
            async_backend: AsyncBackend::Default,
            state_directory: None,
            restore_enabled: true,
        }
    }
}

fn required_argument(
    arguments: &mut impl Iterator<Item = String>,
    name: &str,
) -> Result<String, String> {
    arguments
        .next()
        .ok_or_else(|| format!("{name} requires a value"))
}

fn parse_options() -> Result<Options, String> {
    let mut arguments = std::env::args().skip(1);
    let mut options = Options::default();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--command" => {
                options.command = Some(required_argument(&mut arguments, "--command")?);
            }
            "--state-directory" => {
                options.state_directory = Some(PathBuf::from(required_argument(
                    &mut arguments,
                    "--state-directory",
                )?));
            }
            "--no-session-restore" => {
                options.restore_enabled = false;
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
    Ok(options)
}

fn run_lifecycle_cycle(
    runtime: &GhosttyRuntime,
    options: &Options,
    restored_window: Option<WindowRecipe>,
    restored_drafts: Vec<PaneRestoreDraft>,
) -> Result<(WindowRecipe, Vec<PaneRestoreDraft>), String> {
    let main_loop = glib::MainLoop::new(None, false);
    let shell = ApplicationShell::new(
        runtime,
        options.command.clone(),
        &main_loop,
        restored_window,
        restored_drafts,
    )?;
    let close_loop = main_loop.clone();
    shell.borrow().window().connect_close_request(move |_| {
        close_loop.quit();
        glib::Propagation::Proceed
    });

    let ticking_runtime = runtime.clone();
    let tick_loop = main_loop.clone();
    let observed_window = shell.borrow().window().clone();
    let observed_sidebar = shell.borrow().sidebar_container().clone();
    let ticking_shell = Rc::downgrade(&shell);
    let last_window_size = Rc::new(Cell::new((0, 0)));
    let ticking_window_size = Rc::clone(&last_window_size);
    let last_sidebar_width = Rc::new(Cell::new(0));
    let ticking_sidebar_width = Rc::clone(&last_sidebar_width);
    let tick_source = glib::timeout_add_local(Duration::from_millis(10), move || {
        if let Some(shell) = ticking_shell.upgrade() {
            shell.borrow_mut().sync_agent_targets();
            ApplicationShell::drain_agent_events(&shell);
            let shell = shell.borrow_mut();
            shell.reconcile_sidebar_width();
            shell.reconcile_pane_heights();
        }
        let window_size = (observed_window.width(), observed_window.height());
        if window_size != ticking_window_size.get() && window_size.0 > 0 && window_size.1 > 0 {
            eprintln!(
                "zentty-linux: window-size={}x{}",
                window_size.0, window_size.1
            );
            ticking_window_size.set(window_size);
        }
        let sidebar_width = observed_sidebar.width();
        if sidebar_width != ticking_sidebar_width.get() && sidebar_width > 0 {
            eprintln!("zentty-linux: sidebar-width={sidebar_width}");
            ticking_sidebar_width.set(sidebar_width);
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
    main_loop.run();

    tick_source.remove();
    let window_recipe = shell.borrow().window_recipe();
    let agent_restore_drafts = shell.borrow().agent_restore_drafts();
    shell.borrow_mut().detach_and_close();
    settle_gtk_teardown();
    shell.borrow_mut().release_surfaces()?;
    settle_gtk_teardown();
    while glib::MainContext::default().pending() {
        glib::MainContext::default().iteration(false);
    }
    if shell.borrow().live_children() != 0 {
        return Err(format!(
            "application ended with {} live children",
            shell.borrow().live_children()
        ));
    }
    eprintln!("zentty-linux: lifecycle complete");
    Ok((window_recipe, agent_restore_drafts))
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
    let mut restored_window = launch_decision
        .as_ref()
        .map(|decision| select_restored_window(&decision.envelope.workspace))
        .transpose()?;
    let mut restored_drafts = restored_window.as_ref().map_or_else(Vec::new, |window| {
        launch_decision
            .as_ref()
            .and_then(|decision| {
                decision
                    .envelope
                    .restore_draft_windows
                    .iter()
                    .find(|drafts| drafts.window_id == window.id)
            })
            .map_or_else(Vec::new, |drafts| drafts.pane_drafts.clone())
    });
    store
        .mark_launch_started(reference_timestamp())
        .map_err(|error| error.to_string())?;

    // Ghostty owns process-global initialization that must precede GTK.
    let runtime = GhosttyRuntime::new(options.async_backend).map_err(|error| error.to_string())?;
    gtk::init().map_err(|error| format!("GTK initialization failed: {error}"))?;

    let (window, drafts) =
        run_lifecycle_cycle(&runtime, &options, restored_window, restored_drafts)?;
    restored_window = Some(window);
    restored_drafts = drafts;
    drop(runtime);
    let window = restored_window.expect("positive lifecycle cycle count");
    let window_id = window.id.clone();
    let workspace = WorkspaceRecipe {
        schema_version: Some(WorkspaceRecipe::CURRENT_SCHEMA_VERSION),
        active_window_id: Some(window.id.clone()),
        windows: vec![window],
    };
    let restore_draft_windows = if restored_drafts.is_empty() {
        Vec::new()
    } else {
        vec![SessionRestoreDraftWindow {
            window_id,
            pane_drafts: restored_drafts,
        }]
    };
    store
        .save_snapshot(&SessionRestoreEnvelope {
            schema_version: 1,
            saved_at: reference_timestamp(),
            reason: SaveReason::CleanExit,
            workspace,
            restore_draft_windows,
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
    use super::required_argument;

    #[test]
    fn required_arguments_reject_missing_values() {
        let mut missing = Vec::<String>::new().into_iter();
        assert_eq!(
            required_argument(&mut missing, "--command"),
            Err("--command requires a value".to_owned())
        );
    }
}
