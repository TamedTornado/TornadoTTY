use std::cell::RefCell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use gtk::{gio, prelude::ActionMapExt};
use zentty_core::{WorklaneColor, WorkspaceState};

use super::ApplicationShell;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ParameterSchema {
    None,
    String,
    StringPair,
}

impl ParameterSchema {
    pub(super) fn accepts(self, parameter: Option<&glib::Variant>) -> bool {
        match self {
            Self::None => parameter.is_none(),
            Self::String => parameter.is_some_and(|value| value.str().is_some()),
            Self::StringPair => {
                parameter.is_some_and(|value| value.get::<(String, String)>().is_some())
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Availability {
    Always,
    MultipleColumns,
    MultiplePanesInFocusedColumn,
    MultipleWorkspacePanes,
}

impl Availability {
    pub(super) const fn enabled(
        self,
        columns: usize,
        focused_column_panes: usize,
        workspace_panes: usize,
    ) -> bool {
        match self {
            Self::Always => true,
            Self::MultipleColumns => columns >= 2,
            Self::MultiplePanesInFocusedColumn => focused_column_panes >= 2,
            Self::MultipleWorkspacePanes => workspace_panes >= 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ActionSpec {
    pub(super) name: &'static str,
    pub(super) parameter: ParameterSchema,
    pub(super) availability: Availability,
}

macro_rules! action {
    ($constant:ident, $name:literal, $parameter:ident) => {
        ActionSpec {
            name: $name,
            parameter: ParameterSchema::$parameter,
            availability: Availability::Always,
        }
    };
    ($constant:ident, $name:literal, $parameter:ident, $availability:ident) => {
        ActionSpec {
            name: $name,
            parameter: ParameterSchema::$parameter,
            availability: Availability::$availability,
        }
    };
}

pub(super) const ACTION_TOGGLE_SIDEBAR: &str = "toggle-sidebar";
pub(super) const ACTION_NEW_WINDOW: &str = "new-window";
pub(super) const ACTION_CLOSE_WINDOW: &str = "close-window";
pub(super) const ACTION_NEW_WORKLANE: &str = "new-worklane";
pub(super) const ACTION_SELECT_WORKLANE: &str = "select-worklane";
pub(super) const ACTION_SPLIT_PANE_RIGHT: &str = "split-pane-right";
pub(super) const ACTION_ADD_PANE_RIGHT: &str = "add-pane-right";
pub(super) const ACTION_ADD_PANE_LEFT: &str = "add-pane-left";
pub(super) const ACTION_SPLIT_PANE_BELOW: &str = "split-pane-below";
pub(super) const ACTION_CLOSE_PANE: &str = "close-pane";
pub(super) const ACTION_RENAME_WORKLANE: &str = "rename-worklane";
pub(super) const ACTION_RENAME_PANE: &str = "rename-pane";
pub(super) const ACTION_CYCLE_WORKLANE_COLOR: &str = "cycle-worklane-color";
pub(super) const ACTION_SET_WORKLANE_COLOR: &str = "set-worklane-color";
pub(super) const ACTION_CLOSE_WORKLANE: &str = "close-worklane";
pub(super) const ACTION_CLOSE_ACTIVE_WORKLANE: &str = "close-active-worklane";
pub(super) const ACTION_MOVE_WORKLANE: &str = "move-worklane";
pub(super) const ACTION_REORDER_WORKLANE: &str = "reorder-worklane";
pub(super) const ACTION_MOVE_WORKLANE_UP: &str = "move-worklane-up";
pub(super) const ACTION_MOVE_WORKLANE_DOWN: &str = "move-worklane-down";
pub(super) const ACTION_MOVE_PANE_LEFT: &str = "move-pane-left";
pub(super) const ACTION_MOVE_PANE_RIGHT: &str = "move-pane-right";
pub(super) const ACTION_MOVE_PANE_UP: &str = "move-pane-up";
pub(super) const ACTION_MOVE_PANE_DOWN: &str = "move-pane-down";
pub(super) const ACTION_MOVE_PANE_TO_WORKLANE: &str = "move-pane-to-worklane";
pub(super) const ACTION_MOVE_PANE_TO_NEW_WINDOW: &str = "move-pane-to-new-window";
pub(super) const ACTION_SELECT_PANE: &str = "select-pane";
pub(super) const ACTION_NAVIGATE_BACK: &str = "navigate-back";
pub(super) const ACTION_NAVIGATE_FORWARD: &str = "navigate-forward";
pub(super) const ACTION_NEXT_PANE: &str = "next-pane";
pub(super) const ACTION_PREVIOUS_PANE: &str = "previous-pane";
pub(super) const ACTION_NEXT_WORKLANE: &str = "next-worklane";
pub(super) const ACTION_PREVIOUS_WORKLANE: &str = "previous-worklane";
pub(super) const ACTION_DISMISS_COMMAND_PALETTE: &str = "dismiss-command-palette";
pub(super) const ACTION_FIND: &str = "find";
pub(super) const ACTION_USE_SELECTION_FOR_FIND: &str = "use-selection-for-find";
pub(super) const ACTION_FIND_NEXT: &str = "find-next";
pub(super) const ACTION_FIND_PREVIOUS: &str = "find-previous";
pub(super) const ACTION_CLEAN_COPY: &str = "clean-copy";
pub(super) const ACTION_COPY: &str = "copy";
pub(super) const ACTION_COPY_RAW: &str = "copy-raw";
pub(super) const ACTION_COPY_AS_MARKDOWN: &str = "copy-as-markdown";
pub(super) const ACTION_SELECT_ALL: &str = "select-all";
pub(super) const ACTION_GLOBAL_FIND: &str = "global-find";
pub(super) const ACTION_CLEAR_GLOBAL_FIND: &str = "clear-global-find";
pub(super) const ACTION_GLOBAL_FIND_NEXT: &str = "global-find-next";
pub(super) const ACTION_GLOBAL_FIND_PREVIOUS: &str = "global-find-previous";
pub(super) const ACTION_FOCUS_PANE_LEFT: &str = "focus-pane-left";
pub(super) const ACTION_FOCUS_PANE_RIGHT: &str = "focus-pane-right";
pub(super) const ACTION_FOCUS_PANE_UP: &str = "focus-pane-up";
pub(super) const ACTION_FOCUS_PANE_DOWN: &str = "focus-pane-down";
pub(super) const ACTION_RESIZE_PANE_LEFT: &str = "resize-pane-left";
pub(super) const ACTION_RESIZE_PANE_RIGHT: &str = "resize-pane-right";
pub(super) const ACTION_RESIZE_PANE_UP: &str = "resize-pane-up";
pub(super) const ACTION_RESIZE_PANE_DOWN: &str = "resize-pane-down";
pub(super) const ACTION_ARRANGE_WIDTH_FULL: &str = "arrange-width-full";
pub(super) const ACTION_ARRANGE_WIDTH_HALF: &str = "arrange-width-half";
pub(super) const ACTION_ARRANGE_WIDTH_THIRDS: &str = "arrange-width-thirds";
pub(super) const ACTION_ARRANGE_WIDTH_QUARTERS: &str = "arrange-width-quarters";
pub(super) const ACTION_ARRANGE_HEIGHT_FULL: &str = "arrange-height-full";
pub(super) const ACTION_ARRANGE_HEIGHT_TWO: &str = "arrange-height-two";
pub(super) const ACTION_ARRANGE_HEIGHT_THREE: &str = "arrange-height-three";
pub(super) const ACTION_ARRANGE_HEIGHT_FOUR: &str = "arrange-height-four";
pub(super) const ACTION_ARRANGE_GOLDEN_WIDE: &str = "arrange-golden-wide";
pub(super) const ACTION_ARRANGE_GOLDEN_NARROW: &str = "arrange-golden-narrow";
pub(super) const ACTION_ARRANGE_GOLDEN_TALL: &str = "arrange-golden-tall";
pub(super) const ACTION_ARRANGE_GOLDEN_SHORT: &str = "arrange-golden-short";
pub(super) const ACTION_RESET_PANE_LAYOUT: &str = "reset-pane-layout";
pub(super) const ACTION_RESTORE_CLOSED_PANE: &str = "restore-closed-pane";
pub(super) const ACTION_OPEN_SERVER: &str = "open-server";
pub(super) const ACTION_OPEN_SERVER_BROWSER: &str = "open-server-browser";
pub(super) const ACTION_IGNORE_SERVER_PORT: &str = "ignore-server-port";
pub(super) const ACTION_REFRESH_SERVERS: &str = "refresh-servers";
pub(super) const ACTION_STOP_IGNORING_SERVER_PORT: &str = "stop-ignoring-server-port";
pub(super) const ACTION_STOP_SERVER: &str = "stop-server";
pub(super) const ACTION_RUN_TASK: &str = "run-task";
pub(super) const ACTION_SHOW_TASK_MANAGER: &str = "show-task-manager";
pub(super) const ACTION_REFRESH_REVIEW_STATUS: &str = "refresh-review-status";
pub(super) const ACTION_OPEN_BRANCH_REMOTE: &str = "open-branch-remote";
pub(super) const ACTION_OPEN_PULL_REQUEST: &str = "open-pull-request";
pub(super) const ACTION_OPEN_WITH_PRIMARY: &str = "open-with-primary";
pub(super) const ACTION_OPEN_WITH_TARGET: &str = "open-with-target";
pub(super) const ACTION_SAVE_TEMPLATE: &str = "save-template";
pub(super) const ACTION_ACTIVATE_TEMPLATE: &str = "activate-template";
pub(super) const ACTION_RENAME_TEMPLATE: &str = "rename-template";
pub(super) const ACTION_TOGGLE_TEMPLATE_PIN: &str = "toggle-template-pin";
pub(super) const ACTION_DUPLICATE_TEMPLATE: &str = "duplicate-template";
pub(super) const ACTION_CONVERT_TEMPLATE: &str = "convert-template";
pub(super) const ACTION_DELETE_TEMPLATE: &str = "delete-template";
pub(super) const ACTION_UPDATE_LINKED_TEMPLATE: &str = "update-linked-template";
pub(super) const ACTION_UNLINK_TEMPLATE: &str = "unlink-template";
pub(super) const ACTION_IMPORT_TEMPLATE: &str = "import-template";
pub(super) const ACTION_EXPORT_TEMPLATE: &str = "export-template";
pub(super) const ACTION_EDIT_TEMPLATE: &str = "edit-template";

pub(super) const ACTION_SPECS: &[ActionSpec] = &[
    action!(ACTION_NEW_WINDOW, "new-window", None),
    action!(ACTION_CLOSE_WINDOW, "close-window", None),
    action!(ACTION_TOGGLE_SIDEBAR, "toggle-sidebar", None),
    action!(ACTION_NEW_WORKLANE, "new-worklane", None),
    action!(ACTION_SELECT_WORKLANE, "select-worklane", String),
    action!(ACTION_SPLIT_PANE_RIGHT, "split-pane-right", None),
    action!(ACTION_ADD_PANE_RIGHT, "add-pane-right", None),
    action!(ACTION_ADD_PANE_LEFT, "add-pane-left", None),
    action!(ACTION_SPLIT_PANE_BELOW, "split-pane-below", None),
    action!(ACTION_CLOSE_PANE, "close-pane", None),
    action!(ACTION_RENAME_WORKLANE, "rename-worklane", StringPair),
    action!(ACTION_RENAME_PANE, "rename-pane", StringPair),
    action!(ACTION_CYCLE_WORKLANE_COLOR, "cycle-worklane-color", None),
    action!(ACTION_SET_WORKLANE_COLOR, "set-worklane-color", StringPair),
    action!(ACTION_CLOSE_WORKLANE, "close-worklane", String),
    action!(ACTION_CLOSE_ACTIVE_WORKLANE, "close-active-worklane", None),
    action!(ACTION_MOVE_WORKLANE, "move-worklane", StringPair),
    action!(ACTION_REORDER_WORKLANE, "reorder-worklane", StringPair),
    action!(ACTION_MOVE_WORKLANE_UP, "move-worklane-up", None),
    action!(ACTION_MOVE_WORKLANE_DOWN, "move-worklane-down", None),
    action!(ACTION_MOVE_PANE_LEFT, "move-pane-left", None),
    action!(ACTION_MOVE_PANE_RIGHT, "move-pane-right", None),
    action!(ACTION_MOVE_PANE_UP, "move-pane-up", None),
    action!(ACTION_MOVE_PANE_DOWN, "move-pane-down", None),
    action!(
        ACTION_MOVE_PANE_TO_WORKLANE,
        "move-pane-to-worklane",
        String
    ),
    action!(
        ACTION_MOVE_PANE_TO_NEW_WINDOW,
        "move-pane-to-new-window",
        None,
        MultipleWorkspacePanes
    ),
    action!(ACTION_SELECT_PANE, "select-pane", StringPair),
    action!(ACTION_NAVIGATE_BACK, "navigate-back", None),
    action!(ACTION_NAVIGATE_FORWARD, "navigate-forward", None),
    action!(ACTION_NEXT_PANE, "next-pane", None),
    action!(ACTION_PREVIOUS_PANE, "previous-pane", None),
    action!(ACTION_NEXT_WORKLANE, "next-worklane", None),
    action!(ACTION_PREVIOUS_WORKLANE, "previous-worklane", None),
    action!(
        ACTION_DISMISS_COMMAND_PALETTE,
        "dismiss-command-palette",
        None
    ),
    action!(ACTION_FIND, "find", None),
    action!(
        ACTION_USE_SELECTION_FOR_FIND,
        "use-selection-for-find",
        None
    ),
    action!(ACTION_FIND_NEXT, "find-next", None),
    action!(ACTION_FIND_PREVIOUS, "find-previous", None),
    action!(ACTION_COPY, "copy", None),
    action!(ACTION_CLEAN_COPY, "clean-copy", None),
    action!(ACTION_COPY_RAW, "copy-raw", None),
    action!(ACTION_COPY_AS_MARKDOWN, "copy-as-markdown", None),
    action!(ACTION_SELECT_ALL, "select-all", None),
    action!(ACTION_GLOBAL_FIND, "global-find", None),
    action!(ACTION_CLEAR_GLOBAL_FIND, "clear-global-find", None),
    action!(ACTION_GLOBAL_FIND_NEXT, "global-find-next", None),
    action!(ACTION_GLOBAL_FIND_PREVIOUS, "global-find-previous", None),
    action!(ACTION_FOCUS_PANE_LEFT, "focus-pane-left", None),
    action!(ACTION_FOCUS_PANE_RIGHT, "focus-pane-right", None),
    action!(ACTION_FOCUS_PANE_UP, "focus-pane-up", None),
    action!(ACTION_FOCUS_PANE_DOWN, "focus-pane-down", None),
    action!(
        ACTION_RESIZE_PANE_LEFT,
        "resize-pane-left",
        None,
        MultipleColumns
    ),
    action!(
        ACTION_RESIZE_PANE_RIGHT,
        "resize-pane-right",
        None,
        MultipleColumns
    ),
    action!(
        ACTION_RESIZE_PANE_UP,
        "resize-pane-up",
        None,
        MultiplePanesInFocusedColumn
    ),
    action!(
        ACTION_RESIZE_PANE_DOWN,
        "resize-pane-down",
        None,
        MultiplePanesInFocusedColumn
    ),
    action!(ACTION_ARRANGE_WIDTH_FULL, "arrange-width-full", None),
    action!(ACTION_ARRANGE_WIDTH_HALF, "arrange-width-half", None),
    action!(ACTION_ARRANGE_WIDTH_THIRDS, "arrange-width-thirds", None),
    action!(
        ACTION_ARRANGE_WIDTH_QUARTERS,
        "arrange-width-quarters",
        None
    ),
    action!(ACTION_ARRANGE_HEIGHT_FULL, "arrange-height-full", None),
    action!(ACTION_ARRANGE_HEIGHT_TWO, "arrange-height-two", None),
    action!(ACTION_ARRANGE_HEIGHT_THREE, "arrange-height-three", None),
    action!(ACTION_ARRANGE_HEIGHT_FOUR, "arrange-height-four", None),
    action!(
        ACTION_ARRANGE_GOLDEN_WIDE,
        "arrange-golden-wide",
        None,
        MultipleColumns
    ),
    action!(
        ACTION_ARRANGE_GOLDEN_NARROW,
        "arrange-golden-narrow",
        None,
        MultipleColumns
    ),
    action!(
        ACTION_ARRANGE_GOLDEN_TALL,
        "arrange-golden-tall",
        None,
        MultiplePanesInFocusedColumn
    ),
    action!(
        ACTION_ARRANGE_GOLDEN_SHORT,
        "arrange-golden-short",
        None,
        MultiplePanesInFocusedColumn
    ),
    action!(ACTION_RESET_PANE_LAYOUT, "reset-pane-layout", None),
    action!(ACTION_RESTORE_CLOSED_PANE, "restore-closed-pane", None),
    action!(ACTION_OPEN_SERVER, "open-server", String),
    action!(ACTION_OPEN_SERVER_BROWSER, "open-server-browser", String),
    action!(ACTION_IGNORE_SERVER_PORT, "ignore-server-port", String),
    action!(ACTION_REFRESH_SERVERS, "refresh-servers", None),
    action!(
        ACTION_STOP_IGNORING_SERVER_PORT,
        "stop-ignoring-server-port",
        String
    ),
    action!(ACTION_STOP_SERVER, "stop-server", String),
    action!(ACTION_RUN_TASK, "run-task", String),
    action!(ACTION_SHOW_TASK_MANAGER, "show-task-manager", None),
    action!(ACTION_REFRESH_REVIEW_STATUS, "refresh-review-status", None),
    action!(ACTION_OPEN_BRANCH_REMOTE, "open-branch-remote", None),
    action!(ACTION_OPEN_PULL_REQUEST, "open-pull-request", None),
    action!(ACTION_OPEN_WITH_PRIMARY, "open-with-primary", None),
    action!(ACTION_OPEN_WITH_TARGET, "open-with-target", String),
    action!(ACTION_SAVE_TEMPLATE, "save-template", StringPair),
    action!(ACTION_ACTIVATE_TEMPLATE, "activate-template", String),
    action!(ACTION_RENAME_TEMPLATE, "rename-template", StringPair),
    action!(ACTION_TOGGLE_TEMPLATE_PIN, "toggle-template-pin", String),
    action!(ACTION_DUPLICATE_TEMPLATE, "duplicate-template", String),
    action!(ACTION_CONVERT_TEMPLATE, "convert-template", String),
    action!(ACTION_DELETE_TEMPLATE, "delete-template", String),
    action!(
        ACTION_UPDATE_LINKED_TEMPLATE,
        "update-linked-template",
        None
    ),
    action!(ACTION_UNLINK_TEMPLATE, "unlink-template", None),
    action!(ACTION_IMPORT_TEMPLATE, "import-template", None),
    action!(ACTION_EXPORT_TEMPLATE, "export-template", String),
    action!(ACTION_EDIT_TEMPLATE, "edit-template", StringPair),
];

pub(super) struct ActionRouter {
    group: gio::SimpleActionGroup,
}

impl ActionRouter {
    pub(super) fn install(shell: &Rc<RefCell<ApplicationShell>>) -> Result<Self, String> {
        let group = gio::SimpleActionGroup::new();
        populate(shell, &group);
        validate_registered_group(&group)?;
        shell
            .borrow()
            .window
            .insert_action_group("workspace", Some(&group));
        Ok(Self { group })
    }

    pub(super) fn uninstall(self, window: &gtk::Window) {
        window.insert_action_group("workspace", None::<&gio::SimpleActionGroup>);
        drop(self.group);
    }

    pub(super) fn refresh_availability(
        &self,
        columns: usize,
        focused_column_panes: usize,
        workspace_panes: usize,
    ) {
        for spec in ACTION_SPECS {
            let enabled = spec
                .availability
                .enabled(columns, focused_column_panes, workspace_panes);
            let Some(action) = self
                .group
                .lookup_action(spec.name)
                .and_then(|action| action.downcast::<gio::SimpleAction>().ok())
            else {
                continue;
            };
            action.set_enabled(enabled);
        }
    }
}

fn populate(shell: &Rc<RefCell<ApplicationShell>>, group: &gio::SimpleActionGroup) {
    install_application_actions(shell, group);

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
                .handle(super::SidebarVisibilityEvent::TogglePressed);
            shell.apply_sidebar_visibility();
            shell.sidebar_visibility.mode() != super::SidebarVisibilityMode::Hidden
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
        if let Err(error) = ApplicationShell::create_worklane(&shell) {
            ApplicationShell::report_action_error(&shell, ACTION_NEW_WORKLANE, &error);
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

    install_pane_creation_actions(shell, group);
    install_pane_layout_actions(shell, group);
    install_restore_closed_pane_action(shell, group);

    add_simple_action(shell, group, ACTION_NAVIGATE_BACK, |shell| {
        shell.navigate_history(true);
    });
    add_simple_action(shell, group, ACTION_NAVIGATE_FORWARD, |shell| {
        shell.navigate_history(false);
    });
    add_simple_action(shell, group, ACTION_NEXT_PANE, |shell| {
        shell.select_adjacent_pane(true);
    });
    add_simple_action(shell, group, ACTION_PREVIOUS_PANE, |shell| {
        shell.select_adjacent_pane(false);
    });
    add_simple_action(shell, group, ACTION_NEXT_WORKLANE, |shell| {
        shell.select_adjacent_worklane(true);
    });
    add_simple_action(shell, group, ACTION_PREVIOUS_WORKLANE, |shell| {
        shell.select_adjacent_worklane(false);
    });
    install_search_actions(shell, group);
    install_clipboard_actions(shell, group);
    install_server_actions(shell, group);
    install_task_runner_action(shell, group);
    add_simple_action(shell, group, ACTION_SHOW_TASK_MANAGER, |shell| {
        shell.request_show_task_manager();
    });
    install_project_context_actions(shell, group);
    install_open_with_actions(shell, group);
    install_bookmark_actions(shell, group);
    install_edit_actions(shell, group);
}

fn install_bookmark_actions(shell: &Rc<RefCell<ApplicationShell>>, group: &gio::SimpleActionGroup) {
    let pair = glib::VariantTy::new("(ss)").expect("static action type is valid");
    install_bookmark_pair_actions(shell, group, pair);

    for (name, handler) in [
        (
            ACTION_ACTIVATE_TEMPLATE,
            super::bookmark_runtime::activate
                as fn(&Rc<RefCell<ApplicationShell>>, &str) -> Result<(), String>,
        ),
        (
            ACTION_TOGGLE_TEMPLATE_PIN,
            super::bookmark_runtime::toggle_pin,
        ),
        (
            ACTION_DUPLICATE_TEMPLATE,
            super::bookmark_runtime::duplicate,
        ),
        (ACTION_CONVERT_TEMPLATE, super::bookmark_runtime::convert),
        (ACTION_DELETE_TEMPLATE, super::bookmark_runtime::delete),
    ] {
        let action = gio::SimpleAction::new(name, Some(glib::VariantTy::STRING));
        let weak = Rc::downgrade(shell);
        action.connect_activate(move |_, parameter| {
            let (Some(shell), Some(id)) = (weak.upgrade(), parameter.and_then(glib::Variant::str))
            else {
                return;
            };
            report_bookmark_result(name, handler(&shell, id));
        });
        group.add_action(&action);
    }
    for (name, handler) in [
        (
            ACTION_UPDATE_LINKED_TEMPLATE,
            super::bookmark_runtime::update_linked
                as fn(&Rc<RefCell<ApplicationShell>>) -> Result<(), String>,
        ),
        (ACTION_UNLINK_TEMPLATE, super::bookmark_runtime::unlink),
    ] {
        let action = gio::SimpleAction::new(name, None);
        let weak = Rc::downgrade(shell);
        action.connect_activate(move |_, parameter| {
            if !ParameterSchema::None.accepts(parameter) {
                return;
            }
            if let Some(shell) = weak.upgrade() {
                report_bookmark_result(name, handler(&shell));
            }
        });
        group.add_action(&action);
    }
    install_bookmark_file_actions(shell, group);
}

fn install_bookmark_pair_actions(
    shell: &Rc<RefCell<ApplicationShell>>,
    group: &gio::SimpleActionGroup,
    pair: &glib::VariantTy,
) {
    let save = gio::SimpleAction::new(ACTION_SAVE_TEMPLATE, Some(pair));
    let weak = Rc::downgrade(shell);
    save.connect_activate(move |_, parameter| {
        let (Some(shell), Some((kind, name))) = (
            weak.upgrade(),
            parameter.and_then(glib::Variant::get::<(String, String)>),
        ) else {
            return;
        };
        let kind = if kind == "Bookmark" {
            zentty_core::TemplateKind::Bookmark
        } else if kind == "Preset" {
            zentty_core::TemplateKind::Preset
        } else {
            return;
        };
        report_bookmark_result(
            ACTION_SAVE_TEMPLATE,
            super::bookmark_runtime::save_active(&shell, &name, kind),
        );
    });
    group.add_action(&save);

    let rename = gio::SimpleAction::new(ACTION_RENAME_TEMPLATE, Some(pair));
    let weak = Rc::downgrade(shell);
    rename.connect_activate(move |_, parameter| {
        let (Some(shell), Some((id, name))) = (
            weak.upgrade(),
            parameter.and_then(glib::Variant::get::<(String, String)>),
        ) else {
            return;
        };
        report_bookmark_result(
            ACTION_RENAME_TEMPLATE,
            super::bookmark_runtime::rename(&shell, &id, &name),
        );
    });
    group.add_action(&rename);

    let edit = gio::SimpleAction::new(ACTION_EDIT_TEMPLATE, Some(pair));
    let weak = Rc::downgrade(shell);
    edit.connect_activate(move |_, parameter| {
        let (Some(shell), Some((id, json))) = (
            weak.upgrade(),
            parameter.and_then(glib::Variant::get::<(String, String)>),
        ) else {
            return;
        };
        report_bookmark_result(
            ACTION_EDIT_TEMPLATE,
            super::bookmark_runtime::edit(&shell, &id, &json),
        );
    });
    group.add_action(&edit);
}

fn install_bookmark_file_actions(
    shell: &Rc<RefCell<ApplicationShell>>,
    group: &gio::SimpleActionGroup,
) {
    let export = gio::SimpleAction::new(ACTION_EXPORT_TEMPLATE, Some(glib::VariantTy::STRING));
    let weak = Rc::downgrade(shell);
    export.connect_activate(move |_, parameter| {
        let (Some(shell), Some(id)) = (weak.upgrade(), parameter.and_then(glib::Variant::str))
        else {
            return;
        };
        super::bookmark_runtime::choose_export(&shell, id);
    });
    group.add_action(&export);

    let import = gio::SimpleAction::new(ACTION_IMPORT_TEMPLATE, None);
    let weak = Rc::downgrade(shell);
    import.connect_activate(move |_, parameter| {
        if ParameterSchema::None.accepts(parameter)
            && let Some(shell) = weak.upgrade()
        {
            super::bookmark_runtime::choose_import(&shell);
        }
    });
    group.add_action(&import);
}

fn report_bookmark_result(action: &str, result: Result<(), String>) {
    match result {
        Ok(()) => eprintln!("zentty-linux: action={action} result=ok"),
        Err(error) => eprintln!("zentty-linux: action={action} failed: {error}"),
    }
}

fn install_open_with_actions(
    shell: &Rc<RefCell<ApplicationShell>>,
    group: &gio::SimpleActionGroup,
) {
    let primary = gio::SimpleAction::new(ACTION_OPEN_WITH_PRIMARY, None);
    let weak = Rc::downgrade(shell);
    primary.connect_activate(move |_, parameter| {
        if !ParameterSchema::None.accepts(parameter) {
            eprintln!("zentty-linux: action=open-with-primary rejected parameter-schema");
            return;
        }
        if let Some(shell) = weak.upgrade() {
            super::open_with_runtime::open_primary(&shell.borrow());
        }
    });
    group.add_action(&primary);

    let target = gio::SimpleAction::new(ACTION_OPEN_WITH_TARGET, Some(glib::VariantTy::STRING));
    let weak = Rc::downgrade(shell);
    target.connect_activate(move |_, parameter| {
        let (Some(shell), Some(target_id)) =
            (weak.upgrade(), parameter.and_then(glib::Variant::str))
        else {
            return;
        };
        super::open_with_runtime::open_target(&shell.borrow(), target_id);
    });
    group.add_action(&target);
}

fn install_project_context_actions(
    shell: &Rc<RefCell<ApplicationShell>>,
    group: &gio::SimpleActionGroup,
) {
    for (name, handler) in [
        (
            ACTION_REFRESH_REVIEW_STATUS,
            super::project_context_runtime::refresh_focused as fn(&Rc<RefCell<ApplicationShell>>),
        ),
        (
            ACTION_OPEN_BRANCH_REMOTE,
            super::project_context_runtime::open_focused_branch,
        ),
        (
            ACTION_OPEN_PULL_REQUEST,
            super::project_context_runtime::open_focused_pull_request,
        ),
    ] {
        let action = gio::SimpleAction::new(name, None);
        let weak = Rc::downgrade(shell);
        action.connect_activate(move |_, parameter| {
            if !ParameterSchema::None.accepts(parameter) {
                eprintln!("zentty-linux: action={name} rejected parameter-schema");
                return;
            }
            if let Some(shell) = weak.upgrade() {
                handler(&shell);
            }
        });
        group.add_action(&action);
    }
}

fn install_task_runner_action(
    shell: &Rc<RefCell<ApplicationShell>>,
    group: &gio::SimpleActionGroup,
) {
    let action = gio::SimpleAction::new(ACTION_RUN_TASK, Some(glib::VariantTy::STRING));
    let weak = Rc::downgrade(shell);
    action.connect_activate(move |_, parameter| {
        let (Some(shell), Some(id)) = (weak.upgrade(), parameter.and_then(glib::Variant::str))
        else {
            return;
        };
        super::task_runner_runtime::run_task(&shell, id);
    });
    group.add_action(&action);
}

fn install_server_actions(shell: &Rc<RefCell<ApplicationShell>>, group: &gio::SimpleActionGroup) {
    let action = gio::SimpleAction::new(ACTION_OPEN_SERVER, Some(glib::VariantTy::STRING));
    let weak = Rc::downgrade(shell);
    action.connect_activate(move |_, parameter| {
        let (Some(shell), Some(origin)) = (weak.upgrade(), parameter.and_then(glib::Variant::str))
        else {
            return;
        };
        super::server_runtime::open_server(&shell, origin);
    });
    group.add_action(&action);

    let action = gio::SimpleAction::new(ACTION_OPEN_SERVER_BROWSER, Some(glib::VariantTy::STRING));
    let weak = Rc::downgrade(shell);
    action.connect_activate(move |_, parameter| {
        let (Some(shell), Some(action_id)) =
            (weak.upgrade(), parameter.and_then(glib::Variant::str))
        else {
            return;
        };
        super::server_runtime::open_server_in_browser(&shell, action_id);
    });
    group.add_action(&action);

    for (name, ignored) in [
        (ACTION_IGNORE_SERVER_PORT, true),
        (ACTION_STOP_IGNORING_SERVER_PORT, false),
    ] {
        let action = gio::SimpleAction::new(name, Some(glib::VariantTy::STRING));
        let weak = Rc::downgrade(shell);
        action.connect_activate(move |_, parameter| {
            let (Some(shell), Some(origin)) =
                (weak.upgrade(), parameter.and_then(glib::Variant::str))
            else {
                return;
            };
            super::server_runtime::set_port_ignored(&shell, origin, ignored);
        });
        group.add_action(&action);
    }

    let action = gio::SimpleAction::new(ACTION_REFRESH_SERVERS, None);
    let weak = Rc::downgrade(shell);
    action.connect_activate(move |_, _| {
        if let Some(shell) = weak.upgrade() {
            super::server_runtime::refresh_servers(&shell);
        }
    });
    group.add_action(&action);

    let action = gio::SimpleAction::new(ACTION_STOP_SERVER, Some(glib::VariantTy::STRING));
    let weak = Rc::downgrade(shell);
    action.connect_activate(move |_, parameter| {
        let (Some(shell), Some(origin)) = (weak.upgrade(), parameter.and_then(glib::Variant::str))
        else {
            return;
        };
        super::server_runtime::stop_server(&shell, origin);
    });
    group.add_action(&action);
}

fn install_clipboard_actions(
    shell: &Rc<RefCell<ApplicationShell>>,
    group: &gio::SimpleActionGroup,
) {
    for (name, style) in [
        (ACTION_COPY, super::clipboard_actions::CopyStyle::Default),
        (
            ACTION_CLEAN_COPY,
            super::clipboard_actions::CopyStyle::Clean,
        ),
        (ACTION_COPY_RAW, super::clipboard_actions::CopyStyle::Raw),
        (
            ACTION_COPY_AS_MARKDOWN,
            super::clipboard_actions::CopyStyle::Markdown,
        ),
    ] {
        let action = gio::SimpleAction::new(name, None);
        let weak = Rc::downgrade(shell);
        action.connect_activate(move |_, parameter| {
            if !ParameterSchema::None.accepts(parameter) {
                eprintln!("zentty-linux: action={name} rejected parameter-schema");
                return;
            }
            if let Some(shell) = weak.upgrade() {
                ApplicationShell::copy_focused_selection(&shell, style);
            }
        });
        group.add_action(&action);
    }
    add_simple_action(shell, group, ACTION_SELECT_ALL, |shell| {
        shell.perform_focused_binding_action(ACTION_SELECT_ALL, "select_all");
    });
}

fn install_application_actions(
    shell: &Rc<RefCell<ApplicationShell>>,
    group: &gio::SimpleActionGroup,
) {
    let new_window = gio::SimpleAction::new(ACTION_NEW_WINDOW, None);
    let weak = Rc::downgrade(shell);
    new_window.connect_activate(move |_, _| {
        if let Some(shell) = weak.upgrade() {
            shell.borrow().request_new_window();
        }
    });
    group.add_action(&new_window);

    let close_window = gio::SimpleAction::new(ACTION_CLOSE_WINDOW, None);
    let weak = Rc::downgrade(shell);
    close_window.connect_activate(move |_, _| {
        if let Some(shell) = weak.upgrade() {
            shell.borrow().request_close_window();
        }
    });
    group.add_action(&close_window);

    let move_pane = gio::SimpleAction::new(ACTION_MOVE_PANE_TO_NEW_WINDOW, None);
    let weak = Rc::downgrade(shell);
    move_pane.connect_activate(move |_, _| {
        if let Some(shell) = weak.upgrade() {
            shell.borrow().request_move_pane_to_new_window();
        }
    });
    group.add_action(&move_pane);
}

fn install_edit_actions(shell: &Rc<RefCell<ApplicationShell>>, group: &gio::SimpleActionGroup) {
    let string_pair = glib::VariantTy::new("(ss)").expect("static action type is valid");
    install_worklane_edit_actions(shell, group, string_pair);
    install_pane_rename_action(shell, group, string_pair);

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

    install_pane_transfer_action(shell, group);

    add_simple_action(shell, group, ACTION_CYCLE_WORKLANE_COLOR, |shell| {
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
    add_simple_action(shell, group, ACTION_MOVE_WORKLANE_UP, |shell| {
        shell.move_active_worklane(-1);
    });
    add_simple_action(shell, group, ACTION_MOVE_WORKLANE_DOWN, |shell| {
        shell.move_active_worklane(1);
    });
    add_simple_action(shell, group, ACTION_MOVE_PANE_LEFT, |shell| {
        if shell.state.move_focused_pane_left() {
            eprintln!("zentty-linux: action=move-pane-left");
            shell.render();
            shell.focus_selected_surface();
        }
    });
    add_simple_action(shell, group, ACTION_MOVE_PANE_RIGHT, |shell| {
        if shell.state.move_focused_pane_right() {
            eprintln!("zentty-linux: action=move-pane-right");
            shell.render();
            shell.focus_selected_surface();
        }
    });
    add_simple_action(shell, group, ACTION_MOVE_PANE_UP, |shell| {
        if shell.state.move_focused_pane_up() {
            eprintln!("zentty-linux: action=move-pane-up");
            shell.render();
            shell.focus_selected_surface();
        }
    });
    add_simple_action(shell, group, ACTION_MOVE_PANE_DOWN, |shell| {
        if shell.state.move_focused_pane_down() {
            eprintln!("zentty-linux: action=move-pane-down");
            shell.render();
            shell.focus_selected_surface();
        }
    });
}

fn install_worklane_edit_actions(
    shell: &Rc<RefCell<ApplicationShell>>,
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
            ApplicationShell::focus_terminal_after_present(&shell);
        }
    });
    group.add_action(&rename_worklane);

    let set_worklane_color = gio::SimpleAction::new(ACTION_SET_WORKLANE_COLOR, Some(string_pair));
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
        ApplicationShell::close_worklane(&shell, worklane_id);
    });
    group.add_action(&close_worklane);

    let close_active_worklane = gio::SimpleAction::new(ACTION_CLOSE_ACTIVE_WORKLANE, None);
    let weak = Rc::downgrade(shell);
    close_active_worklane.connect_activate(move |_, _| {
        let Some(shell) = weak.upgrade() else {
            return;
        };
        let worklane_id = shell.borrow().state.active_worklane_id().to_owned();
        ApplicationShell::close_worklane(&shell, &worklane_id);
    });
    group.add_action(&close_active_worklane);

    install_worklane_move_actions(shell, group, string_pair);
}

fn install_worklane_move_actions(
    shell: &Rc<RefCell<ApplicationShell>>,
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
            eprintln!("zentty-linux: action=move-worklane id={worklane_id} direction={direction}");
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

fn install_pane_rename_action(
    shell: &Rc<RefCell<ApplicationShell>>,
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
            ApplicationShell::focus_terminal_after_present(&shell);
        }
    });
    group.add_action(&rename_pane);
}

fn install_pane_transfer_action(
    shell: &Rc<RefCell<ApplicationShell>>,
    group: &gio::SimpleActionGroup,
) {
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

fn install_pane_creation_actions(
    shell: &Rc<RefCell<ApplicationShell>>,
    group: &gio::SimpleActionGroup,
) {
    let split_pane = gio::SimpleAction::new(ACTION_SPLIT_PANE_RIGHT, None);
    let weak = Rc::downgrade(shell);
    split_pane.connect_activate(move |_, _| {
        let Some(shell) = weak.upgrade() else {
            return;
        };
        if let Err(error) = ApplicationShell::split_focused_pane_right(&shell) {
            ApplicationShell::report_action_error(&shell, ACTION_SPLIT_PANE_RIGHT, &error);
        }
    });
    group.add_action(&split_pane);

    let add_pane = gio::SimpleAction::new(ACTION_ADD_PANE_RIGHT, None);
    let weak = Rc::downgrade(shell);
    add_pane.connect_activate(move |_, _| {
        let Some(shell) = weak.upgrade() else {
            return;
        };
        if let Err(error) = ApplicationShell::add_focused_pane_right(&shell) {
            ApplicationShell::report_action_error(&shell, ACTION_ADD_PANE_RIGHT, &error);
        }
    });
    group.add_action(&add_pane);

    let add_pane = gio::SimpleAction::new(ACTION_ADD_PANE_LEFT, None);
    let weak = Rc::downgrade(shell);
    add_pane.connect_activate(move |_, _| {
        let Some(shell) = weak.upgrade() else {
            return;
        };
        if let Err(error) = ApplicationShell::add_focused_pane_left(&shell) {
            ApplicationShell::report_action_error(&shell, ACTION_ADD_PANE_LEFT, &error);
        }
    });
    group.add_action(&add_pane);

    let split_pane = gio::SimpleAction::new(ACTION_SPLIT_PANE_BELOW, None);
    let weak = Rc::downgrade(shell);
    split_pane.connect_activate(move |_, _| {
        let Some(shell) = weak.upgrade() else {
            return;
        };
        if let Err(error) = ApplicationShell::split_focused_pane_below(&shell) {
            ApplicationShell::report_action_error(&shell, ACTION_SPLIT_PANE_BELOW, &error);
        }
    });
    group.add_action(&split_pane);

    let close_pane = gio::SimpleAction::new(ACTION_CLOSE_PANE, None);
    let weak = Rc::downgrade(shell);
    close_pane.connect_activate(move |_, _| {
        let Some(shell) = weak.upgrade() else {
            return;
        };
        ApplicationShell::close_focused_pane(&shell);
    });
    group.add_action(&close_pane);
}

fn install_restore_closed_pane_action(
    shell: &Rc<RefCell<ApplicationShell>>,
    group: &gio::SimpleActionGroup,
) {
    let action = gio::SimpleAction::new(ACTION_RESTORE_CLOSED_PANE, None);
    let weak = Rc::downgrade(shell);
    action.connect_activate(move |_, _| {
        let Some(shell) = weak.upgrade() else {
            return;
        };
        if let Err(error) = ApplicationShell::restore_closed_pane(&shell) {
            ApplicationShell::report_action_error(&shell, ACTION_RESTORE_CLOSED_PANE, &error);
        }
    });
    group.add_action(&action);
}

fn install_search_actions(shell: &Rc<RefCell<ApplicationShell>>, group: &gio::SimpleActionGroup) {
    add_simple_action(shell, group, ACTION_GLOBAL_FIND, |shell| {
        shell.toggle_global_find();
    });
    add_simple_action(shell, group, ACTION_CLEAR_GLOBAL_FIND, |shell| {
        shell.update_global_find_query("");
        shell.global_search_view.focus(false);
    });
    add_simple_action(shell, group, ACTION_GLOBAL_FIND_NEXT, |shell| {
        shell.navigate_global_find(zentty_core::GlobalSearchDirection::Next);
    });
    add_simple_action(shell, group, ACTION_GLOBAL_FIND_PREVIOUS, |shell| {
        shell.navigate_global_find(zentty_core::GlobalSearchDirection::Previous);
    });
    add_simple_action(shell, group, ACTION_FIND, |shell| {
        shell.perform_focused_binding_action(ACTION_FIND, "start_search");
    });
    add_simple_action(shell, group, ACTION_USE_SELECTION_FOR_FIND, |shell| {
        shell.perform_focused_binding_action(ACTION_USE_SELECTION_FOR_FIND, "search_selection");
    });
    add_simple_action(shell, group, ACTION_FIND_NEXT, |shell| {
        shell.perform_focused_binding_action(ACTION_FIND_NEXT, "navigate_search:next");
    });
    add_simple_action(shell, group, ACTION_FIND_PREVIOUS, |shell| {
        shell.perform_focused_binding_action(ACTION_FIND_PREVIOUS, "navigate_search:previous");
    });
}

fn install_pane_layout_actions(
    shell: &Rc<RefCell<ApplicationShell>>,
    group: &gio::SimpleActionGroup,
) {
    for (name, update) in [
        (
            ACTION_FOCUS_PANE_LEFT,
            WorkspaceState::focus_pane_left as fn(&mut WorkspaceState) -> bool,
        ),
        (ACTION_FOCUS_PANE_RIGHT, WorkspaceState::focus_pane_right),
        (ACTION_FOCUS_PANE_UP, WorkspaceState::focus_pane_up),
        (ACTION_FOCUS_PANE_DOWN, WorkspaceState::focus_pane_down),
    ] {
        add_simple_action(shell, group, name, move |shell| {
            if update(&mut shell.state) {
                shell.finish_pane_layout_action(name);
            }
        });
    }
    for (name, direction) in [
        (
            ACTION_RESIZE_PANE_LEFT,
            zentty_core::PaneResizeDirection::Left,
        ),
        (
            ACTION_RESIZE_PANE_RIGHT,
            zentty_core::PaneResizeDirection::Right,
        ),
        (ACTION_RESIZE_PANE_UP, zentty_core::PaneResizeDirection::Up),
        (
            ACTION_RESIZE_PANE_DOWN,
            zentty_core::PaneResizeDirection::Down,
        ),
    ] {
        add_simple_action(shell, group, name, move |shell| {
            if shell.resize_focused_pane_by_cell(direction) {
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
        add_simple_action(shell, group, name, move |shell| {
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
        add_simple_action(shell, group, name, move |shell| {
            if shell.state.arrange_panes_per_column(panes_per_column) {
                shell.finish_pane_layout_action(name);
            }
        });
    }
    for (name, focus_wide) in [
        (ACTION_ARRANGE_GOLDEN_WIDE, true),
        (ACTION_ARRANGE_GOLDEN_NARROW, false),
    ] {
        add_simple_action(shell, group, name, move |shell| {
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
        add_simple_action(shell, group, name, move |shell| {
            if shell.state.arrange_golden_height(focus_tall) {
                shell.finish_pane_layout_action(name);
            }
        });
    }
    add_simple_action(shell, group, ACTION_RESET_PANE_LAYOUT, |shell| {
        let width = f64::from(shell.pane_viewport_width());
        if shell.state.reset_active_layout(width) {
            shell.finish_pane_layout_action(ACTION_RESET_PANE_LAYOUT);
        }
    });
}

pub(super) fn add_simple_action(
    shell: &Rc<RefCell<ApplicationShell>>,
    group: &gio::SimpleActionGroup,
    name: &'static str,
    handler: impl Fn(&mut ApplicationShell) + 'static,
) {
    let action = gio::SimpleAction::new(name, None);
    let weak = Rc::downgrade(shell);
    action.connect_activate(move |_, parameter| {
        if !ParameterSchema::None.accepts(parameter) {
            eprintln!("zentty-linux: action={name} rejected parameter-schema");
            return;
        }
        if let Some(shell) = weak.upgrade() {
            handler(&mut shell.borrow_mut());
        }
    });
    group.add_action(&action);
}

fn validate_registered_group(group: &gio::SimpleActionGroup) -> Result<(), String> {
    let mut actual = group.list_actions();
    actual.sort();
    let mut expected = ACTION_SPECS
        .iter()
        .map(|action| action.name.to_owned())
        .collect::<Vec<_>>();
    expected.sort();
    if actual != expected {
        return Err("GTK action registry differs from the authoritative schema".to_owned());
    }
    for action in ACTION_SPECS {
        let registered = group
            .lookup_action(action.name)
            .ok_or_else(|| format!("GTK action is absent after registration: {}", action.name))?;
        let actual_type = registered.parameter_type().map(|kind| kind.to_string());
        let expected_type = match action.parameter {
            ParameterSchema::None => None,
            ParameterSchema::String => Some("s".to_owned()),
            ParameterSchema::StringPair => Some("(ss)".to_owned()),
        };
        if actual_type != expected_type {
            return Err(format!(
                "GTK action parameter mismatch for {}: expected {expected_type:?}, got {actual_type:?}",
                action.name
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use gtk::glib::variant::ToVariant;
    use gtk::prelude::ActionMapExt;
    use gtk::{gio, glib};

    use super::{
        ACTION_SPECS, ActionSpec, Availability, ParameterSchema, validate_registered_group,
    };

    fn spec(name: &str) -> Option<&'static ActionSpec> {
        ACTION_SPECS.iter().find(|candidate| candidate.name == name)
    }

    #[test]
    fn registry_is_unique_complete_and_typed() {
        assert_eq!(ACTION_SPECS.len(), 94);
        assert_eq!(
            ACTION_SPECS
                .iter()
                .map(|action| action.name)
                .collect::<BTreeSet<_>>()
                .len(),
            ACTION_SPECS.len()
        );
        assert_eq!(
            ACTION_SPECS
                .iter()
                .filter(|action| action.parameter == ParameterSchema::String)
                .map(|action| action.name)
                .collect::<Vec<_>>(),
            [
                "select-worklane",
                "close-worklane",
                "move-pane-to-worklane",
                "open-server",
                "open-server-browser",
                "ignore-server-port",
                "stop-ignoring-server-port",
                "stop-server",
                "run-task",
                "open-with-target",
                "activate-template",
                "toggle-template-pin",
                "duplicate-template",
                "convert-template",
                "delete-template",
                "export-template"
            ]
        );
        assert_eq!(
            ACTION_SPECS
                .iter()
                .filter(|action| action.parameter == ParameterSchema::StringPair)
                .map(|action| action.name)
                .collect::<Vec<_>>(),
            [
                "rename-worklane",
                "rename-pane",
                "set-worklane-color",
                "move-worklane",
                "reorder-worklane",
                "select-pane",
                "save-template",
                "rename-template",
                "edit-template",
            ]
        );
    }

    #[test]
    fn unknown_and_wrong_parameters_are_rejected_without_fallback() {
        assert!(spec("does-not-exist").is_none());
        let string = "pane-1".to_variant();
        let pair = ("worklane-1", "pane-1").to_variant();
        let number = 7_i32.to_variant();
        assert!(ParameterSchema::None.accepts(None));
        assert!(!ParameterSchema::None.accepts(Some(&string)));
        assert!(ParameterSchema::String.accepts(Some(&string)));
        assert!(!ParameterSchema::String.accepts(None));
        assert!(!ParameterSchema::String.accepts(Some(&pair)));
        assert!(!ParameterSchema::String.accepts(Some(&number)));
        assert!(ParameterSchema::StringPair.accepts(Some(&pair)));
        assert!(!ParameterSchema::StringPair.accepts(None));
        assert!(!ParameterSchema::StringPair.accepts(Some(&string)));
        assert!(!ParameterSchema::StringPair.accepts(Some(&number)));
    }

    #[test]
    fn sensitivity_rules_cover_both_topology_dimensions() {
        assert!(Availability::Always.enabled(0, 0, 0));
        assert!(!Availability::MultipleColumns.enabled(1, 4, 4));
        assert!(Availability::MultipleColumns.enabled(2, 1, 2));
        assert!(!Availability::MultiplePanesInFocusedColumn.enabled(4, 1, 4));
        assert!(Availability::MultiplePanesInFocusedColumn.enabled(1, 2, 2));
        assert!(!Availability::MultipleWorkspacePanes.enabled(1, 1, 1));
        assert!(Availability::MultipleWorkspacePanes.enabled(1, 1, 2));

        let conditional = ACTION_SPECS
            .iter()
            .filter(|action| action.availability != Availability::Always)
            .map(|action| (action.name, action.availability))
            .collect::<Vec<_>>();
        assert_eq!(
            conditional,
            [
                (
                    "move-pane-to-new-window",
                    Availability::MultipleWorkspacePanes
                ),
                ("resize-pane-left", Availability::MultipleColumns),
                ("resize-pane-right", Availability::MultipleColumns),
                ("resize-pane-up", Availability::MultiplePanesInFocusedColumn),
                (
                    "resize-pane-down",
                    Availability::MultiplePanesInFocusedColumn
                ),
                ("arrange-golden-wide", Availability::MultipleColumns),
                ("arrange-golden-narrow", Availability::MultipleColumns),
                (
                    "arrange-golden-tall",
                    Availability::MultiplePanesInFocusedColumn
                ),
                (
                    "arrange-golden-short",
                    Availability::MultiplePanesInFocusedColumn
                ),
            ]
        );
    }

    #[test]
    fn constructed_group_rejects_missing_extra_and_wrongly_typed_actions() {
        fn group(
            skipped: Option<&str>,
            extra: Option<&str>,
            wrong_type: Option<&str>,
        ) -> gio::SimpleActionGroup {
            let group = gio::SimpleActionGroup::new();
            for spec in ACTION_SPECS {
                if skipped == Some(spec.name) {
                    continue;
                }
                let parameter = if wrong_type == Some(spec.name) {
                    Some(glib::VariantTy::STRING)
                } else {
                    match spec.parameter {
                        ParameterSchema::None => None,
                        ParameterSchema::String => Some(glib::VariantTy::STRING),
                        ParameterSchema::StringPair => {
                            Some(glib::VariantTy::new("(ss)").expect("static test type is valid"))
                        }
                    }
                };
                group.add_action(&gio::SimpleAction::new(spec.name, parameter));
            }
            if let Some(name) = extra {
                group.add_action(&gio::SimpleAction::new(name, None));
            }
            group
        }

        assert!(validate_registered_group(&group(None, None, None)).is_ok());
        assert!(validate_registered_group(&group(Some("find"), None, None)).is_err());
        assert!(validate_registered_group(&group(None, Some("untracked"), None)).is_err());
        assert!(validate_registered_group(&group(None, None, Some("rename-pane"))).is_err());
    }
}
