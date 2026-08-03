use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Duration;

use gtk::gio;
use gtk::glib::{self, variant::ToVariant};
use gtk::prelude::*;
use zentty_core::{ClosePaneOutcome, WorkspaceState};
use zentty_ghostty::{GhosttyRuntime, GhosttySurface, SurfaceConfig};

const ACTION_NEW_WORKLANE: &str = "new-worklane";
const ACTION_SELECT_WORKLANE: &str = "select-worklane";
const ACTION_SPLIT_PANE_RIGHT: &str = "split-pane-right";
const ACTION_CLOSE_PANE: &str = "close-pane";

pub(crate) struct ApplicationShell {
    window: gtk::Window,
    sidebar: gtk::Box,
    pane_box: gtk::Box,
    state: WorkspaceState,
    surfaces: BTreeMap<String, GhosttySurface>,
    runtime: GhosttyRuntime,
    command: Option<String>,
    main_loop: glib::MainLoop,
    live_children: Rc<Cell<usize>>,
    quit_after_last_terminal_exit: bool,
    next_worklane_number: usize,
    next_pane_number: usize,
}

impl ApplicationShell {
    pub(crate) fn new(
        runtime: &GhosttyRuntime,
        command: Option<String>,
        terminal_count: usize,
        quit_after_last_terminal_exit: bool,
        main_loop: &glib::MainLoop,
    ) -> Result<Rc<RefCell<Self>>, String> {
        let window = gtk::Window::new();
        window.set_title(Some(zentty_core::PRODUCT_NAME));
        window.set_default_size(1000, 700);

        let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 6);
        sidebar.set_width_request(190);
        sidebar.set_margin_top(12);
        sidebar.set_margin_bottom(12);
        sidebar.set_margin_start(12);
        sidebar.set_margin_end(12);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.set_hexpand(true);
        content.set_vexpand(true);
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        toolbar.set_margin_top(6);
        toolbar.set_margin_bottom(6);
        toolbar.set_margin_start(6);
        toolbar.set_margin_end(6);
        let split_button = gtk::Button::with_label("Split pane");
        split_button.set_action_name(Some("workspace.split-pane-right"));
        let close_button = gtk::Button::with_label("Close pane");
        close_button.set_action_name(Some("workspace.close-pane"));
        toolbar.append(&split_button);
        toolbar.append(&close_button);

        let pane_box = gtk::Box::new(gtk::Orientation::Horizontal, 1);
        pane_box.set_homogeneous(true);
        pane_box.set_hexpand(true);
        pane_box.set_vexpand(true);
        content.append(&toolbar);
        content.append(&pane_box);
        root.append(&sidebar);
        root.append(&content);
        window.set_child(Some(&root));

        let state = WorkspaceState::new("worklane-1", "pane-1");
        let shell = Rc::new(RefCell::new(Self {
            window,
            sidebar,
            pane_box,
            state,
            surfaces: BTreeMap::new(),
            runtime: runtime.clone(),
            command,
            main_loop: main_loop.clone(),
            live_children: Rc::new(Cell::new(0)),
            quit_after_last_terminal_exit,
            next_worklane_number: 2,
            next_pane_number: 2,
        }));

        Self::install_actions(&shell);
        Self::create_surface(&shell, "pane-1")?;
        for _ in 1..terminal_count {
            Self::split_focused_pane_right(&shell)?;
        }
        shell.borrow().render();
        Ok(shell)
    }

    pub(crate) fn window(&self) -> &gtk::Window {
        &self.window
    }

    pub(crate) fn live_children(&self) -> usize {
        self.live_children.get()
    }

    pub(crate) fn present(&self) {
        self.window.present();
        self.focus_selected_surface();
    }

    pub(crate) fn detach_and_close(&mut self) {
        remove_all_children(&self.pane_box);
        self.window.set_child(gtk::Widget::NONE);
        self.window.close();
        self.surfaces.clear();
    }

    pub(crate) fn schedule_workspace_actions(shell: &Rc<RefCell<Self>>) {
        let weak = Rc::downgrade(shell);
        let step = Rc::new(Cell::new(0_u8));
        glib::timeout_add_local(Duration::from_millis(100), move || {
            let Some(shell) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let window = shell.borrow().window.clone();
            let result = match step.get() {
                0 => window.activate_action("workspace.new-worklane", None),
                1 | 3 => window.activate_action("workspace.split-pane-right", None),
                2 => window.activate_action(
                    "workspace.select-worklane",
                    Some(&"worklane-1".to_variant()),
                ),
                _ => {
                    eprintln!("zentty-linux: workspace-action-scenario complete");
                    return glib::ControlFlow::Break;
                }
            };
            if let Err(error) = result {
                eprintln!("zentty-linux: workspace-action-scenario failed: {error}");
                shell.borrow().main_loop.quit();
                return glib::ControlFlow::Break;
            }
            step.set(step.get() + 1);
            glib::ControlFlow::Continue
        });
    }

    fn install_actions(shell: &Rc<RefCell<Self>>) {
        let group = gio::SimpleActionGroup::new();

        let new_worklane = gio::SimpleAction::new(ACTION_NEW_WORKLANE, None);
        let weak = Rc::downgrade(shell);
        new_worklane.connect_activate(move |_, _| {
            let Some(shell) = weak.upgrade() else {
                return;
            };
            if let Err(error) = Self::create_worklane(&shell) {
                Self::report_action_error(&shell, ACTION_NEW_WORKLANE, &error);
            }
        });
        group.add_action(&new_worklane);

        let select_worklane =
            gio::SimpleAction::new(ACTION_SELECT_WORKLANE, Some(glib::VariantTy::STRING));
        let weak = Rc::downgrade(shell);
        select_worklane.connect_activate(move |_, parameter| {
            let (Some(shell), Some(id)) = (weak.upgrade(), parameter.and_then(glib::Variant::str))
            else {
                return;
            };
            let mut shell = shell.borrow_mut();
            if shell.state.select_worklane(id) {
                eprintln!("zentty-linux: action=select-worklane id={id}");
                shell.render();
                shell.focus_selected_surface();
            }
        });
        group.add_action(&select_worklane);

        let split_pane = gio::SimpleAction::new(ACTION_SPLIT_PANE_RIGHT, None);
        let weak = Rc::downgrade(shell);
        split_pane.connect_activate(move |_, _| {
            let Some(shell) = weak.upgrade() else {
                return;
            };
            if let Err(error) = Self::split_focused_pane_right(&shell) {
                Self::report_action_error(&shell, ACTION_SPLIT_PANE_RIGHT, &error);
            }
        });
        group.add_action(&split_pane);

        let close_pane = gio::SimpleAction::new(ACTION_CLOSE_PANE, None);
        let weak = Rc::downgrade(shell);
        close_pane.connect_activate(move |_, _| {
            let Some(shell) = weak.upgrade() else {
                return;
            };
            Self::close_focused_pane(&shell);
        });
        group.add_action(&close_pane);

        shell
            .borrow()
            .window
            .insert_action_group("workspace", Some(&group));
    }

    fn create_worklane(shell: &Rc<RefCell<Self>>) -> Result<(), String> {
        let (worklane_id, pane_id) = {
            let mut shell = shell.borrow_mut();
            let worklane_id = format!("worklane-{}", shell.next_worklane_number);
            shell.next_worklane_number += 1;
            let pane_id = shell.take_pane_id();
            if !shell
                .state
                .create_worklane(worklane_id.clone(), pane_id.clone())
            {
                return Err("generated duplicate worklane or pane identity".to_owned());
            }
            (worklane_id, pane_id)
        };
        if let Err(error) = Self::create_surface(shell, &pane_id) {
            shell.borrow_mut().state.close_active_worklane();
            return Err(error);
        }
        let shell_ref = shell.borrow();
        eprintln!("zentty-linux: action=new-worklane id={worklane_id} pane={pane_id}");
        shell_ref.render();
        shell_ref.focus_selected_surface();
        Ok(())
    }

    fn split_focused_pane_right(shell: &Rc<RefCell<Self>>) -> Result<(), String> {
        let pane_id = {
            let mut shell = shell.borrow_mut();
            let pane_id = shell.take_pane_id();
            if !shell.state.split_focused_pane_right(pane_id.clone()) {
                return Err("generated duplicate pane identity".to_owned());
            }
            pane_id
        };
        if let Err(error) = Self::create_surface(shell, &pane_id) {
            let _ = shell.borrow_mut().state.close_focused_pane();
            return Err(error);
        }
        let shell_ref = shell.borrow();
        eprintln!("zentty-linux: action=split-pane-right pane={pane_id}");
        shell_ref.render();
        shell_ref.focus_selected_surface();
        Ok(())
    }

    fn close_focused_pane(shell: &Rc<RefCell<Self>>) {
        let pane_id = shell.borrow().state.focused_pane_id().map(str::to_owned);
        if let Some(pane_id) = pane_id {
            Self::close_pane(shell, &pane_id);
        }
    }

    fn close_pane(shell: &Rc<RefCell<Self>>, pane_id: &str) {
        let mut shell_ref = shell.borrow_mut();
        match shell_ref.state.close_pane(pane_id) {
            ClosePaneOutcome::Closed => {
                shell_ref.surfaces.remove(pane_id);
                eprintln!("zentty-linux: action=close-pane pane={pane_id}");
                shell_ref.render();
                shell_ref.focus_selected_surface();
            }
            ClosePaneOutcome::CloseWindow => shell_ref.main_loop.quit(),
            ClosePaneOutcome::NotFound => {}
        }
    }

    fn create_surface(shell: &Rc<RefCell<Self>>, pane_id: &str) -> Result<(), String> {
        let (runtime, command) = {
            let shell = shell.borrow();
            (shell.runtime.clone(), shell.command.clone())
        };
        let surface = runtime
            .create_surface(&SurfaceConfig {
                command,
                title: zentty_core::PRODUCT_NAME.to_owned(),
            })
            .map_err(|error| error.to_string())?;

        let ready_id = pane_id.to_owned();
        surface.on_initialized(move || {
            eprintln!("zentty-linux: terminal-ready");
            eprintln!("zentty-linux: terminal-ready-pane={ready_id}");
        });
        let title_id = pane_id.to_owned();
        surface.on_title_changed(move |title| {
            eprintln!("zentty-linux: title={title}");
            eprintln!("zentty-linux: title-pane={title_id} value={title}");
        });
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

        let focus_controller = gtk::EventControllerFocus::new();
        let weak = Rc::downgrade(shell);
        let focus_id = pane_id.to_owned();
        focus_controller.connect_enter(move |_| {
            let weak = weak.clone();
            let focus_id = focus_id.clone();
            glib::idle_add_local_once(move || {
                if let Some(shell) = weak.upgrade()
                    && shell.borrow_mut().state.select_pane(&focus_id)
                {
                    eprintln!("zentty-linux: focus-pane pane={focus_id}");
                }
            });
        });
        surface.widget().add_controller(focus_controller);

        let mut shell = shell.borrow_mut();
        shell.live_children.set(shell.live_children.get() + 1);
        shell.surfaces.insert(pane_id.to_owned(), surface);
        Ok(())
    }

    fn handle_child_exit(shell: &Rc<RefCell<Self>>, pane_id: &str) {
        let shell_ref = shell.borrow_mut();
        let remaining = shell_ref.live_children.get().saturating_sub(1);
        shell_ref.live_children.set(remaining);
        if remaining == 0 && shell_ref.quit_after_last_terminal_exit {
            shell_ref.main_loop.quit();
        } else if !shell_ref.quit_after_last_terminal_exit {
            drop(shell_ref);
            Self::close_pane(shell, pane_id);
        }
    }

    fn report_action_error(shell: &Rc<RefCell<Self>>, action: &str, error: &str) {
        eprintln!("zentty-linux: action={action} failed: {error}");
        shell.borrow().main_loop.quit();
    }

    fn take_pane_id(&mut self) -> String {
        let id = format!("pane-{}", self.next_pane_number);
        self.next_pane_number += 1;
        id
    }

    fn render(&self) {
        remove_all_children(&self.sidebar);
        remove_all_children(&self.pane_box);

        let heading = gtk::Label::new(Some("Worklanes"));
        heading.set_xalign(0.0);
        self.sidebar.append(&heading);
        for (index, worklane) in self.state.worklanes().iter().enumerate() {
            let label = worklane
                .title
                .clone()
                .unwrap_or_else(|| format!("Worklane {}", index + 1));
            let button = gtk::Button::with_label(&label);
            button.set_action_name(Some("workspace.select-worklane"));
            button.set_action_target_value(Some(&worklane.id.to_variant()));
            if worklane.id == self.state.active_worklane_id() {
                button.add_css_class("suggested-action");
            }
            self.sidebar.append(&button);
        }
        let new_button = gtk::Button::with_label("New worklane");
        new_button.set_action_name(Some("workspace.new-worklane"));
        self.sidebar.append(&new_button);

        for pane_id in self.state.active_pane_ids() {
            if let Some(surface) = self.surfaces.get(pane_id) {
                self.pane_box.append(surface.widget());
            }
        }
        eprintln!("zentty-linux: topology={}", self.topology_receipt());
    }

    fn focus_selected_surface(&self) {
        if let Some(pane_id) = self.state.focused_pane_id()
            && let Some(surface) = self.surfaces.get(pane_id)
        {
            surface.grab_focus();
        }
    }

    fn topology_receipt(&self) -> String {
        self.state
            .worklanes()
            .iter()
            .map(|worklane| {
                format!(
                    "{}:{}{}",
                    worklane.id,
                    worklane
                        .panes
                        .iter()
                        .map(|pane| pane.id.as_str())
                        .collect::<Vec<_>>()
                        .join(","),
                    if worklane.id == self.state.active_worklane_id() {
                        "*"
                    } else {
                        ""
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("|")
    }
}

fn remove_all_children(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}
