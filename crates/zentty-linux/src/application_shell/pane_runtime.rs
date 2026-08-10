use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

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
    PreserveTmuxTeammate,
    DisposeDuringShutdown,
    #[default]
    IgnoreStale,
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

fn child_exit_disposition(
    is_registered: bool,
    shutting_down: bool,
    preserve_tmux_teammate: bool,
) -> ChildExitDisposition {
    if !is_registered {
        ChildExitDisposition::IgnoreStale
    } else if shutting_down {
        ChildExitDisposition::DisposeDuringShutdown
    } else if preserve_tmux_teammate {
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
}

pub(crate) struct DetachedPaneRuntime {
    surface: GhosttySurface,
    pending_prefill: Option<String>,
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

        let mut shell = shell.borrow_mut();
        shell
            .pane_runtime
            .insert(pane_id, surface, frame, focus_controller)?;
        eprintln!(
            "zentty-linux: surface-owned pane={pane_id} live={}",
            shell.pane_runtime.live_children()
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
        let environment = shell
            .agent_events
            .environment_for_pane(&worklane_id, pane_id)?;
        let restored_command = shell.restored_pane_commands.get(pane_id).cloned();
        if shell.pane_runtime.command().is_none()
            && let Some(command) = &restored_command
        {
            eprintln!("zentty-linux: agent-resume-launch pane={pane_id} command={command}");
        }
        Ok(SurfaceConfig {
            command: shell
                .pane_runtime
                .command()
                .map(str::to_owned)
                .or(restored_command),
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
                Self::apply_pending_restore_prefill(&shell, &ready_id);
                let shell = shell.borrow();
                if shell.shutting_down {
                    return;
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
                    let mut shell = shell.borrow_mut();
                    if shell.shutting_down {
                        return;
                    }
                    let now = unix_time_ms();
                    let agent_changed =
                        shell.state.reconcile_terminal_title(&title_id, &title, now);
                    shell.schedule_codex_transcript_enrichment(&title_id);
                    if shell.state.set_pane_title(&title_id, &title) || agent_changed {
                        shell.refresh_sidebar_metadata();
                    }
                }
            });
        });
        Self::connect_surface_progress_callback(shell, pane_id, surface);
        Self::connect_surface_notification_callback(shell, pane_id, surface);
        let weak = Rc::downgrade(shell);
        let exited_id = pane_id.to_owned();
        surface.on_child_exited(move || {
            eprintln!("zentty-linux: child-exited");
            eprintln!("zentty-linux: child-exited-pane={exited_id}");
            let weak = weak.clone();
            let exited_id = exited_id.clone();
            glib::idle_add_local_once(move || {
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
                if shell.borrow().shutting_down || shell.borrow().global_search.state().visible {
                    return;
                }
                if controller.contains_focus() {
                    let changed = shell.borrow_mut().state.select_pane(&focus_id);
                    if changed {
                        eprintln!("zentty-linux: focus-pane pane={focus_id}");
                        shell.borrow().refresh_sidebar_metadata();
                    }
                }
            });
        });
        focus_controller
    }

    fn apply_pending_restore_prefill(shell: &Rc<RefCell<ApplicationShell>>, pane_id: &str) {
        let prefill = shell.borrow_mut().pane_runtime.take_prefill(pane_id);
        let Some(prefill) = prefill else {
            return;
        };
        let shell = shell.borrow();
        let Some(surface) = shell.pane_runtime.surface(pane_id) else {
            return;
        };
        if let Err(error) = surface.send_text(&prefill) {
            eprintln!("zentty-linux: restore-prefill pane={pane_id} failed={error}");
        } else {
            eprintln!("zentty-linux: restore-prefill pane={pane_id} text={prefill}");
        }
    }

    fn create_pane_frame(
        shell: &Rc<RefCell<ApplicationShell>>,
        pane_id: &str,
        terminal: &gtk::Widget,
    ) -> PaneFrame {
        let weak = Rc::downgrade(shell);
        let control_pane_id = pane_id.to_owned();
        let frame = PaneFrame::new(pane_id, terminal, move |action| {
            if let Some(shell) = weak.upgrade() {
                ApplicationShell::activate_pane_control(&shell, &control_pane_id, action);
            }
        });
        super::remote_paste::install(shell, pane_id, frame.widget().upcast_ref());
        frame
    }

    fn handle_child_exit(shell: &Rc<RefCell<ApplicationShell>>, pane_id: &str) {
        let mut shell_ref = shell.borrow_mut();
        match child_exit_disposition(
            shell_ref.pane_runtime.contains(pane_id),
            shell_ref.shutting_down,
            shell_ref
                .tmux_compat
                .retains_exited_teammate(&shell_ref.state, pane_id),
        ) {
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

#[cfg(test)]
mod tests {
    use super::{
        ChildExitDisposition, RegistrationDecision, RemovalDecision, child_exit_disposition,
        registration_decision, removal_decision,
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
            child_exit_disposition(false, false, false),
            ChildExitDisposition::IgnoreStale
        );
        assert_eq!(
            child_exit_disposition(false, true, true),
            ChildExitDisposition::IgnoreStale
        );
    }

    #[test]
    fn child_exit_during_shutdown_disposes_without_mutating_workspace() {
        assert_eq!(
            child_exit_disposition(true, true, true),
            ChildExitDisposition::DisposeDuringShutdown
        );
    }

    #[test]
    fn active_child_exit_closes_the_workspace_pane() {
        assert_eq!(
            child_exit_disposition(true, false, false),
            ChildExitDisposition::CloseWorkspacePane
        );
    }

    #[test]
    fn exited_tmux_teammate_is_preserved_for_source_respawn() {
        assert_eq!(
            child_exit_disposition(true, false, true),
            ChildExitDisposition::PreserveTmuxTeammate
        );
    }
}
