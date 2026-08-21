use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::{Rc, Weak};
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk, glib};
use zentty_agent_ipc::ServerIpcReply;
use zentty_core::{AppConfig, CloseEvidence, CloseTarget, WindowRecipe};
use zentty_ghostty::GhosttyRuntime;
use zentty_tmux_compat::TmuxCompatReply;

use crate::agent_runtime::AgentRuntime;
use crate::application_shell::{ApplicationHandlers, ApplicationRuntimes, ApplicationShell};
use crate::config_reload::{ConfigDirectoryWatch, ConfigReloadAuthority, ReloadDecision};
use crate::config_store::ConfigSnapshot;
use crate::persistence_coordinator::WindowSnapshot;
use crate::task_manager::TaskManagerController;
use crate::window_set::{CloseWindowDecision, WindowSet};

mod product_cli;

pub(crate) struct ApplicationCycleResult {
    pub(crate) windows: Vec<WindowSnapshot>,
    pub(crate) active_window_id: Option<String>,
}

pub(crate) struct ApplicationCoordinator {
    self_handle: Weak<RefCell<ApplicationCoordinator>>,
    runtime: GhosttyRuntime,
    agent_runtime: Rc<RefCell<AgentRuntime>>,
    tmux_session: crate::tmux_compat::TmuxCompatSession,
    attention_inbox: Rc<RefCell<zentty_core::AttentionInbox>>,
    desktop_notifications: crate::notification_service::AttentionNotificationService,
    fleet_snapshot: Vec<zentty_core::FleetPaneSnapshot>,
    status_notifier: Option<crate::status_notifier::StatusNotifierItem>,
    sleep_inhibition_state: zentty_core::AgentSleepInhibitionState,
    sleep_inhibitor: crate::sleep_inhibitor::SystemdSleepInhibitor,
    command: Option<String>,
    config: AppConfig,
    config_reload: ConfigReloadAuthority,
    config_watch: Option<ConfigDirectoryWatch>,
    main_loop: glib::MainLoop,
    window_set: WindowSet,
    shells: BTreeMap<String, Rc<RefCell<ApplicationShell>>>,
    task_manager: Option<Rc<TaskManagerController>>,
    closing_ids: BTreeSet<String>,
    close_flags: BTreeMap<String, Rc<Cell<bool>>>,
    last_window_sizes: BTreeMap<String, (i32, i32)>,
    last_sidebar_widths: BTreeMap<String, i32>,
    exit_snapshot: Option<ApplicationCycleResult>,
    shutting_down: bool,
    terminal_error: Option<String>,
    teardown_active: Rc<Cell<bool>>,
    shutdown_requested: Rc<Cell<bool>>,
}

impl ApplicationCoordinator {
    pub(crate) fn start(
        runtime: &GhosttyRuntime,
        command: Option<String>,
        main_loop: &glib::MainLoop,
        restored_windows: Vec<WindowSnapshot>,
        active_window_id: Option<&str>,
        config_snapshot: &ConfigSnapshot,
    ) -> Result<Rc<RefCell<Self>>, String> {
        let config = config_snapshot.config.clone();
        let ids = restored_windows
            .iter()
            .map(|snapshot| snapshot.window.id.clone())
            .collect::<Vec<_>>();
        let window_set = WindowSet::restore(ids, active_window_id)
            .map_err(|error| format!("could not compose restored windows: {error:?}"))?;
        let agent_runtime = Rc::new(RefCell::new(AgentRuntime::start()?));
        // Preserve the established process-level opt-in used by agent-team
        // launchers. A persisted enabled setting may opt in as well; an absent
        // (default-false) setting must not erase an explicit environment opt-in
        // before the first pane is created. A live settings change can still
        // disable teams through ApplicationShell::apply_agent_settings.
        if config.agent_teams.enabled {
            agent_runtime.borrow_mut().set_agent_teams_enabled(true);
        }
        agent_runtime
            .borrow_mut()
            .set_agent_integrations(config.agent_integrations.states.clone());
        let sleep_inhibitor_capability =
            crate::sleep_inhibitor::SleepInhibitorCapability::discover();
        eprintln!(
            "zentty-linux: sleep-inhibitor capability={} executable={}",
            if sleep_inhibitor_capability.available() {
                "available"
            } else {
                "unavailable"
            },
            sleep_inhibitor_capability
                .executable
                .as_deref()
                .map_or_else(|| "none".to_owned(), |path| path.display().to_string())
        );
        let coordinator = Rc::new(RefCell::new(Self {
            self_handle: Weak::new(),
            runtime: runtime.clone(),
            agent_runtime,
            tmux_session: crate::tmux_compat::TmuxCompatSession::default(),
            attention_inbox: Rc::new(RefCell::new(zentty_core::AttentionInbox::default())),
            desktop_notifications: crate::notification_service::AttentionNotificationService::new(),
            fleet_snapshot: Vec::new(),
            status_notifier: None,
            sleep_inhibition_state: zentty_core::AgentSleepInhibitionState::default(),
            sleep_inhibitor: crate::sleep_inhibitor::SystemdSleepInhibitor::new(
                sleep_inhibitor_capability,
            ),
            command,
            config,
            config_reload: ConfigReloadAuthority::new(config_snapshot),
            config_watch: None,
            main_loop: main_loop.clone(),
            window_set,
            shells: BTreeMap::new(),
            task_manager: None,
            closing_ids: BTreeSet::new(),
            close_flags: BTreeMap::new(),
            last_window_sizes: BTreeMap::new(),
            last_sidebar_widths: BTreeMap::new(),
            exit_snapshot: None,
            shutting_down: false,
            terminal_error: None,
            teardown_active: Rc::new(Cell::new(false)),
            shutdown_requested: Rc::new(Cell::new(false)),
        }));
        coordinator.borrow_mut().self_handle = Rc::downgrade(&coordinator);

        if restored_windows.is_empty() {
            Self::create_fresh_window(&coordinator)?;
        } else {
            for snapshot in restored_windows {
                if let Err(error) = Self::build_shell(&coordinator, snapshot) {
                    let _ = coordinator.borrow_mut().shutdown_all();
                    return Err(error);
                }
            }
            let (active_id, remaining_ids) = {
                let coordinator = coordinator.borrow();
                let active_id = coordinator.window_set.active_id().map(str::to_owned);
                let remaining_ids = coordinator
                    .window_set
                    .ordered_ids()
                    .iter()
                    .filter(|id| Some(id.as_str()) != coordinator.window_set.active_id())
                    .cloned()
                    .collect::<Vec<_>>();
                (active_id, remaining_ids)
            };
            if let Some(active_id) = active_id {
                if is_wayland_display() {
                    // Wayland intentionally rejects later programmatic focus
                    // stealing. Map the persisted active toplevel first so it
                    // receives the compositor's initial activation token.
                    Self::present_shell(&coordinator, &active_id, true)?;
                    for id in remaining_ids {
                        Self::present_shell(&coordinator, &id, false)?;
                    }
                } else {
                    for id in remaining_ids {
                        Self::present_shell(&coordinator, &id, false)?;
                    }
                    Self::present_shell(&coordinator, &active_id, true)?;
                }
            }
            Self::refresh_worklane_destination_catalogs(&coordinator);
        }
        Self::install_status_notifier(&coordinator);
        Self::install_config_watch(&coordinator)?;
        Ok(coordinator)
    }

    fn install_status_notifier(coordinator: &Rc<RefCell<Self>>) {
        let weak = Rc::downgrade(coordinator);
        let activate = Rc::new(move || {
            let Some(coordinator) = weak.upgrade() else {
                return;
            };
            glib::idle_add_local_once(move || Self::show_agent_fleet(&coordinator));
        });
        coordinator.borrow_mut().status_notifier =
            Some(crate::status_notifier::StatusNotifierItem::new(activate));
    }

    fn install_config_watch(coordinator: &Rc<RefCell<Self>>) -> Result<(), String> {
        let path = coordinator.borrow().config_reload.path().to_path_buf();
        let weak = Rc::downgrade(coordinator);
        let watch = ConfigDirectoryWatch::install(&path, move || {
            let Some(coordinator) = weak.upgrade() else {
                return;
            };
            if let Err(error) = coordinator.borrow_mut().reload_product_config() {
                eprintln!("zentty-linux: config-reload result=projection-failed detail={error}");
            }
            if let Err(error) = Self::install_config_watch(&coordinator) {
                eprintln!("zentty-linux: config-watch result=refresh-failed detail={error}");
            }
        })?;
        coordinator.borrow_mut().config_watch = Some(watch);
        eprintln!("zentty-linux: config-watch path={}", path.display());
        Ok(())
    }

    fn reload_product_config(&mut self) -> Result<(), String> {
        match self.config_reload.observe_disk() {
            ReloadDecision::Unchanged => {
                eprintln!("zentty-linux: config-reload result=unchanged");
            }
            ReloadDecision::RetainLastGood(diagnostic) => {
                eprintln!(
                    "zentty-linux: config-reload result=retained-last-good detail={diagnostic}"
                );
            }
            ReloadDecision::Apply {
                config,
                retained_sections,
            } => {
                let projections = self
                    .shells
                    .values()
                    .map(|shell| {
                        ApplicationShell::validate_reloaded_config(&config)
                            .map(|shortcuts| (Rc::clone(shell), shortcuts))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                ApplicationShell::prepare_reloaded_appearance(&self.config, &config)?;
                for (shell, shortcuts) in projections {
                    shell.borrow_mut().apply_reloaded_config(&config, shortcuts);
                }
                self.config_reload.accept(&config);
                self.config = *config;
                eprintln!(
                    "zentty-linux: config-reload result=applied windows={} retained-sections={}",
                    self.shells.len(),
                    if retained_sections.is_empty() {
                        "none".to_owned()
                    } else {
                        retained_sections.join(",")
                    }
                );
            }
        }
        Ok(())
    }

    fn create_fresh_window(coordinator: &Rc<RefCell<Self>>) -> Result<(), String> {
        let id = {
            let mut coordinator = coordinator.borrow_mut();
            let id = coordinator.window_set.generate_id();
            coordinator
                .window_set
                .insert(id.clone())
                .map_err(|error| format!("could not register fresh window: {error:?}"))?;
            id
        };
        let snapshot = WindowSnapshot {
            window: WindowRecipe {
                id: id.clone(),
                frame: None,
                worklanes: Vec::new(),
                active_worklane_id: None,
            },
            restored_drafts: Vec::new(),
        };
        match Self::build_shell(coordinator, snapshot) {
            Ok(()) => {
                Self::present_shell(coordinator, &id, true)?;
                Self::refresh_worklane_destination_catalogs(coordinator);
                Ok(())
            }
            Err(error) => {
                coordinator.borrow_mut().window_set.close(&id);
                Err(error)
            }
        }
    }

    fn build_shell(
        coordinator: &Rc<RefCell<Self>>,
        snapshot: WindowSnapshot,
    ) -> Result<(), String> {
        Self::build_shell_with_deferred_pane(coordinator, snapshot, None).map(|_| ())
    }

    fn build_shell_with_deferred_pane(
        coordinator: &Rc<RefCell<Self>>,
        snapshot: WindowSnapshot,
        deferred_live_pane_id: Option<&str>,
    ) -> Result<Rc<RefCell<ApplicationShell>>, String> {
        let id = snapshot.window.id.clone();
        let restored_window = (!snapshot.window.worklanes.is_empty()).then_some(snapshot.window);
        let (runtime, agent_runtime, attention_inbox, tmux_session, command, config, main_loop) = {
            let coordinator = coordinator.borrow();
            (
                coordinator.runtime.clone(),
                Rc::clone(&coordinator.agent_runtime),
                Rc::clone(&coordinator.attention_inbox),
                coordinator.tmux_session.clone(),
                coordinator.command.clone(),
                coordinator.config.clone(),
                coordinator.main_loop.clone(),
            )
        };
        let runtimes = ApplicationRuntimes {
            ghostty: runtime,
            agent: agent_runtime,
            config,
            attention_inbox,
            tmux_session,
        };
        let shell = ApplicationShell::new(
            &runtimes,
            command,
            &main_loop,
            restored_window,
            &snapshot.restored_drafts,
            &id,
            deferred_live_pane_id,
        )?;

        Self::install_shell_callbacks(coordinator, &shell, &id);
        coordinator
            .borrow_mut()
            .shells
            .insert(id, Rc::clone(&shell));
        Ok(shell)
    }

    fn present_shell(
        coordinator: &Rc<RefCell<Self>>,
        id: &str,
        schedule_focus: bool,
    ) -> Result<(), String> {
        let shell = coordinator
            .borrow()
            .shells
            .get(id)
            .cloned()
            .ok_or_else(|| format!("registered window {id:?} has no application shell"))?;
        if schedule_focus {
            // Capture the restored selection before GTK maps the window and
            // temporarily focuses its first child.
            ApplicationShell::preserve_initial_terminal_focus(&shell);
        }
        shell.borrow().present();
        eprintln!("zentty-linux: window-opened id={id}");
        Ok(())
    }

    fn refresh_worklane_destination_catalogs(coordinator: &Rc<RefCell<Self>>) {
        let (ordered_ids, shells) = {
            let coordinator = coordinator.borrow();
            (
                coordinator.window_set.ordered_ids().to_vec(),
                coordinator
                    .shells
                    .iter()
                    .map(|(id, shell)| (id.clone(), Rc::clone(shell)))
                    .collect::<BTreeMap<_, _>>(),
            )
        };
        let summaries = shells
            .iter()
            .map(|(id, shell)| (id.clone(), shell.borrow().sidebar_summaries()))
            .collect::<BTreeMap<_, _>>();
        for (source_id, shell) in &shells {
            let mut destination_ids = vec![source_id.clone()];
            destination_ids.extend(
                ordered_ids
                    .iter()
                    .filter(|id| id.as_str() != source_id)
                    .cloned(),
            );
            let groups = destination_ids
                .into_iter()
                .filter_map(|window_id| {
                    Some(crate::sidebar::WorklaneDestinationGroup {
                        summaries: summaries.get(&window_id)?.clone(),
                        window_id,
                    })
                })
                .collect();
            shell.borrow_mut().set_worklane_destination_groups(groups);
            shell.borrow().render();
        }
    }

    #[allow(clippy::too_many_lines)]
    fn install_shell_callbacks(
        coordinator: &Rc<RefCell<Self>>,
        shell: &Rc<RefCell<ApplicationShell>>,
        id: &str,
    ) {
        let weak = Rc::downgrade(coordinator);
        let new_window_handler: Rc<dyn Fn()> = Rc::new(move || {
            let weak = weak.clone();
            glib::idle_add_local_once(move || {
                let Some(coordinator) = weak.upgrade() else {
                    return;
                };
                if coordinator.borrow().shutting_down {
                    return;
                }
                if let Err(error) = Self::create_fresh_window(&coordinator) {
                    eprintln!("zentty-linux: action=new-window error={error}");
                }
            });
        });
        let move_pane_to_new_window_handler = Self::move_pane_handler(coordinator, id);
        let move_pane_to_window_worklane_handler =
            Self::move_pane_to_window_worklane_handler(coordinator, id);
        let show_task_manager_handler = Self::task_manager_handler(coordinator, shell);
        let weak = Rc::downgrade(coordinator);
        let close_id = id.to_owned();
        let close_window_handler: Rc<dyn Fn()> = Rc::new(move || {
            let weak = weak.clone();
            let id = close_id.clone();
            glib::idle_add_local_once(move || {
                if let Some(coordinator) = weak.upgrade() {
                    let result = coordinator.borrow_mut().close_window(&id);
                    if let Err(error) = result {
                        eprintln!("zentty-linux: close-window id={id} error={error}");
                    } else if !coordinator.borrow().shutting_down {
                        Self::refresh_worklane_destination_catalogs(&coordinator);
                    }
                }
            });
        });
        let weak = Rc::downgrade(coordinator);
        let evidence_id = id.to_owned();
        let close_window_evidence_handler: Rc<dyn Fn() -> CloseEvidence> = Rc::new(move || {
            let Some(coordinator) = weak.upgrade() else {
                return CloseEvidence::new(
                    CloseTarget::Window {
                        window_id: evidence_id.clone(),
                    },
                    Vec::new(),
                );
            };
            let coordinator = coordinator.borrow();
            if coordinator.shells.len() == 1 {
                coordinator.application_close_evidence()
            } else {
                coordinator.shells.get(&evidence_id).map_or_else(
                    || {
                        CloseEvidence::new(
                            CloseTarget::Window {
                                window_id: evidence_id.clone(),
                            },
                            Vec::new(),
                        )
                    },
                    |shell| shell.borrow().window_close_evidence(),
                )
            }
        });
        let weak = Rc::downgrade(coordinator);
        let shutdown_requested = Rc::clone(&coordinator.borrow().shutdown_requested);
        let quit_handler: Rc<dyn Fn()> = Rc::new(move || {
            if shutdown_requested.replace(true) {
                return;
            }
            let weak = weak.clone();
            let shutdown_requested = Rc::clone(&shutdown_requested);
            glib::idle_add_local_once(move || {
                if let Some(coordinator) = weak.upgrade()
                    && let Err(error) = coordinator.borrow_mut().quit_application()
                {
                    shutdown_requested.set(false);
                    eprintln!("zentty-linux: application quit failed: {error}");
                }
            });
        });
        let weak = Rc::downgrade(coordinator);
        let quit_evidence_handler: Rc<dyn Fn() -> CloseEvidence> = Rc::new(move || {
            weak.upgrade().map_or_else(
                || CloseEvidence::new(CloseTarget::Application, Vec::new()),
                |coordinator| {
                    coordinator.try_borrow().map_or_else(
                        |_| CloseEvidence::new(CloseTarget::Application, Vec::new()),
                        |coordinator| coordinator.application_close_evidence(),
                    )
                },
            )
        });
        let application_action_handler = Self::application_action_handler(coordinator, id);
        shell
            .borrow_mut()
            .set_application_handlers(ApplicationHandlers {
                new_window: new_window_handler,
                move_pane_to_new_window: move_pane_to_new_window_handler,
                show_task_manager: show_task_manager_handler,
                close_window: close_window_handler,
                close_window_evidence: close_window_evidence_handler,
                quit: quit_handler,
                quit_evidence: quit_evidence_handler,
                application_action: application_action_handler,
            });
        shell
            .borrow_mut()
            .set_move_pane_to_window_worklane_handler(move_pane_to_window_worklane_handler);

        let weak = Rc::downgrade(coordinator);
        let weak_shell = Rc::downgrade(shell);
        let active_id = id.to_owned();
        let teardown_active = Rc::clone(&coordinator.borrow().teardown_active);
        shell
            .borrow()
            .window()
            .connect_is_active_notify(move |window| {
                if teardown_active.get() || !window.is_active() {
                    return;
                }
                if let Some(shell) = weak_shell.upgrade() {
                    shell.borrow().settle_active_window_focus();
                }
                if let Some(coordinator) = weak.upgrade()
                    && coordinator.borrow_mut().window_set.mark_active(&active_id)
                {
                    eprintln!("zentty-linux: active-window={active_id}");
                }
            });

        let weak_shell = Rc::downgrade(shell);
        let close_flag = Rc::new(Cell::new(false));
        coordinator
            .borrow_mut()
            .close_flags
            .insert(id.to_owned(), Rc::clone(&close_flag));
        shell.borrow().window().connect_close_request(move |_| {
            if close_flag.get() {
                return glib::Propagation::Proceed;
            }
            if let Some(shell) = weak_shell.upgrade() {
                shell.borrow().request_close_window();
            }
            glib::Propagation::Stop
        });
    }

    fn application_action_handler(
        coordinator: &Rc<RefCell<Self>>,
        source_window_id: &str,
    ) -> Rc<dyn Fn(crate::application_shell::ApplicationAction)> {
        let weak = Rc::downgrade(coordinator);
        let source_window_id = source_window_id.to_owned();
        Rc::new(move |action| {
            let cross_window_activation = match &action {
                crate::application_shell::ApplicationAction::ActivateFleetPane {
                    target, ..
                } => target.window_id != source_window_id,
                _ => false,
            };
            if cross_window_activation {
                if let Some(coordinator) = weak.upgrade() {
                    Self::handle_application_action(&coordinator, action);
                }
                return;
            }
            let weak = weak.clone();
            glib::idle_add_local_once(move || {
                let Some(coordinator) = weak.upgrade() else {
                    return;
                };
                Self::handle_application_action(&coordinator, action);
            });
        })
    }

    fn handle_application_action(
        coordinator: &Rc<RefCell<Self>>,
        action: crate::application_shell::ApplicationAction,
    ) {
        let mut refresh_attention = false;
        match action {
            crate::application_shell::ApplicationAction::ActivateAttention(target) => {
                let shell = coordinator.borrow().shells.get(&target.window_id).cloned();
                let activated = shell.as_ref().is_some_and(|shell| {
                    shell.borrow_mut().activate_attention_target(&target, None)
                });
                if activated {
                    eprintln!(
                        "zentty-linux: attention-activate window={} worklane={} pane={} result=focused",
                        target.window_id, target.worklane_id, target.pane_id
                    );
                } else {
                    coordinator
                        .borrow()
                        .attention_inbox
                        .borrow_mut()
                        .resolve_target(&target, current_time_ms());
                    refresh_attention = true;
                    eprintln!(
                        "zentty-linux: attention-activate window={} worklane={} pane={} result=stale",
                        target.window_id, target.worklane_id, target.pane_id
                    );
                }
            }
            crate::application_shell::ApplicationAction::ActivateFleetPane {
                target,
                activation,
            } => {
                let shell = coordinator.borrow().shells.get(&target.window_id).cloned();
                let activated = shell.as_ref().is_some_and(|shell| {
                    shell
                        .borrow_mut()
                        .activate_attention_target(&target, Some(&activation))
                });
                eprintln!(
                    "zentty-linux: fleet-activation-context event-time={} startup-id={}",
                    activation
                        .event_time
                        .map_or_else(|| "none".to_owned(), |value| value.to_string()),
                    if activation.startup_id.is_some() {
                        "present"
                    } else {
                        "absent"
                    }
                );
                eprintln!(
                    "zentty-linux: fleet-activate window={} worklane={} pane={} result={}",
                    target.window_id,
                    target.worklane_id,
                    target.pane_id,
                    if activated { "targeted" } else { "stale" }
                );
            }
            crate::application_shell::ApplicationAction::DismissAttention(id) => {
                coordinator
                    .borrow()
                    .attention_inbox
                    .borrow_mut()
                    .dismiss(id);
                refresh_attention = true;
                eprintln!("zentty-linux: attention-dismiss id={id}");
            }
            crate::application_shell::ApplicationAction::ClearAttention => {
                coordinator.borrow().attention_inbox.borrow_mut().clear();
                refresh_attention = true;
                eprintln!("zentty-linux: attention-clear");
            }
            crate::application_shell::ApplicationAction::AgentCaffeinationChanged(enabled) => {
                coordinator.borrow_mut().config.agent_caffeination.enabled = enabled;
                eprintln!(
                    "zentty-linux: sleep-inhibitor setting-enabled={enabled} source=agents-settings"
                );
            }
            crate::application_shell::ApplicationAction::StatusNotifierChanged(enabled) => {
                coordinator.borrow_mut().config.menu_bar.show_status_item = enabled;
                eprintln!(
                    "zentty-linux: status-notifier setting-enabled={enabled} source=agents-settings"
                );
            }
        }
        if refresh_attention {
            for shell in coordinator.borrow().shells.values() {
                shell.borrow().refresh_attention_inbox();
            }
        }
    }

    fn move_pane_handler(coordinator: &Rc<RefCell<Self>>, source_id: &str) -> Rc<dyn Fn(String)> {
        let weak = Rc::downgrade(coordinator);
        let source_id = source_id.to_owned();
        Rc::new(move |pane_id| {
            let weak = weak.clone();
            let source_id = source_id.clone();
            glib::idle_add_local_once(move || {
                let Some(coordinator) = weak.upgrade() else {
                    return;
                };
                if coordinator.borrow().shutting_down {
                    return;
                }
                if let Err(error) =
                    Self::move_pane_to_new_window(&coordinator, &source_id, &pane_id)
                {
                    eprintln!(
                        "zentty-linux: action=move-pane-to-new-window pane={pane_id} error={error}"
                    );
                }
            });
        })
    }

    fn move_pane_to_window_worklane_handler(
        coordinator: &Rc<RefCell<Self>>,
        source_id: &str,
    ) -> Rc<dyn Fn(String, String, String)> {
        let weak = Rc::downgrade(coordinator);
        let source_id = source_id.to_owned();
        Rc::new(
            move |pane_id, destination_window_id, destination_worklane_id| {
                let weak = weak.clone();
                let source_id = source_id.clone();
                glib::idle_add_local_once(move || {
                    let Some(coordinator) = weak.upgrade() else {
                        return;
                    };
                    if coordinator.borrow().shutting_down {
                        return;
                    }
                    if let Err(error) = Self::move_pane_to_existing_worklane(
                        &coordinator,
                        &source_id,
                        &pane_id,
                        &destination_window_id,
                        &destination_worklane_id,
                    ) {
                        eprintln!(
                            "zentty-linux: action=move-pane-to-window-worklane pane={pane_id} destination-window={destination_window_id} destination-worklane={destination_worklane_id} error={error}"
                        );
                    }
                });
            },
        )
    }

    fn task_manager_handler(
        coordinator: &Rc<RefCell<Self>>,
        shell: &Rc<RefCell<ApplicationShell>>,
    ) -> Rc<dyn Fn()> {
        let weak = Rc::downgrade(coordinator);
        let parent = shell.borrow().window().clone();
        Rc::new(move || {
            let weak = weak.clone();
            let parent = parent.clone();
            glib::idle_add_local_once(move || {
                let Some(coordinator) = weak.upgrade() else {
                    return;
                };
                if let Err(error) = Self::show_task_manager(&coordinator, Some(&parent)) {
                    eprintln!("zentty-linux: action=show-task-manager error={error}");
                }
            });
        })
    }

    fn show_task_manager(
        coordinator: &Rc<RefCell<Self>>,
        parent: Option<&gtk::Window>,
    ) -> Result<(), String> {
        let existing = coordinator.borrow().task_manager.clone();
        let controller = if let Some(existing) = existing {
            existing
        } else {
            let weak = Rc::downgrade(coordinator);
            let sources = Rc::new(move || {
                weak.upgrade().map_or_else(Vec::new, |coordinator| {
                    coordinator
                        .borrow()
                        .window_set
                        .ordered_ids()
                        .iter()
                        .filter_map(|id| coordinator.borrow().shells.get(id).cloned())
                        .flat_map(|shell| shell.borrow().task_manager_pane_sources())
                        .collect()
                })
            });
            let weak = Rc::downgrade(coordinator);
            let focus = Rc::new(move |window_id: &str, worklane_id: &str, pane_id: &str| {
                let Some(coordinator) = weak.upgrade() else {
                    return;
                };
                let shell = coordinator.borrow().shells.get(window_id).cloned();
                if let Some(shell) = shell {
                    ApplicationShell::focus_task_manager_pane(&shell, worklane_id, pane_id);
                }
            });
            let weak = Rc::downgrade(coordinator);
            let close = Rc::new(move |window_id: &str, _worklane_id: &str, pane_id: &str| {
                let Some(coordinator) = weak.upgrade() else {
                    return;
                };
                let shell = coordinator.borrow().shells.get(window_id).cloned();
                if let Some(shell) = shell {
                    ApplicationShell::close_task_manager_pane(&shell, pane_id);
                }
            });
            let controller = TaskManagerController::new(sources, focus, close)?;
            coordinator.borrow_mut().task_manager = Some(Rc::clone(&controller));
            controller
        };
        TaskManagerController::show(&controller, parent);
        Ok(())
    }

    fn move_pane_to_new_window(
        coordinator: &Rc<RefCell<Self>>,
        source_id: &str,
        pane_id: &str,
    ) -> Result<(), String> {
        let (source, destination_id) = {
            let mut coordinator = coordinator.borrow_mut();
            let source = coordinator
                .shells
                .get(source_id)
                .cloned()
                .ok_or_else(|| format!("source window {source_id:?} is unavailable"))?;
            let destination_id = coordinator.window_set.generate_id();
            (source, destination_id)
        };
        let mut suffix = 1_u64;
        let destination_worklane_id = loop {
            let candidate = format!("{destination_id}-worklane-{suffix}");
            if !source.borrow().has_worklane(&candidate) {
                break candidate;
            }
            suffix = suffix
                .checked_add(1)
                .ok_or_else(|| "could not allocate destination worklane identity".to_owned())?;
        };
        let transfer = source
            .borrow_mut()
            .extract_live_pane_to_new_window(pane_id, &destination_worklane_id)?;
        let mut destination_recipe = transfer.destination_recipe.clone();
        destination_recipe.id.clone_from(&destination_id);
        let snapshot = WindowSnapshot {
            window: destination_recipe,
            restored_drafts: transfer.model.destination.agent_restore_drafts(),
        };

        if let Err(error) = coordinator
            .borrow_mut()
            .window_set
            .insert(destination_id.clone())
            .map_err(|error| format!("could not register destination window: {error:?}"))
        {
            ApplicationShell::rollback_live_pane_window_transfer(&source, transfer)?;
            return Err(error);
        }

        let destination =
            match Self::build_shell_with_deferred_pane(coordinator, snapshot, Some(pane_id)) {
                Ok(destination) => destination,
                Err(error) => {
                    coordinator.borrow_mut().window_set.close(&destination_id);
                    ApplicationShell::rollback_live_pane_window_transfer(&source, transfer)?;
                    return Err(error);
                }
            };

        let crate::application_shell::ExtractedWindowPane {
            model,
            destination_recipe,
            runtime,
            source_before,
        } = transfer;
        if let Err((error, runtime)) =
            ApplicationShell::adopt_live_pane_window_transfer(&destination, pane_id, runtime)
        {
            {
                let mut coordinator = coordinator.borrow_mut();
                coordinator.shells.remove(&destination_id);
                coordinator.window_set.close(&destination_id);
                coordinator.teardown_shell(&destination_id, &destination)?;
            }
            ApplicationShell::rollback_live_pane_window_transfer(
                &source,
                crate::application_shell::ExtractedWindowPane {
                    model,
                    destination_recipe,
                    runtime,
                    source_before,
                },
            )?;
            return Err(error);
        }

        source.borrow_mut().sync_agent_targets();
        destination.borrow_mut().sync_agent_targets();
        coordinator
            .borrow_mut()
            .window_set
            .mark_active(&destination_id);
        Self::present_shell(coordinator, &destination_id, true)?;
        Self::refresh_worklane_destination_catalogs(coordinator);
        eprintln!(
            "zentty-linux: action=move-pane-to-new-window pane={pane_id} source={source_id} destination={destination_id}"
        );
        Ok(())
    }

    fn move_pane_to_existing_worklane(
        coordinator: &Rc<RefCell<Self>>,
        source_id: &str,
        pane_id: &str,
        destination_id: &str,
        destination_worklane_id: &str,
    ) -> Result<(), String> {
        if source_id == destination_id {
            return Err("cross-window transfer requires a different destination window".to_owned());
        }
        let (source, destination) = {
            let coordinator = coordinator.borrow();
            let source = coordinator
                .shells
                .get(source_id)
                .cloned()
                .ok_or_else(|| format!("source window {source_id:?} is unavailable"))?;
            let destination = coordinator
                .shells
                .get(destination_id)
                .cloned()
                .ok_or_else(|| format!("destination window {destination_id:?} is unavailable"))?;
            if !destination.borrow().has_worklane(destination_worklane_id) {
                return Err(format!(
                    "destination worklane {destination_worklane_id:?} is unavailable"
                ));
            }
            (source, destination)
        };
        let transfer = source
            .borrow_mut()
            .extract_live_pane_for_existing_window(pane_id)?;
        let source_window_should_close = transfer.model.source_window_should_close;
        let crate::application_shell::ExtractedExistingWindowPane {
            model,
            runtime,
            source_before,
        } = transfer;
        if let Err((error, runtime)) = ApplicationShell::adopt_live_pane_in_existing_window(
            &destination,
            model,
            runtime,
            destination_worklane_id,
        ) {
            ApplicationShell::rollback_live_pane_existing_window_transfer(
                &source,
                pane_id,
                source_before,
                runtime,
            )?;
            return Err(error);
        }

        destination.borrow_mut().sync_agent_targets();
        coordinator
            .borrow_mut()
            .window_set
            .mark_active(destination_id);
        if source_window_should_close {
            coordinator.borrow_mut().close_window(source_id)?;
        } else {
            source.borrow_mut().sync_agent_targets();
        }
        Self::present_shell(coordinator, destination_id, true)?;
        Self::refresh_worklane_destination_catalogs(coordinator);
        eprintln!(
            "zentty-linux: action=move-pane-to-window-worklane pane={pane_id} source={source_id} destination-window={destination_id} destination-worklane={destination_worklane_id}"
        );
        Ok(())
    }

    fn close_window(&mut self, id: &str) -> Result<(), String> {
        if self.shutting_down || self.closing_ids.contains(id) {
            return Ok(());
        }
        let decision = self.window_set.close(id);
        if decision == CloseWindowDecision::UnknownWindow {
            return Ok(());
        }
        let Some(shell) = self.shells.remove(id) else {
            return Err(format!("registered window {id:?} has no application shell"));
        };
        self.attention_inbox.borrow_mut().resolve_stale(
            id,
            &std::collections::HashSet::new(),
            current_time_ms(),
        );
        self.last_window_sizes.remove(id);
        self.last_sidebar_widths.remove(id);
        if decision == CloseWindowDecision::QuitApplication {
            self.exit_snapshot = Some(ApplicationCycleResult {
                windows: vec![snapshot_shell(&shell)],
                active_window_id: Some(id.to_owned()),
            });
            self.shutting_down = true;
        } else {
            shell.borrow_mut().forget_tmux_worklanes()?;
        }
        self.teardown_shell(id, &shell)?;
        eprintln!(
            "zentty-linux: window-closed id={id} remaining={}",
            self.shells.len()
        );
        if decision == CloseWindowDecision::QuitApplication {
            self.main_loop.quit();
        } else if let Some(active_id) = self.window_set.active_id()
            && let Some(active) = self.shells.get(active_id)
        {
            active.borrow().present();
        }
        Ok(())
    }

    fn quit_application(&mut self) -> Result<(), String> {
        if self.shutting_down {
            return Ok(());
        }
        self.exit_snapshot = Some(self.snapshot());
        self.shutting_down = true;
        self.shutdown_all()?;
        self.main_loop.quit();
        Ok(())
    }

    fn application_close_evidence(&self) -> CloseEvidence {
        CloseEvidence::new(
            CloseTarget::Application,
            self.shells
                .values()
                .flat_map(|shell| shell.borrow().window_close_evidence().panes)
                .collect(),
        )
    }

    fn teardown_shell(
        &mut self,
        id: &str,
        shell: &Rc<RefCell<ApplicationShell>>,
    ) -> Result<(), String> {
        self.teardown_active.set(true);
        self.closing_ids.insert(id.to_owned());
        if let Some(flag) = self.close_flags.get(id) {
            flag.set(true);
        }
        shell.borrow_mut().detach_for_shutdown();
        settle_gtk_teardown();
        let release_result = shell.borrow_mut().release_surfaces();
        settle_gtk_teardown();
        shell.borrow().close_detached_window();
        settle_gtk_teardown();
        self.teardown_active.set(false);
        release_result?;
        if shell.borrow().live_children() != 0 {
            return Err(format!(
                "window {id:?} ended with {} live children",
                shell.borrow().live_children()
            ));
        }
        self.closing_ids.remove(id);
        self.close_flags.remove(id);
        Ok(())
    }

    fn shutdown_all(&mut self) -> Result<(), String> {
        if let Some(task_manager) = self.task_manager.take() {
            task_manager.shutdown();
        }
        let ids = self.window_set.ordered_ids().to_vec();
        let mut first_error = None;
        for id in ids {
            if let Some(shell) = self.shells.remove(&id)
                && let Err(error) = self.teardown_shell(&id, &shell)
            {
                first_error.get_or_insert(error);
            }
            self.window_set.close(&id);
        }
        if let Some(error) = first_error {
            Err(error)
        } else {
            Ok(())
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn tick(&mut self) -> Result<(), String> {
        for shell in self.shells.values() {
            shell.borrow_mut().sync_agent_targets();
        }
        let (events, tmux_commands, server_commands, product_commands) = {
            let runtime = self.agent_runtime.borrow();
            (
                runtime.drain(),
                runtime.drain_tmux(),
                runtime.drain_servers(),
                runtime.drain_products(),
            )
        };
        self.handle_product_commands(product_commands);
        let mut events_by_window = BTreeMap::<String, Vec<_>>::new();
        for event in events {
            events_by_window
                .entry(event.target.window_id.clone())
                .or_default()
                .push(event);
        }
        let mut tmux_by_window = BTreeMap::<String, Vec<_>>::new();
        for command in tmux_commands {
            if self.shells.contains_key(&command.target.window_id) {
                tmux_by_window
                    .entry(command.target.window_id.clone())
                    .or_default()
                    .push(command);
            } else {
                let target = command.target.clone();
                let reply = TmuxCompatReply::failure(
                    "stale_target",
                    format!("window {:?} is no longer available", target.window_id),
                )
                .map_err(|error| format!("could not create stale-target reply: {error}"))?;
                if let Err(error) = command.respond(reply) {
                    eprintln!(
                        "zentty-linux: tmux-stale-target-response window={} pane={} error={error}",
                        target.window_id, target.pane_id
                    );
                }
            }
        }
        let mut servers_by_window = self.route_server_commands(server_commands)?;
        let stale_window_ids = events_by_window
            .keys()
            .filter(|window_id| !self.shells.contains_key(*window_id))
            .cloned()
            .collect::<Vec<_>>();
        for window_id in stale_window_ids {
            for event in events_by_window.remove(&window_id).unwrap_or_default() {
                eprintln!(
                    "zentty-linux: agent-event-dropped pane={} reason=stale-window window={}",
                    event.target.pane_id, event.target.window_id
                );
            }
        }
        for (id, shell) in &self.shells {
            for command in servers_by_window.remove(id).unwrap_or_default() {
                let reply = crate::application_shell::server_runtime::handle_ipc(
                    shell,
                    &command.target,
                    &command.request,
                );
                if let Err(error) = command.respond(reply) {
                    eprintln!("zentty-linux: server-response window={id} error={error}");
                }
            }
            ApplicationShell::apply_agent_inputs(
                shell,
                tmux_by_window.remove(id).unwrap_or_default(),
                events_by_window.remove(id).unwrap_or_default(),
            );
            let shell = shell.borrow_mut();
            shell.reconcile_sidebar_width();
            shell.reconcile_pane_heights();
            let window_size = (shell.window().width(), shell.window().height());
            if window_size.0 > 0
                && window_size.1 > 0
                && self.last_window_sizes.get(id) != Some(&window_size)
            {
                eprintln!(
                    "zentty-linux: window-size={}x{}",
                    window_size.0, window_size.1
                );
                self.last_window_sizes.insert(id.clone(), window_size);
            }
            let sidebar_width = shell.sidebar_container().width();
            if sidebar_width > 0 && self.last_sidebar_widths.get(id) != Some(&sidebar_width) {
                eprintln!("zentty-linux: sidebar-width={sidebar_width}");
                self.last_sidebar_widths.insert(id.clone(), sidebar_width);
            }
        }
        let attention_changed = self.attention_inbox.borrow_mut().advance(current_time_ms());
        let deliveries = self.attention_inbox.borrow_mut().drain_deliveries();
        for delivery in deliveries {
            if !delivery.desktop_allowed {
                eprintln!(
                    "zentty-linux: desktop-attention id={} result=suppressed reason=actively-viewed window={} worklane={} pane={}",
                    delivery.item.id,
                    delivery.item.target.window_id,
                    delivery.item.target.worklane_id,
                    delivery.item.target.pane_id,
                );
                continue;
            }
            match self
                .desktop_notifications
                .send_attention(&delivery.item, &self.config.notifications)
            {
                Ok(id) => eprintln!(
                    "zentty-linux: desktop-attention id={} service-id={} result=sent window={} worklane={} pane={}",
                    delivery.item.id,
                    id,
                    delivery.item.target.window_id,
                    delivery.item.target.worklane_id,
                    delivery.item.target.pane_id,
                ),
                Err(error) => eprintln!(
                    "zentty-linux: desktop-attention id={} result=unavailable detail={error}",
                    delivery.item.id
                ),
            }
        }
        for target in self.desktop_notifications.drain_activations() {
            let activated = self
                .shells
                .get(&target.window_id)
                .is_some_and(|shell| shell.borrow_mut().activate_attention_target(&target, None));
            eprintln!(
                "zentty-linux: desktop-attention-activate window={} worklane={} pane={} result={}",
                target.window_id,
                target.worklane_id,
                target.pane_id,
                if activated { "focused" } else { "stale" }
            );
            if !activated {
                self.attention_inbox
                    .borrow_mut()
                    .resolve_target(&target, current_time_ms());
            }
        }
        if attention_changed {
            let inbox = self.attention_inbox.borrow();
            eprintln!(
                "zentty-linux: attention-inbox commit items={} unresolved={}",
                inbox.items().len(),
                inbox.unresolved_count()
            );
        }
        for shell in self.shells.values() {
            shell.borrow().refresh_attention_inbox();
        }
        self.refresh_fleet();
        self.runtime.tick().map_err(|error| error.to_string())
    }

    fn refresh_fleet(&mut self) {
        let owned_sources = self
            .shells
            .values()
            .map(|shell| shell.borrow().fleet_source())
            .collect::<Vec<_>>();
        let sources = owned_sources
            .iter()
            .map(
                |(window_id, window_title, worklanes)| zentty_core::FleetWindowSource {
                    window_id,
                    window_title,
                    worklanes,
                },
            )
            .collect::<Vec<_>>();
        let snapshot = zentty_core::build_fleet_snapshots(&sources);
        let has_running_agent = owned_sources.iter().any(|(_, _, worklanes)| {
            worklanes.iter().any(|worklane| {
                worklane.pane_rows.iter().any(|pane| {
                    pane.agent_status
                        .as_ref()
                        .is_some_and(|status| status.phase == zentty_core::AgentPhase::Running)
                })
            })
        });
        self.refresh_sleep_inhibitor(has_running_agent);
        if let Some(status_notifier) = self.status_notifier.as_mut() {
            status_notifier.refresh(self.config.menu_bar.show_status_item, &snapshot);
        }
        if snapshot == self.fleet_snapshot {
            return;
        }
        let summary = zentty_core::FleetSummary::from_snapshots(&snapshot);
        let progress_count = snapshot
            .iter()
            .filter(|pane| pane.progress.is_some())
            .count();
        eprintln!(
            "zentty-linux: fleet-refresh windows={} agents={} waiting={} stopped={} compacting={} active={} idle={} progress={}",
            owned_sources.len(),
            snapshot.len(),
            summary.waiting_count,
            summary.stopped_count,
            summary.compacting_count,
            summary.active_count,
            summary.idle_count,
            progress_count,
        );
        for shell in self.shells.values() {
            shell.borrow().refresh_fleet(&snapshot);
        }
        self.fleet_snapshot = snapshot;
    }

    fn show_agent_fleet(coordinator: &Rc<RefCell<Self>>) {
        let active_id = coordinator
            .borrow()
            .window_set
            .active_id()
            .map(str::to_owned);
        let Some(active_id) = active_id else {
            return;
        };
        if let Err(error) = Self::present_shell(coordinator, &active_id, true) {
            eprintln!("zentty-linux: status-notifier activation=failed detail={error}");
            return;
        }
        if let Some(shell) = coordinator.borrow().shells.get(&active_id) {
            shell.borrow().show_agent_fleet();
            eprintln!("zentty-linux: status-notifier activation=opened-fleet window={active_id}");
        }
    }

    fn refresh_sleep_inhibitor(&mut self, has_running_agent: bool) {
        let enabled = self.config.agent_caffeination.enabled;
        let previous_deadline = self.sleep_inhibition_state.release_deadline_ms();
        let transition =
            self.sleep_inhibition_state
                .update(enabled, has_running_agent, monotonic_time_ms());
        let current_deadline = self.sleep_inhibition_state.release_deadline_ms();
        if previous_deadline.is_none() && current_deadline.is_some() {
            eprintln!("zentty-linux: sleep-inhibitor state=release-pending delay-ms=10000");
        } else if previous_deadline.is_some()
            && current_deadline.is_none()
            && transition == zentty_core::SleepInhibitionTransition::None
            && enabled
            && has_running_agent
        {
            eprintln!("zentty-linux: sleep-inhibitor state=release-cancelled");
        }
        match transition {
            zentty_core::SleepInhibitionTransition::Acquire => {
                if let Err(error) = self.sleep_inhibitor.acquire() {
                    eprintln!(
                        "zentty-linux: sleep-inhibitor state=failed detail={}",
                        sanitize_log_field(&error)
                    );
                    self.sleep_inhibition_state
                        .mark_backend_lost(enabled && has_running_agent);
                }
            }
            zentty_core::SleepInhibitionTransition::Release => {
                self.sleep_inhibitor.release(if enabled {
                    "idle-debounce-complete"
                } else {
                    "setting-disabled"
                });
            }
            zentty_core::SleepInhibitionTransition::None => {}
        }
        if self.sleep_inhibitor.poll() == crate::sleep_inhibitor::LeasePoll::Lost {
            self.sleep_inhibition_state
                .mark_backend_lost(enabled && has_running_agent);
        }
    }

    fn route_server_commands(
        &self,
        commands: Vec<zentty_agent_ipc::AuthenticatedServerRequest>,
    ) -> Result<BTreeMap<String, Vec<zentty_agent_ipc::AuthenticatedServerRequest>>, String> {
        let mut by_window = BTreeMap::<String, Vec<_>>::new();
        for command in commands {
            if self.shells.contains_key(&command.target.window_id) {
                by_window
                    .entry(command.target.window_id.clone())
                    .or_default()
                    .push(command);
                continue;
            }
            let target = command.target.clone();
            let reply = ServerIpcReply::failure(
                "stale_target",
                format!("window {:?} is no longer available", target.window_id),
            )
            .map_err(|error| format!("could not create stale server reply: {error}"))?;
            if let Err(error) = command.respond(reply) {
                eprintln!(
                    "zentty-linux: server-stale-target-response window={} pane={} error={error}",
                    target.window_id, target.pane_id
                );
            }
        }
        Ok(by_window)
    }

    pub(crate) fn record_terminal_error(&mut self, error: String) {
        if self.terminal_error.is_none() {
            self.terminal_error = Some(error);
        }
        self.main_loop.quit();
    }

    pub(crate) fn teardown_flag(&self) -> Rc<Cell<bool>> {
        Rc::clone(&self.teardown_active)
    }

    pub(crate) fn snapshot(&self) -> ApplicationCycleResult {
        ApplicationCycleResult {
            windows: self
                .window_set
                .ordered_ids()
                .iter()
                .filter_map(|id| self.shells.get(id))
                .map(snapshot_shell)
                .collect(),
            active_window_id: self.window_set.active_id().map(str::to_owned),
        }
    }

    pub(crate) fn finish(&mut self) -> Result<ApplicationCycleResult, String> {
        let _ = self.sleep_inhibition_state.force_release();
        self.sleep_inhibitor.release("application-shutdown");
        if let Some(error) = self.terminal_error.take() {
            let _ = self.shutdown_all();
            return Err(error);
        }
        let result = self.exit_snapshot.take().unwrap_or_else(|| self.snapshot());
        if !self.shells.is_empty() {
            self.shutting_down = true;
            self.shutdown_all()?;
        }
        while glib::MainContext::default().pending() {
            glib::MainContext::default().iteration(false);
        }
        eprintln!("zentty-linux: lifecycle-windows={}", result.windows.len());
        eprintln!("zentty-linux: lifecycle complete");
        Ok(result)
    }
}

fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

fn monotonic_time_ms() -> u64 {
    u64::try_from(glib::monotonic_time().max(0) / 1_000).unwrap_or(u64::MAX)
}

fn sanitize_log_field(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .take(512)
        .collect()
}

fn is_wayland_display() -> bool {
    gdk::Display::default().is_some_and(|display| display.type_().name() == "GdkWaylandDisplay")
}

fn snapshot_shell(shell: &Rc<RefCell<ApplicationShell>>) -> WindowSnapshot {
    let shell = shell.borrow();
    WindowSnapshot {
        window: shell.window_recipe(),
        restored_drafts: shell.agent_restore_drafts(),
    }
}

fn settle_gtk_teardown() {
    let settle_loop = glib::MainLoop::new(None, false);
    let quit_loop = settle_loop.clone();
    glib::timeout_add_local_once(Duration::from_millis(50), move || quit_loop.quit());
    settle_loop.run();
}
