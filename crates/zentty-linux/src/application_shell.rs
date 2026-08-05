use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Duration;

use gtk::glib::{self, variant::ToVariant};
use gtk::prelude::*;
use gtk::{gdk, gio};
use zentty_core::{
    ClosePaneOutcome, ColumnRecipe, CommandPaletteItem, PaneLayoutPolicy, PaneRecipe,
    PaneReference, PaneRightInsertionBehavior, SidebarWidthPreference, WindowRecipe, WorklaneColor,
    WorklaneRecipe, WorkspaceState,
};
use zentty_ghostty::{GhosttyRuntime, GhosttySurface, SurfaceConfig};

use crate::{
    command_palette::CommandPaletteView,
    pane_controls::{self, PaneControlAction, PaneFrame, PanePresentation},
    pane_scroll_switch::{PaneScrollSwitch, ScrollSwitchResult, ScrollUnit},
    pane_search::{SearchShortcut, resolve_shortcut},
    peek_scroll_navigation::{
        Direction as PeekScrollDirection, PeekScrollNavigation, Result as PeekScrollResult,
        ScrollUnit as PeekScrollUnit,
    },
    sidebar,
    sidebar_visibility::{Event as SidebarVisibilityEvent, Mode as SidebarVisibilityMode},
    window_chrome::WindowChrome,
    worklane_peek::{
        self, Direction as PeekDirection, PanePreview, Phase as PeekPhase,
        SpatialDirection as PeekSpatialDirection, WorklanePeekView,
    },
};

const ACTION_TOGGLE_SIDEBAR: &str = "toggle-sidebar";
const ACTION_NEW_WORKLANE: &str = "new-worklane";
const ACTION_SELECT_WORKLANE: &str = "select-worklane";
const ACTION_SPLIT_PANE_RIGHT: &str = "split-pane-right";
const ACTION_ADD_PANE_RIGHT: &str = "add-pane-right";
const ACTION_ADD_PANE_LEFT: &str = "add-pane-left";
const ACTION_SPLIT_PANE_BELOW: &str = "split-pane-below";
const ACTION_CLOSE_PANE: &str = "close-pane";
const ACTION_RENAME_WORKLANE: &str = "rename-worklane";
const ACTION_RENAME_PANE: &str = "rename-pane";
const ACTION_CYCLE_WORKLANE_COLOR: &str = "cycle-worklane-color";
const ACTION_SET_WORKLANE_COLOR: &str = "set-worklane-color";
const ACTION_CLOSE_WORKLANE: &str = "close-worklane";
const ACTION_MOVE_WORKLANE: &str = "move-worklane";
const ACTION_REORDER_WORKLANE: &str = "reorder-worklane";
const ACTION_MOVE_WORKLANE_UP: &str = "move-worklane-up";
const ACTION_MOVE_WORKLANE_DOWN: &str = "move-worklane-down";
const ACTION_MOVE_PANE_LEFT: &str = "move-pane-left";
const ACTION_MOVE_PANE_RIGHT: &str = "move-pane-right";
const ACTION_MOVE_PANE_UP: &str = "move-pane-up";
const ACTION_MOVE_PANE_DOWN: &str = "move-pane-down";
const ACTION_MOVE_PANE_TO_WORKLANE: &str = "move-pane-to-worklane";
const ACTION_SELECT_PANE: &str = "select-pane";
const ACTION_NAVIGATE_BACK: &str = "navigate-back";
const ACTION_NAVIGATE_FORWARD: &str = "navigate-forward";
const ACTION_NEXT_PANE: &str = "next-pane";
const ACTION_PREVIOUS_PANE: &str = "previous-pane";
const ACTION_NEXT_WORKLANE: &str = "next-worklane";
const ACTION_PREVIOUS_WORKLANE: &str = "previous-worklane";
const ACTION_DISMISS_COMMAND_PALETTE: &str = "dismiss-command-palette";
const ACTION_FIND: &str = "find";
const ACTION_USE_SELECTION_FOR_FIND: &str = "use-selection-for-find";
const ACTION_FIND_NEXT: &str = "find-next";
const ACTION_FIND_PREVIOUS: &str = "find-previous";
const ACTION_FOCUS_PANE_LEFT: &str = "focus-pane-left";
const ACTION_FOCUS_PANE_RIGHT: &str = "focus-pane-right";
const ACTION_FOCUS_PANE_UP: &str = "focus-pane-up";
const ACTION_FOCUS_PANE_DOWN: &str = "focus-pane-down";
const ACTION_ARRANGE_WIDTH_FULL: &str = "arrange-width-full";
const ACTION_ARRANGE_WIDTH_HALF: &str = "arrange-width-half";
const ACTION_ARRANGE_WIDTH_THIRDS: &str = "arrange-width-thirds";
const ACTION_ARRANGE_WIDTH_QUARTERS: &str = "arrange-width-quarters";
const ACTION_ARRANGE_HEIGHT_FULL: &str = "arrange-height-full";
const ACTION_ARRANGE_HEIGHT_TWO: &str = "arrange-height-two";
const ACTION_ARRANGE_HEIGHT_THREE: &str = "arrange-height-three";
const ACTION_ARRANGE_HEIGHT_FOUR: &str = "arrange-height-four";
const ACTION_ARRANGE_GOLDEN_WIDE: &str = "arrange-golden-wide";
const ACTION_ARRANGE_GOLDEN_NARROW: &str = "arrange-golden-narrow";
const ACTION_ARRANGE_GOLDEN_TALL: &str = "arrange-golden-tall";
const ACTION_ARRANGE_GOLDEN_SHORT: &str = "arrange-golden-short";
const ACTION_RESET_PANE_LAYOUT: &str = "reset-pane-layout";
const ACTION_RESTORE_CLOSED_PANE: &str = "restore-closed-pane";
const PRIMARY_RIGHT_BEHAVIOR: PaneRightInsertionBehavior = PaneRightInsertionBehavior::VisibleSplit;
const WORKLANE_PEEK_TAB_HOLD_THRESHOLD: Duration = Duration::from_millis(200);

pub(crate) struct ApplicationShell {
    window: gtk::Window,
    chrome: WindowChrome,
    body: gtk::Paned,
    sidebar_reservation: gtk::Box,
    sidebar_hover_rail: gtk::Box,
    sidebar: gtk::Box,
    sidebar_scroll: gtk::ScrolledWindow,
    pane_scroll: gtk::ScrolledWindow,
    pane_box: gtk::Box,
    state: WorkspaceState,
    surfaces: BTreeMap<String, GhosttySurface>,
    pane_frames: BTreeMap<String, PaneFrame>,
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
    sidebar_reveal_generation: Rc<Cell<u64>>,
    sidebar_visibility: crate::sidebar_visibility::State,
    sidebar_visibility_generation: u64,
    peek_phase: PeekPhase,
    peek_generation: u64,
    peek_tab_down: bool,
    peek_view: WorklanePeekView,
    command_palette: CommandPaletteView,
    last_pane_viewport_height: Cell<i32>,
    workspace_actions: Option<gio::SimpleActionGroup>,
    pending_prefills: BTreeMap<String, String>,
}

struct ShellWidgets {
    window: gtk::Window,
    chrome: WindowChrome,
    body: gtk::Paned,
    sidebar: gtk::Box,
    sidebar_scroll: gtk::ScrolledWindow,
    sidebar_reservation: gtk::Box,
    sidebar_hover_rail: gtk::Box,
    pane_scroll: gtk::ScrolledWindow,
    pane_box: gtk::Box,
    peek_view: WorklanePeekView,
    command_palette: CommandPaletteView,
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
        sidebar::install_styles();
        pane_controls::install_styles();
        let ShellWidgets {
            window,
            chrome,
            body,
            sidebar,
            sidebar_scroll,
            sidebar_reservation,
            sidebar_hover_rail,
            pane_scroll,
            pane_box,
            peek_view,
            command_palette,
        } = build_shell_widgets();

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
        let initial_pane_ids = workspace_pane_ids(&state);
        let preferred_sidebar_width = Rc::new(Cell::new(SidebarWidthPreference::DEFAULT));
        let adjusting_sidebar_width = Rc::new(Cell::new(false));
        let sidebar_reveal_generation = Rc::new(Cell::new(0));
        let shell = Rc::new(RefCell::new(Self {
            window,
            chrome,
            body: body.clone(),
            sidebar_reservation,
            sidebar_hover_rail,
            sidebar,
            sidebar_scroll,
            pane_scroll,
            pane_box,
            state,
            surfaces: BTreeMap::new(),
            pane_frames: BTreeMap::new(),
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
            sidebar_reveal_generation,
            sidebar_visibility: crate::sidebar_visibility::State::default(),
            sidebar_visibility_generation: 0,
            peek_phase: PeekPhase::Idle,
            peek_generation: 0,
            peek_tab_down: false,
            peek_view,
            command_palette,
            last_pane_viewport_height: Cell::new(0),
            workspace_actions: None,
            pending_prefills: BTreeMap::new(),
        }));

        install_sidebar_width_tracking(
            &body,
            &shell.borrow().sidebar_scroll,
            preferred_sidebar_width,
            adjusting_sidebar_width,
        );

        Self::install_actions(&shell);
        Self::install_sidebar_visibility(&shell);
        Self::install_pane_traversal_shortcuts(&shell);
        Self::install_peek_scroll_navigation(&shell);
        Self::install_pane_scroll_switching(&shell);
        Self::install_command_palette_shortcut(&shell);
        Self::install_search_shortcuts(&shell);
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
        self.sidebar_scroll.set_width_request(target);
        self.sidebar_reservation.set_width_request(target);
        if self.sidebar_visibility.mode() == SidebarVisibilityMode::PinnedOpen
            && self.body.position() != target
        {
            self.adjusting_sidebar_width.set(true);
            self.body.set_position(target);
            self.adjusting_sidebar_width.set(false);
        }
        self.refresh_right_insertion_behavior();
    }

    pub(crate) fn reconcile_pane_heights(&self) {
        self.apply_pane_height_requests(false);
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
        self.peek_phase = PeekPhase::Idle;
        self.peek_tab_down = false;
        self.peek_view.hide();
        self.command_palette.hide();
        gtk::prelude::GtkWindowExt::set_focus(&self.window, gtk::Widget::NONE);
        self.window.set_default_widget(gtk::Widget::NONE);
        for (pane_id, controller) in std::mem::take(&mut self.focus_controllers) {
            if let Some(surface) = self.surfaces.get(&pane_id) {
                surface.widget().remove_controller(&controller);
            }
        }
        clear_pane_columns(&self.pane_box);
        for frame in self.pane_frames.values() {
            frame.detach_terminal();
        }
        self.pane_frames.clear();
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
                4 | 5 => window.activate_action("workspace.navigate-back", None),
                6 | 7 => window.activate_action("workspace.navigate-forward", None),
                8 => window.activate_action(
                    "workspace.rename-worklane",
                    Some(&("worklane-1", "  Frontend  ").to_variant()),
                ),
                9 => window.activate_action(
                    "workspace.rename-pane",
                    Some(&("pane-4", "  Review Shell  ").to_variant()),
                ),
                10 => window.activate_action(
                    "workspace.set-worklane-color",
                    Some(&("worklane-1", "red").to_variant()),
                ),
                11 => window.activate_action(
                    "workspace.move-worklane",
                    Some(&("worklane-1", "down").to_variant()),
                ),
                12 => window.activate_action("workspace.move-pane-left", None),
                13 => window.activate_action("workspace.split-pane-below", None),
                14 => window.activate_action("workspace.move-pane-up", None),
                15 => window.activate_action("workspace.move-pane-down", None),
                16 => window.activate_action(
                    "workspace.move-pane-to-worklane",
                    Some(&"worklane-2".to_variant()),
                ),
                17 => window.activate_action("workspace.next-pane", None),
                18 => window.activate_action("workspace.previous-pane", None),
                19 => window.activate_action("workspace.next-worklane", None),
                20 => window.activate_action("workspace.previous-worklane", None),
                21 if close_worklane_when_complete => window
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

    pub(crate) fn schedule_pane_search_actions(
        shell: &Rc<RefCell<Self>>,
        quit_when_complete: bool,
    ) {
        let weak = Rc::downgrade(shell);
        let step = Rc::new(Cell::new(0_u8));
        glib::timeout_add_local(Duration::from_millis(120), move || {
            let Some(shell) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let window = shell.borrow().window.clone();
            let result = match step.get() {
                0 => require_invalid_binding_action_rejected(&shell),
                1 => window.activate_action("workspace.find", None),
                2 => {
                    shell.borrow().perform_focused_binding_action(
                        "search-scenario-query",
                        "search:selectable",
                    );
                    Ok(())
                }
                3 => require_focused_search_state(&shell, true, Some(3), None),
                4 => window.activate_action("workspace.find-next", None),
                5 => require_focused_search_state(&shell, true, Some(3), Some(true)),
                6 => window.activate_action("workspace.find-previous", None),
                7 => {
                    shell
                        .borrow()
                        .perform_focused_binding_action("search-scenario-end", "end_search");
                    Ok(())
                }
                8 => require_focused_search_state(&shell, false, None, Some(false)),
                _ => {
                    eprintln!("zentty-linux: pane-search-action-scenario complete");
                    if quit_when_complete {
                        shell.borrow().main_loop.quit();
                    }
                    return glib::ControlFlow::Break;
                }
            };
            if let Err(error) = result {
                eprintln!("zentty-linux: pane-search-action-scenario failed: {error}");
                shell.borrow().main_loop.quit();
                return glib::ControlFlow::Break;
            }
            step.set(step.get() + 1);
            glib::ControlFlow::Continue
        });
    }

    pub(crate) fn schedule_pane_layout_actions(
        shell: &Rc<RefCell<Self>>,
        quit_when_complete: bool,
    ) {
        let weak = Rc::downgrade(shell);
        let step = Rc::new(Cell::new(0_u8));
        glib::timeout_add_local(Duration::from_millis(100), move || {
            let Some(shell) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let window = shell.borrow().window.clone();
            let result = match step.get() {
                0 => require_golden_action_availability(&shell, false, false),
                1 => window.activate_action("workspace.add-pane-left", None),
                2 => window.activate_action("workspace.split-pane-right", None),
                3 => window.activate_action("workspace.split-pane-below", None),
                4 => window.activate_action("workspace.focus-pane-up", None),
                5 => window.activate_action("workspace.focus-pane-left", None),
                6 => window.activate_action("workspace.focus-pane-right", None),
                7 => window.activate_action("workspace.focus-pane-down", None),
                8 => window.activate_action("workspace.arrange-width-thirds", None),
                9 => window.activate_action("workspace.arrange-height-two", None),
                10 => require_golden_action_availability(&shell, true, true),
                11 => window.activate_action("workspace.arrange-width-half", None),
                12 => window.activate_action("workspace.arrange-golden-tall", None),
                13 | 15 => require_rendered_golden_height(&shell, "pane-4", "pane-1", true),
                14 => {
                    window.set_default_size(1000, 820);
                    Ok(())
                }
                16 => window.activate_action("workspace.arrange-golden-wide", None),
                17 => require_rendered_golden_width(&shell, "pane-4", "pane-2", true),
                18 => window.activate_action("workspace.reset-pane-layout", None),
                19 => window.activate_action("workspace.arrange-height-full", None),
                20 => require_golden_action_availability(&shell, true, false),
                21 => window.activate_action("workspace.arrange-width-quarters", None),
                22 => window.activate_action("workspace.arrange-golden-narrow", None),
                23 => require_rendered_golden_width(&shell, "pane-4", "pane-1", false),
                24 => require_pane_layout_scenario_state(&shell),
                _ => {
                    eprintln!("zentty-linux: pane-layout-action-scenario complete");
                    if quit_when_complete {
                        shell.borrow().main_loop.quit();
                    }
                    return glib::ControlFlow::Break;
                }
            };
            if let Err(error) = result {
                eprintln!("zentty-linux: pane-layout-action-scenario failed: {error}");
                shell.borrow().main_loop.quit();
                return glib::ControlFlow::Break;
            }
            step.set(step.get() + 1);
            glib::ControlFlow::Continue
        });
    }

    pub(crate) fn schedule_closed_pane_restore(
        shell: &Rc<RefCell<Self>>,
        quit_when_complete: bool,
    ) {
        let weak = Rc::downgrade(shell);
        let step = Rc::new(Cell::new(0_u8));
        glib::timeout_add_local(Duration::from_millis(180), move || {
            let Some(shell) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let result = match step.get() {
                0 => {
                    Self::close_focused_pane(&shell);
                    Ok(())
                }
                1 => {
                    let window = shell.borrow().window.clone();
                    window.activate_action("workspace.restore-closed-pane", None)
                }
                2 => require_closed_pane_restore_state(&shell),
                _ => {
                    eprintln!("zentty-linux: closed-pane-restore-scenario complete");
                    if quit_when_complete {
                        shell.borrow().main_loop.quit();
                    }
                    return glib::ControlFlow::Break;
                }
            };
            if let Err(error) = result {
                eprintln!("zentty-linux: closed-pane-restore-scenario failed: {error}");
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
                let mut shell = shell.borrow_mut();
                shell
                    .sidebar_visibility
                    .handle(SidebarVisibilityEvent::TogglePressed);
                shell.apply_sidebar_visibility();
                shell.sidebar_visibility.mode() != SidebarVisibilityMode::Hidden
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

        let dismiss_palette = gio::SimpleAction::new(ACTION_DISMISS_COMMAND_PALETTE, None);
        let weak = Rc::downgrade(shell);
        dismiss_palette.connect_activate(move |_, _| {
            let Some(shell) = weak.upgrade() else {
                return;
            };
            shell.borrow().command_palette.hide();
            shell.borrow().focus_selected_surface();
        });
        group.add_action(&dismiss_palette);

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

        Self::install_pane_creation_actions(shell, &group);
        Self::install_pane_layout_actions(shell, &group);

        Self::install_restore_closed_pane_action(shell, &group);

        Self::add_simple_action(shell, &group, ACTION_NAVIGATE_BACK, |shell| {
            shell.navigate_history(true);
        });
        Self::add_simple_action(shell, &group, ACTION_NAVIGATE_FORWARD, |shell| {
            shell.navigate_history(false);
        });
        Self::add_simple_action(shell, &group, ACTION_NEXT_PANE, |shell| {
            shell.select_adjacent_pane(true);
        });
        Self::add_simple_action(shell, &group, ACTION_PREVIOUS_PANE, |shell| {
            shell.select_adjacent_pane(false);
        });
        Self::add_simple_action(shell, &group, ACTION_NEXT_WORKLANE, |shell| {
            shell.select_adjacent_worklane(true);
        });
        Self::add_simple_action(shell, &group, ACTION_PREVIOUS_WORKLANE, |shell| {
            shell.select_adjacent_worklane(false);
        });
        Self::install_search_actions(shell, &group);

        Self::install_edit_actions(shell, &group);

        shell
            .borrow()
            .window
            .insert_action_group("workspace", Some(&group));
        shell.borrow_mut().workspace_actions = Some(group);
    }

    fn install_restore_closed_pane_action(
        shell: &Rc<RefCell<Self>>,
        group: &gio::SimpleActionGroup,
    ) {
        let action = gio::SimpleAction::new(ACTION_RESTORE_CLOSED_PANE, None);
        let weak = Rc::downgrade(shell);
        action.connect_activate(move |_, _| {
            let Some(shell) = weak.upgrade() else {
                return;
            };
            if let Err(error) = Self::restore_closed_pane(&shell) {
                Self::report_action_error(&shell, ACTION_RESTORE_CLOSED_PANE, &error);
            }
        });
        group.add_action(&action);
    }

    fn install_search_actions(shell: &Rc<RefCell<Self>>, group: &gio::SimpleActionGroup) {
        Self::add_simple_action(shell, group, ACTION_FIND, |shell| {
            shell.perform_focused_binding_action(ACTION_FIND, "start_search");
        });
        Self::add_simple_action(shell, group, ACTION_USE_SELECTION_FOR_FIND, |shell| {
            shell.perform_focused_binding_action(ACTION_USE_SELECTION_FOR_FIND, "search_selection");
        });
        Self::add_simple_action(shell, group, ACTION_FIND_NEXT, |shell| {
            shell.perform_focused_binding_action(ACTION_FIND_NEXT, "navigate_search:next");
        });
        Self::add_simple_action(shell, group, ACTION_FIND_PREVIOUS, |shell| {
            shell.perform_focused_binding_action(ACTION_FIND_PREVIOUS, "navigate_search:previous");
        });
    }

    fn install_pane_layout_actions(shell: &Rc<RefCell<Self>>, group: &gio::SimpleActionGroup) {
        for (name, update) in [
            (
                ACTION_FOCUS_PANE_LEFT,
                WorkspaceState::focus_pane_left as fn(&mut WorkspaceState) -> bool,
            ),
            (ACTION_FOCUS_PANE_RIGHT, WorkspaceState::focus_pane_right),
            (ACTION_FOCUS_PANE_UP, WorkspaceState::focus_pane_up),
            (ACTION_FOCUS_PANE_DOWN, WorkspaceState::focus_pane_down),
        ] {
            Self::add_simple_action(shell, group, name, move |shell| {
                if update(&mut shell.state) {
                    shell.finish_pane_layout_action(name);
                }
            });
        }
        for (name, visible_columns) in [
            (ACTION_ARRANGE_WIDTH_FULL, 1),
            (ACTION_ARRANGE_WIDTH_HALF, 2),
            (ACTION_ARRANGE_WIDTH_THIRDS, 3),
            (ACTION_ARRANGE_WIDTH_QUARTERS, 4),
        ] {
            Self::add_simple_action(shell, group, name, move |shell| {
                let width = f64::from(shell.pane_viewport_width());
                if shell.state.arrange_columns(visible_columns, width) {
                    shell.finish_pane_layout_action(name);
                }
            });
        }
        for (name, panes_per_column) in [
            (ACTION_ARRANGE_HEIGHT_FULL, 1),
            (ACTION_ARRANGE_HEIGHT_TWO, 2),
            (ACTION_ARRANGE_HEIGHT_THREE, 3),
            (ACTION_ARRANGE_HEIGHT_FOUR, 4),
        ] {
            Self::add_simple_action(shell, group, name, move |shell| {
                if shell.state.arrange_panes_per_column(panes_per_column) {
                    shell.finish_pane_layout_action(name);
                }
            });
        }
        for (name, focus_wide) in [
            (ACTION_ARRANGE_GOLDEN_WIDE, true),
            (ACTION_ARRANGE_GOLDEN_NARROW, false),
        ] {
            Self::add_simple_action(shell, group, name, move |shell| {
                let width = f64::from(shell.pane_viewport_width());
                if shell.state.arrange_golden_width(focus_wide, width) {
                    shell.finish_pane_layout_action(name);
                }
            });
        }
        for (name, focus_tall) in [
            (ACTION_ARRANGE_GOLDEN_TALL, true),
            (ACTION_ARRANGE_GOLDEN_SHORT, false),
        ] {
            Self::add_simple_action(shell, group, name, move |shell| {
                if shell.state.arrange_golden_height(focus_tall) {
                    shell.finish_pane_layout_action(name);
                }
            });
        }
        Self::add_simple_action(shell, group, ACTION_RESET_PANE_LAYOUT, |shell| {
            let width = f64::from(shell.pane_viewport_width());
            if shell.state.reset_active_layout(width) {
                shell.finish_pane_layout_action(ACTION_RESET_PANE_LAYOUT);
            }
        });
    }

    fn finish_pane_layout_action(&self, action: &str) {
        eprintln!(
            "zentty-linux: action={action} pane={}",
            self.state.focused_pane_id().unwrap_or("none")
        );
        self.render();
        self.focus_selected_surface();
        self.scroll_panes_to_focused();
    }

    fn install_sidebar_visibility(shell: &Rc<RefCell<Self>>) {
        let rail_motion = gtk::EventControllerMotion::new();
        let weak = Rc::downgrade(shell);
        rail_motion.connect_enter(move |_, _, _| {
            if let Some(shell) = weak.upgrade() {
                let Ok(mut shell) = shell.try_borrow_mut() else {
                    return;
                };
                shell
                    .sidebar_visibility
                    .handle(SidebarVisibilityEvent::HoverRailEntered);
                shell.apply_sidebar_visibility();
            }
        });
        let weak = Rc::downgrade(shell);
        rail_motion.connect_leave(move |_| {
            if let Some(shell) = weak.upgrade() {
                let changed = shell.try_borrow_mut().is_ok_and(|mut shell| {
                    shell
                        .sidebar_visibility
                        .handle(SidebarVisibilityEvent::HoverRailExited);
                    true
                });
                if changed {
                    Self::schedule_sidebar_dismissal(&shell);
                }
            }
        });
        shell
            .borrow()
            .sidebar_hover_rail
            .add_controller(rail_motion);

        let sidebar_motion = gtk::EventControllerMotion::new();
        let weak = Rc::downgrade(shell);
        sidebar_motion.connect_enter(move |_, _, _| {
            if let Some(shell) = weak.upgrade() {
                let Ok(mut shell) = shell.try_borrow_mut() else {
                    return;
                };
                shell.sidebar_visibility_generation =
                    shell.sidebar_visibility_generation.wrapping_add(1);
                shell
                    .sidebar_visibility
                    .handle(SidebarVisibilityEvent::SidebarEntered);
            }
        });
        let weak = Rc::downgrade(shell);
        sidebar_motion.connect_leave(move |_| {
            if let Some(shell) = weak.upgrade() {
                let changed = shell.try_borrow_mut().is_ok_and(|mut shell| {
                    shell
                        .sidebar_visibility
                        .handle(SidebarVisibilityEvent::SidebarExited);
                    true
                });
                if changed {
                    Self::schedule_sidebar_dismissal(&shell);
                }
            }
        });
        shell.borrow().sidebar_scroll.add_controller(sidebar_motion);
    }

    fn schedule_sidebar_dismissal(shell: &Rc<RefCell<Self>>) {
        let generation = {
            let Ok(mut shell) = shell.try_borrow_mut() else {
                return;
            };
            shell.sidebar_visibility_generation =
                shell.sidebar_visibility_generation.wrapping_add(1);
            shell.sidebar_visibility_generation
        };
        let weak = Rc::downgrade(shell);
        glib::timeout_add_local_once(Duration::from_millis(250), move || {
            let Some(shell) = weak.upgrade() else {
                return;
            };
            let Ok(mut shell) = shell.try_borrow_mut() else {
                return;
            };
            if shell.sidebar_visibility_generation != generation {
                return;
            }
            if shell
                .sidebar_visibility
                .handle(SidebarVisibilityEvent::DismissTimerElapsed)
            {
                shell.apply_sidebar_visibility();
                shell.focus_selected_surface();
            }
        });
    }

    fn apply_sidebar_visibility(&mut self) {
        self.sidebar_visibility_generation = self.sidebar_visibility_generation.wrapping_add(1);
        let width = SidebarWidthPreference::clamped(
            self.preferred_sidebar_width.get(),
            self.window.width().max(1),
        );
        self.sidebar_scroll.set_width_request(width);
        match self.sidebar_visibility.mode() {
            SidebarVisibilityMode::PinnedOpen => {
                if self.body.start_child().is_none() {
                    self.body.set_start_child(Some(&self.sidebar_reservation));
                }
                self.sidebar_reservation.set_width_request(width);
                self.body.set_position(width);
                self.sidebar_scroll
                    .remove_css_class("zentty-sidebar-floating");
                self.sidebar_scroll.set_visible(true);
                self.sidebar_hover_rail.set_visible(false);
                eprintln!("zentty-linux: sidebar-visibility=pinned-open");
            }
            SidebarVisibilityMode::Hidden => {
                self.body.set_start_child(None::<&gtk::Widget>);
                self.sidebar_scroll.set_visible(false);
                self.sidebar_hover_rail.set_visible(true);
                eprintln!("zentty-linux: sidebar-visibility=hidden");
            }
            SidebarVisibilityMode::HoverPeek => {
                self.body.set_start_child(None::<&gtk::Widget>);
                self.sidebar_scroll.add_css_class("zentty-sidebar-floating");
                self.sidebar_scroll.set_visible(true);
                self.sidebar_hover_rail.set_visible(true);
                eprintln!("zentty-linux: sidebar-visibility=hover-peek");
            }
        }
    }

    fn install_pane_traversal_shortcuts(shell: &Rc<RefCell<Self>>) {
        let controller = gtk::EventControllerKey::new();
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(shell);
        controller.connect_key_pressed(move |_, key, _, modifiers| {
            let is_tab = key == gdk::Key::Tab || key == gdk::Key::ISO_Left_Tab;
            let Some(shell) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if !shell.borrow().peek_phase.is_active()
                && is_restore_closed_pane_shortcut(key, modifiers)
            {
                Self::activate_restore_closed_pane_shortcut(&shell);
                return glib::Propagation::Stop;
            }
            if !shell.borrow().peek_phase.is_active()
                && modifiers.contains(gdk::ModifierType::CONTROL_MASK)
                && modifiers.contains(gdk::ModifierType::SUPER_MASK)
                && (key == gdk::Key::Up || key == gdk::Key::Down)
            {
                shell
                    .borrow_mut()
                    .move_active_worklane(if key == gdk::Key::Up { -1 } else { 1 });
                return glib::Propagation::Stop;
            }
            if !shell.borrow().peek_phase.is_active()
                && modifiers.contains(gdk::ModifierType::CONTROL_MASK)
                && (key == gdk::Key::Page_Down || key == gdk::Key::Page_Up)
            {
                shell
                    .borrow_mut()
                    .select_adjacent_worklane(key == gdk::Key::Page_Down);
                return glib::Propagation::Stop;
            }
            if !shell.borrow().peek_phase.is_active()
                && modifiers.contains(gdk::ModifierType::ALT_MASK)
                && (key == gdk::Key::Left || key == gdk::Key::Right)
            {
                shell.borrow_mut().navigate_history(key == gdk::Key::Left);
                return glib::Propagation::Stop;
            }
            if is_tab && modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
                let repeated = {
                    let mut shell = shell.borrow_mut();
                    let repeated = shell.peek_tab_down;
                    shell.peek_tab_down = true;
                    repeated
                };
                if repeated {
                    // GTK key-pressed signals include keyboard auto-repeat.
                    // A held physical Tab is one command, not a stream of taps.
                    return glib::Propagation::Stop;
                }
                let forward = key != gdk::Key::ISO_Left_Tab
                    && !modifiers.contains(gdk::ModifierType::SHIFT_MASK);
                let direction = if forward {
                    PeekDirection::Forward
                } else {
                    PeekDirection::Backward
                };
                Self::handle_peek_tab(&shell, direction);
                return glib::Propagation::Stop;
            }
            if key == gdk::Key::Escape && shell.borrow().peek_phase.is_active() {
                Self::cancel_peek(&shell);
                return glib::Propagation::Stop;
            }
            if modifiers.contains(gdk::ModifierType::CONTROL_MASK)
                && matches!(shell.borrow().peek_phase, PeekPhase::Peeking { .. })
            {
                let direction = match key {
                    gdk::Key::Left => Some(PeekSpatialDirection::Left),
                    gdk::Key::Right => Some(PeekSpatialDirection::Right),
                    gdk::Key::Up => Some(PeekSpatialDirection::Up),
                    gdk::Key::Down => Some(PeekSpatialDirection::Down),
                    _ => None,
                };
                if let Some(direction) = direction {
                    Self::spatially_navigate_peek(&shell, direction);
                    return glib::Propagation::Stop;
                }
            }
            if matches!(shell.borrow().peek_phase, PeekPhase::Peeking { .. }) {
                // A visible picker owns the interaction. No ordinary key
                // press may leak into the still-focused Ghostty surface.
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        let weak = Rc::downgrade(shell);
        controller.connect_key_released(move |_, key, _, _| {
            let Some(shell) = weak.upgrade() else {
                return;
            };
            if key == gdk::Key::Tab || key == gdk::Key::ISO_Left_Tab {
                shell.borrow_mut().peek_tab_down = false;
                Self::handle_peek_tab_released(&shell);
            } else if key == gdk::Key::Control_L || key == gdk::Key::Control_R {
                shell.borrow_mut().peek_tab_down = false;
                Self::commit_peek(&shell);
            }
        });
        shell.borrow().window.add_controller(controller);
    }

    fn activate_restore_closed_pane_shortcut(shell: &Rc<RefCell<Self>>) {
        let window = shell.borrow().window.clone();
        if let Err(error) = window.activate_action("workspace.restore-closed-pane", None) {
            eprintln!("zentty-linux: restore shortcut failed: {error}");
        }
    }

    fn handle_peek_tab(shell: &Rc<RefCell<Self>>, direction: PeekDirection) {
        let phase = shell.borrow().peek_phase.clone();
        match phase {
            PeekPhase::Idle => {
                let generation = {
                    let mut shell = shell.borrow_mut();
                    shell.peek_generation = shell.peek_generation.wrapping_add(1);
                    let generation = shell.peek_generation;
                    shell.peek_phase = PeekPhase::Armed {
                        generation,
                        pending: direction,
                    };
                    generation
                };
                eprintln!("zentty-linux: worklane-peek=armed generation={generation}");
                let weak = Rc::downgrade(shell);
                glib::timeout_add_local_once(WORKLANE_PEEK_TAB_HOLD_THRESHOLD, move || {
                    let Some(shell) = weak.upgrade() else {
                        return;
                    };
                    Self::open_peek_after_tab_hold(&shell, generation);
                });
            }
            PeekPhase::Armed { .. } => {}
            PeekPhase::Peeking {
                original,
                current,
                traversal,
            } => {
                let Some(next) = worklane_peek::step(&traversal, &current, direction) else {
                    return;
                };
                eprintln!(
                    "zentty-linux: worklane-peek=preview worklane={} pane={}",
                    next.worklane_id, next.pane_id
                );
                shell.borrow_mut().peek_phase = PeekPhase::Peeking {
                    original,
                    current: next,
                    traversal,
                };
                Self::refresh_peek_view(shell);
            }
        }
    }

    fn handle_peek_tab_released(shell: &Rc<RefCell<Self>>) {
        let phase = shell.borrow().peek_phase.clone();
        if let PeekPhase::Armed { pending, .. } = phase {
            shell.borrow_mut().peek_phase = PeekPhase::Idle;
            shell
                .borrow_mut()
                .select_adjacent_pane(pending == PeekDirection::Forward);
            eprintln!("zentty-linux: worklane-peek=quick-tab-release");
        }
    }

    fn open_peek_after_tab_hold(shell: &Rc<RefCell<Self>>, generation: u64) {
        if !shell.borrow().peek_tab_down
            || !matches!(
                shell.borrow().peek_phase,
                PeekPhase::Armed {
                    generation: armed_generation,
                    ..
                } if armed_generation == generation
            )
        {
            return;
        }
        Self::open_peek(shell);
    }

    fn open_peek(shell: &Rc<RefCell<Self>>) {
        if matches!(shell.borrow().peek_phase, PeekPhase::Peeking { .. }) {
            return;
        }
        let traversal = shell.borrow().pane_references_in_sidebar_order();
        let Some(origin) = shell.borrow().current_pane_reference() else {
            Self::cancel_peek(shell);
            return;
        };
        shell.borrow_mut().peek_phase = PeekPhase::Peeking {
            original: origin.clone(),
            current: origin,
            traversal,
        };
        eprintln!("zentty-linux: worklane-peek=open trigger=tab-hold");
        Self::refresh_peek_view(shell);
    }

    fn commit_peek(shell: &Rc<RefCell<Self>>) {
        let phase = shell.borrow().peek_phase.clone();
        match phase {
            PeekPhase::Idle => {}
            PeekPhase::Armed { pending, .. } => {
                shell.borrow_mut().peek_phase = PeekPhase::Idle;
                shell
                    .borrow_mut()
                    .select_adjacent_pane(pending == PeekDirection::Forward);
                eprintln!("zentty-linux: worklane-peek=quick-modifier-release");
            }
            PeekPhase::Peeking { current, .. } => {
                shell.borrow_mut().peek_phase = PeekPhase::Idle;
                shell.borrow().peek_view.hide();
                shell.borrow_mut().select_pane_reference(&current, true);
                eprintln!(
                    "zentty-linux: worklane-peek=commit worklane={} pane={}",
                    current.worklane_id, current.pane_id
                );
            }
        }
    }

    fn cancel_peek(shell: &Rc<RefCell<Self>>) {
        let phase = shell.borrow().peek_phase.clone();
        shell.borrow_mut().peek_phase = PeekPhase::Idle;
        shell.borrow().peek_view.hide();
        if let PeekPhase::Peeking { original, .. } = phase {
            shell.borrow_mut().select_pane_reference(&original, true);
        }
        eprintln!("zentty-linux: worklane-peek=cancel");
    }

    fn spatially_navigate_peek(shell: &Rc<RefCell<Self>>, direction: PeekSpatialDirection) {
        let phase = shell.borrow().peek_phase.clone();
        let PeekPhase::Peeking {
            original,
            current,
            traversal,
        } = phase
        else {
            return;
        };
        let Some(target) =
            worklane_peek::spatial_target(shell.borrow().state.worklanes(), &current, direction)
        else {
            return;
        };
        shell.borrow_mut().peek_phase = PeekPhase::Peeking {
            original,
            current: target.clone(),
            traversal,
        };
        eprintln!(
            "zentty-linux: worklane-peek=spatial worklane={} pane={}",
            target.worklane_id, target.pane_id
        );
        Self::refresh_peek_view(shell);
    }

    fn preview_peek_selection(shell: &Rc<RefCell<Self>>, target: &PaneReference) {
        let phase = shell.borrow().peek_phase.clone();
        let PeekPhase::Peeking {
            original,
            traversal,
            ..
        } = phase
        else {
            return;
        };
        if !traversal.contains(target) {
            return;
        }
        shell.borrow_mut().peek_phase = PeekPhase::Peeking {
            original,
            current: target.clone(),
            traversal,
        };
        eprintln!(
            "zentty-linux: worklane-peek=click worklane={} pane={}",
            target.worklane_id, target.pane_id
        );
        Self::refresh_peek_view(shell);
    }

    fn refresh_peek_view(shell: &Rc<RefCell<Self>>) {
        let selected = shell.borrow().peek_phase.selected().cloned();
        let Some(selected) = selected else {
            return;
        };
        let previews = shell.borrow().peek_previews();
        let weak = Rc::downgrade(shell);
        shell
            .borrow()
            .peek_view
            .render(previews, &selected, move |target| {
                if let Some(shell) = weak.upgrade() {
                    Self::preview_peek_selection(&shell, &target);
                }
            });
    }

    fn install_pane_scroll_switching(shell: &Rc<RefCell<Self>>) {
        let controller = gtk::EventControllerScroll::new(
            gtk::EventControllerScrollFlags::BOTH_AXES | gtk::EventControllerScrollFlags::KINETIC,
        );
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        let gesture = Rc::new(RefCell::new(PaneScrollSwitch::default()));
        let beginning = Rc::clone(&gesture);
        controller.connect_scroll_begin(move |_| beginning.borrow_mut().reset());
        let ending = Rc::clone(&gesture);
        controller.connect_scroll_end(move |_| ending.borrow_mut().reset());
        let weak = Rc::downgrade(shell);
        controller.connect_scroll(move |controller, dx, dy| {
            let unit = match controller.unit() {
                gdk::ScrollUnit::Wheel => ScrollUnit::Wheel,
                gdk::ScrollUnit::Surface => ScrollUnit::Surface,
                _ => return glib::Propagation::Proceed,
            };
            if unit == ScrollUnit::Wheel {
                gesture.borrow_mut().reset();
            }
            let shifted = controller
                .current_event_state()
                .contains(gdk::ModifierType::SHIFT_MASK);
            match gesture
                .borrow_mut()
                .handle(dx, dy, shifted, unit, glib::monotonic_time())
            {
                ScrollSwitchResult::Previous => {
                    if let Some(shell) = weak.upgrade() {
                        shell.borrow_mut().select_adjacent_pane(false);
                    }
                    glib::Propagation::Stop
                }
                ScrollSwitchResult::Next => {
                    if let Some(shell) = weak.upgrade() {
                        shell.borrow_mut().select_adjacent_pane(true);
                    }
                    glib::Propagation::Stop
                }
                ScrollSwitchResult::Consumed => glib::Propagation::Stop,
                ScrollSwitchResult::Unhandled => glib::Propagation::Proceed,
            }
        });
        shell.borrow().pane_scroll.add_controller(controller);
    }

    fn install_command_palette_shortcut(shell: &Rc<RefCell<Self>>) {
        let controller = gtk::EventControllerKey::new();
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(shell);
        controller.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(shell) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if shell.borrow().command_palette.is_visible() && key == gdk::Key::Escape {
                shell.borrow().command_palette.hide();
                shell.borrow().focus_selected_surface();
                return glib::Propagation::Stop;
            }
            let required = gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK;
            if !matches!(key, gdk::Key::p | gdk::Key::P)
                || !modifiers.contains(required)
                || modifiers.intersects(gdk::ModifierType::ALT_MASK | gdk::ModifierType::SUPER_MASK)
            {
                return glib::Propagation::Proceed;
            }
            let shell_ref = shell.borrow();
            if shell_ref.command_palette.is_visible() {
                shell_ref.command_palette.hide();
                shell_ref.focus_selected_surface();
            } else {
                let (items, current) = shell_ref.command_palette_items();
                shell_ref.command_palette.show(
                    items,
                    shell_ref.state.recent_pane_references(),
                    current,
                );
            }
            glib::Propagation::Stop
        });
        shell.borrow().window.add_controller(controller);
    }

    fn install_search_shortcuts(shell: &Rc<RefCell<Self>>) {
        let controller = gtk::EventControllerKey::new();
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(shell);
        controller.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(shell) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if shell.borrow().command_palette.is_visible() || shell.borrow().peek_phase.is_active()
            {
                return glib::Propagation::Proceed;
            }
            if key == gdk::Key::Escape {
                let hidden = {
                    let shell = shell.borrow();
                    let Some(pane_id) = shell.state.focused_pane_id() else {
                        return glib::Propagation::Proceed;
                    };
                    let Some(surface) = shell.surfaces.get(pane_id) else {
                        return glib::Propagation::Proceed;
                    };
                    let Some(overlay) = find_ghostty_search_overlay(surface.widget()) else {
                        return glib::Propagation::Proceed;
                    };
                    let Some(entry) = find_search_entry(&overlay) else {
                        return glib::Propagation::Proceed;
                    };
                    if !overlay.property::<bool>("active") || entry.text().is_empty() {
                        false
                    } else {
                        overlay.set_property("active", false);
                        eprintln!("zentty-linux: action=hide-search pane={pane_id}");
                        true
                    }
                };
                if hidden {
                    shell.borrow().focus_selected_surface();
                    return glib::Propagation::Stop;
                }
            }
            let Some(shortcut) = resolve_shortcut(key, modifiers) else {
                return glib::Propagation::Proceed;
            };
            let action = match shortcut {
                SearchShortcut::Find => ACTION_FIND,
                SearchShortcut::UseSelection => ACTION_USE_SELECTION_FOR_FIND,
                SearchShortcut::Next => ACTION_FIND_NEXT,
                SearchShortcut::Previous => ACTION_FIND_PREVIOUS,
            };
            // Action activation is synchronous. Clone the GTK handle before
            // dispatch so the action callback can mutably borrow the shell.
            let window = shell.borrow().window.clone();
            let _ = window.activate_action(&format!("workspace.{action}"), None);
            glib::Propagation::Stop
        });
        shell.borrow().window.add_controller(controller);
    }

    fn command_palette_items(&self) -> (Vec<CommandPaletteItem>, Option<PaneReference>) {
        let current = self
            .state
            .focused_pane_id()
            .map(|pane_id| PaneReference::new(self.state.active_worklane_id(), pane_id));
        let mut items = self
            .state
            .worklanes()
            .iter()
            .enumerate()
            .flat_map(|(index, worklane)| {
                let lane_title = worklane
                    .title
                    .clone()
                    .unwrap_or_else(|| format!("Worklane {}", index + 1));
                worklane.columns.iter().flat_map(move |column| {
                    let lane_title = lane_title.clone();
                    column.panes.iter().map(move |pane| {
                        CommandPaletteItem::pane(
                            pane.custom_title
                                .clone()
                                .unwrap_or_else(|| pane.live_title.clone()),
                            lane_title.clone(),
                            PaneReference::new(&worklane.id, &pane.id),
                        )
                    })
                })
            })
            .collect::<Vec<_>>();
        items.extend(Self::command_palette_action_items());
        (items, current)
    }

    #[allow(clippy::too_many_lines)] // Interim until the source command registry is ported.
    fn command_palette_action_items() -> Vec<CommandPaletteItem> {
        vec![
            CommandPaletteItem::action(
                "New Worklane",
                "Create another worklane",
                "workspace lane",
                ACTION_NEW_WORKLANE,
            ),
            CommandPaletteItem::action(
                "Split Right",
                "Split the focused pane into a visible right column",
                "pane column",
                ACTION_SPLIT_PANE_RIGHT,
            ),
            CommandPaletteItem::action(
                "Add Pane Right",
                "Add a full-width pane to the right of the focused column",
                "pane column canvas",
                ACTION_ADD_PANE_RIGHT,
            ),
            CommandPaletteItem::action(
                "Add Pane Left",
                "Add a full-width pane to the left of the focused column",
                "pane column canvas",
                ACTION_ADD_PANE_LEFT,
            ),
            CommandPaletteItem::action(
                "New Pane Below",
                "Split the focused pane vertically",
                "pane split down",
                ACTION_SPLIT_PANE_BELOW,
            ),
            CommandPaletteItem::action(
                "Toggle Sidebar",
                "Show or hide the worklane sidebar",
                "navigation",
                ACTION_TOGGLE_SIDEBAR,
            ),
            CommandPaletteItem::action(
                "Close Pane",
                "Close the focused pane",
                "terminal",
                ACTION_CLOSE_PANE,
            ),
            CommandPaletteItem::action(
                "Navigate Back",
                "Return to the previously focused pane",
                "history browser previous",
                ACTION_NAVIGATE_BACK,
            ),
            CommandPaletteItem::action(
                "Navigate Forward",
                "Move forward through pane focus history",
                "history browser next",
                ACTION_NAVIGATE_FORWARD,
            ),
            CommandPaletteItem::action(
                "Focus Next Pane",
                "Focus the next pane in sidebar order",
                "navigation terminal",
                ACTION_NEXT_PANE,
            ),
            CommandPaletteItem::action(
                "Focus Previous Pane",
                "Focus the previous pane in sidebar order",
                "navigation terminal",
                ACTION_PREVIOUS_PANE,
            ),
            CommandPaletteItem::action(
                "Next Worklane",
                "Focus the next worklane",
                "navigation workspace lane",
                ACTION_NEXT_WORKLANE,
            ),
            CommandPaletteItem::action(
                "Previous Worklane",
                "Focus the previous worklane",
                "navigation workspace lane",
                ACTION_PREVIOUS_WORKLANE,
            ),
            CommandPaletteItem::action(
                "Move Worklane Up",
                "Move the active worklane earlier in the sidebar",
                "reorder workspace lane",
                ACTION_MOVE_WORKLANE_UP,
            ),
            CommandPaletteItem::action(
                "Move Worklane Down",
                "Move the active worklane later in the sidebar",
                "reorder workspace lane",
                ACTION_MOVE_WORKLANE_DOWN,
            ),
            CommandPaletteItem::action(
                "Move Pane Left",
                "Move the focused pane one column left",
                "reorder terminal column",
                ACTION_MOVE_PANE_LEFT,
            ),
            CommandPaletteItem::action(
                "Move Pane Right",
                "Move the focused pane one column right",
                "reorder terminal column",
                ACTION_MOVE_PANE_RIGHT,
            ),
            CommandPaletteItem::action(
                "Move Pane Up",
                "Move the focused pane upward in its column",
                "reorder terminal split",
                ACTION_MOVE_PANE_UP,
            ),
            CommandPaletteItem::action(
                "Move Pane Down",
                "Move the focused pane downward in its column",
                "reorder terminal split",
                ACTION_MOVE_PANE_DOWN,
            ),
            CommandPaletteItem::action(
                "Focus Left Pane",
                "Focus the neighboring column to the left",
                "navigation terminal column",
                ACTION_FOCUS_PANE_LEFT,
            ),
            CommandPaletteItem::action(
                "Focus Right Pane",
                "Focus the neighboring column to the right",
                "navigation terminal column",
                ACTION_FOCUS_PANE_RIGHT,
            ),
            CommandPaletteItem::action(
                "Focus Up In Column",
                "Focus the pane above in the current column",
                "navigation terminal split",
                ACTION_FOCUS_PANE_UP,
            ),
            CommandPaletteItem::action(
                "Focus Down In Column",
                "Focus the pane below in the current column",
                "navigation terminal split",
                ACTION_FOCUS_PANE_DOWN,
            ),
            CommandPaletteItem::action(
                "Arrange Width: Full Width",
                "Make every column one viewport wide",
                "layout pane columns",
                ACTION_ARRANGE_WIDTH_FULL,
            ),
            CommandPaletteItem::action(
                "Arrange Width: Half Width",
                "Fit two equal columns in the viewport",
                "layout pane columns",
                ACTION_ARRANGE_WIDTH_HALF,
            ),
            CommandPaletteItem::action(
                "Arrange Width: Thirds",
                "Fit three equal columns in the viewport",
                "layout pane columns",
                ACTION_ARRANGE_WIDTH_THIRDS,
            ),
            CommandPaletteItem::action(
                "Arrange Width: Quarters",
                "Fit four equal columns in the viewport",
                "layout pane columns",
                ACTION_ARRANGE_WIDTH_QUARTERS,
            ),
            CommandPaletteItem::action(
                "Arrange Height: Full Height",
                "Place one pane in each column",
                "layout pane rows",
                ACTION_ARRANGE_HEIGHT_FULL,
            ),
            CommandPaletteItem::action(
                "Arrange Height: 2 Per Column",
                "Reflow panes two per column",
                "layout pane rows",
                ACTION_ARRANGE_HEIGHT_TWO,
            ),
            CommandPaletteItem::action(
                "Arrange Height: 3 Per Column",
                "Reflow panes three per column",
                "layout pane rows",
                ACTION_ARRANGE_HEIGHT_THREE,
            ),
            CommandPaletteItem::action(
                "Arrange Height: 4 Per Column",
                "Reflow panes four per column",
                "layout pane rows",
                ACTION_ARRANGE_HEIGHT_FOUR,
            ),
            CommandPaletteItem::action(
                "Arrange Width: Golden — Focus Wide",
                "Give the focused column the larger golden share",
                "layout pane golden ratio",
                ACTION_ARRANGE_GOLDEN_WIDE,
            ),
            CommandPaletteItem::action(
                "Arrange Width: Golden — Focus Narrow",
                "Give the focused column the smaller golden share",
                "layout pane golden ratio",
                ACTION_ARRANGE_GOLDEN_NARROW,
            ),
            CommandPaletteItem::action(
                "Arrange Height: Golden — Focus Tall",
                "Give the focused pane the larger golden share",
                "layout pane golden ratio",
                ACTION_ARRANGE_GOLDEN_TALL,
            ),
            CommandPaletteItem::action(
                "Arrange Height: Golden — Focus Short",
                "Give the focused pane the smaller golden share",
                "layout pane golden ratio",
                ACTION_ARRANGE_GOLDEN_SHORT,
            ),
            CommandPaletteItem::action(
                "Reset Pane Layout",
                "Restore default column widths and equal pane heights",
                "layout pane reset",
                ACTION_RESET_PANE_LAYOUT,
            ),
            CommandPaletteItem::action(
                "Cycle Worklane Color",
                "Choose the next worklane identity color",
                "appearance workspace lane",
                ACTION_CYCLE_WORKLANE_COLOR,
            ),
            CommandPaletteItem::action(
                "Find",
                "Search the focused terminal's real scrollback",
                "search pane terminal",
                ACTION_FIND,
            ),
            CommandPaletteItem::action(
                "Use Selection for Find",
                "Search for the focused terminal selection",
                "search pane selection terminal",
                ACTION_USE_SELECTION_FOR_FIND,
            ),
            CommandPaletteItem::action(
                "Find Next",
                "Select the next terminal search match",
                "search pane navigation",
                ACTION_FIND_NEXT,
            ),
            CommandPaletteItem::action(
                "Find Previous",
                "Select the previous terminal search match",
                "search pane navigation",
                ACTION_FIND_PREVIOUS,
            ),
        ]
    }

    fn perform_focused_binding_action(&self, action: &str, binding: &str) {
        let Some(pane_id) = self.state.focused_pane_id() else {
            eprintln!("zentty-linux: action={action} error=no-focused-pane");
            return;
        };
        let Some(surface) = self.surfaces.get(pane_id) else {
            eprintln!("zentty-linux: action={action} pane={pane_id} error=no-live-surface");
            return;
        };
        match surface.perform_binding_action(binding) {
            Ok(()) => eprintln!("zentty-linux: action={action} pane={pane_id} binding={binding:?}"),
            Err(error) => {
                eprintln!("zentty-linux: action={action} pane={pane_id} error={error}");
            }
        }
    }

    fn install_peek_scroll_navigation(shell: &Rc<RefCell<Self>>) {
        let controller = gtk::EventControllerScroll::new(
            gtk::EventControllerScrollFlags::BOTH_AXES | gtk::EventControllerScrollFlags::KINETIC,
        );
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        let gesture = Rc::new(RefCell::new(PeekScrollNavigation::default()));
        let beginning = Rc::clone(&gesture);
        controller.connect_scroll_begin(move |_| beginning.borrow_mut().reset());
        let ending = Rc::clone(&gesture);
        controller.connect_scroll_end(move |_| ending.borrow_mut().reset());
        let weak = Rc::downgrade(shell);
        controller.connect_scroll(move |controller, dx, dy| {
            let Some(shell) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if !matches!(shell.borrow().peek_phase, PeekPhase::Peeking { .. }) {
                gesture.borrow_mut().reset();
                return glib::Propagation::Proceed;
            }
            let unit = match controller.unit() {
                gdk::ScrollUnit::Wheel => PeekScrollUnit::Wheel,
                gdk::ScrollUnit::Surface => PeekScrollUnit::Surface,
                _ => return glib::Propagation::Stop,
            };
            if unit == PeekScrollUnit::Wheel {
                gesture.borrow_mut().reset();
            }
            match gesture.borrow_mut().handle(dx, dy, unit) {
                PeekScrollResult::Navigate(direction) => {
                    let direction = match direction {
                        PeekScrollDirection::Left => PeekSpatialDirection::Left,
                        PeekScrollDirection::Right => PeekSpatialDirection::Right,
                        PeekScrollDirection::Up => PeekSpatialDirection::Up,
                        PeekScrollDirection::Down => PeekSpatialDirection::Down,
                    };
                    Self::spatially_navigate_peek(&shell, direction);
                    glib::Propagation::Stop
                }
                PeekScrollResult::Consumed | PeekScrollResult::Unhandled => glib::Propagation::Stop,
            }
        });
        shell.borrow().peek_view.widget().add_controller(controller);
    }

    fn install_pane_creation_actions(shell: &Rc<RefCell<Self>>, group: &gio::SimpleActionGroup) {
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

        let add_pane = gio::SimpleAction::new(ACTION_ADD_PANE_RIGHT, None);
        let weak = Rc::downgrade(shell);
        add_pane.connect_activate(move |_, _| {
            let Some(shell) = weak.upgrade() else {
                return;
            };
            if let Err(error) = Self::add_focused_pane_right(&shell) {
                Self::report_action_error(&shell, ACTION_ADD_PANE_RIGHT, &error);
            }
        });
        group.add_action(&add_pane);

        let add_pane = gio::SimpleAction::new(ACTION_ADD_PANE_LEFT, None);
        let weak = Rc::downgrade(shell);
        add_pane.connect_activate(move |_, _| {
            let Some(shell) = weak.upgrade() else {
                return;
            };
            if let Err(error) = Self::add_focused_pane_left(&shell) {
                Self::report_action_error(&shell, ACTION_ADD_PANE_LEFT, &error);
            }
        });
        group.add_action(&add_pane);

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
    }

    fn install_edit_actions(shell: &Rc<RefCell<Self>>, group: &gio::SimpleActionGroup) {
        let string_pair = glib::VariantTy::new("(ss)").expect("static action type is valid");
        Self::install_worklane_edit_actions(shell, group, string_pair);
        Self::install_pane_rename_action(shell, group, string_pair);

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
            if shell.state.select_worklane_and_pane(&worklane_id, &pane_id) {
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

    fn install_pane_rename_action(
        shell: &Rc<RefCell<Self>>,
        group: &gio::SimpleActionGroup,
        string_pair: &glib::VariantTy,
    ) {
        let rename_pane = gio::SimpleAction::new(ACTION_RENAME_PANE, Some(string_pair));
        let weak = Rc::downgrade(shell);
        rename_pane.connect_activate(move |_, parameter| {
            let (Some(shell), Some((pane_id, title))) = (
                weak.upgrade(),
                parameter.and_then(glib::Variant::get::<(String, String)>),
            ) else {
                return;
            };
            let changed = {
                let mut shell_ref = shell.borrow_mut();
                let changed = shell_ref
                    .state
                    .set_pane_custom_title(&pane_id, Some(&title));
                if changed {
                    shell_ref.refresh_sidebar_metadata();
                }
                changed
            };
            if changed {
                eprintln!("zentty-linux: action=rename-pane id={pane_id} title={title:?}");
                Self::schedule_terminal_focus(&shell);
            }
        });
        group.add_action(&rename_pane);
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
            let changed = {
                let mut shell_ref = shell.borrow_mut();
                let changed = shell_ref
                    .state
                    .set_worklane_title(&worklane_id, Some(&title));
                if changed {
                    shell_ref.refresh_sidebar_metadata();
                }
                changed
            };
            if changed {
                eprintln!("zentty-linux: action=rename-worklane id={worklane_id} title={title:?}");
                Self::schedule_terminal_focus(&shell);
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

        Self::install_worklane_move_actions(shell, group, string_pair);
    }

    fn install_worklane_move_actions(
        shell: &Rc<RefCell<Self>>,
        group: &gio::SimpleActionGroup,
        string_pair: &glib::VariantTy,
    ) {
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

        let reorder_worklane = gio::SimpleAction::new(ACTION_REORDER_WORKLANE, Some(string_pair));
        let weak = Rc::downgrade(shell);
        reorder_worklane.connect_activate(move |_, parameter| {
            let (Some(shell), Some((worklane_id, placement))) = (
                weak.upgrade(),
                parameter.and_then(glib::Variant::get::<(String, String)>),
            ) else {
                return;
            };
            let Some((edge, target_id)) = placement.split_once(':') else {
                return;
            };
            let mut shell = shell.borrow_mut();
            let filtered = shell
                .state
                .worklanes()
                .iter()
                .filter(|worklane| worklane.id != worklane_id)
                .map(|worklane| worklane.id.clone())
                .collect::<Vec<_>>();
            let Some(target_index) = filtered.iter().position(|id| id == target_id) else {
                return;
            };
            let insertion_index = match edge {
                "before" => target_index,
                "after" => target_index + 1,
                _ => return,
            };
            if shell
                .state
                .reorder_worklane(&worklane_id, insertion_index)
            {
                eprintln!(
                    "zentty-linux: action=reorder-worklane id={worklane_id} insertion={insertion_index} order={} active={} pane={}",
                    shell
                        .state
                        .worklanes()
                        .iter()
                        .map(|worklane| worklane.id.as_str())
                        .collect::<Vec<_>>()
                        .join(","),
                    shell.state.active_worklane_id(),
                    shell.state.focused_pane_id().unwrap_or("none")
                );
                shell.render_sidebar();
                shell.focus_selected_surface();
            }
        });
        group.add_action(&reorder_worklane);
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

    fn schedule_terminal_focus(shell: &Rc<RefCell<Self>>) {
        let weak = Rc::downgrade(shell);
        glib::timeout_add_local_once(Duration::from_millis(50), move || {
            if let Some(shell) = weak.upgrade() {
                shell.borrow().present();
            }
        });
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
        let width = f64::from(PaneLayoutPolicy::visible_split_width(
            shell.borrow().pane_viewport_width(),
        ));
        Self::split_focused_pane(shell, ACTION_SPLIT_PANE_RIGHT, move |state, pane_id| {
            state.split_focused_pane_right_visibly(pane_id, width)
        })?;
        Self::scroll_panes_to_end(shell);
        Ok(())
    }

    fn add_focused_pane_right(shell: &Rc<RefCell<Self>>) -> Result<(), String> {
        let width = shell.borrow().focused_column_render_width();
        Self::split_focused_pane(shell, ACTION_ADD_PANE_RIGHT, move |state, pane_id| {
            state.add_pane_right_without_resizing(pane_id, f64::from(width))
        })?;
        Self::scroll_panes_to_end(shell);
        Ok(())
    }

    fn add_focused_pane_left(shell: &Rc<RefCell<Self>>) -> Result<(), String> {
        let width = shell.borrow().focused_column_render_width();
        Self::split_focused_pane(shell, ACTION_ADD_PANE_LEFT, move |state, pane_id| {
            state.insert_focused_pane_left(pane_id, f64::from(width))
        })?;
        shell.borrow().scroll_panes_to_focused();
        Ok(())
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

    fn scroll_panes_to_end(shell: &Rc<RefCell<Self>>) {
        let weak = Rc::downgrade(shell);
        glib::timeout_add_local_once(Duration::from_millis(50), move || {
            if let Some(shell) = weak.upgrade() {
                let adjustment = shell.borrow().pane_scroll.hadjustment();
                let maximum = (adjustment.upper() - adjustment.page_size()).max(0.0);
                adjustment.set_value(maximum);
                eprintln!(
                    "zentty-linux: pane-scroll value={:.0} maximum={maximum:.0}",
                    adjustment.value()
                );
            }
        });
    }

    fn scroll_panes_to_focused(&self) {
        let worklane = self.state.active_worklane();
        let widths = self.resolved_column_widths();
        let focused_index = worklane
            .columns
            .iter()
            .position(|column| column.id == worklane.focused_column_id)
            .unwrap_or(0);
        let target = widths
            .iter()
            .take(focused_index)
            .fold(0_i32, |total, width| {
                total.saturating_add(*width + PaneLayoutPolicy::INTER_PANE_SPACING)
            });
        let adjustment = self.pane_scroll.hadjustment();
        glib::timeout_add_local_once(Duration::from_millis(50), move || {
            let maximum = (adjustment.upper() - adjustment.page_size()).max(0.0);
            adjustment.set_value(f64::from(target).min(maximum));
            eprintln!(
                "zentty-linux: pane-scroll-focused value={:.0} maximum={maximum:.0}",
                adjustment.value()
            );
        });
    }

    fn activate_pane_control(shell: &Rc<RefCell<Self>>, pane_id: &str, action: PaneControlAction) {
        {
            let mut shell_ref = shell.borrow_mut();
            if shell_ref.shutting_down || !shell_ref.state.select_pane(pane_id) {
                return;
            }
            shell_ref.refresh_sidebar_metadata();
        }
        eprintln!(
            "zentty-linux: pane-control action={} target={pane_id}",
            action.id()
        );
        match action {
            PaneControlAction::SplitRight => {
                if let Err(error) = Self::split_focused_pane_right(shell) {
                    Self::report_action_error(shell, ACTION_SPLIT_PANE_RIGHT, &error);
                }
            }
            PaneControlAction::AddPaneRight => {
                if let Err(error) = Self::add_focused_pane_right(shell) {
                    Self::report_action_error(shell, ACTION_ADD_PANE_RIGHT, &error);
                }
            }
            PaneControlAction::NewPaneBelow => {
                if let Err(error) = Self::split_focused_pane_below(shell) {
                    Self::report_action_error(shell, ACTION_SPLIT_PANE_BELOW, &error);
                }
            }
            PaneControlAction::ClosePane => Self::close_pane(shell, pane_id),
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
            ClosePaneOutcome::CloseWindow => {
                if let Err(error) = shell_ref.remove_live_surface(pane_id) {
                    drop(shell_ref);
                    Self::report_action_error(shell, ACTION_CLOSE_PANE, &error);
                    return;
                }
                eprintln!("zentty-linux: action=close-pane pane={pane_id} close-window=true");
                shell_ref.main_loop.quit();
            }
            ClosePaneOutcome::NotFound => {}
        }
    }

    fn restore_closed_pane(shell: &Rc<RefCell<Self>>) -> Result<(), String> {
        let restored = {
            let mut shell = shell.borrow_mut();
            let pane_id = shell.take_pane_id();
            shell.state.restore_closed_pane(pane_id)
        };
        let Some(restored) = restored else {
            eprintln!("zentty-linux: action=restore-closed-pane available=false");
            return Ok(());
        };
        if let Some(prefill) = &restored.prefill_text {
            shell
                .borrow_mut()
                .pending_prefills
                .insert(restored.pane_id.clone(), prefill.clone());
        }
        if let Err(error) = Self::create_surface(shell, &restored.pane_id) {
            let mut shell = shell.borrow_mut();
            shell.pending_prefills.remove(&restored.pane_id);
            let _ = shell.state.close_pane_after_child_exit(&restored.pane_id);
            return Err(error);
        }
        let shell_ref = shell.borrow();
        eprintln!(
            "zentty-linux: action=restore-closed-pane pane={} worklane={} cwd={} prefill={}",
            restored.pane_id,
            restored.worklane_id,
            restored.working_directory.as_deref().unwrap_or("none"),
            restored.prefill_text.as_deref().unwrap_or("none")
        );
        shell_ref.render();
        shell_ref.focus_selected_surface();
        Ok(())
    }

    fn create_surface(shell: &Rc<RefCell<Self>>, pane_id: &str) -> Result<(), String> {
        let surface = {
            let shell = shell.borrow();
            shell
                .runtime
                .create_surface(&shell.surface_config(pane_id))
                .map_err(|error| error.to_string())?
        };

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
                if let Some(surface) = shell.surfaces.get(&ready_id) {
                    observe_ghostty_search_state(surface.widget(), &ready_id);
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
        focus_controller.connect_enter(move |controller| {
            let weak = weak.clone();
            let focus_id = focus_id.clone();
            let controller = controller.clone();
            glib::idle_add_local_once(move || {
                let Some(shell) = weak.upgrade() else {
                    return;
                };
                if shell.borrow().shutting_down {
                    return;
                }
                // Ghostty owns focusable descendants inside its embedding
                // widget. Widget::has_focus only describes the wrapper itself,
                // while EventControllerFocus::contains_focus covers the
                // controller widget and its descendants.
                if controller.contains_focus() {
                    let changed = shell.borrow_mut().state.select_pane(&focus_id);
                    if changed {
                        eprintln!("zentty-linux: focus-pane pane={focus_id}");
                        shell.borrow().refresh_sidebar_metadata();
                    }
                }
            });
        });
        surface.widget().add_controller(focus_controller.clone());

        let frame = Self::create_pane_frame(shell, pane_id, surface.widget());

        let mut shell = shell.borrow_mut();
        shell.live_children.set(shell.live_children.get() + 1);
        shell
            .focus_controllers
            .insert(pane_id.to_owned(), focus_controller);
        shell.pane_frames.insert(pane_id.to_owned(), frame);
        shell.surfaces.insert(pane_id.to_owned(), surface);
        Ok(())
    }

    fn surface_config(&self, pane_id: &str) -> SurfaceConfig {
        SurfaceConfig {
            command: self.command.clone(),
            title: zentty_core::PRODUCT_NAME.to_owned(),
            working_directory: self
                .state
                .pane(pane_id)
                .and_then(|pane| pane.working_directory.clone()),
        }
    }

    fn apply_pending_restore_prefill(shell: &Rc<RefCell<Self>>, pane_id: &str) {
        let prefill = shell.borrow_mut().pending_prefills.remove(pane_id);
        let Some(prefill) = prefill else {
            return;
        };
        let shell = shell.borrow();
        let Some(surface) = shell.surfaces.get(pane_id) else {
            return;
        };
        if let Err(error) = surface.send_text(&prefill) {
            eprintln!("zentty-linux: restore-prefill pane={pane_id} failed={error}");
        } else {
            eprintln!("zentty-linux: restore-prefill pane={pane_id} text={prefill}");
        }
    }

    fn create_pane_frame(
        shell: &Rc<RefCell<Self>>,
        pane_id: &str,
        terminal: &gtk::Widget,
    ) -> PaneFrame {
        let weak = Rc::downgrade(shell);
        let control_pane_id = pane_id.to_owned();
        PaneFrame::new(pane_id, terminal, move |action| {
            if let Some(shell) = weak.upgrade() {
                Self::activate_pane_control(&shell, &control_pane_id, action);
            }
        })
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
            let outcome = shell_ref.state.close_pane_after_child_exit(pane_id);
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
            let outcome = shell_ref.state.close_pane_after_child_exit(pane_id);
            if let Err(error) = shell_ref.remove_surface(pane_id) {
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
            if let Some(frame) = self.pane_frames.remove(pane_id) {
                if let Some(parent) = frame
                    .widget()
                    .parent()
                    .and_then(|parent| parent.downcast::<gtk::Box>().ok())
                {
                    parent.remove(frame.widget());
                }
                frame.detach_terminal();
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
        self.refresh_pane_presentation();

        let column_widths = self.resolved_column_widths();
        let single_column = column_widths.len() == 1;
        let content_width = column_widths
            .iter()
            .copied()
            .fold(0_i32, i32::saturating_add)
            .saturating_add(
                i32::try_from(column_widths.len().saturating_sub(1)).unwrap_or(i32::MAX),
            )
            .max(self.pane_viewport_width());
        self.pane_box.set_width_request(content_width);
        eprintln!(
            "zentty-linux: pane-layout viewport={} content={} columns={}",
            self.pane_viewport_width(),
            content_width,
            self.state
                .active_columns()
                .iter()
                .zip(&column_widths)
                .map(|(column, width)| format!("{}:{width}", column.id))
                .collect::<Vec<_>>()
                .join(",")
        );

        for (column, width) in self.state.active_columns().iter().zip(column_widths) {
            let column_box = gtk::Box::new(gtk::Orientation::Vertical, 1);
            column_box.set_homogeneous(false);
            column_box.set_width_request(width);
            column_box.set_hexpand(single_column);
            column_box.set_vexpand(true);
            for pane in &column.panes {
                if let Some(frame) = self.pane_frames.get(&pane.id) {
                    column_box.append(frame.widget());
                }
            }
            self.pane_box.append(&column_box);
        }
        self.apply_pane_height_requests(true);
        self.refresh_pane_layout_action_availability();
        eprintln!("zentty-linux: topology={}", self.topology_receipt());
        eprintln!("zentty-linux: geometry={}", self.geometry_receipt());
    }

    fn pane_viewport_width(&self) -> i32 {
        let allocated = self.pane_scroll.width();
        if allocated > 1 {
            return allocated;
        }
        self.window
            .default_width()
            .saturating_sub(SidebarWidthPreference::DEFAULT)
            .max(200)
    }

    fn pane_viewport_height(&self) -> i32 {
        let allocated = self.pane_box.height();
        if allocated > 1 {
            return allocated;
        }
        self.window.default_height().saturating_sub(52).max(200)
    }

    fn apply_pane_height_requests(&self, force: bool) {
        let viewport_height = self.pane_viewport_height();
        if !force && self.last_pane_viewport_height.get() == viewport_height {
            return;
        }
        self.last_pane_viewport_height.set(viewport_height);
        for column in self.state.active_columns() {
            let pane_count = column.panes.len();
            if pane_count == 1 {
                if let Some(frame) = self.pane_frames.get(&column.panes[0].id) {
                    frame.widget().set_height_request(-1);
                    frame.widget().set_vexpand(true);
                }
                continue;
            }
            let heights = model_heights_to_pixels(&column.pane_heights, viewport_height);
            for (pane, height) in column.panes.iter().zip(heights) {
                if let Some(frame) = self.pane_frames.get(&pane.id) {
                    frame.widget().set_height_request(height);
                    frame.widget().set_vexpand(false);
                }
            }
            eprintln!(
                "zentty-linux: pane-height-layout column={} viewport={viewport_height} panes={}",
                column.id,
                column
                    .panes
                    .iter()
                    .filter_map(|pane| self.pane_frames.get(&pane.id).map(|frame| {
                        format!("{}:{}", pane.id, frame.widget().height_request())
                    }))
                    .collect::<Vec<_>>()
                    .join(",")
            );
        }
    }

    fn refresh_pane_layout_action_availability(&self) {
        let columns = self.state.active_columns();
        let width_available = columns.len() >= 2;
        let focused_column = self.state.active_worklane().focused_column_id.as_str();
        let height_available = columns
            .iter()
            .find(|column| column.id == focused_column)
            .is_some_and(|column| column.panes.len() >= 2);
        for name in [ACTION_ARRANGE_GOLDEN_WIDE, ACTION_ARRANGE_GOLDEN_NARROW] {
            self.set_workspace_action_enabled(name, width_available);
        }
        for name in [ACTION_ARRANGE_GOLDEN_TALL, ACTION_ARRANGE_GOLDEN_SHORT] {
            self.set_workspace_action_enabled(name, height_available);
        }
    }

    fn set_workspace_action_enabled(&self, name: &str, enabled: bool) {
        let Some(group) = &self.workspace_actions else {
            return;
        };
        let Some(action) = group
            .lookup_action(name)
            .and_then(|action| action.downcast::<gio::SimpleAction>().ok())
        else {
            return;
        };
        action.set_enabled(enabled);
    }

    fn resolved_column_widths(&self) -> Vec<i32> {
        let columns = self.state.active_columns();
        let viewport_width = self.pane_viewport_width();
        let fallback = viewport_width
            .saturating_sub(i32::try_from(columns.len().saturating_sub(1)).unwrap_or(i32::MAX))
            .checked_div(i32::try_from(columns.len()).unwrap_or(i32::MAX).max(1))
            .unwrap_or(viewport_width)
            .max(1);
        columns
            .iter()
            .map(|column| {
                if columns.len() == 1 || column.width <= 1.0 {
                    fallback
                } else {
                    model_width_to_pixels(column.width)
                }
            })
            .collect()
    }

    fn focused_column_render_width(&self) -> i32 {
        let worklane = self.state.active_worklane();
        let widths = self.resolved_column_widths();
        worklane
            .columns
            .iter()
            .position(|column| column.id == worklane.focused_column_id)
            .and_then(|index| widths.get(index).copied())
            .unwrap_or_else(|| self.pane_viewport_width())
    }

    fn render_sidebar(&self) {
        let summaries = self.state.sidebar_summaries();
        sidebar::render(&self.sidebar, &self.window, &summaries);
        self.chrome.render(
            &summaries,
            self.state.can_navigate_back(),
            self.state.can_navigate_forward(),
        );
        self.schedule_active_worklane_reveal();
    }

    fn refresh_sidebar_metadata(&self) {
        let summaries = self.state.sidebar_summaries();
        if !sidebar::update_metadata(&self.sidebar, &summaries) {
            sidebar::render(&self.sidebar, &self.window, &summaries);
        }
        self.chrome.render(
            &summaries,
            self.state.can_navigate_back(),
            self.state.can_navigate_forward(),
        );
        self.schedule_active_worklane_reveal();
        self.refresh_pane_presentation();
        self.refresh_pane_layout_action_availability();
    }

    fn schedule_active_worklane_reveal(&self) {
        let generation = self.sidebar_reveal_generation.get().wrapping_add(1);
        self.sidebar_reveal_generation.set(generation);
        let tracker = Rc::clone(&self.sidebar_reveal_generation);
        let sidebar = self.sidebar.clone();
        let scroll = self.sidebar_scroll.clone();
        let worklane_id = self.state.active_worklane_id().to_owned();
        glib::idle_add_local_once(move || {
            if tracker.get() != generation {
                return;
            }
            let tracker_again = Rc::clone(&tracker);
            glib::idle_add_local_once(move || {
                if tracker_again.get() != generation {
                    return;
                }
                let Some(card) = sidebar::worklane_card(&sidebar, &worklane_id) else {
                    return;
                };
                let Some(bounds) = card.compute_bounds(&sidebar) else {
                    return;
                };
                let adjustment = scroll.vadjustment();
                let top = adjustment.value();
                let card_top = f64::from(bounds.y());
                if let Some((card_top, card_bottom)) = sidebar::reveal_range(
                    top,
                    adjustment.page_size(),
                    card_top,
                    f64::from(bounds.height()),
                ) {
                    adjustment.clamp_page(card_top, card_bottom);
                    eprintln!(
                        "zentty-linux: sidebar-reveal worklane={} value={}",
                        worklane_id,
                        adjustment.value()
                    );
                }
                eprintln!(
                    "zentty-linux: sidebar-active-visible worklane={} value={}",
                    worklane_id,
                    adjustment.value()
                );
            });
        });
    }

    fn navigate_history(&mut self, backward: bool) {
        let previous_worklane = self.state.active_worklane_id().to_owned();
        let changed = if backward {
            self.state.navigate_back()
        } else {
            self.state.navigate_forward()
        };
        if !changed {
            // Navigation purges closed/stale references while searching. Even
            // without a target, refresh button sensitivity to reflect the
            // resulting history stacks.
            self.refresh_sidebar_metadata();
            return;
        }
        eprintln!(
            "zentty-linux: action={} worklane={} pane={}",
            if backward {
                ACTION_NAVIGATE_BACK
            } else {
                ACTION_NAVIGATE_FORWARD
            },
            self.state.active_worklane_id(),
            self.state.focused_pane_id().unwrap_or("none")
        );
        if previous_worklane == self.state.active_worklane_id() {
            self.refresh_sidebar_metadata();
        } else {
            self.render();
        }
        self.focus_selected_surface();
    }

    fn select_adjacent_pane(&mut self, forward: bool) {
        let previous_worklane = self.state.active_worklane_id().to_owned();
        if !self.state.select_adjacent_pane(forward) {
            return;
        }
        eprintln!(
            "zentty-linux: action={} worklane={} pane={}",
            if forward {
                ACTION_NEXT_PANE
            } else {
                ACTION_PREVIOUS_PANE
            },
            self.state.active_worklane_id(),
            self.state.focused_pane_id().unwrap_or("none")
        );
        if previous_worklane == self.state.active_worklane_id() {
            self.refresh_sidebar_metadata();
        } else {
            self.render();
        }
        self.focus_selected_surface();
    }

    fn select_adjacent_worklane(&mut self, forward: bool) {
        if !self.state.select_adjacent_worklane(forward) {
            return;
        }
        eprintln!(
            "zentty-linux: action={} worklane={} pane={}",
            if forward {
                ACTION_NEXT_WORKLANE
            } else {
                ACTION_PREVIOUS_WORKLANE
            },
            self.state.active_worklane_id(),
            self.state.focused_pane_id().unwrap_or("none")
        );
        self.render();
        self.focus_selected_surface();
    }

    fn pane_references_in_sidebar_order(&self) -> Vec<PaneReference> {
        self.state
            .worklanes()
            .iter()
            .flat_map(|worklane| {
                worklane.columns.iter().flat_map(|column| {
                    column
                        .panes
                        .iter()
                        .map(|pane| PaneReference::new(&worklane.id, &pane.id))
                })
            })
            .collect()
    }

    fn current_pane_reference(&self) -> Option<PaneReference> {
        self.state
            .focused_pane_id()
            .map(|pane_id| PaneReference::new(self.state.active_worklane_id(), pane_id))
    }

    fn select_pane_reference(&mut self, target: &PaneReference, focus_terminal: bool) {
        let previous_worklane = self.state.active_worklane_id().to_owned();
        if !self
            .state
            .select_worklane_and_pane(&target.worklane_id, &target.pane_id)
        {
            return;
        }
        if previous_worklane == self.state.active_worklane_id() {
            self.refresh_sidebar_metadata();
        } else {
            self.render();
        }
        if focus_terminal {
            self.focus_selected_surface();
        }
    }

    fn peek_previews(&self) -> Vec<PanePreview> {
        self.state
            .worklanes()
            .iter()
            .enumerate()
            .flat_map(|(worklane_index, worklane)| {
                let worklane_title = worklane
                    .title
                    .clone()
                    .unwrap_or_else(|| format!("Worklane {}", worklane_index + 1));
                worklane.columns.iter().flat_map(move |column| {
                    let worklane_title = worklane_title.clone();
                    column.panes.iter().filter_map(move |pane| {
                        let terminal = self.surfaces.get(&pane.id)?.widget().clone();
                        Some(PanePreview {
                            reference: PaneReference::new(&worklane.id, &pane.id),
                            worklane_title: worklane_title.clone(),
                            pane_title: pane
                                .custom_title
                                .clone()
                                .unwrap_or_else(|| pane.live_title.clone()),
                            terminal,
                        })
                    })
                })
            })
            .collect()
    }

    fn refresh_pane_presentation(&self) {
        let focused_pane_id = self.state.focused_pane_id();
        let worklane_color = self.state.active_worklane().color;
        for pane_id in self.state.active_pane_ids() {
            if let Some(frame) = self.pane_frames.get(pane_id) {
                frame.set_presentation(PanePresentation {
                    focused: Some(pane_id) == focused_pane_id,
                    worklane_color,
                });
            }
        }
        self.refresh_right_insertion_behavior();
    }

    fn refresh_right_insertion_behavior(&self) {
        for pane_id in self.state.active_pane_ids() {
            if let Some(frame) = self.pane_frames.get(pane_id) {
                // Linux does not yet provide Zentty's horizontal gesture,
                // Worklane Peek, and recent-pane management. Keep the
                // pane-local primary action visible until that navigation
                // ecosystem makes full-width offscreen panes discoverable.
                frame.set_right_behavior(PRIMARY_RIGHT_BEHAVIOR);
            }
        }
    }

    fn focus_selected_surface(&self) {
        if let Some(pane_id) = self.state.focused_pane_id()
            && let Some(surface) = self.surfaces.get(pane_id)
        {
            gtk::prelude::GtkWindowExt::set_focus(&self.window, Some(surface.widget()));
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

fn observe_ghostty_search_state(root: &gtk::Widget, pane_id: &str) {
    let Some(overlay) = find_ghostty_search_overlay(root) else {
        eprintln!("zentty-linux: search-overlay pane={pane_id} error=not-found");
        return;
    };
    log_ghostty_search_state(&overlay, pane_id);
    let pane_id = pane_id.to_owned();
    overlay.connect_notify_local(None, move |overlay, property| {
        if matches!(
            property.name(),
            "active"
                | "has-search-total"
                | "search-total"
                | "has-search-selected"
                | "search-selected"
                | "halign-target"
                | "valign-target"
        ) {
            log_ghostty_search_state(overlay, &pane_id);
        }
    });
}

fn find_ghostty_search_overlay(root: &gtk::Widget) -> Option<gtk::Widget> {
    let mut child = root.first_child();
    while let Some(widget) = child {
        if widget.type_().name() == "GhosttySearchOverlay" {
            return Some(widget);
        }
        if let Some(found) = find_ghostty_search_overlay(&widget) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}

fn find_search_entry(root: &gtk::Widget) -> Option<gtk::SearchEntry> {
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Ok(entry) = widget.clone().downcast::<gtk::SearchEntry>() {
            return Some(entry);
        }
        if let Some(entry) = find_search_entry(&widget) {
            return Some(entry);
        }
        child = widget.next_sibling();
    }
    None
}

fn log_ghostty_search_state(overlay: &gtk::Widget, pane_id: &str) {
    let active = overlay.property::<bool>("active");
    let total = overlay
        .property::<bool>("has-search-total")
        .then(|| overlay.property::<u64>("search-total"));
    let selected = overlay
        .property::<bool>("has-search-selected")
        .then(|| overlay.property::<u64>("search-selected"));
    let horizontal = overlay.property::<gtk::Align>("halign-target");
    let vertical = overlay.property::<gtk::Align>("valign-target");
    eprintln!(
        "zentty-linux: search-state pane={pane_id} active={active} total={total:?} selected={selected:?} halign={horizontal:?} valign={vertical:?}"
    );
}

fn require_focused_search_state(
    shell: &Rc<RefCell<ApplicationShell>>,
    expected_active: bool,
    expected_total: Option<u64>,
    expected_selection_presence: Option<bool>,
) -> Result<(), glib::BoolError> {
    let shell = shell.borrow();
    let pane_id = shell
        .state
        .focused_pane_id()
        .ok_or_else(|| glib::bool_error!("no focused pane"))?;
    let surface = shell
        .surfaces
        .get(pane_id)
        .ok_or_else(|| glib::bool_error!("focused pane has no live surface"))?;
    let overlay = find_ghostty_search_overlay(surface.widget())
        .ok_or_else(|| glib::bool_error!("Ghostty search overlay is missing"))?;
    let active = overlay.property::<bool>("active");
    let total = overlay
        .property::<bool>("has-search-total")
        .then(|| overlay.property::<u64>("search-total"));
    let has_selection = overlay.property::<bool>("has-search-selected");
    if active != expected_active
        || total != expected_total
        || expected_selection_presence.is_some_and(|expected| expected != has_selection)
    {
        return Err(glib::bool_error!(
            "unexpected search state: active={active} total={total:?} selected={has_selection}"
        ));
    }
    eprintln!(
        "zentty-linux: pane-search-action-scenario verified active={active} total={total:?} selected={has_selection}"
    );
    Ok(())
}

fn require_invalid_binding_action_rejected(
    shell: &Rc<RefCell<ApplicationShell>>,
) -> Result<(), glib::BoolError> {
    let shell = shell.borrow();
    let pane_id = shell
        .state
        .focused_pane_id()
        .ok_or_else(|| glib::bool_error!("no focused pane"))?;
    let surface = shell
        .surfaces
        .get(pane_id)
        .ok_or_else(|| glib::bool_error!("focused pane has no live surface"))?;
    if surface
        .perform_binding_action("zentty_invalid_binding_action")
        .is_ok()
    {
        return Err(glib::bool_error!(
            "Ghostty accepted an invalid embedding binding action"
        ));
    }
    eprintln!("zentty-linux: pane-search-action-scenario invalid-binding=rejected");
    Ok(())
}

fn require_pane_layout_scenario_state(
    shell: &Rc<RefCell<ApplicationShell>>,
) -> Result<(), glib::BoolError> {
    let shell = shell.borrow();
    let columns = shell.state.active_columns();
    let pane_ids = shell.state.active_pane_ids();
    if pane_ids != ["pane-2", "pane-3", "pane-4", "pane-1"]
        || columns.len() != 4
        || columns.iter().any(|column| column.panes.len() != 1)
        || shell.state.focused_pane_id() != Some("pane-4")
        || columns[2].width.partial_cmp(&columns[3].width) != Some(std::cmp::Ordering::Less)
    {
        return Err(glib::bool_error!(
            "unexpected pane-layout scenario: panes={pane_ids:?} columns={} focus={:?} widths={:?}",
            columns.len(),
            shell.state.focused_pane_id(),
            columns
                .iter()
                .map(|column| column.width)
                .collect::<Vec<_>>()
        ));
    }
    if shell.surfaces.len() != 4
        || pane_ids
            .iter()
            .any(|pane_id| !shell.surfaces.contains_key(*pane_id))
    {
        return Err(glib::bool_error!(
            "pane-layout scenario detached a real Ghostty surface"
        ));
    }
    eprintln!(
        "zentty-linux: pane-layout-action-scenario verified panes={pane_ids:?} columns={} focus=pane-4 golden=narrow",
        columns.len()
    );
    Ok(())
}

fn is_restore_closed_pane_shortcut(key: gdk::Key, modifiers: gdk::ModifierType) -> bool {
    key == gdk::Key::t
        && modifiers.contains(gdk::ModifierType::CONTROL_MASK)
        && modifiers.contains(gdk::ModifierType::SHIFT_MASK)
        && !modifiers.intersects(gdk::ModifierType::ALT_MASK | gdk::ModifierType::SUPER_MASK)
}

fn require_closed_pane_restore_state(
    shell: &Rc<RefCell<ApplicationShell>>,
) -> Result<(), glib::BoolError> {
    let shell = shell.borrow();
    let focused = shell
        .state
        .focused_pane_id()
        .ok_or_else(|| glib::bool_error!("restored pane is not focused"))?;
    let pane = shell
        .state
        .pane(focused)
        .ok_or_else(|| glib::bool_error!("restored pane model is missing"))?;
    if focused == "pane-agent"
        || pane.working_directory.as_deref().is_none_or(str::is_empty)
        || shell.state.active_pane_ids().len() != 2
        || shell.surfaces.len() != 2
        || shell.live_children.get() != 2
        || shell.pending_prefills.contains_key(focused)
    {
        return Err(glib::bool_error!(
            "closed-pane restore did not replace one model/surface/child and consume its prefill"
        ));
    }
    eprintln!(
        "zentty-linux: closed-pane-restore-scenario verified pane={focused} cwd={} surfaces={} children={}",
        pane.working_directory.as_deref().unwrap_or("none"),
        shell.surfaces.len(),
        shell.live_children.get()
    );
    Ok(())
}

fn require_golden_action_availability(
    shell: &Rc<RefCell<ApplicationShell>>,
    width_expected: bool,
    height_expected: bool,
) -> Result<(), glib::BoolError> {
    let shell = shell.borrow();
    let group = shell
        .workspace_actions
        .as_ref()
        .ok_or_else(|| glib::bool_error!("workspace action group is missing"))?;
    for (name, expected) in [
        (ACTION_ARRANGE_GOLDEN_WIDE, width_expected),
        (ACTION_ARRANGE_GOLDEN_NARROW, width_expected),
        (ACTION_ARRANGE_GOLDEN_TALL, height_expected),
        (ACTION_ARRANGE_GOLDEN_SHORT, height_expected),
    ] {
        let action = group
            .lookup_action(name)
            .ok_or_else(|| glib::bool_error!("layout action is missing: {name}"))?;
        if action.is_enabled() != expected {
            return Err(glib::bool_error!(
                "layout action availability mismatch: {name} actual={} expected={expected}",
                action.is_enabled()
            ));
        }
    }
    eprintln!(
        "zentty-linux: pane-layout-action-scenario golden-availability width={width_expected} height={height_expected}"
    );
    Ok(())
}

fn require_rendered_golden_height(
    shell: &Rc<RefCell<ApplicationShell>>,
    focused_pane_id: &str,
    neighbor_pane_id: &str,
    focus_tall: bool,
) -> Result<(), glib::BoolError> {
    let shell = shell.borrow();
    let focused = shell
        .pane_frames
        .get(focused_pane_id)
        .ok_or_else(|| glib::bool_error!("missing focused pane frame"))?;
    let neighbor = shell
        .pane_frames
        .get(neighbor_pane_id)
        .ok_or_else(|| glib::bool_error!("missing neighboring pane frame"))?;
    let focused_height = focused.widget().height();
    let neighbor_height = neighbor.widget().height();
    let actual_ratio = f64::from(focused_height) / f64::from(focused_height + neighbor_height);
    let golden_major = (1.0 + 5.0_f64.sqrt()) / (3.0 + 5.0_f64.sqrt());
    let expected_ratio = if focus_tall {
        golden_major
    } else {
        1.0 - golden_major
    };
    let ordered = if focus_tall {
        focused_height > neighbor_height
    } else {
        focused_height < neighbor_height
    };
    if focused_height <= 0
        || neighbor_height <= 0
        || !ordered
        || (actual_ratio - expected_ratio).abs() > 0.02
    {
        return Err(glib::bool_error!(
            "rendered golden height missing: focus={focused_pane_id}:{focused_height} neighbor={neighbor_pane_id}:{neighbor_height} ratio={actual_ratio:.3} expected={expected_ratio:.3} tall={focus_tall}"
        ));
    }
    eprintln!(
        "zentty-linux: pane-layout-action-scenario rendered-golden-height focus={focused_pane_id}:{focused_height} neighbor={neighbor_pane_id}:{neighbor_height} ratio={actual_ratio:.3} tall={focus_tall}"
    );
    Ok(())
}

fn require_rendered_golden_width(
    shell: &Rc<RefCell<ApplicationShell>>,
    focused_pane_id: &str,
    neighbor_pane_id: &str,
    focus_wide: bool,
) -> Result<(), glib::BoolError> {
    let shell = shell.borrow();
    let focused = shell
        .pane_frames
        .get(focused_pane_id)
        .ok_or_else(|| glib::bool_error!("missing focused pane frame"))?;
    let neighbor = shell
        .pane_frames
        .get(neighbor_pane_id)
        .ok_or_else(|| glib::bool_error!("missing neighboring pane frame"))?;
    let focused_width = focused.widget().width();
    let neighbor_width = neighbor.widget().width();
    let actual_ratio = f64::from(focused_width) / f64::from(focused_width + neighbor_width);
    let golden_major = (1.0 + 5.0_f64.sqrt()) / (3.0 + 5.0_f64.sqrt());
    let expected_ratio = if focus_wide {
        golden_major
    } else {
        1.0 - golden_major
    };
    let ordered = if focus_wide {
        focused_width > neighbor_width
    } else {
        focused_width < neighbor_width
    };
    if focused_width <= 0
        || neighbor_width <= 0
        || !ordered
        || (actual_ratio - expected_ratio).abs() > 0.02
    {
        return Err(glib::bool_error!(
            "rendered golden width missing: focus={focused_pane_id}:{focused_width} neighbor={neighbor_pane_id}:{neighbor_width} ratio={actual_ratio:.3} expected={expected_ratio:.3} wide={focus_wide}"
        ));
    }
    eprintln!(
        "zentty-linux: pane-layout-action-scenario rendered-golden-width focus={focused_pane_id}:{focused_width} neighbor={neighbor_pane_id}:{neighbor_width} ratio={actual_ratio:.3} wide={focus_wide}"
    );
    Ok(())
}

fn install_sidebar_width_tracking(
    body: &gtk::Paned,
    sidebar: &gtk::ScrolledWindow,
    preferred_width: Rc<Cell<i32>>,
    adjusting_width: Rc<Cell<bool>>,
) {
    let sidebar = sidebar.clone();
    body.connect_position_notify(move |body| {
        if adjusting_width.get() || body.width() <= 0 {
            return;
        }
        let position = body.position();
        let clamped = SidebarWidthPreference::clamped(position, body.width());
        preferred_width.set(clamped);
        sidebar.set_width_request(clamped);
        if position != clamped {
            adjusting_width.set(true);
            body.set_position(clamped);
            adjusting_width.set(false);
        }
        eprintln!("zentty-linux: sidebar-preferred-width={clamped}");
    });
}

fn build_shell_widgets() -> ShellWidgets {
    let window = gtk::Window::new();
    window.set_title(Some(zentty_core::PRODUCT_NAME));
    window.set_default_size(1000, 700);
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
    pane_box.set_hexpand(true);
    pane_box.set_vexpand(true);
    let pane_scroll = gtk::ScrolledWindow::new();
    pane_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
    pane_scroll.set_hexpand(true);
    pane_scroll.set_vexpand(true);
    pane_scroll.set_child(Some(&pane_box));
    content.append(&pane_scroll);
    let sidebar_reservation = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sidebar_reservation.set_width_request(SidebarWidthPreference::DEFAULT);
    body.set_start_child(Some(&sidebar_reservation));
    body.set_end_child(Some(&content));
    let chrome = WindowChrome::new();
    let (overlay, peek_view, command_palette, sidebar_hover_rail) =
        build_root(&chrome, &body, &sidebar_scroll);
    window.set_child(Some(&overlay));
    ShellWidgets {
        window,
        chrome,
        body,
        sidebar,
        sidebar_scroll,
        sidebar_reservation,
        sidebar_hover_rail,
        pane_scroll,
        pane_box,
        peek_view,
        command_palette,
    }
}

fn build_root(
    chrome: &WindowChrome,
    body: &gtk::Paned,
    sidebar: &gtk::ScrolledWindow,
) -> (gtk::Overlay, WorklanePeekView, CommandPaletteView, gtk::Box) {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(chrome.widget());
    root.append(body);
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&root));
    let hover_rail = gtk::Box::new(gtk::Orientation::Vertical, 0);
    hover_rail.set_width_request(8);
    hover_rail.set_halign(gtk::Align::Start);
    hover_rail.set_valign(gtk::Align::Fill);
    hover_rail.set_margin_top(38);
    hover_rail.set_visible(false);
    overlay.add_overlay(&hover_rail);
    sidebar.set_width_request(SidebarWidthPreference::DEFAULT);
    sidebar.set_halign(gtk::Align::Start);
    sidebar.set_valign(gtk::Align::Fill);
    sidebar.set_margin_top(38);
    overlay.add_overlay(sidebar);
    let peek_view = WorklanePeekView::new();
    overlay.add_overlay(peek_view.widget());
    let command_palette = CommandPaletteView::new();
    overlay.add_overlay(command_palette.widget());
    (overlay, peek_view, command_palette, hover_rail)
}

fn workspace_pane_ids(state: &WorkspaceState) -> Vec<String> {
    state
        .worklanes()
        .iter()
        .flat_map(|worklane| &worklane.columns)
        .flat_map(|column| &column.panes)
        .map(|pane| pane.id.clone())
        .collect()
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

fn model_width_to_pixels(width: f64) -> i32 {
    let rounded = width.round().clamp(1.0, f64::from(i32::MAX));
    // The value is finite and clamped to the complete positive i32 range.
    #[allow(clippy::cast_possible_truncation)]
    {
        rounded as i32
    }
}

fn model_heights_to_pixels(weights: &[f64], viewport_height: i32) -> Vec<i32> {
    if weights.is_empty() {
        return Vec::new();
    }
    let total_weight = weights.iter().sum::<f64>();
    let valid = total_weight.is_finite()
        && total_weight > 0.0
        && weights
            .iter()
            .all(|weight| weight.is_finite() && *weight > 0.0);
    let resolved_total = if valid {
        total_weight
    } else {
        small_count_as_f64(weights.len())
    };
    let spacing = i32::try_from(weights.len().saturating_sub(1)).unwrap_or(i32::MAX);
    let usable_height = viewport_height.saturating_sub(spacing).max(1);
    let mut assigned = 0_i32;
    weights
        .iter()
        .enumerate()
        .map(|(index, weight)| {
            let remaining = usable_height.saturating_sub(assigned).max(1);
            let height = if index + 1 == weights.len() {
                remaining
            } else {
                let resolved_weight = if valid { *weight } else { 1.0 };
                let share = f64::from(usable_height) * (resolved_weight / resolved_total);
                model_width_to_pixels(share).min(remaining)
            };
            assigned = assigned.saturating_add(height);
            height
        })
        .collect()
}

fn small_count_as_f64(count: usize) -> f64 {
    f64::from(u32::try_from(count).unwrap_or(u32::MAX))
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

#[cfg(test)]
mod allocation_tests {
    use super::model_heights_to_pixels;

    #[test]
    fn pane_height_pixels_preserve_ratios_spacing_and_invalid_weight_fallback() {
        let golden = model_heights_to_pixels(&[0.618_033_988_75, 0.381_966_011_25], 648);
        let golden_content = golden.iter().sum::<i32>();
        assert_eq!(golden_content + 1, 648);
        assert!((f64::from(golden[0]) / f64::from(golden_content) - 0.618).abs() < 0.002);

        let equal = model_heights_to_pixels(&[1.0, 1.0, 1.0], 648);
        assert_eq!(equal.iter().sum::<i32>() + 2, 648);
        assert!(equal.windows(2).all(|pair| (pair[0] - pair[1]).abs() <= 1));

        assert_eq!(model_heights_to_pixels(&[0.0, f64::NAN], 101), [50, 50]);
        assert!(model_heights_to_pixels(&[], 648).is_empty());
    }
}
