use std::cell::RefCell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use gtk::{gio, prelude::ActionMapExt};
use zentty_core::{CommandPaletteItem, CommandPaletteTarget, WorklaneColor};

use crate::source_ui;

use super::ApplicationShell;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ParameterSchema {
    None,
    String,
    StringPair,
    StringTriple,
    U64,
}

impl ParameterSchema {
    pub(super) fn accepts(self, parameter: Option<&glib::Variant>) -> bool {
        match self {
            Self::None => parameter.is_none(),
            Self::String => parameter.is_some_and(|value| value.str().is_some()),
            Self::StringPair => {
                parameter.is_some_and(|value| value.get::<(String, String)>().is_some())
            }
            Self::StringTriple => {
                parameter.is_some_and(|value| value.get::<(String, String, String)>().is_some())
            }
            Self::U64 => parameter.is_some_and(|value| value.get::<u64>().is_some()),
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PaletteMetadata {
    pub(super) title: &'static str,
    pub(super) subtitle: &'static str,
    pub(super) keywords: &'static str,
    pub(super) recent_eligible: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PaletteDisposition {
    Ordinary(PaletteMetadata),
    Contextual(&'static str),
    Excluded(&'static str),
}

impl PaletteDisposition {
    pub(super) const fn ordinary(
        title: &'static str,
        subtitle: &'static str,
        keywords: &'static str,
        recent_eligible: bool,
    ) -> Self {
        Self::Ordinary(PaletteMetadata {
            title,
            subtitle,
            keywords,
            recent_eligible,
        })
    }
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
    pub(super) palette: PaletteDisposition,
}

macro_rules! ordinary_action {
    ($constant:ident, $name:literal, $parameter:ident, $title:expr, $subtitle:literal, $keywords:literal, $recent:literal) => {
        ActionSpec {
            name: $name,
            parameter: ParameterSchema::$parameter,
            availability: Availability::Always,
            palette: PaletteDisposition::ordinary($title, $subtitle, $keywords, $recent),
        }
    };
    ($constant:ident, $name:literal, $parameter:ident, $availability:ident, $title:expr, $subtitle:literal, $keywords:literal, $recent:literal) => {
        ActionSpec {
            name: $name,
            parameter: ParameterSchema::$parameter,
            availability: Availability::$availability,
            palette: PaletteDisposition::ordinary($title, $subtitle, $keywords, $recent),
        }
    };
}

macro_rules! contextual_action {
    ($constant:ident, $name:literal, $parameter:ident, $owner:literal) => {
        ActionSpec {
            name: $name,
            parameter: ParameterSchema::$parameter,
            availability: Availability::Always,
            palette: PaletteDisposition::Contextual($owner),
        }
    };
    ($constant:ident, $name:literal, $parameter:ident, $availability:ident, $owner:literal) => {
        ActionSpec {
            name: $name,
            parameter: ParameterSchema::$parameter,
            availability: Availability::$availability,
            palette: PaletteDisposition::Contextual($owner),
        }
    };
}

macro_rules! excluded_action {
    ($constant:ident, $name:literal, $parameter:ident, $reason:literal) => {
        ActionSpec {
            name: $name,
            parameter: ParameterSchema::$parameter,
            availability: Availability::Always,
            palette: PaletteDisposition::Excluded($reason),
        }
    };
    ($constant:ident, $name:literal, $parameter:ident, $availability:ident, $reason:literal) => {
        ActionSpec {
            name: $name,
            parameter: ParameterSchema::$parameter,
            availability: Availability::$availability,
            palette: PaletteDisposition::Excluded($reason),
        }
    };
}

pub(super) const ACTION_ACTIVATE_ATTENTION: &str = "activate-attention";
pub(super) const ACTION_DISMISS_ATTENTION: &str = "dismiss-attention";
pub(super) const ACTION_CLEAR_ATTENTION: &str = "clear-attention";
pub(super) const ACTION_ACTIVATE_FLEET_PANE: &str = "activate-fleet-pane";
pub(super) const ACTION_SHOW_AGENT_FLEET: &str = "show-agent-fleet";
pub(super) const ACTION_QUIT_APPLICATION: &str = "quit-application";

pub(super) const ACTION_TOGGLE_SIDEBAR: &str = "toggle-sidebar";
pub(super) const ACTION_SHOW_COMMAND_PALETTE: &str = "show-command-palette";
pub(super) const ACTION_OPEN_SETTINGS: &str = "open-settings";
pub(super) const ACTION_OPEN_SETTINGS_SECTION: &str = "open-settings-section";
pub(super) const ACTION_SHOW_ABOUT: &str = "show-about";
pub(super) const ACTION_RELOAD_CONFIG: &str = "reload-config";
pub(super) const ACTION_TOGGLE_LIGHT_DARK_THEME: &str = "toggle-light-dark-theme";
pub(super) const ACTION_USE_DARK_THEME: &str = "use-dark-theme";
pub(super) const ACTION_USE_LIGHT_THEME: &str = "use-light-theme";
pub(super) const ACTION_USE_AUTO_THEME: &str = "use-auto-theme";
pub(super) const ACTION_OPEN_BOOKMARKS: &str = "open-bookmarks";
pub(super) const ACTION_JUMP_LATEST_ATTENTION: &str = "jump-latest-attention";
pub(super) const ACTION_NEW_WINDOW: &str = "new-window";
pub(super) const ACTION_CLOSE_WINDOW: &str = "close-window";
pub(super) const ACTION_TOGGLE_FULLSCREEN: &str = "toggle-fullscreen";
pub(super) const ACTION_MINIMIZE_WINDOW: &str = "minimize-window";
pub(super) const ACTION_NEW_WORKLANE: &str = "new-worklane";
pub(super) const ACTION_SELECT_WORKLANE: &str = "select-worklane";
pub(super) const ACTION_SPLIT_PANE_RIGHT: &str = "split-pane-right";
pub(super) const ACTION_NEW_PANE_RIGHT: &str = "new-pane-right";
pub(super) const ACTION_ADD_PANE_RIGHT: &str = "add-pane-right";
pub(super) const ACTION_ADD_PANE_LEFT: &str = "add-pane-left";
pub(super) const ACTION_SPLIT_PANE_BELOW: &str = "split-pane-below";
pub(super) const ACTION_CLOSE_PANE: &str = "close-pane";
pub(super) const ACTION_RENAME_WORKLANE: &str = "rename-worklane";
pub(super) const ACTION_RENAME_PANE: &str = "rename-pane";
pub(super) const ACTION_RENAME_CURRENT_WORKLANE: &str = "rename-current-worklane";
pub(super) const ACTION_RENAME_CURRENT_PANE: &str = "rename-current-pane";
pub(super) const ACTION_COPY_PANE_PATH: &str = "copy-pane-path";
pub(super) const ACTION_DUPLICATE_PANE: &str = "duplicate-pane";
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
pub(super) const ACTION_MOVE_PANE_TO_WINDOW_WORKLANE: &str = "move-pane-to-window-worklane";
pub(super) const ACTION_MOVE_PANE_TO_NEW_WORKLANE: &str = "move-pane-to-new-worklane";
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
pub(super) const ACTION_OPEN_SELECTED_SERVER: &str = "open-selected-server";
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
    ordinary_action!(
        ACTION_NEW_WINDOW,
        "new-window",
        None,
        "New Window",
        "Create another Zentty window",
        "application window",
        true
    ),
    ordinary_action!(
        ACTION_CLOSE_WINDOW,
        "close-window",
        None,
        "Close Window",
        "Close this Zentty window",
        "application window",
        true
    ),
    ordinary_action!(
        ACTION_TOGGLE_FULLSCREEN,
        "toggle-fullscreen",
        None,
        "Toggle Full Screen",
        "Enter or leave compositor-managed full screen",
        "window fullscreen f11",
        true
    ),
    ordinary_action!(
        ACTION_MINIMIZE_WINDOW,
        "minimize-window",
        None,
        "Minimize Window",
        "Minimize this window through the compositor",
        "window hide",
        true
    ),
    ordinary_action!(
        ACTION_TOGGLE_SIDEBAR,
        "toggle-sidebar",
        None,
        "Toggle Sidebar",
        "Show or hide the worklane sidebar",
        "navigation",
        true
    ),
    excluded_action!(
        ACTION_SHOW_COMMAND_PALETTE,
        "show-command-palette",
        None,
        "the open palette cannot invoke itself"
    ),
    ordinary_action!(
        ACTION_OPEN_SETTINGS,
        "open-settings",
        None,
        "Settings",
        "Open Tornado TTY settings",
        "preferences configuration general",
        true
    ),
    ordinary_action!(
        ACTION_SHOW_ABOUT,
        "show-about",
        None,
        "About Tornado TTY",
        "Build identity, documentation, source, and third-party licenses",
        "version commit license privacy trust",
        true
    ),
    contextual_action!(
        ACTION_OPEN_SETTINGS_SECTION,
        "open-settings-section",
        String,
        "settings navigation provider"
    ),
    ordinary_action!(
        ACTION_RELOAD_CONFIG,
        "reload-config",
        None,
        "Reload Configuration",
        "Reload Ghostty configuration in every existing terminal",
        "settings config ghostty refresh",
        true
    ),
    ordinary_action!(
        ACTION_TOGGLE_LIGHT_DARK_THEME,
        "toggle-light-dark-theme",
        None,
        "Toggle Light/Dark Theme",
        "Switch between the remembered light and dark themes",
        "appearance colors automatic",
        true
    ),
    ordinary_action!(
        ACTION_USE_DARK_THEME,
        "use-dark-theme",
        None,
        "Use Dark Theme",
        "Use the remembered dark terminal theme",
        "appearance colors",
        true
    ),
    ordinary_action!(
        ACTION_USE_LIGHT_THEME,
        "use-light-theme",
        None,
        "Use Light Theme",
        "Use the remembered light terminal theme",
        "appearance colors",
        true
    ),
    ordinary_action!(
        ACTION_USE_AUTO_THEME,
        "use-auto-theme",
        None,
        "Use Auto Theme",
        "Follow the Linux desktop light or dark appearance",
        "appearance colors automatic system",
        true
    ),
    ordinary_action!(
        ACTION_OPEN_BOOKMARKS,
        "open-bookmarks",
        None,
        "Show Bookmarks & Presets",
        "Open the bookmarks and presets browser",
        "bookmark preset workspace template",
        true
    ),
    ordinary_action!(
        ACTION_JUMP_LATEST_ATTENTION,
        "jump-latest-attention",
        None,
        "Jump to Latest Attention Item",
        "Focus the newest unresolved agent notification",
        "notification agent needs input approval",
        true
    ),
    contextual_action!(
        ACTION_ACTIVATE_ATTENTION,
        "activate-attention",
        StringTriple,
        "attention inbox provider"
    ),
    contextual_action!(
        ACTION_DISMISS_ATTENTION,
        "dismiss-attention",
        U64,
        "attention inbox provider"
    ),
    excluded_action!(
        ACTION_CLEAR_ATTENTION,
        "clear-attention",
        None,
        "attention inbox bulk control only"
    ),
    contextual_action!(
        ACTION_ACTIVATE_FLEET_PANE,
        "activate-fleet-pane",
        StringTriple,
        "agent fleet provider"
    ),
    ordinary_action!(
        ACTION_SHOW_AGENT_FLEET,
        "show-agent-fleet",
        None,
        "Agent Status",
        "Inspect agent activity across every Zentty window",
        "fleet waiting running idle approval",
        true
    ),
    excluded_action!(
        ACTION_QUIT_APPLICATION,
        "quit-application",
        None,
        "destructive application-global chrome action"
    ),
    ordinary_action!(
        ACTION_NEW_WORKLANE,
        "new-worklane",
        None,
        "New Worklane",
        "Create another worklane",
        "workspace lane",
        true
    ),
    contextual_action!(
        ACTION_SELECT_WORKLANE,
        "select-worklane",
        String,
        "live pane/worklane provider"
    ),
    ordinary_action!(
        ACTION_SPLIT_PANE_RIGHT,
        "split-pane-right",
        None,
        "Split Right",
        "Split the focused pane into a visible right column",
        "pane column",
        true
    ),
    ordinary_action!(
        ACTION_NEW_PANE_RIGHT,
        "new-pane-right",
        None,
        "Add Pane Right",
        "Add a pane using the adaptive visible-split or full-width policy",
        "pane column canvas adaptive",
        true
    ),
    ordinary_action!(
        ACTION_ADD_PANE_RIGHT,
        "add-pane-right",
        None,
        source_ui::ADD_PANE_RIGHT_WITHOUT_RESIZING,
        "Add a full-width pane without resizing existing columns",
        "pane column canvas horizontal scroll",
        true
    ),
    ordinary_action!(
        ACTION_ADD_PANE_LEFT,
        "add-pane-left",
        None,
        "Add Pane Left",
        "Add a full-width pane to the left of the focused column",
        "pane column canvas",
        true
    ),
    ordinary_action!(
        ACTION_SPLIT_PANE_BELOW,
        "split-pane-below",
        None,
        "New Pane Below",
        "Split the focused pane vertically",
        "pane split down",
        true
    ),
    ordinary_action!(
        ACTION_CLOSE_PANE,
        "close-pane",
        None,
        "Close Pane",
        "Close the focused pane",
        "terminal",
        true
    ),
    contextual_action!(
        ACTION_RENAME_WORKLANE,
        "rename-worklane",
        StringPair,
        "worklane context menu"
    ),
    contextual_action!(
        ACTION_RENAME_PANE,
        "rename-pane",
        StringPair,
        "pane context menu"
    ),
    ordinary_action!(
        ACTION_RENAME_CURRENT_WORKLANE,
        "rename-current-worklane",
        None,
        "Rename Worklane…",
        "Rename the active worklane",
        "workspace lane title label",
        true
    ),
    ordinary_action!(
        ACTION_RENAME_CURRENT_PANE,
        "rename-current-pane",
        None,
        "Rename Pane…",
        "Rename the focused pane",
        "terminal title label",
        true
    ),
    ordinary_action!(
        ACTION_COPY_PANE_PATH,
        "copy-pane-path",
        None,
        "Copy Path",
        "Copy the focused pane working directory",
        "clipboard directory cwd",
        true
    ),
    ordinary_action!(
        ACTION_DUPLICATE_PANE,
        "duplicate-pane",
        None,
        "Duplicate This Pane",
        "Create another pane with the same launch context",
        "terminal copy clone",
        true
    ),
    ordinary_action!(
        ACTION_CYCLE_WORKLANE_COLOR,
        "cycle-worklane-color",
        None,
        "Cycle Worklane Color",
        "Choose the next worklane identity color",
        "appearance workspace lane",
        true
    ),
    contextual_action!(
        ACTION_SET_WORKLANE_COLOR,
        "set-worklane-color",
        StringPair,
        "worklane color menu"
    ),
    contextual_action!(
        ACTION_CLOSE_WORKLANE,
        "close-worklane",
        String,
        "worklane context menu"
    ),
    ordinary_action!(
        ACTION_CLOSE_ACTIVE_WORKLANE,
        "close-active-worklane",
        None,
        source_ui::CLOSE_WORKLANE,
        "Close the active worklane and all of its panes",
        "workspace lane remove",
        true
    ),
    contextual_action!(
        ACTION_MOVE_WORKLANE,
        "move-worklane",
        StringPair,
        "worklane drag provider"
    ),
    contextual_action!(
        ACTION_REORDER_WORKLANE,
        "reorder-worklane",
        StringPair,
        "worklane reorder controls"
    ),
    ordinary_action!(
        ACTION_MOVE_WORKLANE_UP,
        "move-worklane-up",
        None,
        "Move Worklane Up",
        "Move the active worklane earlier in the sidebar",
        "reorder workspace lane",
        true
    ),
    ordinary_action!(
        ACTION_MOVE_WORKLANE_DOWN,
        "move-worklane-down",
        None,
        "Move Worklane Down",
        "Move the active worklane later in the sidebar",
        "reorder workspace lane",
        true
    ),
    ordinary_action!(
        ACTION_MOVE_PANE_LEFT,
        "move-pane-left",
        None,
        "Move Pane Left",
        "Move the focused pane one column left",
        "reorder terminal column",
        true
    ),
    ordinary_action!(
        ACTION_MOVE_PANE_RIGHT,
        "move-pane-right",
        None,
        "Move Pane Right",
        "Move the focused pane one column right",
        "reorder terminal column",
        true
    ),
    ordinary_action!(
        ACTION_MOVE_PANE_UP,
        "move-pane-up",
        None,
        "Move Pane Up",
        "Move the focused pane upward in its column",
        "reorder terminal split",
        true
    ),
    ordinary_action!(
        ACTION_MOVE_PANE_DOWN,
        "move-pane-down",
        None,
        "Move Pane Down",
        "Move the focused pane downward in its column",
        "reorder terminal split",
        true
    ),
    contextual_action!(
        ACTION_MOVE_PANE_TO_WORKLANE,
        "move-pane-to-worklane",
        String,
        "pane destination provider"
    ),
    contextual_action!(
        ACTION_MOVE_PANE_TO_WINDOW_WORKLANE,
        "move-pane-to-window-worklane",
        StringPair,
        "cross-window pane destination provider"
    ),
    ordinary_action!(
        ACTION_MOVE_PANE_TO_NEW_WORKLANE,
        "move-pane-to-new-worklane",
        None,
        MultipleWorkspacePanes,
        "Move Pane to New Worklane",
        "Move the focused pane into a new worklane",
        "pane terminal workspace detach",
        true
    ),
    ordinary_action!(
        ACTION_MOVE_PANE_TO_NEW_WINDOW,
        "move-pane-to-new-window",
        None,
        MultipleWorkspacePanes,
        source_ui::MOVE_PANE_TO_NEW_WINDOW,
        "Move the focused live terminal into a new Zentty window",
        "pane terminal window detach",
        true
    ),
    contextual_action!(
        ACTION_SELECT_PANE,
        "select-pane",
        StringPair,
        "live pane/worklane provider"
    ),
    ordinary_action!(
        ACTION_NAVIGATE_BACK,
        "navigate-back",
        None,
        "Navigate Back",
        "Return to the previously focused pane",
        "history browser previous",
        true
    ),
    ordinary_action!(
        ACTION_NAVIGATE_FORWARD,
        "navigate-forward",
        None,
        "Navigate Forward",
        "Move forward through pane focus history",
        "history browser next",
        true
    ),
    ordinary_action!(
        ACTION_NEXT_PANE,
        "next-pane",
        None,
        "Focus Next Pane",
        "Focus the next pane in sidebar order",
        "navigation terminal",
        true
    ),
    ordinary_action!(
        ACTION_PREVIOUS_PANE,
        "previous-pane",
        None,
        "Focus Previous Pane",
        "Focus the previous pane in sidebar order",
        "navigation terminal",
        true
    ),
    ordinary_action!(
        ACTION_NEXT_WORKLANE,
        "next-worklane",
        None,
        "Next Worklane",
        "Focus the next worklane",
        "navigation workspace lane",
        true
    ),
    ordinary_action!(
        ACTION_PREVIOUS_WORKLANE,
        "previous-worklane",
        None,
        "Previous Worklane",
        "Focus the previous worklane",
        "navigation workspace lane",
        true
    ),
    excluded_action!(
        ACTION_DISMISS_COMMAND_PALETTE,
        "dismiss-command-palette",
        None,
        "palette lifecycle control"
    ),
    ordinary_action!(
        ACTION_FIND,
        "find",
        None,
        "Find",
        "Search the focused terminal's real scrollback",
        "search pane terminal",
        true
    ),
    ordinary_action!(
        ACTION_USE_SELECTION_FOR_FIND,
        "use-selection-for-find",
        None,
        "Use Selection for Find",
        "Search for the focused terminal selection",
        "search pane selection terminal",
        true
    ),
    ordinary_action!(
        ACTION_FIND_NEXT,
        "find-next",
        None,
        "Find Next",
        "Select the next terminal search match",
        "search pane navigation",
        true
    ),
    ordinary_action!(
        ACTION_FIND_PREVIOUS,
        "find-previous",
        None,
        "Find Previous",
        "Select the previous terminal search match",
        "search pane navigation",
        true
    ),
    ordinary_action!(
        ACTION_COPY,
        "copy",
        None,
        source_ui::COPY,
        "Copy the focused terminal selection",
        "clipboard selection default",
        true
    ),
    ordinary_action!(
        ACTION_CLEAN_COPY,
        "clean-copy",
        None,
        "Clean Copy",
        "Copy the selection after conservative terminal-text cleanup",
        "clipboard selection format ansi prompt url path",
        true
    ),
    ordinary_action!(
        ACTION_COPY_RAW,
        "copy-raw",
        None,
        "Copy Raw",
        "Copy the selection without Zentty transformations",
        "clipboard selection original escape hatch",
        true
    ),
    ordinary_action!(
        ACTION_COPY_AS_MARKDOWN,
        "copy-as-markdown",
        None,
        "Copy as Markdown",
        "Reflow a Markdown selection while preserving its structure",
        "clipboard selection markdown format",
        true
    ),
    ordinary_action!(
        ACTION_SELECT_ALL,
        "select-all",
        None,
        source_ui::SELECT_ALL,
        "Select all text in the focused terminal",
        "terminal selection clipboard",
        true
    ),
    ordinary_action!(
        ACTION_GLOBAL_FIND,
        "global-find",
        None,
        "Global Find",
        "Search across every live pane in this window",
        "search all panes worklanes",
        true
    ),
    excluded_action!(
        ACTION_CLEAR_GLOBAL_FIND,
        "clear-global-find",
        None,
        "global-search overlay lifecycle control"
    ),
    excluded_action!(
        ACTION_GLOBAL_FIND_NEXT,
        "global-find-next",
        None,
        "global-search overlay navigation control"
    ),
    excluded_action!(
        ACTION_GLOBAL_FIND_PREVIOUS,
        "global-find-previous",
        None,
        "global-search overlay navigation control"
    ),
    ordinary_action!(
        ACTION_FOCUS_PANE_LEFT,
        "focus-pane-left",
        None,
        "Focus Left Pane",
        "Focus the neighboring column to the left",
        "navigation terminal column",
        true
    ),
    ordinary_action!(
        ACTION_FOCUS_PANE_RIGHT,
        "focus-pane-right",
        None,
        "Focus Right Pane",
        "Focus the neighboring column to the right",
        "navigation terminal column",
        true
    ),
    ordinary_action!(
        ACTION_FOCUS_PANE_UP,
        "focus-pane-up",
        None,
        "Focus Up In Column",
        "Focus the pane above in the current column",
        "navigation terminal split",
        true
    ),
    ordinary_action!(
        ACTION_FOCUS_PANE_DOWN,
        "focus-pane-down",
        None,
        "Focus Down In Column",
        "Focus the pane below in the current column",
        "navigation terminal split",
        true
    ),
    ordinary_action!(
        ACTION_RESIZE_PANE_LEFT,
        "resize-pane-left",
        None,
        MultipleColumns,
        source_ui::RESIZE_PANE_LEFT,
        "Move the focused pane's horizontal edge left by one terminal cell",
        "layout pane resize keyboard",
        true
    ),
    ordinary_action!(
        ACTION_RESIZE_PANE_RIGHT,
        "resize-pane-right",
        None,
        MultipleColumns,
        source_ui::RESIZE_PANE_RIGHT,
        "Move the focused pane's horizontal edge right by one terminal cell",
        "layout pane resize keyboard",
        true
    ),
    ordinary_action!(
        ACTION_RESIZE_PANE_UP,
        "resize-pane-up",
        None,
        MultiplePanesInFocusedColumn,
        source_ui::RESIZE_PANE_UP,
        "Move the preferred focused-pane divider up by one terminal cell",
        "layout pane resize keyboard",
        true
    ),
    ordinary_action!(
        ACTION_RESIZE_PANE_DOWN,
        "resize-pane-down",
        None,
        MultiplePanesInFocusedColumn,
        source_ui::RESIZE_PANE_DOWN,
        "Move the preferred focused-pane divider down by one terminal cell",
        "layout pane resize keyboard",
        true
    ),
    ordinary_action!(
        ACTION_ARRANGE_WIDTH_FULL,
        "arrange-width-full",
        None,
        "Arrange Width: Full Width",
        "Make every column one viewport wide",
        "layout pane columns",
        true
    ),
    ordinary_action!(
        ACTION_ARRANGE_WIDTH_HALF,
        "arrange-width-half",
        None,
        "Arrange Width: Half Width",
        "Fit two equal columns in the viewport",
        "layout pane columns",
        true
    ),
    ordinary_action!(
        ACTION_ARRANGE_WIDTH_THIRDS,
        "arrange-width-thirds",
        None,
        "Arrange Width: Thirds",
        "Fit three equal columns in the viewport",
        "layout pane columns",
        true
    ),
    ordinary_action!(
        ACTION_ARRANGE_WIDTH_QUARTERS,
        "arrange-width-quarters",
        None,
        "Arrange Width: Quarters",
        "Fit four equal columns in the viewport",
        "layout pane columns",
        true
    ),
    ordinary_action!(
        ACTION_ARRANGE_HEIGHT_FULL,
        "arrange-height-full",
        None,
        "Arrange Height: Full Height",
        "Place one pane in each column",
        "layout pane rows",
        true
    ),
    ordinary_action!(
        ACTION_ARRANGE_HEIGHT_TWO,
        "arrange-height-two",
        None,
        "Arrange Height: 2 Per Column",
        "Reflow panes two per column",
        "layout pane rows",
        true
    ),
    ordinary_action!(
        ACTION_ARRANGE_HEIGHT_THREE,
        "arrange-height-three",
        None,
        "Arrange Height: 3 Per Column",
        "Reflow panes three per column",
        "layout pane rows",
        true
    ),
    ordinary_action!(
        ACTION_ARRANGE_HEIGHT_FOUR,
        "arrange-height-four",
        None,
        "Arrange Height: 4 Per Column",
        "Reflow panes four per column",
        "layout pane rows",
        true
    ),
    ordinary_action!(
        ACTION_ARRANGE_GOLDEN_WIDE,
        "arrange-golden-wide",
        None,
        MultipleColumns,
        "Arrange Width: Golden — Focus Wide",
        "Give the focused column the larger golden share",
        "layout pane golden ratio",
        true
    ),
    ordinary_action!(
        ACTION_ARRANGE_GOLDEN_NARROW,
        "arrange-golden-narrow",
        None,
        MultipleColumns,
        "Arrange Width: Golden — Focus Narrow",
        "Give the focused column the smaller golden share",
        "layout pane golden ratio",
        true
    ),
    ordinary_action!(
        ACTION_ARRANGE_GOLDEN_TALL,
        "arrange-golden-tall",
        None,
        MultiplePanesInFocusedColumn,
        "Arrange Height: Golden — Focus Tall",
        "Give the focused pane the larger golden share",
        "layout pane golden ratio",
        true
    ),
    ordinary_action!(
        ACTION_ARRANGE_GOLDEN_SHORT,
        "arrange-golden-short",
        None,
        MultiplePanesInFocusedColumn,
        "Arrange Height: Golden — Focus Short",
        "Give the focused pane the smaller golden share",
        "layout pane golden ratio",
        true
    ),
    ordinary_action!(
        ACTION_RESET_PANE_LAYOUT,
        "reset-pane-layout",
        None,
        "Reset Pane Layout",
        "Restore default column widths and equal pane heights",
        "layout pane reset",
        true
    ),
    ordinary_action!(
        ACTION_RESTORE_CLOSED_PANE,
        "restore-closed-pane",
        None,
        source_ui::UNDO_CLOSE_PANE,
        "Reopen the most recently closed pane",
        "terminal restore reopen",
        true
    ),
    contextual_action!(
        ACTION_OPEN_SERVER,
        "open-server",
        String,
        "development-server provider"
    ),
    contextual_action!(
        ACTION_OPEN_SELECTED_SERVER,
        "open-selected-server",
        None,
        "development-server provider"
    ),
    contextual_action!(
        ACTION_OPEN_SERVER_BROWSER,
        "open-server-browser",
        String,
        "development-server provider"
    ),
    contextual_action!(
        ACTION_IGNORE_SERVER_PORT,
        "ignore-server-port",
        String,
        "development-server provider"
    ),
    ordinary_action!(
        ACTION_REFRESH_SERVERS,
        "refresh-servers",
        None,
        "Refresh Development Servers",
        "Rescan real listening processes now",
        "server browser port listener",
        true
    ),
    contextual_action!(
        ACTION_STOP_IGNORING_SERVER_PORT,
        "stop-ignoring-server-port",
        String,
        "development-server provider"
    ),
    contextual_action!(
        ACTION_STOP_SERVER,
        "stop-server",
        String,
        "development-server provider"
    ),
    contextual_action!(ACTION_RUN_TASK, "run-task", String, "task-runner provider"),
    ordinary_action!(
        ACTION_SHOW_TASK_MANAGER,
        "show-task-manager",
        None,
        "Task Manager",
        "Inspect CPU, memory, and process trees for every pane",
        "diagnostics processes performance",
        true
    ),
    ordinary_action!(
        ACTION_REFRESH_REVIEW_STATUS,
        "refresh-review-status",
        None,
        "Refresh Git and Review Status",
        "Refresh repository, branch, dirty tree, and pull-request state",
        "git github pull request ci approval conflict",
        true
    ),
    ordinary_action!(
        ACTION_OPEN_BRANCH_REMOTE,
        "open-branch-remote",
        None,
        "Open Branch on Remote",
        "Open the focused branch on its configured Git remote",
        "git github gitlab bitbucket browser",
        true
    ),
    ordinary_action!(
        ACTION_OPEN_PULL_REQUEST,
        "open-pull-request",
        None,
        "Open Pull Request",
        "Open the pull request associated with the focused branch",
        "git github review browser pr",
        true
    ),
    contextual_action!(
        ACTION_OPEN_WITH_PRIMARY,
        "open-with-primary",
        None,
        "Open With provider"
    ),
    contextual_action!(
        ACTION_OPEN_WITH_TARGET,
        "open-with-target",
        String,
        "Open With provider"
    ),
    excluded_action!(
        ACTION_SAVE_TEMPLATE,
        "save-template",
        StringPair,
        "bookmark and preset editor"
    ),
    excluded_action!(
        ACTION_ACTIVATE_TEMPLATE,
        "activate-template",
        String,
        "bookmark and preset browser"
    ),
    excluded_action!(
        ACTION_RENAME_TEMPLATE,
        "rename-template",
        StringPair,
        "bookmark and preset editor"
    ),
    excluded_action!(
        ACTION_TOGGLE_TEMPLATE_PIN,
        "toggle-template-pin",
        String,
        "bookmark and preset browser"
    ),
    excluded_action!(
        ACTION_DUPLICATE_TEMPLATE,
        "duplicate-template",
        String,
        "bookmark and preset browser"
    ),
    excluded_action!(
        ACTION_CONVERT_TEMPLATE,
        "convert-template",
        String,
        "bookmark and preset browser"
    ),
    excluded_action!(
        ACTION_DELETE_TEMPLATE,
        "delete-template",
        String,
        "bookmark and preset browser"
    ),
    excluded_action!(
        ACTION_UPDATE_LINKED_TEMPLATE,
        "update-linked-template",
        None,
        "bookmark and preset editor"
    ),
    excluded_action!(
        ACTION_UNLINK_TEMPLATE,
        "unlink-template",
        None,
        "bookmark and preset editor"
    ),
    excluded_action!(
        ACTION_IMPORT_TEMPLATE,
        "import-template",
        None,
        "bookmark and preset editor"
    ),
    excluded_action!(
        ACTION_EXPORT_TEMPLATE,
        "export-template",
        String,
        "bookmark and preset editor"
    ),
    excluded_action!(
        ACTION_EDIT_TEMPLATE,
        "edit-template",
        StringPair,
        "bookmark and preset editor"
    ),
];

pub(super) struct ActionRouter {
    group: gio::SimpleActionGroup,
    native_window_group: gio::SimpleActionGroup,
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
        let native_window_group = gio::SimpleActionGroup::new();
        populate_native_window_actions(shell, &native_window_group);
        shell
            .borrow()
            .window
            .insert_action_group("win", Some(&native_window_group));
        Ok(Self {
            group,
            native_window_group,
        })
    }

    pub(super) fn uninstall(self, window: &gtk::Window) {
        window.insert_action_group("workspace", None::<&gio::SimpleActionGroup>);
        window.insert_action_group("win", None::<&gio::SimpleActionGroup>);
        drop(self.group);
        drop(self.native_window_group);
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

    pub(super) fn ordinary_palette_items(&self) -> Vec<CommandPaletteItem> {
        ordinary_palette_items_for_group(&self.group)
            .expect("the installed ActionRouter group was validated during construction")
    }

    pub(super) fn set_enabled(&self, name: &str, enabled: bool) -> Result<(), String> {
        let action = self
            .group
            .lookup_action(name)
            .and_then(|action| action.downcast::<gio::SimpleAction>().ok())
            .ok_or_else(|| format!("cannot update unavailable registered action: {name}"))?;
        action.set_enabled(enabled);
        Ok(())
    }

    pub(super) fn set_selection_copy_enabled(&self, enabled: bool) {
        if let Some(action) = self
            .native_window_group
            .lookup_action("copy")
            .and_then(|action| action.downcast::<gio::SimpleAction>().ok())
        {
            action.set_enabled(enabled);
        }
        for name in [
            ACTION_COPY,
            ACTION_CLEAN_COPY,
            ACTION_COPY_RAW,
            ACTION_COPY_AS_MARKDOWN,
        ] {
            if let Some(action) = self
                .group
                .lookup_action(name)
                .and_then(|action| action.downcast::<gio::SimpleAction>().ok())
            {
                action.set_enabled(enabled);
            }
        }
    }

    pub(super) fn validate_palette_items(
        &self,
        items: &[CommandPaletteItem],
    ) -> Result<(), String> {
        validate_palette_items_for_group(&self.group, items)
    }
}

fn populate_native_window_actions(
    shell: &Rc<RefCell<ApplicationShell>>,
    group: &gio::SimpleActionGroup,
) {
    let copy = gio::SimpleAction::new("copy", None);
    copy.set_enabled(false);
    let weak = Rc::downgrade(shell);
    copy.connect_activate(move |_, _| {
        if let Some(shell) = weak.upgrade() {
            ApplicationShell::copy_focused_selection(
                &shell,
                super::clipboard_actions::CopyStyle::Default,
            );
        }
    });
    group.add_action(&copy);

    for (name, binding) in [
        ("paste", "paste_from_clipboard"),
        ("clear", "clear_screen"),
        ("reset", "reset"),
    ] {
        let action = gio::SimpleAction::new(name, None);
        let weak = Rc::downgrade(shell);
        action.connect_activate(move |_, _| {
            if let Some(shell) = weak.upgrade() {
                shell.borrow().perform_focused_binding_action(name, binding);
            }
        });
        group.add_action(&action);
    }

    let new_window = gio::SimpleAction::new("new-window", None);
    let weak = Rc::downgrade(shell);
    new_window.connect_activate(move |_, _| {
        if let Some(shell) = weak.upgrade() {
            shell.borrow().request_new_window();
        }
    });
    group.add_action(&new_window);

    let close = gio::SimpleAction::new("close", None);
    let weak = Rc::downgrade(shell);
    close.connect_activate(move |_, _| {
        if let Some(shell) = weak.upgrade() {
            shell.borrow().request_close_window();
        }
    });
    group.add_action(&close);
}

fn ordinary_palette_items_for_group(
    group: &gio::SimpleActionGroup,
) -> Result<Vec<CommandPaletteItem>, String> {
    ACTION_SPECS
        .iter()
        .filter_map(|spec| match spec.palette {
            PaletteDisposition::Ordinary(metadata) => Some((spec, metadata)),
            PaletteDisposition::Contextual(_) | PaletteDisposition::Excluded(_) => None,
        })
        .map(|(spec, metadata)| {
            let action = group.lookup_action(spec.name).ok_or_else(|| {
                format!(
                    "ordinary palette action is absent from the installed ActionRouter: {}",
                    spec.name
                )
            })?;
            let mut item = CommandPaletteItem::action(
                metadata.title,
                metadata.subtitle,
                metadata.keywords,
                spec.name,
            )
            .with_recent_eligibility(metadata.recent_eligible);
            item.enabled = action.is_enabled();
            Ok(item)
        })
        .collect()
}

fn validate_palette_items_for_group(
    group: &gio::SimpleActionGroup,
    items: &[CommandPaletteItem],
) -> Result<(), String> {
    for item in items {
        let (action_name, actual_schema) = match &item.target {
            CommandPaletteTarget::Pane(_) => (ACTION_SELECT_PANE, ParameterSchema::StringPair),
            CommandPaletteTarget::Action(action) => (*action, ParameterSchema::None),
            CommandPaletteTarget::ParameterizedAction { action, .. } => {
                (*action, ParameterSchema::String)
            }
            CommandPaletteTarget::TripleParameterizedAction { action, .. } => {
                (*action, ParameterSchema::StringTriple)
            }
        };
        let spec = ACTION_SPECS
            .iter()
            .find(|spec| spec.name == action_name)
            .ok_or_else(|| format!("palette item references unknown action: {action_name}"))?;
        if spec.parameter != actual_schema {
            return Err(format!(
                "palette item parameter mismatch for {action_name}: expected {:?}, got {actual_schema:?}",
                spec.parameter
            ));
        }
        if matches!(spec.palette, PaletteDisposition::Excluded(_)) {
            return Err(format!(
                "palette item references explicitly excluded action: {action_name}"
            ));
        }
        let action = group.lookup_action(action_name).ok_or_else(|| {
            format!("palette item action is absent from installed ActionRouter: {action_name}")
        })?;
        if action.is_enabled() != item.enabled {
            return Err(format!(
                "palette availability differs from installed action for {action_name}"
            ));
        }
    }
    Ok(())
}

fn populate(shell: &Rc<RefCell<ApplicationShell>>, group: &gio::SimpleActionGroup) {
    install_application_actions(shell, group);
    install_primary_ui_actions(shell, group);

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

fn install_primary_ui_actions(
    shell: &Rc<RefCell<ApplicationShell>>,
    group: &gio::SimpleActionGroup,
) {
    let toggle_sidebar = gio::SimpleAction::new(ACTION_TOGGLE_SIDEBAR, None);
    let weak = Rc::downgrade(shell);
    toggle_sidebar.connect_activate(move |_, _| {
        let Some(shell) = weak.upgrade() else { return };
        let (visible, sidebar) = {
            let mut shell = shell.borrow_mut();
            shell
                .sidebar_visibility
                .handle(super::SidebarVisibilityEvent::TogglePressed);
            shell.apply_sidebar_visibility();
            shell.config.sidebar.width = shell.preferred_sidebar_width.get();
            shell.config.sidebar.visibility = shell.sidebar_visibility.persisted_mode();
            (
                shell.sidebar_visibility.mode() != super::SidebarVisibilityMode::Hidden,
                shell.config.sidebar,
            )
        };
        super::persist_sidebar_config(sidebar, "toggle");
        let weak = Rc::downgrade(&shell);
        glib::idle_add_local_once(move || {
            if let Some(shell) = weak.upgrade() {
                shell.borrow().focus_selected_surface();
            }
        });
        eprintln!("zentty-linux: action=toggle-sidebar visible={visible}");
    });
    group.add_action(&toggle_sidebar);
    install_settings_shortcut_actions(shell, group);
    add_simple_action(shell, group, ACTION_OPEN_BOOKMARKS, |shell| {
        if !crate::bookmarks_view::open_from(shell.sidebar.upcast_ref()) {
            eprintln!("zentty-linux: action=open-bookmarks unavailable");
        }
    });
    install_attention_actions(shell, group);
    install_fleet_actions(shell, group);
    add_simple_action(shell, group, ACTION_JUMP_LATEST_ATTENTION, |shell| {
        shell.request_latest_attention();
    });
}

fn install_attention_actions(
    shell: &Rc<RefCell<ApplicationShell>>,
    group: &gio::SimpleActionGroup,
) {
    let triple = glib::VariantTy::new("(sss)").expect("static action type is valid");
    let activate = gio::SimpleAction::new(ACTION_ACTIVATE_ATTENTION, Some(triple));
    let weak = Rc::downgrade(shell);
    activate.connect_activate(move |_, parameter| {
        let (Some(shell), Some((window_id, worklane_id, pane_id))) = (
            weak.upgrade(),
            parameter.and_then(glib::Variant::get::<(String, String, String)>),
        ) else {
            return;
        };
        shell
            .borrow()
            .request_application_action(super::ApplicationAction::ActivateAttention(
                zentty_core::AttentionTarget::new(window_id, worklane_id, pane_id),
            ));
    });
    group.add_action(&activate);

    let dismiss = gio::SimpleAction::new(ACTION_DISMISS_ATTENTION, Some(glib::VariantTy::UINT64));
    let weak = Rc::downgrade(shell);
    dismiss.connect_activate(move |_, parameter| {
        let (Some(shell), Some(id)) = (
            weak.upgrade(),
            parameter.and_then(glib::Variant::get::<u64>),
        ) else {
            return;
        };
        shell
            .borrow()
            .request_application_action(super::ApplicationAction::DismissAttention(id));
    });
    group.add_action(&dismiss);

    add_simple_action(shell, group, ACTION_CLEAR_ATTENTION, |shell| {
        shell.request_application_action(super::ApplicationAction::ClearAttention);
    });
}

fn install_fleet_actions(shell: &Rc<RefCell<ApplicationShell>>, group: &gio::SimpleActionGroup) {
    add_simple_action(shell, group, ACTION_SHOW_AGENT_FLEET, |shell| {
        shell.show_agent_fleet();
    });
    let triple = glib::VariantTy::new("(sss)").expect("static action type is valid");
    let activate = gio::SimpleAction::new(ACTION_ACTIVATE_FLEET_PANE, Some(triple));
    let weak = Rc::downgrade(shell);
    activate.connect_activate(move |_, parameter| {
        let (Some(shell), Some(target)) = (weak.upgrade(), fleet_target(parameter)) else {
            return;
        };
        let activation = shell.borrow().take_window_activation();
        shell
            .borrow()
            .request_application_action(super::ApplicationAction::ActivateFleetPane {
                target,
                activation,
            });
    });
    group.add_action(&activate);

    // Quit evidence walks every live shell, including the shell that owns this
    // action. Do not enter through `add_simple_action`, whose callback holds a
    // mutable borrow of that same shell while the evidence collector runs.
    let quit = gio::SimpleAction::new(ACTION_QUIT_APPLICATION, None);
    let weak = Rc::downgrade(shell);
    quit.connect_activate(move |_, parameter| {
        if !ParameterSchema::None.accepts(parameter) {
            eprintln!("zentty-linux: action={ACTION_QUIT_APPLICATION} rejected parameter-schema");
            return;
        }
        if let Some(shell) = weak.upgrade() {
            shell.borrow().request_quit();
        }
    });
    group.add_action(&quit);
}

fn fleet_target(parameter: Option<&glib::Variant>) -> Option<zentty_core::AttentionTarget> {
    let (window_id, worklane_id, pane_id) =
        parameter.and_then(glib::Variant::get::<(String, String, String)>)?;
    if [window_id.as_str(), worklane_id.as_str(), pane_id.as_str()]
        .iter()
        .any(|value| value.trim().is_empty())
    {
        return None;
    }
    Some(zentty_core::AttentionTarget::new(
        window_id,
        worklane_id,
        pane_id,
    ))
}

fn install_settings_shortcut_actions(
    shell: &Rc<RefCell<ApplicationShell>>,
    group: &gio::SimpleActionGroup,
) {
    add_simple_action(shell, group, ACTION_SHOW_COMMAND_PALETTE, |shell| {
        shell.toggle_command_palette();
    });
    add_simple_action(shell, group, ACTION_SHOW_ABOUT, |shell| {
        shell.request_show_about();
    });
    add_simple_action(shell, group, ACTION_RELOAD_CONFIG, |shell| {
        shell.reload_ghostty_config();
    });
    add_simple_action(shell, group, ACTION_TOGGLE_LIGHT_DARK_THEME, |shell| {
        let _ = shell.apply_theme_mode_command(zentty_core::ThemeModeCommand::Toggle);
    });
    add_simple_action(shell, group, ACTION_USE_DARK_THEME, |shell| {
        let _ = shell.apply_theme_mode_command(zentty_core::ThemeModeCommand::Dark);
    });
    add_simple_action(shell, group, ACTION_USE_LIGHT_THEME, |shell| {
        let _ = shell.apply_theme_mode_command(zentty_core::ThemeModeCommand::Light);
    });
    add_simple_action(shell, group, ACTION_USE_AUTO_THEME, |shell| {
        let _ = shell.apply_theme_mode_command(zentty_core::ThemeModeCommand::Automatic);
    });
    let open_settings = gio::SimpleAction::new(ACTION_OPEN_SETTINGS, None);
    let weak = Rc::downgrade(shell);
    open_settings.connect_activate(move |_, parameter| {
        if !ParameterSchema::None.accepts(parameter) {
            eprintln!("zentty-linux: action={ACTION_OPEN_SETTINGS} rejected parameter-schema");
            return;
        }
        // Defer secondary-Wayland-toplevel presentation beyond the originating
        // physical key event so the compositor can transfer activation.
        let weak = weak.clone();
        glib::idle_add_local_once(move || {
            if let Some(shell) = weak.upgrade() {
                shell.borrow_mut().request_show_shortcut_settings();
            }
        });
    });
    group.add_action(&open_settings);
    let open_settings_section = gio::SimpleAction::new(
        ACTION_OPEN_SETTINGS_SECTION,
        Some(&String::static_variant_type()),
    );
    let weak = Rc::downgrade(shell);
    open_settings_section.connect_activate(move |_, parameter| {
        let Some(value) = parameter.and_then(glib::Variant::str) else {
            eprintln!(
                "zentty-linux: action={ACTION_OPEN_SETTINGS_SECTION} rejected parameter-schema"
            );
            return;
        };
        let Some(section) = crate::settings_navigation::SettingsSection::parse(value) else {
            eprintln!(
                "zentty-linux: action={ACTION_OPEN_SETTINGS_SECTION} rejected section={value:?}"
            );
            return;
        };
        let weak = weak.clone();
        glib::idle_add_local_once(move || {
            if let Some(shell) = weak.upgrade() {
                shell.borrow_mut().request_show_settings(section);
            }
        });
    });
    group.add_action(&open_settings_section);
    add_simple_action(shell, group, ACTION_RENAME_CURRENT_WORKLANE, |shell| {
        shell.request_rename_current_worklane();
    });
    add_simple_action(shell, group, ACTION_RENAME_CURRENT_PANE, |shell| {
        shell.request_rename_current_pane();
    });
    add_simple_action(shell, group, ACTION_COPY_PANE_PATH, |shell| {
        shell.copy_focused_pane_path();
    });
    let duplicate_pane = gio::SimpleAction::new(ACTION_DUPLICATE_PANE, None);
    let weak = Rc::downgrade(shell);
    duplicate_pane.connect_activate(move |_, parameter| {
        if !ParameterSchema::None.accepts(parameter) {
            eprintln!("zentty-linux: action={ACTION_DUPLICATE_PANE} rejected parameter-schema");
            return;
        }
        let Some(shell) = weak.upgrade() else { return };
        if let Err(error) = ApplicationShell::duplicate_focused_pane(&shell) {
            ApplicationShell::report_action_error(&shell, ACTION_DUPLICATE_PANE, &error);
        }
    });
    group.add_action(&duplicate_pane);
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
    let selected = gio::SimpleAction::new(ACTION_OPEN_SELECTED_SERVER, None);
    let weak = Rc::downgrade(shell);
    selected.connect_activate(move |_, parameter| {
        if !ParameterSchema::None.accepts(parameter) {
            eprintln!("zentty-linux: action=open-selected-server rejected parameter-schema");
            return;
        }
        if let Some(shell) = weak.upgrade() {
            super::server_runtime::open_selected_server(&shell);
        }
    });
    group.add_action(&selected);

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

    add_simple_action(shell, group, ACTION_TOGGLE_FULLSCREEN, |shell| {
        shell.toggle_fullscreen();
    });
    add_simple_action(shell, group, ACTION_MINIMIZE_WINDOW, |shell| {
        shell.minimize_window();
    });

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
        let changed = shell
            .borrow_mut()
            .apply_worklane_title_operation(&worklane_id, Some(&title));
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
        if shell.apply_worklane_color_operation(&worklane_id, color) {
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
            let order = shell
                .state
                .worklanes()
                .iter()
                .map(|worklane| worklane.id.clone())
                .collect::<Vec<_>>();
            eprintln!(
                "zentty-linux: action=reorder-worklane id={worklane_id} insertion={insertion_index} order={} active={} pane={}",
                order.join(","),
                shell.state.active_worklane_id(),
                shell.state.focused_pane_id().unwrap_or("none")
            );
            let projected = crate::sidebar::project_worklane_order(&shell.sidebar, &order);
            eprintln!(
                "zentty-linux: worklane-order-projected order={} matches-model={projected}",
                crate::sidebar::rendered_worklane_order(&shell.sidebar).join(",")
            );
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
        let changed = shell
            .borrow_mut()
            .apply_pane_title_operation(&pane_id, Some(&title));
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

    let string_pair = glib::VariantTy::new("(ss)").expect("static action type is valid");
    let move_pane = gio::SimpleAction::new(ACTION_MOVE_PANE_TO_WINDOW_WORKLANE, Some(string_pair));
    let weak = Rc::downgrade(shell);
    move_pane.connect_activate(move |_, parameter| {
        let (Some(shell), Some((destination_window_id, destination_worklane_id))) = (
            weak.upgrade(),
            parameter.and_then(glib::Variant::get::<(String, String)>),
        ) else {
            return;
        };
        shell
            .borrow()
            .request_move_pane_to_window_worklane(destination_window_id, destination_worklane_id);
    });
    group.add_action(&move_pane);

    let move_to_new_worklane = gio::SimpleAction::new(ACTION_MOVE_PANE_TO_NEW_WORKLANE, None);
    let weak = Rc::downgrade(shell);
    move_to_new_worklane.connect_activate(move |_, _| {
        let Some(shell) = weak.upgrade() else {
            return;
        };
        let mut shell = shell.borrow_mut();
        let pane_id = shell.state.focused_pane_id().map(str::to_owned);
        let worklane_id = shell.take_worklane_id();
        let placement = shell.config.worklanes.new_worklane_placement;
        let width = f64::from(shell.pane_viewport_width());
        if shell
            .state
            .isolate_focused_pane_in_new_worklane(worklane_id.clone(), placement, width)
        {
            eprintln!(
                "zentty-linux: action=move-pane-to-new-worklane pane={} target={worklane_id}",
                pane_id.as_deref().unwrap_or("unknown")
            );
            shell.render();
            shell.focus_selected_surface();
        }
    });
    group.add_action(&move_to_new_worklane);
}

fn install_pane_creation_actions(
    shell: &Rc<RefCell<ApplicationShell>>,
    group: &gio::SimpleActionGroup,
) {
    let new_pane = gio::SimpleAction::new(ACTION_NEW_PANE_RIGHT, None);
    let weak = Rc::downgrade(shell);
    new_pane.connect_activate(move |_, _| {
        let Some(shell) = weak.upgrade() else {
            return;
        };
        if let Err(error) = ApplicationShell::create_focused_pane_right(&shell) {
            ApplicationShell::report_action_error(&shell, ACTION_NEW_PANE_RIGHT, &error);
        }
    });
    group.add_action(&new_pane);

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
    for (name, direction) in [
        (
            ACTION_FOCUS_PANE_LEFT,
            super::application_commands::PaneFocusDirection::Left,
        ),
        (
            ACTION_FOCUS_PANE_RIGHT,
            super::application_commands::PaneFocusDirection::Right,
        ),
        (
            ACTION_FOCUS_PANE_UP,
            super::application_commands::PaneFocusDirection::Up,
        ),
        (
            ACTION_FOCUS_PANE_DOWN,
            super::application_commands::PaneFocusDirection::Down,
        ),
    ] {
        add_simple_action(shell, group, name, move |shell| {
            if let Err(message) = shell.apply_focus_operation(Some(direction)) {
                eprintln!("zentty-linux: action={name} result=unchanged detail={message}");
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
            ParameterSchema::StringTriple => Some("(sss)".to_owned()),
            ParameterSchema::U64 => Some("t".to_owned()),
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
    use gtk::prelude::{ActionMapExt, Cast};
    use gtk::{gio, glib};
    use zentty_core::{CommandPaletteItem, CommandPaletteTarget};

    use super::{
        ACTION_ADD_PANE_LEFT, ACTION_ADD_PANE_RIGHT, ACTION_DISMISS_COMMAND_PALETTE,
        ACTION_NEW_PANE_RIGHT, ACTION_NEW_WINDOW, ACTION_OPEN_SETTINGS_SECTION, ACTION_SELECT_PANE,
        ACTION_SPECS, ACTION_SPLIT_PANE_RIGHT, ActionSpec, Availability, PaletteDisposition,
        ParameterSchema, fleet_target, ordinary_palette_items_for_group,
        validate_palette_items_for_group, validate_registered_group,
    };

    fn spec(name: &str) -> Option<&'static ActionSpec> {
        ACTION_SPECS.iter().find(|candidate| candidate.name == name)
    }

    #[test]
    fn registry_is_unique_complete_and_typed() {
        assert_eq!(ACTION_SPECS.len(), 121);
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
                "open-settings-section",
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
                "move-pane-to-window-worklane",
                "select-pane",
                "save-template",
                "rename-template",
                "edit-template",
            ]
        );
    }

    #[test]
    fn every_registered_action_has_explicit_palette_policy() {
        let mut classified = 0;
        for spec in ACTION_SPECS {
            match spec.palette {
                PaletteDisposition::Ordinary(metadata) => {
                    classified += 1;
                    assert_eq!(spec.parameter, ParameterSchema::None, "{}", spec.name);
                    assert!(!metadata.title.trim().is_empty(), "{}", spec.name);
                    assert!(!metadata.subtitle.trim().is_empty(), "{}", spec.name);
                    assert!(!metadata.keywords.trim().is_empty(), "{}", spec.name);
                }
                PaletteDisposition::Contextual(owner) | PaletteDisposition::Excluded(owner) => {
                    classified += 1;
                    assert!(!owner.trim().is_empty(), "{}", spec.name);
                }
            }
        }
        assert_eq!(classified, ACTION_SPECS.len());
    }

    #[test]
    fn ordinary_palette_projection_uses_registered_actions_and_live_enabled_state() {
        let group = gio::SimpleActionGroup::new();
        for spec in ACTION_SPECS {
            let parameter = match spec.parameter {
                ParameterSchema::None => None,
                ParameterSchema::String => Some(glib::VariantTy::STRING),
                ParameterSchema::StringPair => {
                    Some(glib::VariantTy::new("(ss)").expect("static type is valid"))
                }
                ParameterSchema::StringTriple => {
                    Some(glib::VariantTy::new("(sss)").expect("static type is valid"))
                }
                ParameterSchema::U64 => Some(glib::VariantTy::UINT64),
            };
            let action = gio::SimpleAction::new(spec.name, parameter);
            if spec.name == "move-pane-to-new-window" {
                action.set_enabled(false);
            }
            group.add_action(&action);
        }

        let items = ordinary_palette_items_for_group(&group).unwrap();
        let titles = items
            .iter()
            .map(|item| item.title.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(titles.len(), items.len(), "palette titles must be unique");
        assert!(titles.contains("Reload Configuration"));
        assert!(titles.contains("Show Bookmarks & Presets"));
        assert!(titles.contains("Duplicate This Pane"));
        let item = |title: &str| items.iter().find(|item| item.title == title).unwrap();
        assert_eq!(
            item("Split Right").target,
            CommandPaletteTarget::Action(ACTION_SPLIT_PANE_RIGHT)
        );
        assert_eq!(
            item("Add Pane Right").target,
            CommandPaletteTarget::Action(ACTION_NEW_PANE_RIGHT)
        );
        assert!(item("Add Pane Right").subtitle.contains("adaptive"));
        assert_eq!(
            item("Add Pane Right Without Resizing").target,
            CommandPaletteTarget::Action(ACTION_ADD_PANE_RIGHT)
        );
        assert!(
            item("Add Pane Right Without Resizing")
                .subtitle
                .contains("without resizing")
        );
        assert_eq!(
            item("Add Pane Left").target,
            CommandPaletteTarget::Action(ACTION_ADD_PANE_LEFT)
        );
        assert!(
            !items
                .iter()
                .find(|item| item.title == "Move Pane to New Window")
                .unwrap()
                .enabled
        );

        group.remove_action("new-window");
        assert!(ordinary_palette_items_for_group(&group).is_err());
    }

    #[test]
    fn contextual_palette_targets_are_registered_typed_enabled_and_not_excluded() {
        let group = gio::SimpleActionGroup::new();
        for spec in ACTION_SPECS {
            let parameter = match spec.parameter {
                ParameterSchema::None => None,
                ParameterSchema::String => Some(glib::VariantTy::STRING),
                ParameterSchema::StringPair => {
                    Some(glib::VariantTy::new("(ss)").expect("static type is valid"))
                }
                ParameterSchema::StringTriple => {
                    Some(glib::VariantTy::new("(sss)").expect("static type is valid"))
                }
                ParameterSchema::U64 => Some(glib::VariantTy::UINT64),
            };
            group.add_action(&gio::SimpleAction::new(spec.name, parameter));
        }
        let valid = CommandPaletteItem::parameterized_action(
            "General Settings",
            "Jump to General",
            "settings",
            ACTION_OPEN_SETTINGS_SECTION,
            "general",
        );
        assert!(validate_palette_items_for_group(&group, &[valid]).is_ok());

        let wrong_schema = CommandPaletteItem::parameterized_action(
            "Pane",
            "Wrong schema",
            "pane",
            ACTION_SELECT_PANE,
            "pane-1",
        );
        assert!(validate_palette_items_for_group(&group, &[wrong_schema]).is_err());

        let excluded = CommandPaletteItem::action(
            "Dismiss",
            "Excluded lifecycle action",
            "dismiss",
            ACTION_DISMISS_COMMAND_PALETTE,
        );
        assert!(validate_palette_items_for_group(&group, &[excluded]).is_err());

        let disabled_action = group
            .lookup_action(ACTION_NEW_WINDOW)
            .unwrap()
            .downcast::<gio::SimpleAction>()
            .unwrap();
        disabled_action.set_enabled(false);
        let stale = CommandPaletteItem::action(
            "New Window",
            "Create another window",
            "window",
            ACTION_NEW_WINDOW,
        );
        assert!(validate_palette_items_for_group(&group, &[stale]).is_err());
    }

    #[test]
    fn fleet_target_rejects_missing_wrong_and_blank_typed_parameters() {
        assert!(fleet_target(None).is_none());
        let wrong = "pane".to_variant();
        assert!(fleet_target(Some(&wrong)).is_none());
        for value in [
            ("", "worklane", "pane"),
            ("window", " ", "pane"),
            ("window", "worklane", "\n"),
        ] {
            let value = value.to_variant();
            assert!(fleet_target(Some(&value)).is_none());
        }
        let valid = ("window", "worklane", "pane").to_variant();
        assert_eq!(
            fleet_target(Some(&valid)),
            Some(zentty_core::AttentionTarget::new(
                "window", "worklane", "pane"
            ))
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
                    "move-pane-to-new-worklane",
                    Availability::MultipleWorkspacePanes
                ),
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
                        ParameterSchema::StringTriple => {
                            Some(glib::VariantTy::new("(sss)").expect("static test type is valid"))
                        }
                        ParameterSchema::U64 => Some(glib::VariantTy::UINT64),
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
