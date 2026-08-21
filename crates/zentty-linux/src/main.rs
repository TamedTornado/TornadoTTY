#![forbid(unsafe_code)]

mod about_catalog;
mod about_view;
mod agent_fleet;
mod agent_runtime;
mod agent_status_view;
mod agents_settings;
mod appearance_settings;
mod application;
mod application_shell;
mod attention_inbox;
mod bookmarks_view;
mod closed_pane_archive;
mod codex_enrichment;
mod command_palette;
mod config_reload;
mod config_store;
mod custom_sound_store;
mod dev_server_settings;
mod diagnostic_store;
mod diagnostics_runtime;
mod docker_discovery;
mod general_settings;
mod global_search_view;
mod notification_service;
mod notifications_settings;
mod open_with_settings;
mod pane_controls;
mod pane_dividers;
mod pane_scroll_switch;
mod peek_scroll_navigation;
mod persistence_coordinator;
mod project_icon_view;
mod restore_notice;
mod server_discovery;
mod settings_navigation;
mod settings_shell;
mod shortcut_settings;
mod sidebar;
mod sidebar_visibility;
mod sleep_inhibitor;
mod source_ui;
mod status_notifier;
mod task_manager;
mod theme_catalog;
mod theme_preview;
mod tmux_compat;
mod tmux_store;
mod updates_privacy_settings;
mod window_chrome;
mod window_set;
mod worklane_peek;
mod workspace_pane_settings;

use application::ApplicationCoordinator;
use config_store::ConfigStore;
use gtk::glib;
use persistence_coordinator::{PersistenceCoordinator, WindowSnapshot, default_state_directory};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
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
    restored_windows: Vec<WindowSnapshot>,
    active_window_id: Option<&str>,
    persistence: &Rc<RefCell<PersistenceCoordinator>>,
    default_working_directory: &str,
    config: &config_store::ConfigSnapshot,
) -> Result<application::ApplicationCycleResult, String> {
    let main_loop = glib::MainLoop::new(None, false);
    let application = ApplicationCoordinator::start(
        runtime,
        options.command.clone(),
        &main_loop,
        restored_windows,
        active_window_id,
        config,
    )?;
    let teardown_active = application.borrow().teardown_flag();

    let tick_loop = main_loop.clone();
    let ticking_application = Rc::downgrade(&application);
    let tick_teardown_active = Rc::clone(&teardown_active);
    let tick_source = glib::timeout_add_local(Duration::from_millis(10), move || {
        if tick_teardown_active.get() {
            return glib::ControlFlow::Continue;
        }
        let Some(application) = ticking_application.upgrade() else {
            return glib::ControlFlow::Break;
        };
        let tick_result = application.borrow_mut().tick();
        if let Err(error) = tick_result {
            eprintln!("zentty-linux: {error}");
            application.borrow_mut().record_terminal_error(error);
            tick_loop.quit();
        }
        glib::ControlFlow::Continue
    });

    let persistence_source = install_live_snapshot_source(
        &application,
        persistence,
        default_working_directory.to_owned(),
        teardown_active,
    );

    if let Err(error) = persistence.borrow_mut().complete_launch() {
        eprintln!("zentty-linux: Failed to consume restore snapshot after launch: {error}");
    }
    main_loop.run();

    tick_source.remove();
    persistence_source.remove();
    application.borrow_mut().finish()
}

fn install_live_snapshot_source(
    application: &Rc<RefCell<ApplicationCoordinator>>,
    persistence: &Rc<RefCell<PersistenceCoordinator>>,
    default_working_directory: String,
    teardown_active: Rc<Cell<bool>>,
) -> glib::SourceId {
    let application = Rc::downgrade(application);
    let persistence = Rc::clone(persistence);
    let epoch = Instant::now();
    glib::timeout_add_local(Duration::from_millis(50), move || {
        if teardown_active.get() {
            return glib::ControlFlow::Continue;
        }
        let Some(application) = application.upgrade() else {
            return glib::ControlFlow::Break;
        };
        let snapshot = application.borrow().snapshot();
        if snapshot.windows.is_empty() {
            return glib::ControlFlow::Continue;
        }
        let now = epoch.elapsed();
        let mut persistence = persistence.borrow_mut();
        persistence.observe_live_snapshot(
            snapshot.windows,
            snapshot.active_window_id,
            &default_working_directory,
            now,
        );
        if let Err(error) = persistence.flush_live_snapshot_if_due(now, reference_timestamp()) {
            eprintln!("zentty-linux: Failed to persist live restore snapshot: {error}");
        }
        for error in persistence.drain_live_snapshot_errors() {
            eprintln!("zentty-linux: Failed to persist live restore snapshot: {error}");
        }
        glib::ControlFlow::Continue
    })
}

fn run() -> Result<(), String> {
    let options = parse_options()?;
    let config = ConfigStore::load_default()?;
    if let Some(warning) = config.warning.as_deref() {
        eprintln!("zentty-linux: {warning}");
    }
    eprintln!(
        "zentty-linux: config-loaded path={} automatic-clean-copy={}",
        config.path.display(),
        config.config.clipboard.always_clean_copies
    );
    diagnostics_runtime::install_local_panic_capture(config.config.error_reporting.enabled);
    diagnostics_runtime::maybe_inject_controlled_crash(config.config.error_reporting.enabled);
    if appearance_needs_startup_projection(&config.config.appearance) {
        let spec = config.config.appearance.theme_spec();
        ConfigStore::install_default_fallback_theme_if_referenced(&spec)?;
        let ghostty_config = ConfigStore::update_default_ghostty_theme(&spec)?;
        if let Some(opacity) = config.config.appearance.background_opacity {
            ConfigStore::update_default_ghostty_value("background-opacity", &opacity.to_string())?;
        }
        eprintln!(
            "zentty-linux: appearance-startup-projection path={}",
            ghostty_config.display()
        );
    }

    let state_directory = match &options.state_directory {
        Some(path) => path.clone(),
        None => default_state_directory()?,
    };
    let (persistence, launch) = PersistenceCoordinator::start(
        &state_directory,
        options.restore_enabled && config.config.restore.restore_workspace_on_launch,
        reference_timestamp(),
    )?;
    if let Some(warning) = launch.warning.as_deref() {
        eprintln!("zentty-linux: {warning}");
    }
    let persistence = Rc::new(RefCell::new(persistence));
    let default_working_directory = std::env::current_dir()
        .map_err(|error| format!("could not determine the launch working directory: {error}"))?;
    let default_working_directory = default_working_directory.to_string_lossy();

    // Ghostty owns process-global initialization that must precede GTK.
    let runtime = GhosttyRuntime::new(options.async_backend).map_err(|error| error.to_string())?;
    gtk::init().map_err(|error| format!("GTK initialization failed: {error}"))?;

    let active_window_id = launch.active_window_id.clone();
    let result = run_lifecycle_cycle(
        &runtime,
        &options,
        launch.windows,
        active_window_id.as_deref(),
        &persistence,
        &default_working_directory,
        &config,
    )?;
    drop(runtime);
    persistence.borrow_mut().save_clean_exit(
        result.windows,
        result.active_window_id,
        &default_working_directory,
        reference_timestamp(),
    )?;
    Ok(())
}

fn reference_timestamp() -> f64 {
    const APPLE_REFERENCE_EPOCH: f64 = 978_307_200.0;
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |duration| {
            duration.as_secs_f64() - APPLE_REFERENCE_EPOCH
        })
}

fn appearance_needs_startup_projection(appearance: &zentty_core::AppearanceConfig) -> bool {
    appearance != &zentty_core::AppearanceConfig::default()
}

fn main() -> ExitCode {
    match sleep_inhibitor::run_helper_if_requested() {
        Ok(true) => return ExitCode::SUCCESS,
        Ok(false) => {}
        Err(error) => {
            eprintln!("zentty-linux: {error}");
            return ExitCode::FAILURE;
        }
    }
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
    use super::{appearance_needs_startup_projection, required_argument};

    #[test]
    fn required_arguments_reject_missing_values() {
        let mut missing = Vec::<String>::new().into_iter();
        assert_eq!(
            required_argument(&mut missing, "--command"),
            Err("--command requires a value".to_owned())
        );
    }

    #[test]
    fn only_explicit_nondefault_appearance_projects_before_ghostty_starts() {
        let mut appearance = zentty_core::AppearanceConfig::default();
        assert!(!appearance_needs_startup_projection(&appearance));
        appearance.preferred_dark_theme_name = Some("Abernathy".to_owned());
        assert!(appearance_needs_startup_projection(&appearance));
    }
}
