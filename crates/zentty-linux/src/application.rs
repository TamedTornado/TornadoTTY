use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk, glib};
use zentty_core::{AppConfig, WindowRecipe};
use zentty_ghostty::GhosttyRuntime;
use zentty_tmux_compat::TmuxCompatReply;

use crate::agent_runtime::AgentRuntime;
use crate::application_shell::{ApplicationRuntimes, ApplicationShell};
use crate::persistence_coordinator::WindowSnapshot;
use crate::window_set::{CloseWindowDecision, WindowSet};

pub(crate) struct ApplicationCycleResult {
    pub(crate) windows: Vec<WindowSnapshot>,
    pub(crate) active_window_id: Option<String>,
}

pub(crate) struct ApplicationCoordinator {
    runtime: GhosttyRuntime,
    agent_runtime: Rc<RefCell<AgentRuntime>>,
    command: Option<String>,
    config: AppConfig,
    main_loop: glib::MainLoop,
    window_set: WindowSet,
    shells: BTreeMap<String, Rc<RefCell<ApplicationShell>>>,
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
        config: AppConfig,
    ) -> Result<Rc<RefCell<Self>>, String> {
        let ids = restored_windows
            .iter()
            .map(|snapshot| snapshot.window.id.clone())
            .collect::<Vec<_>>();
        let window_set = WindowSet::restore(ids, active_window_id)
            .map_err(|error| format!("could not compose restored windows: {error:?}"))?;
        let agent_runtime = Rc::new(RefCell::new(AgentRuntime::start()?));
        let coordinator = Rc::new(RefCell::new(Self {
            runtime: runtime.clone(),
            agent_runtime,
            command,
            config,
            main_loop: main_loop.clone(),
            window_set,
            shells: BTreeMap::new(),
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
        }
        Ok(coordinator)
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
            Ok(()) => Self::present_shell(coordinator, &id, true),
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
        let (runtime, agent_runtime, command, config, main_loop) = {
            let coordinator = coordinator.borrow();
            (
                coordinator.runtime.clone(),
                Rc::clone(&coordinator.agent_runtime),
                coordinator.command.clone(),
                coordinator.config.clone(),
                coordinator.main_loop.clone(),
            )
        };
        let runtimes = ApplicationRuntimes {
            ghostty: runtime,
            agent: agent_runtime,
            config,
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
        shell.borrow().present();
        if schedule_focus {
            ApplicationShell::focus_terminal_after_present(&shell);
        }
        eprintln!("zentty-linux: window-opened id={id}");
        Ok(())
    }

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
        let weak = Rc::downgrade(coordinator);
        let close_id = id.to_owned();
        let close_window_handler: Rc<dyn Fn()> = Rc::new(move || {
            let weak = weak.clone();
            let id = close_id.clone();
            glib::idle_add_local_once(move || {
                if let Some(coordinator) = weak.upgrade()
                    && let Err(error) = coordinator.borrow_mut().close_window(&id)
                {
                    eprintln!("zentty-linux: close-window id={id} error={error}");
                }
            });
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
        shell.borrow_mut().set_application_handlers(
            new_window_handler,
            move_pane_to_new_window_handler,
            close_window_handler,
            quit_handler,
        );

        let weak = Rc::downgrade(coordinator);
        let active_id = id.to_owned();
        let teardown_active = Rc::clone(&coordinator.borrow().teardown_active);
        shell
            .borrow()
            .window()
            .connect_is_active_notify(move |window| {
                if teardown_active.get() || !window.is_active() {
                    return;
                }
                if let Some(coordinator) = weak.upgrade()
                    && coordinator.borrow_mut().window_set.mark_active(&active_id)
                {
                    eprintln!("zentty-linux: active-window={active_id}");
                }
            });

        let weak = Rc::downgrade(coordinator);
        let closing_id = id.to_owned();
        let close_flag = Rc::new(Cell::new(false));
        coordinator
            .borrow_mut()
            .close_flags
            .insert(id.to_owned(), Rc::clone(&close_flag));
        shell.borrow().window().connect_close_request(move |_| {
            if close_flag.get() {
                return glib::Propagation::Proceed;
            }
            let Some(coordinator) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let weak = Rc::downgrade(&coordinator);
            let id = closing_id.clone();
            glib::idle_add_local_once(move || {
                if let Some(coordinator) = weak.upgrade()
                    && let Err(error) = coordinator.borrow_mut().close_window(&id)
                {
                    eprintln!("zentty-linux: close-window id={id} error={error}");
                }
            });
            glib::Propagation::Stop
        });
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
        eprintln!(
            "zentty-linux: action=move-pane-to-new-window pane={pane_id} source={source_id} destination={destination_id}"
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
        self.last_window_sizes.remove(id);
        self.last_sidebar_widths.remove(id);
        if decision == CloseWindowDecision::QuitApplication {
            self.exit_snapshot = Some(ApplicationCycleResult {
                windows: vec![snapshot_shell(&shell)],
                active_window_id: Some(id.to_owned()),
            });
            self.shutting_down = true;
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
        shell.borrow_mut().detach_and_close();
        settle_gtk_teardown();
        let release_result = shell.borrow_mut().release_surfaces();
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

    pub(crate) fn tick(&mut self) -> Result<(), String> {
        for shell in self.shells.values() {
            shell.borrow_mut().sync_agent_targets();
        }
        let (events, tmux_commands) = {
            let runtime = self.agent_runtime.borrow();
            (runtime.drain(), runtime.drain_tmux())
        };
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
        self.runtime.tick().map_err(|error| error.to_string())
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
