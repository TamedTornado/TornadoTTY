use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Duration;

use gtk::gio;
use gtk::glib::{self, variant::ToVariant};
use gtk::prelude::*;
use zentty_core::{
    ClosePaneOutcome, ColumnRecipe, PaneRecipe, SidebarWidthPreference, WindowRecipe,
    WorklaneColor, WorklaneRecipe, WorkspaceState,
};
use zentty_ghostty::{GhosttyRuntime, GhosttySurface, SurfaceConfig};

use crate::{sidebar, window_chrome::WindowChrome};

const ACTION_TOGGLE_SIDEBAR: &str = "toggle-sidebar";
const ACTION_NEW_WORKLANE: &str = "new-worklane";
const ACTION_SELECT_WORKLANE: &str = "select-worklane";
const ACTION_SPLIT_PANE_RIGHT: &str = "split-pane-right";
const ACTION_SPLIT_PANE_BELOW: &str = "split-pane-below";
const ACTION_CLOSE_PANE: &str = "close-pane";
const ACTION_RENAME_WORKLANE: &str = "rename-worklane";
const ACTION_CYCLE_WORKLANE_COLOR: &str = "cycle-worklane-color";
const ACTION_SET_WORKLANE_COLOR: &str = "set-worklane-color";
const ACTION_CLOSE_WORKLANE: &str = "close-worklane";
const ACTION_MOVE_WORKLANE: &str = "move-worklane";
const ACTION_MOVE_WORKLANE_UP: &str = "move-worklane-up";
const ACTION_MOVE_WORKLANE_DOWN: &str = "move-worklane-down";
const ACTION_MOVE_PANE_LEFT: &str = "move-pane-left";
const ACTION_MOVE_PANE_RIGHT: &str = "move-pane-right";
const ACTION_MOVE_PANE_UP: &str = "move-pane-up";
const ACTION_MOVE_PANE_DOWN: &str = "move-pane-down";
const ACTION_MOVE_PANE_TO_WORKLANE: &str = "move-pane-to-worklane";
const ACTION_SELECT_PANE: &str = "select-pane";

pub(crate) struct ApplicationShell {
    window: gtk::Window,
    chrome: WindowChrome,
    body: gtk::Paned,
    sidebar: gtk::Box,
    sidebar_scroll: gtk::ScrolledWindow,
    pane_box: gtk::Box,
    state: WorkspaceState,
    surfaces: BTreeMap<String, GhosttySurface>,
    focus_controllers: BTreeMap<String, gtk::EventControllerFocus>,
    runtime: GhosttyRuntime,
    command: Option<String>,
    main_loop: glib::MainLoop,
    live_children: Rc<Cell<usize>>,
    quit_after_last_terminal_exit: bool,
    next_worklane_number: usize,
    next_pane_number: usize,
    window_template: WindowRecipe,
    shutting_down: bool,
    preferred_sidebar_width: Rc<Cell<i32>>,
    adjusting_sidebar_width: Rc<Cell<bool>>,
}

impl ApplicationShell {
    pub(crate) fn new(
        runtime: &GhosttyRuntime,
        command: Option<String>,
        terminal_count: usize,
        quit_after_last_terminal_exit: bool,
        main_loop: &glib::MainLoop,
        restored_window: Option<WindowRecipe>,
    ) -> Result<Rc<RefCell<Self>>, String> {
        let window = gtk::Window::new();
        window.set_title(Some(zentty_core::PRODUCT_NAME));
        window.set_default_size(1000, 700);
        sidebar::install_styles();

        let body = gtk::Paned::new(gtk::Orientation::Horizontal);
        body.set_position(SidebarWidthPreference::DEFAULT);
        body.set_resize_start_child(false);
        body.set_shrink_start_child(true);
        let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let sidebar_scroll = gtk::ScrolledWindow::new();
        sidebar_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        sidebar_scroll.set_child(Some(&sidebar));

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.set_hexpand(true);
        content.set_vexpand(true);

        let pane_box = gtk::Box::new(gtk::Orientation::Horizontal, 1);
        pane_box.set_homogeneous(true);
        pane_box.set_hexpand(true);
        pane_box.set_vexpand(true);
        content.append(&pane_box);
        body.set_start_child(Some(&sidebar_scroll));
        body.set_end_child(Some(&content));

        let chrome = WindowChrome::new();
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(chrome.widget());
        root.append(&body);
        window.set_child(Some(&root));

        let window_template = restored_window.unwrap_or_else(default_window_recipe);
        let state = WorkspaceState::from_window_recipe(&window_template)
            .map_err(|error| format!("workspace restore failed: {error}"))?;
        let next_worklane_number = next_numeric_identity(
            state
                .worklanes()
                .iter()
                .map(|worklane| worklane.id.as_str()),
            "worklane-",
        );
        let next_pane_number = next_numeric_identity(
            state
                .worklanes()
                .iter()
                .flat_map(|worklane| &worklane.columns)
                .flat_map(|column| &column.panes)
                .map(|pane| pane.id.as_str()),
            "pane-",
        );
        let initial_pane_ids = state
            .worklanes()
            .iter()
            .flat_map(|worklane| &worklane.columns)
            .flat_map(|column| &column.panes)
            .map(|pane| pane.id.clone())
            .collect::<Vec<_>>();
        let preferred_sidebar_width = Rc::new(Cell::new(SidebarWidthPreference::DEFAULT));
        let adjusting_sidebar_width = Rc::new(Cell::new(false));
        let shell = Rc::new(RefCell::new(Self {
            window,
            chrome,
            body: body.clone(),
            sidebar,
            sidebar_scroll,
            pane_box,
            state,
            surfaces: BTreeMap::new(),
            focus_controllers: BTreeMap::new(),
            runtime: runtime.clone(),
            command,
            main_loop: main_loop.clone(),
            live_children: Rc::new(Cell::new(0)),
            quit_after_last_terminal_exit,
            next_worklane_number,
            next_pane_number,
            window_template,
            shutting_down: false,
            preferred_sidebar_width: Rc::clone(&preferred_sidebar_width),
            adjusting_sidebar_width: Rc::clone(&adjusting_sidebar_width),
        }));

        install_sidebar_width_tracking(&body, preferred_sidebar_width, adjusting_sidebar_width);

        Self::install_actions(&shell);
        for pane_id in initial_pane_ids {
            Self::create_surface(&shell, &pane_id)?;
        }
        let active_terminal_count = shell.borrow().state.active_pane_ids().len();
        for _ in active_terminal_count..terminal_count {
            Self::split_focused_pane_right(&shell)?;
        }
        shell.borrow().render();
        Ok(shell)
    }

    pub(crate) fn window(&self) -> &gtk::Window {
        &self.window
    }

    pub(crate) fn sidebar_container(&self) -> &gtk::ScrolledWindow {
        &self.sidebar_scroll
    }

    pub(crate) fn reconcile_sidebar_width(&self) {
        let available_width = self.body.width();
        if available_width <= 0 {
            return;
        }
        let target =
            SidebarWidthPreference::clamped(self.preferred_sidebar_width.get(), available_width);
        if self.body.position() != target {
            self.adjusting_sidebar_width.set(true);
            self.body.set_position(target);
            self.adjusting_sidebar_width.set(false);
        }
    }

    pub(crate) fn live_children(&self) -> usize {
        self.live_children.get()
    }

    pub(crate) fn window_recipe(&self) -> WindowRecipe {
        self.state.to_window_recipe(&self.window_template)
    }

    pub(crate) fn present(&self) {
        self.window.present();
        self.focus_selected_surface();
    }

    pub(crate) fn detach_and_close(&mut self) {
        self.shutting_down = true;
        gtk::prelude::GtkWindowExt::set_focus(&self.window, gtk::Widget::NONE);
        self.window.set_default_widget(gtk::Widget::NONE);
        for (pane_id, controller) in std::mem::take(&mut self.focus_controllers) {
            if let Some(surface) = self.surfaces.get(&pane_id) {
                surface.widget().remove_controller(&controller);
            }
        }
        clear_pane_columns(&self.pane_box);
        // The shell retains `sidebar` after detaching the root widget. Clear
        // its cards explicitly so their menu popovers and window-capturing
        // callbacks are finalized before Ghostty's process-global teardown.
        sidebar::clear(&self.sidebar);
        self.window.set_child(gtk::Widget::NONE);
        self.window.close();
    }

    pub(crate) fn release_surfaces(&mut self) -> Result<(), String> {
        for (_, surface) in std::mem::take(&mut self.surfaces) {
            surface.dispose().map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub(crate) fn schedule_workspace_actions(
        shell: &Rc<RefCell<Self>>,
        quit_when_complete: bool,
        close_worklane_when_complete: bool,
    ) {
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
                4 => window.activate_action(
                    "workspace.rename-worklane",
                    Some(&("worklane-1", "  Frontend  ").to_variant()),
                ),
                5 => window.activate_action(
                    "workspace.set-worklane-color",
                    Some(&("worklane-1", "red").to_variant()),
                ),
                6 => window.activate_action(
                    "workspace.move-worklane",
                    Some(&("worklane-1", "down").to_variant()),
                ),
                7 => window.activate_action("workspace.move-pane-left", None),
                8 => window.activate_action("workspace.split-pane-below", None),
                9 => window.activate_action("workspace.move-pane-up", None),
                10 => window.activate_action("workspace.move-pane-down", None),
                11 => window.activate_action(
                    "workspace.move-pane-to-worklane",
                    Some(&"worklane-2".to_variant()),
                ),
                12 if close_worklane_when_complete => window
                    .activate_action("workspace.close-worklane", Some(&"worklane-1".to_variant())),
                _ => {
                    eprintln!("zentty-linux: workspace-action-scenario complete");
                    if quit_when_complete {
                        shell.borrow().main_loop.quit();
                    }
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

        let toggle_sidebar = gio::SimpleAction::new(ACTION_TOGGLE_SIDEBAR, None);
        let weak = Rc::downgrade(shell);
        toggle_sidebar.connect_activate(move |_, _| {
            let Some(shell) = weak.upgrade() else {
                return;
            };
            let visible = {
                let shell = shell.borrow();
                let visible = !shell.sidebar_scroll.is_visible();
                shell.sidebar_scroll.set_visible(visible);
                eprintln!(
                    "zentty-linux: sidebar-visibility={}",
                    if visible { "pinned-open" } else { "hidden" }
                );
                visible
            };
            let weak = Rc::downgrade(&shell);
            glib::idle_add_local_once(move || {
                if let Some(shell) = weak.upgrade() {
                    shell.borrow().focus_selected_surface();
                }
            });
            eprintln!("zentty-linux: action=toggle-sidebar visible={visible}");
        });
        group.add_action(&toggle_sidebar);

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
            let changed = shell.state.active_worklane_id() != id;
            if shell.state.select_worklane(id) {
                eprintln!("zentty-linux: action=select-worklane id={id}");
                if changed {
                    shell.render();
                } else {
                    shell.refresh_sidebar_metadata();
                }
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

        let split_pane = gio::SimpleAction::new(ACTION_SPLIT_PANE_BELOW, None);
        let weak = Rc::downgrade(shell);
        split_pane.connect_activate(move |_, _| {
            let Some(shell) = weak.upgrade() else {
                return;
            };
            if let Err(error) = Self::split_focused_pane_below(&shell) {
                Self::report_action_error(&shell, ACTION_SPLIT_PANE_BELOW, &error);
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

        Self::install_edit_actions(shell, &group);

        shell
            .borrow()
            .window
            .insert_action_group("workspace", Some(&group));
    }

    fn install_edit_actions(shell: &Rc<RefCell<Self>>, group: &gio::SimpleActionGroup) {
        let string_pair = glib::VariantTy::new("(ss)").expect("static action type is valid");
        Self::install_worklane_edit_actions(shell, group, string_pair);

        let select_pane = gio::SimpleAction::new(ACTION_SELECT_PANE, Some(string_pair));
        let weak = Rc::downgrade(shell);
        select_pane.connect_activate(move |_, parameter| {
            let (Some(shell), Some((worklane_id, pane_id))) = (
                weak.upgrade(),
                parameter.and_then(glib::Variant::get::<(String, String)>),
            ) else {
                return;
            };
            let mut shell = shell.borrow_mut();
            let worklane_changed = shell.state.active_worklane_id() != worklane_id;
            if shell.state.select_worklane(&worklane_id) && shell.state.select_pane(&pane_id) {
                eprintln!("zentty-linux: action=select-pane worklane={worklane_id} pane={pane_id}");
                if worklane_changed {
                    shell.render();
                } else {
                    shell.refresh_sidebar_metadata();
                }
                shell.focus_selected_surface();
            }
        });
        group.add_action(&select_pane);

        Self::install_pane_transfer_action(shell, group);

        Self::add_simple_action(shell, group, ACTION_CYCLE_WORKLANE_COLOR, |shell| {
            let active_id = shell.state.active_worklane_id().to_owned();
            let current = shell.state.active_worklane().color;
            let next = match current {
                None => Some(WorklaneColor::Red),
                Some(color) => WorklaneColor::ALL
                    .iter()
                    .position(|candidate| *candidate == color)
                    .and_then(|index| WorklaneColor::ALL.get(index + 1).copied()),
            };
            if shell.state.set_worklane_color(&active_id, next) {
                eprintln!(
                    "zentty-linux: action=cycle-worklane-color id={active_id} color={}",
                    next.map_or("none", WorklaneColor::as_str)
                );
                shell.refresh_sidebar_metadata();
            }
        });
        Self::add_simple_action(shell, group, ACTION_MOVE_WORKLANE_UP, |shell| {
            shell.move_active_worklane(-1);
        });
        Self::add_simple_action(shell, group, ACTION_MOVE_WORKLANE_DOWN, |shell| {
            shell.move_active_worklane(1);
        });
        Self::add_simple_action(shell, group, ACTION_MOVE_PANE_LEFT, |shell| {
            if shell.state.move_focused_pane_left() {
                eprintln!("zentty-linux: action=move-pane-left");
                shell.render();
                shell.focus_selected_surface();
            }
        });
        Self::add_simple_action(shell, group, ACTION_MOVE_PANE_RIGHT, |shell| {
            if shell.state.move_focused_pane_right() {
                eprintln!("zentty-linux: action=move-pane-right");
                shell.render();
                shell.focus_selected_surface();
            }
        });
        Self::add_simple_action(shell, group, ACTION_MOVE_PANE_UP, |shell| {
            if shell.state.move_focused_pane_up() {
                eprintln!("zentty-linux: action=move-pane-up");
                shell.render();
                shell.focus_selected_surface();
            }
        });
        Self::add_simple_action(shell, group, ACTION_MOVE_PANE_DOWN, |shell| {
            if shell.state.move_focused_pane_down() {
                eprintln!("zentty-linux: action=move-pane-down");
                shell.render();
                shell.focus_selected_surface();
            }
        });
    }

    fn install_worklane_edit_actions(
        shell: &Rc<RefCell<Self>>,
        group: &gio::SimpleActionGroup,
        string_pair: &glib::VariantTy,
    ) {
        let rename_worklane = gio::SimpleAction::new(ACTION_RENAME_WORKLANE, Some(string_pair));
        let weak = Rc::downgrade(shell);
        rename_worklane.connect_activate(move |_, parameter| {
            let (Some(shell), Some((worklane_id, title))) = (
                weak.upgrade(),
                parameter.and_then(glib::Variant::get::<(String, String)>),
            ) else {
                return;
            };
            let mut shell = shell.borrow_mut();
            if shell.state.set_worklane_title(&worklane_id, Some(&title)) {
                eprintln!("zentty-linux: action=rename-worklane id={worklane_id} title={title:?}");
                shell.refresh_sidebar_metadata();
            }
        });
        group.add_action(&rename_worklane);

        let set_worklane_color =
            gio::SimpleAction::new(ACTION_SET_WORKLANE_COLOR, Some(string_pair));
        let weak = Rc::downgrade(shell);
        set_worklane_color.connect_activate(move |_, parameter| {
            let (Some(shell), Some((worklane_id, color_name))) = (
                weak.upgrade(),
                parameter.and_then(glib::Variant::get::<(String, String)>),
            ) else {
                return;
            };
            let color = if color_name.is_empty() {
                None
            } else if let Some(color) = WorklaneColor::named(&color_name) {
                Some(color)
            } else {
                eprintln!(
                    "zentty-linux: action=set-worklane-color rejected id={worklane_id} color={color_name}"
                );
                return;
            };
            let mut shell = shell.borrow_mut();
            if shell.state.set_worklane_color(&worklane_id, color) {
                eprintln!(
                    "zentty-linux: action=set-worklane-color id={worklane_id} color={}",
                    color.map_or("none", WorklaneColor::as_str)
                );
                shell.refresh_sidebar_metadata();
            }
        });
        group.add_action(&set_worklane_color);

        let close_worklane =
            gio::SimpleAction::new(ACTION_CLOSE_WORKLANE, Some(glib::VariantTy::STRING));
        let weak = Rc::downgrade(shell);
        close_worklane.connect_activate(move |_, parameter| {
            let (Some(shell), Some(worklane_id)) =
                (weak.upgrade(), parameter.and_then(glib::Variant::str))
            else {
                return;
            };
            Self::close_worklane(&shell, worklane_id);
        });
        group.add_action(&close_worklane);

        let move_worklane = gio::SimpleAction::new(ACTION_MOVE_WORKLANE, Some(string_pair));
        let weak = Rc::downgrade(shell);
        move_worklane.connect_activate(move |_, parameter| {
            let (Some(shell), Some((worklane_id, direction))) = (
                weak.upgrade(),
                parameter.and_then(glib::Variant::get::<(String, String)>),
            ) else {
                return;
            };
            let delta = match direction.as_str() {
                "up" => -1,
                "down" => 1,
                _ => return,
            };
            let mut shell = shell.borrow_mut();
            if shell.move_worklane(&worklane_id, delta) {
                eprintln!(
                    "zentty-linux: action=move-worklane id={worklane_id} direction={direction}"
                );
            }
        });
        group.add_action(&move_worklane);
    }

    fn install_pane_transfer_action(shell: &Rc<RefCell<Self>>, group: &gio::SimpleActionGroup) {
        let move_pane =
            gio::SimpleAction::new(ACTION_MOVE_PANE_TO_WORKLANE, Some(glib::VariantTy::STRING));
        let weak = Rc::downgrade(shell);
        move_pane.connect_activate(move |_, parameter| {
            let (Some(shell), Some(target_worklane_id)) =
                (weak.upgrade(), parameter.and_then(glib::Variant::str))
            else {
                return;
            };
            let mut shell = shell.borrow_mut();
            let pane_id = shell.state.focused_pane_id().map(str::to_owned);
            if shell
                .state
                .transfer_focused_pane_to_worklane(target_worklane_id)
            {
                eprintln!(
                    "zentty-linux: action=move-pane-to-worklane pane={} target={target_worklane_id}",
                    pane_id.as_deref().unwrap_or("unknown")
                );
                shell.render();
                shell.focus_selected_surface();
            }
        });
        group.add_action(&move_pane);
    }

    fn add_simple_action(
        shell: &Rc<RefCell<Self>>,
        group: &gio::SimpleActionGroup,
        name: &'static str,
        handler: impl Fn(&mut Self) + 'static,
    ) {
        let action = gio::SimpleAction::new(name, None);
        let weak = Rc::downgrade(shell);
        action.connect_activate(move |_, _| {
            if let Some(shell) = weak.upgrade() {
                handler(&mut shell.borrow_mut());
            }
        });
        group.add_action(&action);
    }

    fn move_active_worklane(&mut self, delta: isize) {
        let active_id = self.state.active_worklane_id().to_owned();
        self.move_worklane(&active_id, delta);
    }

    fn move_worklane(&mut self, worklane_id: &str, delta: isize) -> bool {
        let Some(index) = self
            .state
            .worklanes()
            .iter()
            .position(|worklane| worklane.id == worklane_id)
        else {
            return false;
        };
        let Some(target) = index.checked_add_signed(delta) else {
            return false;
        };
        if self.state.move_worklane(worklane_id, target) {
            eprintln!("zentty-linux: action=move-worklane target={target}");
            self.render_sidebar();
            return true;
        }
        false
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
        Self::split_focused_pane(shell, ACTION_SPLIT_PANE_RIGHT, |state, pane_id| {
            state.split_focused_pane_right(pane_id)
        })
    }

    fn split_focused_pane_below(shell: &Rc<RefCell<Self>>) -> Result<(), String> {
        Self::split_focused_pane(shell, ACTION_SPLIT_PANE_BELOW, |state, pane_id| {
            state.split_focused_pane_below(pane_id)
        })
    }

    fn split_focused_pane(
        shell: &Rc<RefCell<Self>>,
        action: &str,
        update: impl FnOnce(&mut WorkspaceState, String) -> bool,
    ) -> Result<(), String> {
        let pane_id = {
            let mut shell = shell.borrow_mut();
            let pane_id = shell.take_pane_id();
            if !update(&mut shell.state, pane_id.clone()) {
                return Err("generated duplicate pane identity".to_owned());
            }
            pane_id
        };
        if let Err(error) = Self::create_surface(shell, &pane_id) {
            let _ = shell.borrow_mut().state.close_focused_pane();
            return Err(error);
        }
        let shell_ref = shell.borrow();
        eprintln!("zentty-linux: action={action} pane={pane_id}");
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

    fn close_worklane(shell: &Rc<RefCell<Self>>, worklane_id: &str) {
        let pane_ids = {
            let shell = shell.borrow();
            shell
                .state
                .worklanes()
                .iter()
                .find(|worklane| worklane.id == worklane_id)
                .map(|worklane| {
                    worklane
                        .columns
                        .iter()
                        .flat_map(|column| &column.panes)
                        .map(|pane| pane.id.clone())
                        .collect::<Vec<_>>()
                })
        };
        let Some(pane_ids) = pane_ids else {
            return;
        };
        let mut shell_ref = shell.borrow_mut();
        if !shell_ref.state.close_worklane(worklane_id) {
            return;
        }
        for pane_id in &pane_ids {
            if let Err(error) = shell_ref.remove_live_surface(pane_id) {
                drop(shell_ref);
                Self::report_action_error(shell, ACTION_CLOSE_WORKLANE, &error);
                return;
            }
        }
        eprintln!(
            "zentty-linux: action=close-worklane id={worklane_id} panes={}",
            pane_ids.len()
        );
        shell_ref.render();
        shell_ref.focus_selected_surface();
    }

    fn close_pane(shell: &Rc<RefCell<Self>>, pane_id: &str) {
        let mut shell_ref = shell.borrow_mut();
        match shell_ref.state.close_pane(pane_id) {
            ClosePaneOutcome::Closed => {
                if let Err(error) = shell_ref.remove_live_surface(pane_id) {
                    drop(shell_ref);
                    Self::report_action_error(shell, ACTION_CLOSE_PANE, &error);
                    return;
                }
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
                let shell = shell.borrow();
                if shell.shutting_down {
                    return;
                }
                if shell.state.focused_pane_id() == Some(ready_id.as_str()) {
                    shell.focus_selected_surface();
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
                    if shell.state.set_pane_title(&title_id, &title) {
                        shell.refresh_sidebar_metadata();
                    }
                }
            });
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
                let Some(shell) = weak.upgrade() else {
                    return;
                };
                if shell.borrow().shutting_down {
                    return;
                }
                let still_focused = shell
                    .borrow()
                    .surfaces
                    .get(&focus_id)
                    .is_some_and(|surface| surface.widget().has_focus());
                if still_focused {
                    let changed = shell.borrow_mut().state.select_pane(&focus_id);
                    if changed {
                        eprintln!("zentty-linux: focus-pane pane={focus_id}");
                        shell.borrow().refresh_sidebar_metadata();
                    }
                }
            });
        });
        surface.widget().add_controller(focus_controller.clone());

        let mut shell = shell.borrow_mut();
        shell.live_children.set(shell.live_children.get() + 1);
        shell
            .focus_controllers
            .insert(pane_id.to_owned(), focus_controller);
        shell.surfaces.insert(pane_id.to_owned(), surface);
        Ok(())
    }

    fn handle_child_exit(shell: &Rc<RefCell<Self>>, pane_id: &str) {
        let mut shell_ref = shell.borrow_mut();
        if !shell_ref.surfaces.contains_key(pane_id) {
            eprintln!("zentty-linux: child-exit-after-dispose pane={pane_id} ignored");
            return;
        }
        let remaining = shell_ref.live_children.get().saturating_sub(1);
        shell_ref.live_children.set(remaining);
        if shell_ref.shutting_down {
            return;
        }
        if shell_ref.quit_after_last_terminal_exit {
            let outcome = shell_ref.state.close_pane(pane_id);
            if let Err(error) = shell_ref.remove_surface(pane_id) {
                eprintln!("zentty-linux: child-exit cleanup failed: {error}");
                shell_ref.main_loop.quit();
                return;
            }
            match outcome {
                ClosePaneOutcome::Closed => shell_ref.render(),
                ClosePaneOutcome::CloseWindow => clear_pane_columns(&shell_ref.pane_box),
                ClosePaneOutcome::NotFound => {}
            }
            if remaining == 0 {
                shell_ref.main_loop.quit();
            }
        } else {
            drop(shell_ref);
            Self::close_pane(shell, pane_id);
        }
    }

    fn report_action_error(shell: &Rc<RefCell<Self>>, action: &str, error: &str) {
        eprintln!("zentty-linux: action={action} failed: {error}");
        shell.borrow().main_loop.quit();
    }

    fn remove_surface(&mut self, pane_id: &str) -> Result<(), String> {
        if let Some(controller) = self.focus_controllers.remove(pane_id)
            && let Some(surface) = self.surfaces.get(pane_id)
        {
            surface.widget().remove_controller(&controller);
        }
        if let Some(surface) = self.surfaces.remove(pane_id) {
            if let Some(parent) = surface
                .widget()
                .parent()
                .and_then(|parent| parent.downcast::<gtk::Box>().ok())
            {
                parent.remove(surface.widget());
            }
            surface.dispose().map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn remove_live_surface(&mut self, pane_id: &str) -> Result<(), String> {
        if self.surfaces.contains_key(pane_id) {
            self.live_children
                .set(self.live_children.get().saturating_sub(1));
        }
        self.remove_surface(pane_id)
    }

    fn take_pane_id(&mut self) -> String {
        let id = format!("pane-{}", self.next_pane_number);
        self.next_pane_number += 1;
        id
    }

    fn render(&self) {
        clear_pane_columns(&self.pane_box);
        self.render_sidebar();

        for column in self.state.active_columns() {
            let column_box = gtk::Box::new(gtk::Orientation::Vertical, 1);
            column_box.set_homogeneous(true);
            column_box.set_hexpand(true);
            column_box.set_vexpand(true);
            for pane in &column.panes {
                if let Some(surface) = self.surfaces.get(&pane.id) {
                    column_box.append(surface.widget());
                }
            }
            self.pane_box.append(&column_box);
        }
        eprintln!("zentty-linux: topology={}", self.topology_receipt());
        eprintln!("zentty-linux: geometry={}", self.geometry_receipt());
    }

    fn render_sidebar(&self) {
        let summaries = self.state.sidebar_summaries();
        sidebar::render(&self.sidebar, &self.window, &summaries);
        self.chrome.render(&summaries);
    }

    fn refresh_sidebar_metadata(&self) {
        let summaries = self.state.sidebar_summaries();
        if !sidebar::update_metadata(&self.sidebar, &summaries) {
            sidebar::render(&self.sidebar, &self.window, &summaries);
        }
        self.chrome.render(&summaries);
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
                    "{}[title={},color={}]:{}{}",
                    worklane.id,
                    worklane.title.as_deref().unwrap_or("none"),
                    worklane.color.map_or("none", WorklaneColor::as_str),
                    worklane
                        .columns
                        .iter()
                        .flat_map(|column| &column.panes)
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

    fn geometry_receipt(&self) -> String {
        self.state
            .worklanes()
            .iter()
            .map(|worklane| {
                format!(
                    "{}:{}",
                    worklane.id,
                    worklane
                        .columns
                        .iter()
                        .map(|column| format!(
                            "{}[{}]",
                            column.id,
                            column
                                .panes
                                .iter()
                                .map(|pane| pane.id.as_str())
                                .collect::<Vec<_>>()
                                .join(",")
                        ))
                        .collect::<Vec<_>>()
                        .join(",")
                )
            })
            .collect::<Vec<_>>()
            .join("|")
    }
}

fn install_sidebar_width_tracking(
    body: &gtk::Paned,
    preferred_width: Rc<Cell<i32>>,
    adjusting_width: Rc<Cell<bool>>,
) {
    body.connect_position_notify(move |body| {
        if adjusting_width.get() || body.width() <= 0 {
            return;
        }
        let position = body.position();
        let clamped = SidebarWidthPreference::clamped(position, body.width());
        preferred_width.set(clamped);
        if position != clamped {
            adjusting_width.set(true);
            body.set_position(clamped);
            adjusting_width.set(false);
        }
        eprintln!("zentty-linux: sidebar-preferred-width={clamped}");
    });
}

fn clear_pane_columns(container: &gtk::Box) {
    while let Some(column_widget) = container.first_child() {
        if let Ok(column) = column_widget.clone().downcast::<gtk::Box>() {
            while let Some(surface) = column.first_child() {
                column.remove(&surface);
            }
        }
        container.remove(&column_widget);
    }
}

fn next_numeric_identity<'a>(ids: impl Iterator<Item = &'a str>, prefix: &str) -> usize {
    ids.filter_map(|id| id.strip_prefix(prefix)?.parse::<usize>().ok())
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

fn default_window_recipe() -> WindowRecipe {
    WindowRecipe {
        id: "window-1".to_owned(),
        frame: None,
        worklanes: vec![WorklaneRecipe {
            id: "worklane-1".to_owned(),
            title: None,
            next_pane_number: 2,
            focused_column_id: Some("column-worklane-1".to_owned()),
            columns: vec![ColumnRecipe {
                id: "column-worklane-1".to_owned(),
                width: 1.0,
                focused_pane_id: Some("pane-1".to_owned()),
                last_focused_pane_id: Some("pane-1".to_owned()),
                pane_heights: vec![1.0],
                panes: vec![PaneRecipe {
                    id: "pane-1".to_owned(),
                    custom_title: None,
                    title_seed: Some("shell".to_owned()),
                    working_directory: None,
                    last_activity_title: None,
                    last_run_command: None,
                }],
            }],
            color: None,
            bookmark_origin_id: None,
        }],
        active_worklane_id: Some("worklane-1".to_owned()),
    }
}
