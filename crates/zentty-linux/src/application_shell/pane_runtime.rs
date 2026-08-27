use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::time::Duration;

use gtk::{glib, prelude::*};
use zentty_core::{ClosePaneOutcome, TerminalProgressState};
use zentty_ghostty::{GhosttyRuntime, GhosttySurface, ProgressState, SurfaceConfig};

use crate::pane_controls::PaneFrame;

use super::{ApplicationShell, observe_ghostty_search_state, unix_time_ms};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum RegistrationDecision {
    Register,
    #[default]
    RejectDuplicate,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum RemovalDecision {
    Remove,
    #[default]
    IgnoreStale,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ChildExitDisposition {
    CloseWorkspacePane,
    RecoverFailedRestore,
    ReturnRestoredAgentToShell,
    PreserveTmuxTeammate,
    DisposeDuringShutdown,
    #[default]
    IgnoreStale,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ChildExitContext {
    registration: ChildRegistration,
    lifecycle: ChildLifecycle,
    ownership: ChildOwnership,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ChildRegistration {
    Registered,
    #[default]
    Stale,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ChildLifecycle {
    #[default]
    Active,
    ShuttingDown,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ChildOwnership {
    #[default]
    Ordinary,
    TmuxTeammate,
    PendingRestore,
    RunningRestore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RestoreLaunchState {
    Pending,
    Running,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RestoreLaunch {
    command: String,
    state: RestoreLaunchState,
}

impl RestoreLaunch {
    fn confirm(&mut self) -> bool {
        if self.state == RestoreLaunchState::Running {
            return false;
        }
        self.state = RestoreLaunchState::Running;
        true
    }
}

fn registration_decision(already_registered: bool) -> RegistrationDecision {
    if already_registered {
        RegistrationDecision::RejectDuplicate
    } else {
        RegistrationDecision::Register
    }
}

fn removal_decision(is_registered: bool) -> RemovalDecision {
    if is_registered {
        RemovalDecision::Remove
    } else {
        RemovalDecision::IgnoreStale
    }
}

fn child_exit_disposition(context: ChildExitContext) -> ChildExitDisposition {
    if context.registration == ChildRegistration::Stale {
        ChildExitDisposition::IgnoreStale
    } else if context.lifecycle == ChildLifecycle::ShuttingDown {
        ChildExitDisposition::DisposeDuringShutdown
    } else if context.ownership == ChildOwnership::PendingRestore {
        ChildExitDisposition::RecoverFailedRestore
    } else if context.ownership == ChildOwnership::RunningRestore {
        ChildExitDisposition::ReturnRestoredAgentToShell
    } else if context.ownership == ChildOwnership::TmuxTeammate {
        ChildExitDisposition::PreserveTmuxTeammate
    } else {
        ChildExitDisposition::CloseWorkspacePane
    }
}

/// Owns the per-window projection from durable pane IDs to live Ghostty/GTK
/// objects. `WorkspaceState` remains the authority for durable topology; this
/// coordinator is the sole authority for whether a pane has a live terminal.
pub(super) struct PaneRuntimeCoordinator {
    runtime: GhosttyRuntime,
    command: Option<String>,
    surfaces: BTreeMap<String, GhosttySurface>,
    frames: BTreeMap<String, PaneFrame>,
    focus_controllers: BTreeMap<String, gtk::EventControllerFocus>,
    deferred_panes: BTreeSet<String>,
    live_children: Cell<usize>,
    pending_prefills: BTreeMap<String, String>,
    pending_launches: BTreeMap<String, PendingPaneLaunch>,
    explicit_environments: BTreeMap<String, BTreeMap<String, String>>,
    restore_launches: BTreeMap<String, RestoreLaunch>,
}

pub(super) struct PendingPaneLaunch {
    pub(super) command: String,
    pub(super) environment: Vec<(String, String)>,
}

pub(crate) struct DetachedPaneRuntime {
    surface: GhosttySurface,
    pending_prefill: Option<String>,
    explicit_environment: Option<Rc<BTreeMap<String, String>>>,
}

impl PaneRuntimeCoordinator {
    pub(super) fn new(runtime: &GhosttyRuntime, command: Option<String>) -> Self {
        Self {
            runtime: runtime.clone(),
            command,
            surfaces: BTreeMap::new(),
            frames: BTreeMap::new(),
            focus_controllers: BTreeMap::new(),
            deferred_panes: BTreeSet::new(),
            live_children: Cell::new(0),
            pending_prefills: BTreeMap::new(),
            pending_launches: BTreeMap::new(),
            explicit_environments: BTreeMap::new(),
            restore_launches: BTreeMap::new(),
        }
    }

    pub(super) fn runtime(&self) -> GhosttyRuntime {
        self.runtime.clone()
    }

    pub(super) fn command(&self) -> Option<&str> {
        self.command.as_deref()
    }

    pub(super) fn live_children(&self) -> usize {
        self.live_children.get()
    }

    pub(super) fn surface(&self, pane_id: &str) -> Option<&GhosttySurface> {
        self.surfaces.get(pane_id)
    }

    pub(super) fn frame(&self, pane_id: &str) -> Option<&PaneFrame> {
        self.frames.get(pane_id)
    }

    pub(super) fn contains(&self, pane_id: &str) -> bool {
        self.surfaces.contains_key(pane_id)
    }

    pub(super) fn live_pane_ids(&self) -> Vec<String> {
        self.surfaces.keys().cloned().collect()
    }

    pub(super) fn is_deferred(&self, pane_id: &str) -> bool {
        self.deferred_panes.contains(pane_id)
    }

    pub(super) fn mark_deferred(&mut self, pane_id: &str) -> Result<(), String> {
        if self.contains(pane_id) {
            return Err(format!("pane {pane_id} already has a live surface"));
        }
        if !self.deferred_panes.insert(pane_id.to_owned()) {
            return Err(format!("pane {pane_id} is already launch-deferred"));
        }
        Ok(())
    }

    pub(super) fn insert(
        &mut self,
        pane_id: &str,
        surface: GhosttySurface,
        frame: PaneFrame,
        focus_controller: gtk::EventControllerFocus,
    ) -> Result<(), String> {
        if registration_decision(self.contains(pane_id)) == RegistrationDecision::RejectDuplicate {
            return Err(format!("pane {pane_id} already has a live surface"));
        }
        self.focus_controllers
            .insert(pane_id.to_owned(), focus_controller);
        self.frames.insert(pane_id.to_owned(), frame);
        self.surfaces.insert(pane_id.to_owned(), surface);
        self.deferred_panes.remove(pane_id);
        self.live_children.set(self.live_children.get() + 1);
        Ok(())
    }

    pub(super) fn queue_prefill(&mut self, pane_id: &str, prefill: String) {
        self.pending_prefills.insert(pane_id.to_owned(), prefill);
    }

    pub(super) fn cancel_prefill(&mut self, pane_id: &str) {
        self.pending_prefills.remove(pane_id);
    }

    pub(super) fn take_prefill(&mut self, pane_id: &str) -> Option<String> {
        self.pending_prefills.remove(pane_id)
    }

    pub(super) fn queue_launch(
        &mut self,
        pane_id: &str,
        command: String,
        environment: Vec<(String, String)>,
    ) {
        self.explicit_environments
            .insert(pane_id.to_owned(), environment.iter().cloned().collect());
        self.pending_launches.insert(
            pane_id.to_owned(),
            PendingPaneLaunch {
                command,
                environment,
            },
        );
    }

    pub(super) fn cancel_launch(&mut self, pane_id: &str) {
        self.pending_launches.remove(pane_id);
        self.explicit_environments.remove(pane_id);
    }

    pub(super) fn confirm_restored_agent(&mut self, pane_id: &str) -> bool {
        let Some(launch) = self.restore_launches.get_mut(pane_id) else {
            return false;
        };
        launch.confirm()
    }

    pub(super) fn take_launch(&mut self, pane_id: &str) -> Option<PendingPaneLaunch> {
        self.pending_launches.remove(pane_id)
    }

    pub(super) fn explicit_environment(&self, pane_id: &str) -> Option<&BTreeMap<String, String>> {
        self.explicit_environments.get(pane_id)
    }

    pub(super) fn detach_widgets(&mut self) {
        for (pane_id, controller) in std::mem::take(&mut self.focus_controllers) {
            if let Some(surface) = self.surfaces.get(&pane_id) {
                surface.widget().remove_controller(&controller);
            }
        }
        for frame in self.frames.values() {
            frame.detach_terminal();
        }
        self.frames.clear();
    }

    pub(super) fn detach_for_window_transfer(
        &mut self,
        pane_id: &str,
    ) -> Result<DetachedPaneRuntime, String> {
        if self.deferred_panes.contains(pane_id) {
            return Err(format!("pane {pane_id} is launch-deferred"));
        }
        if !self.surfaces.contains_key(pane_id)
            || !self.frames.contains_key(pane_id)
            || !self.focus_controllers.contains_key(pane_id)
        {
            return Err(format!("pane {pane_id} has no complete live runtime"));
        }
        let controller = self
            .focus_controllers
            .remove(pane_id)
            .ok_or_else(|| format!("pane {pane_id} lost its focus controller"))?;
        let surface = self
            .surfaces
            .remove(pane_id)
            .ok_or_else(|| format!("pane {pane_id} lost its live surface"))?;
        surface.widget().remove_controller(&controller);
        let frame = self
            .frames
            .remove(pane_id)
            .ok_or_else(|| format!("pane {pane_id} lost its terminal frame"))?;
        if let Some(parent) = frame
            .widget()
            .parent()
            .and_then(|parent| parent.downcast::<gtk::Box>().ok())
        {
            parent.remove(frame.widget());
        }
        frame.detach_terminal();
        surface.disconnect_callbacks();
        self.live_children
            .set(self.live_children.get().saturating_sub(1));
        Ok(DetachedPaneRuntime {
            surface,
            pending_prefill: self.pending_prefills.remove(pane_id),
            explicit_environment: self.explicit_environments.remove(pane_id).map(Rc::new),
        })
    }

    pub(super) fn adopt_window_transfer(
        shell: &Rc<RefCell<ApplicationShell>>,
        pane_id: &str,
        transfer: DetachedPaneRuntime,
    ) -> Result<(), (String, DetachedPaneRuntime)> {
        if shell.borrow().pane_runtime.contains(pane_id) {
            return Err((
                format!("pane {pane_id} already has a live surface"),
                transfer,
            ));
        }
        Self::connect_surface_callbacks(shell, pane_id, &transfer.surface);
        let focus_controller = Self::make_surface_focus_controller(shell, pane_id);
        transfer
            .surface
            .widget()
            .add_controller(focus_controller.clone());
        let frame = Self::create_pane_frame(shell, pane_id, transfer.surface.widget());
        let mut shell = shell.borrow_mut();
        if let Some(prefill) = transfer.pending_prefill {
            shell.pane_runtime.queue_prefill(pane_id, prefill);
        }
        if let Some(environment) = transfer.explicit_environment {
            shell
                .pane_runtime
                .explicit_environments
                .insert(pane_id.to_owned(), (*environment).clone());
        }
        shell
            .pane_runtime
            .focus_controllers
            .insert(pane_id.to_owned(), focus_controller);
        shell.pane_runtime.frames.insert(pane_id.to_owned(), frame);
        shell
            .pane_runtime
            .surfaces
            .insert(pane_id.to_owned(), transfer.surface);
        shell.pane_runtime.deferred_panes.remove(pane_id);
        shell
            .pane_runtime
            .live_children
            .set(shell.pane_runtime.live_children.get() + 1);
        Ok(())
    }

    pub(super) fn remove(
        &mut self,
        pane_id: &str,
        child_already_exited: bool,
    ) -> Result<bool, String> {
        if self.deferred_panes.remove(pane_id) {
            self.pending_prefills.remove(pane_id);
            self.pending_launches.remove(pane_id);
            self.explicit_environments.remove(pane_id);
            return Ok(true);
        }
        if removal_decision(self.contains(pane_id)) == RemovalDecision::IgnoreStale {
            return Ok(false);
        }
        if let Some(controller) = self.focus_controllers.remove(pane_id)
            && let Some(surface) = self.surfaces.get(pane_id)
        {
            surface.widget().remove_controller(&controller);
        }
        if let Some(frame) = self.frames.remove(pane_id) {
            if let Some(parent) = frame
                .widget()
                .parent()
                .and_then(|parent| parent.downcast::<gtk::Box>().ok())
            {
                parent.remove(frame.widget());
            }
            frame.detach_terminal();
        }
        self.pending_prefills.remove(pane_id);
        self.pending_launches.remove(pane_id);
        self.explicit_environments.remove(pane_id);
        let dispose_result = self
            .surfaces
            .remove(pane_id)
            .map(|surface| surface.dispose().map_err(|error| error.to_string()))
            .transpose();
        if !child_already_exited {
            self.live_children
                .set(self.live_children.get().saturating_sub(1));
        }
        dispose_result?;
        Ok(true)
    }

    pub(super) fn note_child_exit(&mut self, pane_id: &str) -> bool {
        if removal_decision(self.contains(pane_id)) == RemovalDecision::IgnoreStale {
            return false;
        }
        self.live_children
            .set(self.live_children.get().saturating_sub(1));
        true
    }

    pub(super) fn release_all(&mut self) -> Result<(), String> {
        self.deferred_panes.clear();
        let pane_ids = self.surfaces.keys().cloned().collect::<Vec<_>>();
        for pane_id in pane_ids {
            self.remove(&pane_id, false)?;
        }
        Ok(())
    }

    pub(super) fn create_surface(
        shell: &Rc<RefCell<ApplicationShell>>,
        pane_id: &str,
    ) -> Result<(), String> {
        Self::create_surface_configured(shell, pane_id, None)
    }

    pub(super) fn create_surface_with_command(
        shell: &Rc<RefCell<ApplicationShell>>,
        pane_id: &str,
        command: String,
    ) -> Result<(), String> {
        Self::create_surface_configured(shell, pane_id, Some(command))
    }

    fn create_surface_configured(
        shell: &Rc<RefCell<ApplicationShell>>,
        pane_id: &str,
        command: Option<String>,
    ) -> Result<(), String> {
        let (runtime, config) = {
            let mut shell = shell.borrow_mut();
            if shell.pane_runtime.contains(pane_id) {
                return Err(format!("pane {pane_id} already has a live surface"));
            }
            let runtime = shell.pane_runtime.runtime();
            let mut config = Self::surface_config(&mut shell, pane_id)?;
            if command.is_some() {
                config.command = command;
            }
            (runtime, config)
        };
        eprintln!(
            "zentty-linux: surface-config pane={pane_id} cwd={}",
            config.working_directory.as_deref().unwrap_or("none")
        );
        let surface = match runtime.create_surface(&config) {
            Ok(surface) => surface,
            Err(error) => {
                shell.borrow_mut().agent_events.unregister_pane(pane_id);
                return Err(error.to_string());
            }
        };

        Self::connect_surface_callbacks(shell, pane_id, &surface);
        let focus_controller = Self::make_surface_focus_controller(shell, pane_id);
        surface.widget().add_controller(focus_controller.clone());
        let frame = Self::create_pane_frame(shell, pane_id, surface.widget());

        {
            let mut shell_ref = shell.borrow_mut();
            shell_ref
                .pane_runtime
                .insert(pane_id, surface, frame, focus_controller)?;
        }
        eprintln!(
            "zentty-linux: surface-owned pane={pane_id} live={}",
            shell.borrow().pane_runtime.live_children()
        );
        Ok(())
    }

    fn surface_config(
        shell: &mut ApplicationShell,
        pane_id: &str,
    ) -> Result<SurfaceConfig, String> {
        let worklane_id = shell
            .state
            .worklane_id_for_pane(pane_id)
            .ok_or_else(|| format!("pane {pane_id} has no worklane"))?
            .to_owned();
        let mut environment = shell
            .agent_events
            .environment_for_pane(&worklane_id, pane_id)?;
        shell.refresh_opencode_theme_sources();
        let pending_launch = shell.pane_runtime.take_launch(pane_id);
        if let Some(launch) = &pending_launch {
            for (key, value) in &launch.environment {
                if let Some(existing) = environment.iter_mut().find(|(name, _)| name == key) {
                    existing.1.clone_from(value);
                } else {
                    environment.push((key.clone(), value.clone()));
                }
            }
        }
        let restored_command = shell.restored_pane_commands.remove(pane_id);
        if shell.pane_runtime.command().is_none()
            && let Some(command) = &restored_command
        {
            shell.pane_runtime.restore_launches.insert(
                pane_id.to_owned(),
                RestoreLaunch {
                    command: command.clone(),
                    state: RestoreLaunchState::Pending,
                },
            );
            eprintln!("zentty-linux: agent-resume-launch pane={pane_id} command={command}");
        }
        let restored_surface_command = restored_command.as_deref().map(|command| {
            let command = format!("exec env PATH=\"$ZENTTY_ALL_WRAPPER_BIN_DIRS:$PATH\" {command}");
            crate::tmux_compat::shell_wrapped_command(
                &command,
                std::env::var("SHELL").ok().as_deref(),
            )
        });
        Ok(SurfaceConfig {
            command: shell
                .pane_runtime
                .command()
                .map(str::to_owned)
                .or_else(|| pending_launch.map(|launch| launch.command))
                .or(restored_surface_command),
            title: zentty_core::PRODUCT_NAME.to_owned(),
            working_directory: shell
                .state
                .pane(pane_id)
                .and_then(|pane| pane.working_directory.clone()),
            environment,
        })
    }

    fn connect_surface_callbacks(
        shell: &Rc<RefCell<ApplicationShell>>,
        pane_id: &str,
        surface: &GhosttySurface,
    ) {
        let ready_id = pane_id.to_owned();
        let weak = Rc::downgrade(shell);
        surface.on_initialized(move || {
            eprintln!("zentty-linux: terminal-ready");
            eprintln!("zentty-linux: terminal-ready-pane={ready_id}");
            let weak = weak.clone();
            let ready_id = ready_id.clone();
            glib::idle_add_local_once(move || {
                let Some(shell) = weak.upgrade() else {
                    return;
                };
                Self::schedule_pending_prefill(&shell, &ready_id);
                let mut shell = shell.borrow_mut();
                if shell.shutting_down {
                    return;
                }
                if shell.pending_initial_focus.as_deref() == Some(&ready_id) {
                    shell.pending_initial_focus = None;
                    if shell.state.select_pane(&ready_id) {
                        shell.refresh_sidebar_metadata();
                    }
                    shell.focus_selected_surface_unchecked();
                    eprintln!("zentty-linux: focus-pane pane={ready_id}");
                }
                if let Some(surface) = shell.pane_runtime.surface(&ready_id) {
                    observe_ghostty_search_state(
                        surface.widget(),
                        &ready_id,
                        shell.self_handle.borrow().clone(),
                    );
                }
            });
        });
        let title_id = pane_id.to_owned();
        let weak = Rc::downgrade(shell);
        surface.on_title_changed(move |title| {
            eprintln!("zentty-linux: title={title}");
            eprintln!("zentty-linux: title-pane={title_id} value={title}");
            let weak = weak.clone();
            let title_id = title_id.clone();
            let title = title.clone();
            glib::idle_add_local_once(move || {
                if let Some(shell) = weak.upgrade() {
                    let animate = {
                        let mut shell_ref = shell.borrow_mut();
                        if shell_ref.shutting_down {
                            return;
                        }
                        let now = unix_time_ms();
                        let agent_changed = shell_ref
                            .state
                            .reconcile_terminal_title(&title_id, &title, now);
                        if agent_changed {
                            shell_ref.schedule_codex_transcript_enrichment(&title_id);
                        }
                        let display_title = zentty_core::stable_codex_terminal_title(&title)
                            .unwrap_or_else(|| title.clone());
                        let title_changed =
                            shell_ref.state.set_pane_title(&title_id, &display_title);
                        if title_changed {
                            super::project_context_runtime::mark_pane_for_process_refresh(
                                &mut shell_ref,
                                &title_id,
                            );
                        }
                        if title_changed || agent_changed {
                            shell_ref.refresh_sidebar_metadata();
                        }
                        shell_ref.reconcile_codex_title_animation(&title_id, &title)
                    };
                    if animate {
                        ApplicationShell::ensure_codex_title_animation_tick(&shell);
                    }
                }
            });
        });
        Self::connect_surface_progress_callback(shell, pane_id, surface);
        Self::connect_surface_notification_callback(shell, pane_id, surface);
        let menu_id = pane_id.to_owned();
        let weak = Rc::downgrade(shell);
        surface.on_context_menu(move || {
            let Some(shell) = weak.upgrade() else {
                return;
            };
            let has_selection = {
                let shell_ref = shell.borrow();
                shell_ref
                    .pane_runtime
                    .surface(&menu_id)
                    .is_some_and(|surface| {
                        surface
                            .read_selection()
                            .is_ok_and(|selection| !selection.is_empty())
                    })
            };
            let shell_ref = shell.borrow();
            if let Some(router) = &shell_ref.action_router {
                router.set_native_copy_enabled(has_selection);
            }
            eprintln!(
                "zentty-linux: terminal-context-menu pane={menu_id} selection={has_selection}"
            );
        });
        let weak = Rc::downgrade(shell);
        let exited_id = pane_id.to_owned();
        surface.on_child_exited(move || {
            eprintln!("zentty-linux: child-exited");
            eprintln!("zentty-linux: child-exited-pane={exited_id}");
            let weak = weak.clone();
            let exited_id = exited_id.clone();
            // A child exit owns pane teardown. Do not queue it at idle
            // priority: a continuously rendering surface can otherwise starve
            // the callback indefinitely on a slow or instrumented renderer.
            glib::timeout_add_local_once(Duration::ZERO, move || {
                if let Some(shell) = weak.upgrade() {
                    Self::handle_child_exit(&shell, &exited_id);
                }
            });
        });
    }

    fn connect_surface_progress_callback(
        shell: &Rc<RefCell<ApplicationShell>>,
        pane_id: &str,
        surface: &GhosttySurface,
    ) {
        let progress_id = pane_id.to_owned();
        let weak = Rc::downgrade(shell);
        surface.on_progress_report(move |report| {
            let state = match report.state {
                ProgressState::Remove => TerminalProgressState::Remove,
                ProgressState::Set => TerminalProgressState::Set,
                ProgressState::Error => TerminalProgressState::Error,
                ProgressState::Indeterminate => TerminalProgressState::Indeterminate,
                ProgressState::Pause => TerminalProgressState::Pause,
            };
            let state_name = match state {
                TerminalProgressState::Remove => "remove",
                TerminalProgressState::Set => "set",
                TerminalProgressState::Error => "error",
                TerminalProgressState::Indeterminate => "indeterminate",
                TerminalProgressState::Pause => "pause",
            };
            eprintln!(
                "zentty-linux: terminal-progress pane={} state={} progress={}",
                progress_id,
                state_name,
                report
                    .progress
                    .map_or_else(|| "none".to_owned(), |value| value.to_string())
            );
            let weak = weak.clone();
            let progress_id = progress_id.clone();
            glib::idle_add_local_once(move || {
                let Some(shell) = weak.upgrade() else {
                    return;
                };
                let mut shell = shell.borrow_mut();
                if shell.shutting_down {
                    return;
                }
                if shell
                    .state
                    .reconcile_terminal_progress(&progress_id, state, unix_time_ms())
                {
                    shell.refresh_sidebar_metadata();
                }
            });
        });
    }

    fn connect_surface_notification_callback(
        shell: &Rc<RefCell<ApplicationShell>>,
        pane_id: &str,
        surface: &GhosttySurface,
    ) {
        let notification_id = pane_id.to_owned();
        let weak = Rc::downgrade(shell);
        surface.on_desktop_notification(move |title, body| {
            eprintln!(
                "zentty-linux: terminal-notification pane={notification_id} title={title:?} body={body:?}"
            );
            let weak = weak.clone();
            let notification_id = notification_id.clone();
            glib::idle_add_local_once(move || {
                let Some(shell) = weak.upgrade() else {
                    return;
                };
                let mut shell = shell.borrow_mut();
                if shell.shutting_down {
                    return;
                }
                if shell.state.reconcile_terminal_notification(
                    &notification_id,
                    Some(&title),
                    Some(&body),
                    unix_time_ms(),
                ) {
                    shell.refresh_sidebar_metadata();
                }
            });
        });
    }

    fn make_surface_focus_controller(
        shell: &Rc<RefCell<ApplicationShell>>,
        pane_id: &str,
    ) -> gtk::EventControllerFocus {
        let focus_controller = gtk::EventControllerFocus::new();
        let weak = Rc::downgrade(shell);
        let focus_id = pane_id.to_owned();
        focus_controller.connect_enter(move |controller| {
            let weak = weak.clone();
            let focus_id = focus_id.clone();
            let controller = controller.clone();
            glib::idle_add_local_once(move || {
                let Some(shell) = weak.upgrade() else {
                    return;
                };
                let apply = {
                    let shell = shell.borrow();
                    let blocker = if shell.shutting_down {
                        SurfaceFocusBlocker::Shutdown
                    } else if shell.global_search.state().visible {
                        SurfaceFocusBlocker::GlobalSearch
                    } else if shell.command_palette.is_visible() {
                        SurfaceFocusBlocker::CommandPalette
                    } else {
                        SurfaceFocusBlocker::None
                    };
                    surface_focus_event_should_apply(blocker, controller.contains_focus())
                };
                if !apply {
                    return;
                }
                let changed = shell.borrow_mut().state.select_pane(&focus_id);
                if changed {
                    eprintln!("zentty-linux: focus-pane pane={focus_id}");
                    shell.borrow().refresh_sidebar_metadata();
                }
            });
        });
        focus_controller
    }

    fn schedule_pending_prefill(shell: &Rc<RefCell<ApplicationShell>>, pane_id: &str) {
        if !shell
            .borrow()
            .pane_runtime
            .pending_prefills
            .contains_key(pane_id)
        {
            return;
        }
        let weak = Rc::downgrade(shell);
        let pane_id = pane_id.to_owned();
        let mut attempts = 0_u8;
        glib::timeout_add_local(std::time::Duration::from_millis(10), move || {
            let Some(shell) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            attempts = attempts.saturating_add(1);
            let process_started = shell
                .borrow()
                .pane_runtime
                .surface(&pane_id)
                .and_then(GhosttySurface::foreground_process_id)
                .is_some();
            if !process_started && attempts < 100 {
                return glib::ControlFlow::Continue;
            }
            let prefill = shell.borrow_mut().pane_runtime.take_prefill(&pane_id);
            let Some(prefill) = prefill else {
                return glib::ControlFlow::Break;
            };
            let shell = shell.borrow();
            let Some(surface) = shell.pane_runtime.surface(&pane_id) else {
                return glib::ControlFlow::Break;
            };
            if let Err(error) = surface.send_text(&prefill) {
                write_prefill_receipt(&format!(
                    "zentty-linux: pane-prefill pane={pane_id} failed={error}\n"
                ));
            } else {
                write_prefill_receipt(&format!(
                    "zentty-linux: pane-prefill pane={pane_id} process-started={process_started} bytes={}\n",
                    prefill.len()
                ));
            }
            glib::ControlFlow::Break
        });
    }

    fn create_pane_frame(
        shell: &Rc<RefCell<ApplicationShell>>,
        pane_id: &str,
        terminal: &gtk::Widget,
    ) -> PaneFrame {
        let weak = Rc::downgrade(shell);
        let control_pane_id = pane_id.to_owned();
        let pointer_weak = Rc::downgrade(shell);
        let pointer_pane_id = pane_id.to_owned();
        let frame = PaneFrame::new(
            pane_id,
            terminal,
            move |action| {
                if let Some(shell) = weak.upgrade() {
                    ApplicationShell::activate_pane_control(&shell, &control_pane_id, action);
                }
            },
            move |present| {
                let pointer_weak = pointer_weak.clone();
                let pointer_pane_id = pointer_pane_id.clone();
                glib::idle_add_local_once(move || {
                    if let Some(shell) = pointer_weak.upgrade() {
                        ApplicationShell::handle_pane_pointer_presence(
                            &shell,
                            &pointer_pane_id,
                            present,
                        );
                    }
                });
            },
        );
        super::remote_paste::install(shell, pane_id, frame.widget().upcast_ref(), terminal);
        frame
    }

    fn present_restore_failure(shell: &Rc<RefCell<ApplicationShell>>, pane_id: &str) {
        let retry_weak = Rc::downgrade(shell);
        let retry_pane = pane_id.to_owned();
        let shell_weak = Rc::downgrade(shell);
        let shell_pane = pane_id.to_owned();
        let remove_weak = Rc::downgrade(shell);
        let remove_pane = pane_id.to_owned();
        let shell_ref = shell.borrow();
        let Some(frame) = shell_ref.pane_runtime.frame(pane_id) else {
            return;
        };
        frame.show_restore_failure(
            move || {
                let Some(shell) = retry_weak.upgrade() else {
                    return;
                };
                let command = {
                    let mut shell_ref = shell.borrow_mut();
                    let Some(command) = shell_ref.failed_restore_commands.remove(&retry_pane)
                    else {
                        return;
                    };
                    if let Err(error) = shell_ref.pane_runtime.remove(&retry_pane, false) {
                        eprintln!(
                            "zentty-linux: agent-restore-retry pane={retry_pane} result=failed-cleanup detail={error}"
                        );
                        return;
                    }
                    shell_ref
                        .restored_pane_commands
                        .insert(retry_pane.clone(), command.clone());
                    command
                };
                eprintln!("zentty-linux: agent-restore-retry pane={retry_pane} result=started");
                if let Err(error) = Self::create_surface(&shell, &retry_pane) {
                    eprintln!(
                        "zentty-linux: agent-restore-retry pane={retry_pane} result=failed detail={error}"
                    );
                    {
                        let mut shell_ref = shell.borrow_mut();
                        shell_ref
                            .pane_runtime
                            .restore_launches
                            .remove(&retry_pane);
                        shell_ref.restored_pane_commands.remove(&retry_pane);
                        shell_ref
                            .failed_restore_commands
                            .insert(retry_pane.clone(), command);
                    }
                    if let Err(fallback_error) = Self::create_surface(&shell, &retry_pane) {
                        eprintln!(
                            "zentty-linux: agent-restore-retry pane={retry_pane} result=fallback-failed detail={fallback_error}"
                        );
                        shell.borrow().main_loop.quit();
                    } else {
                        shell.borrow().render();
                        Self::present_restore_failure(&shell, &retry_pane);
                    }
                } else {
                    shell.borrow().render();
                }
            },
            move || {
                let Some(shell) = shell_weak.upgrade() else {
                    return;
                };
                let mut shell_ref = shell.borrow_mut();
                shell_ref.failed_restore_commands.remove(&shell_pane);
                if let Some(frame) = shell_ref.pane_runtime.frame(&shell_pane) {
                    frame.clear_restore_failure();
                }
                shell_ref.focus_selected_surface();
                eprintln!(
                    "zentty-linux: agent-restore-recovery pane={shell_pane} choice=open-shell"
                );
            },
            move || {
                if let Some(shell) = remove_weak.upgrade() {
                    eprintln!(
                        "zentty-linux: agent-restore-recovery pane={remove_pane} choice=remove-pane"
                    );
                    ApplicationShell::request_close_pane(&shell, &remove_pane);
                }
            },
        );
    }

    fn child_exit_context(shell: &ApplicationShell, pane_id: &str) -> ChildExitContext {
        let ownership = if let Some(launch) = shell.pane_runtime.restore_launches.get(pane_id) {
            match launch.state {
                RestoreLaunchState::Pending => ChildOwnership::PendingRestore,
                RestoreLaunchState::Running => ChildOwnership::RunningRestore,
            }
        } else if shell
            .tmux_compat
            .retains_exited_teammate(&shell.state, pane_id)
        {
            ChildOwnership::TmuxTeammate
        } else {
            ChildOwnership::Ordinary
        };
        ChildExitContext {
            registration: if shell.pane_runtime.contains(pane_id) {
                ChildRegistration::Registered
            } else {
                ChildRegistration::Stale
            },
            lifecycle: if shell.shutting_down {
                ChildLifecycle::ShuttingDown
            } else {
                ChildLifecycle::Active
            },
            ownership,
        }
    }

    fn return_completed_restore_to_shell(shell: &Rc<RefCell<ApplicationShell>>, pane_id: &str) {
        let mut shell_ref = shell.borrow_mut();
        let _ = shell_ref.pane_runtime.note_child_exit(pane_id);
        shell_ref.agent_events.unregister_pane(pane_id);
        shell_ref.pane_runtime.restore_launches.remove(pane_id);
        shell_ref.failed_restore_commands.remove(pane_id);
        let _ = shell_ref.state.clear_failed_agent_restore(pane_id);
        if let Some(working_directory) = shell_ref
            .state
            .effective_working_directory_for_pane(pane_id)
        {
            eprintln!(
                "zentty-linux: pane-context-owner pane={pane_id} owner=shell cwd={working_directory}"
            );
        }
        if let Err(error) = shell_ref.pane_runtime.remove(pane_id, true) {
            eprintln!("zentty-linux: completed restore cleanup failed: {error}");
            shell_ref.main_loop.quit();
            return;
        }
        drop(shell_ref);
        if let Err(error) = Self::create_surface(shell, pane_id) {
            eprintln!("zentty-linux: completed restore shell fallback failed: {error}");
            shell.borrow().main_loop.quit();
            return;
        }
        let shell_ref = shell.borrow();
        shell_ref.render();
        shell_ref.focus_selected_surface();
        eprintln!(
            "zentty-linux: agent-restore-launch pane={pane_id} result=completed fallback=shell"
        );
    }

    fn handle_child_exit(shell: &Rc<RefCell<ApplicationShell>>, pane_id: &str) {
        let mut shell_ref = shell.borrow_mut();
        match child_exit_disposition(Self::child_exit_context(&shell_ref, pane_id)) {
            ChildExitDisposition::IgnoreStale => {
                eprintln!("zentty-linux: child-exit-after-dispose pane={pane_id} ignored");
                return;
            }
            ChildExitDisposition::DisposeDuringShutdown => {
                let _ = shell_ref.pane_runtime.note_child_exit(pane_id);
                shell_ref.agent_events.unregister_pane(pane_id);
                if let Err(error) = shell_ref.pane_runtime.remove(pane_id, true) {
                    eprintln!("zentty-linux: shutdown child-exit cleanup failed: {error}");
                    shell_ref.main_loop.quit();
                }
                return;
            }
            ChildExitDisposition::PreserveTmuxTeammate => {
                let _ = shell_ref.pane_runtime.note_child_exit(pane_id);
                shell_ref.agent_events.unregister_pane(pane_id);
                if let Err(error) = shell_ref.pane_runtime.remove(pane_id, true) {
                    eprintln!("zentty-linux: tmux teammate child-exit cleanup failed: {error}");
                    shell_ref.main_loop.quit();
                } else {
                    eprintln!("zentty-linux: tmux-teammate-exited pane={pane_id} awaiting=respawn");
                }
                return;
            }
            ChildExitDisposition::RecoverFailedRestore => {
                let _ = shell_ref.pane_runtime.note_child_exit(pane_id);
                shell_ref.agent_events.unregister_pane(pane_id);
                let restore_command = shell_ref
                    .pane_runtime
                    .restore_launches
                    .remove(pane_id)
                    .map_or_else(String::new, |launch| launch.command);
                shell_ref
                    .failed_restore_commands
                    .insert(pane_id.to_owned(), restore_command);
                let _ = shell_ref.state.clear_failed_agent_restore(pane_id);
                if let Err(error) = shell_ref.pane_runtime.remove(pane_id, true) {
                    eprintln!("zentty-linux: failed restore cleanup failed: {error}");
                    shell_ref.main_loop.quit();
                    return;
                }
                eprintln!(
                    "zentty-linux: agent-restore-launch pane={pane_id} result=failed fallback=shell"
                );
                drop(shell_ref);
                if let Err(error) = Self::create_surface(shell, pane_id) {
                    eprintln!("zentty-linux: failed restore shell fallback failed: {error}");
                    shell.borrow().main_loop.quit();
                    return;
                }
                let shell_ref = shell.borrow();
                shell_ref.render();
                shell_ref.focus_selected_surface();
                drop(shell_ref);
                Self::present_restore_failure(shell, pane_id);
                return;
            }
            ChildExitDisposition::ReturnRestoredAgentToShell => {
                drop(shell_ref);
                Self::return_completed_restore_to_shell(shell, pane_id);
                return;
            }
            ChildExitDisposition::CloseWorkspacePane => {
                let _ = shell_ref.pane_runtime.note_child_exit(pane_id);
            }
        }
        let outcome = shell_ref.state.close_pane_after_child_exit(pane_id);
        shell_ref.agent_events.unregister_pane(pane_id);
        if let Err(error) = shell_ref.pane_runtime.remove(pane_id, true) {
            eprintln!("zentty-linux: child-exit cleanup failed: {error}");
            shell_ref.main_loop.quit();
            return;
        }
        match outcome {
            ClosePaneOutcome::Closed => {
                shell_ref.render();
                shell_ref.focus_selected_surface();
            }
            ClosePaneOutcome::CloseWindow => shell_ref.main_loop.quit(),
            ClosePaneOutcome::NotFound => {}
        }
    }
}

fn write_prefill_receipt(receipt: &str) {
    use std::io::Write;

    // Ghostty's native Debug logger shares stderr with the Rust shell. Build
    // the complete non-secret receipt first, then submit its short record in a
    // single write so native records cannot be spliced between its fields.
    let _ = std::io::stderr().lock().write_all(receipt.as_bytes());
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SurfaceFocusBlocker {
    None,
    Shutdown,
    GlobalSearch,
    CommandPalette,
}

fn surface_focus_event_should_apply(
    blocker: SurfaceFocusBlocker,
    surface_contains_focus: bool,
) -> bool {
    blocker == SurfaceFocusBlocker::None && surface_contains_focus
}

#[cfg(test)]
mod tests {
    use super::{
        ChildExitContext, ChildExitDisposition, ChildLifecycle, ChildOwnership, ChildRegistration,
        RegistrationDecision, RemovalDecision, RestoreLaunch, RestoreLaunchState,
        SurfaceFocusBlocker, child_exit_disposition, registration_decision, removal_decision,
        surface_focus_event_should_apply,
    };

    #[test]
    fn duplicate_registration_is_rejected() {
        assert_eq!(
            registration_decision(true),
            RegistrationDecision::RejectDuplicate
        );
    }

    #[test]
    fn absent_registration_is_admitted() {
        assert_eq!(registration_decision(false), RegistrationDecision::Register);
    }

    #[test]
    fn stale_and_repeated_close_is_ignored() {
        assert_eq!(removal_decision(false), RemovalDecision::IgnoreStale);
    }

    #[test]
    fn live_close_is_removed() {
        assert_eq!(removal_decision(true), RemovalDecision::Remove);
    }

    #[test]
    fn callback_after_detach_is_ignored_as_stale() {
        assert_eq!(
            child_exit_disposition(ChildExitContext::default()),
            ChildExitDisposition::IgnoreStale
        );
        assert_eq!(
            child_exit_disposition(ChildExitContext {
                lifecycle: ChildLifecycle::ShuttingDown,
                ownership: ChildOwnership::PendingRestore,
                ..ChildExitContext::default()
            }),
            ChildExitDisposition::IgnoreStale
        );
    }

    #[test]
    fn child_exit_during_shutdown_disposes_without_mutating_workspace() {
        assert_eq!(
            child_exit_disposition(ChildExitContext {
                registration: ChildRegistration::Registered,
                lifecycle: ChildLifecycle::ShuttingDown,
                ownership: ChildOwnership::PendingRestore,
            }),
            ChildExitDisposition::DisposeDuringShutdown
        );
    }

    #[test]
    fn active_child_exit_closes_the_workspace_pane() {
        assert_eq!(
            child_exit_disposition(ChildExitContext {
                registration: ChildRegistration::Registered,
                ..ChildExitContext::default()
            }),
            ChildExitDisposition::CloseWorkspacePane
        );
    }

    #[test]
    fn exited_tmux_teammate_is_preserved_for_source_respawn() {
        assert_eq!(
            child_exit_disposition(ChildExitContext {
                registration: ChildRegistration::Registered,
                ownership: ChildOwnership::TmuxTeammate,
                ..ChildExitContext::default()
            }),
            ChildExitDisposition::PreserveTmuxTeammate
        );
    }

    #[test]
    fn failed_restore_launch_preserves_workspace_for_shell_fallback() {
        assert_eq!(
            child_exit_disposition(ChildExitContext {
                registration: ChildRegistration::Registered,
                ownership: ChildOwnership::PendingRestore,
                ..ChildExitContext::default()
            }),
            ChildExitDisposition::RecoverFailedRestore
        );
    }

    #[test]
    fn confirmed_restore_exit_returns_the_same_pane_to_a_shell() {
        assert_eq!(
            child_exit_disposition(ChildExitContext {
                registration: ChildRegistration::Registered,
                ownership: ChildOwnership::RunningRestore,
                ..ChildExitContext::default()
            }),
            ChildExitDisposition::ReturnRestoredAgentToShell
        );
    }

    #[test]
    fn restore_confirmation_is_one_way_and_retains_command_ownership() {
        let mut launch = RestoreLaunch {
            command: "codex resume bro".to_owned(),
            state: RestoreLaunchState::Pending,
        };
        assert!(launch.confirm());
        assert_eq!(launch.state, RestoreLaunchState::Running);
        assert_eq!(launch.command, "codex resume bro");
        assert!(!launch.confirm());
        assert_eq!(launch.state, RestoreLaunchState::Running);
    }

    #[test]
    fn deferred_surface_focus_cannot_steal_focus_from_overlays() {
        assert!(surface_focus_event_should_apply(
            SurfaceFocusBlocker::None,
            true
        ));
        for blocker in [
            SurfaceFocusBlocker::Shutdown,
            SurfaceFocusBlocker::GlobalSearch,
            SurfaceFocusBlocker::CommandPalette,
        ] {
            assert!(!surface_focus_event_should_apply(blocker, true));
        }
        assert!(!surface_focus_event_should_apply(
            SurfaceFocusBlocker::None,
            false
        ));
    }
}
