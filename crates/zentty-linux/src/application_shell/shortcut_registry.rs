use std::collections::HashSet;

use zentty_core::{KeyboardShortcut, ShortcutDefinition};

#[allow(clippy::wildcard_imports)]
// This registry intentionally mirrors the authoritative action schema.
use super::action_router::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ShortcutCategory {
    General,
    Worklanes,
    Panes,
    Notifications,
}

impl ShortcutCategory {
    pub(crate) const ALL: [Self; 4] = [
        Self::General,
        Self::Worklanes,
        Self::Panes,
        Self::Notifications,
    ];

    pub(crate) const fn title(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Worklanes => "Worklanes",
            Self::Panes => "Panes",
            Self::Notifications => "Notifications",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ShortcutCommandSpec {
    pub(crate) command_id: &'static str,
    pub(crate) title: &'static str,
    pub(crate) description: &'static str,
    pub(crate) category: ShortcutCategory,
    pub(crate) action: &'static str,
    pub(crate) default: Option<&'static str>,
}

macro_rules! command {
    ($id:literal, $title:literal, $description:literal, $category:ident, $action:ident, $default:expr) => {
        ShortcutCommandSpec {
            command_id: $id,
            title: $title,
            description: $description,
            category: ShortcutCategory::$category,
            action: $action,
            default: $default,
        }
    };
}

/// Source command IDs remain stable for TOML interchange with Zentty on macOS.
/// Command maps to Linux Control, Control maps to Super, and Option maps to Alt.
pub(crate) const COMMANDS: &[ShortcutCommandSpec] = &[
    command!(
        "sidebar.toggle",
        "Toggle Sidebar",
        "Show or hide the sidebar.",
        General,
        ACTION_TOGGLE_SIDEBAR,
        Some("command+s")
    ),
    command!(
        "command_palette.show",
        "Command Palette",
        "Search and run Zentty commands.",
        General,
        ACTION_SHOW_COMMAND_PALETTE,
        Some("command+shift+p")
    ),
    command!(
        "app.open_settings",
        "Open Settings",
        "Open Zentty settings.",
        General,
        ACTION_OPEN_SETTINGS,
        Some("command+,")
    ),
    command!(
        "app.reload_config",
        "Reload Configuration",
        "Reload Ghostty configuration in every existing terminal.",
        General,
        ACTION_RELOAD_CONFIG,
        None
    ),
    command!(
        "theme.toggle_light_dark",
        "Toggle Light/Dark Theme",
        "Switch between the remembered light and dark themes.",
        General,
        ACTION_TOGGLE_LIGHT_DARK_THEME,
        None
    ),
    command!(
        "theme.use_dark",
        "Use Dark Theme",
        "Use the remembered dark terminal theme.",
        General,
        ACTION_USE_DARK_THEME,
        None
    ),
    command!(
        "theme.use_light",
        "Use Light Theme",
        "Use the remembered light terminal theme.",
        General,
        ACTION_USE_LIGHT_THEME,
        None
    ),
    command!(
        "theme.use_auto",
        "Use Auto Theme",
        "Follow the Linux desktop light or dark appearance.",
        General,
        ACTION_USE_AUTO_THEME,
        None
    ),
    command!(
        "bookmarks.openPopover",
        "Show Bookmarks & Presets",
        "Open the bookmarks and presets browser.",
        General,
        ACTION_OPEN_BOOKMARKS,
        Some("command+shift+b")
    ),
    command!(
        "notifications.jump_latest",
        "Jump To Latest Attention Item",
        "Focus the most recently updated pane that needs input.",
        Notifications,
        ACTION_JUMP_LATEST_ATTENTION,
        Some("command+shift+u")
    ),
    command!(
        "navigate.back",
        "Navigate Back",
        "Go back to the pane you were in before.",
        General,
        ACTION_NAVIGATE_BACK,
        Some("command+[")
    ),
    command!(
        "navigate.forward",
        "Navigate Forward",
        "Go forward again after navigating back.",
        General,
        ACTION_NAVIGATE_FORWARD,
        Some("command+]")
    ),
    command!(
        "worklane.new",
        "New Worklane",
        "Create and focus a new worklane.",
        Worklanes,
        ACTION_NEW_WORKLANE,
        Some("command+t")
    ),
    command!(
        "worklane.rename",
        "Rename Worklane",
        "Give the active worklane a custom name, or clear it.",
        Worklanes,
        ACTION_RENAME_CURRENT_WORKLANE,
        None
    ),
    command!(
        "worklane.next",
        "Next Worklane",
        "Select the next worklane. Control+Tab keeps its hold-to-peek behavior.",
        Worklanes,
        ACTION_NEXT_WORKLANE,
        None
    ),
    command!(
        "worklane.previous",
        "Previous Worklane",
        "Select the previous worklane. Control+Shift+Tab keeps its hold-to-peek behavior.",
        Worklanes,
        ACTION_PREVIOUS_WORKLANE,
        None
    ),
    command!(
        "worklane.move_up",
        "Move Worklane Up",
        "Move the current worklane upward.",
        Worklanes,
        ACTION_MOVE_WORKLANE_UP,
        Some("command+control+up")
    ),
    command!(
        "worklane.move_down",
        "Move Worklane Down",
        "Move the current worklane downward.",
        Worklanes,
        ACTION_MOVE_WORKLANE_DOWN,
        Some("command+control+down")
    ),
    command!(
        "pane.search.find",
        "Find",
        "Search in the focused pane.",
        Panes,
        ACTION_FIND,
        Some("command+f")
    ),
    command!(
        "window.search.find",
        "Global Find",
        "Search across all panes.",
        Panes,
        ACTION_GLOBAL_FIND,
        Some("command+shift+f")
    ),
    command!(
        "pane.search.selection",
        "Use Selection for Find",
        "Search for the focused pane selection.",
        Panes,
        ACTION_USE_SELECTION_FOR_FIND,
        Some("command+e")
    ),
    command!(
        "pane.search.next",
        "Find Next",
        "Select the next pane-search result.",
        Panes,
        ACTION_FIND_NEXT,
        Some("command+g")
    ),
    command!(
        "pane.search.previous",
        "Find Previous",
        "Select the previous pane-search result.",
        Panes,
        ACTION_FIND_PREVIOUS,
        Some("command+shift+g")
    ),
    command!(
        "pane.rename",
        "Rename Pane",
        "Give the focused pane a custom name, or clear it.",
        Panes,
        ACTION_RENAME_CURRENT_PANE,
        None
    ),
    command!(
        "pane.copy_path",
        "Copy Path",
        "Copy the working directory path from the focused pane.",
        Panes,
        ACTION_COPY_PANE_PATH,
        Some("command+shift+c")
    ),
    command!(
        "pane.duplicate",
        "Duplicate This Pane",
        "Duplicate the focused pane in a new column, keeping its working directory.",
        Panes,
        ACTION_DUPLICATE_PANE,
        None
    ),
    command!(
        "clipboard.clean_copy",
        "Clean Copy",
        "Copy after cleaning terminal artifacts.",
        General,
        ACTION_CLEAN_COPY,
        Some("command+control+c")
    ),
    command!(
        "clipboard.copy_raw",
        "Copy Raw",
        "Copy the terminal selection unchanged.",
        General,
        ACTION_COPY_RAW,
        None
    ),
    command!(
        "clipboard.copy_markdown",
        "Copy as Markdown",
        "Copy a Markdown-formatted selection.",
        General,
        ACTION_COPY_AS_MARKDOWN,
        None
    ),
    command!(
        "pane.move_to_new_window",
        "Move Pane to New Window",
        "Move the focused pane to a new window.",
        Panes,
        ACTION_MOVE_PANE_TO_NEW_WINDOW,
        None
    ),
    command!(
        "pane.split.horizontal",
        "Add Pane Right",
        "Add a pane to the right using the normal layout policy.",
        Panes,
        ACTION_ADD_PANE_RIGHT,
        Some("command+d")
    ),
    command!(
        "pane.split.right.force",
        "Split Right Visibly",
        "Force a visible split to the right.",
        Panes,
        ACTION_SPLIT_PANE_RIGHT,
        None
    ),
    command!(
        "pane.add_right.force",
        "Add Pane Right Without Resizing",
        "Add a right-hand pane without resizing columns.",
        Panes,
        ACTION_ADD_PANE_RIGHT,
        None
    ),
    command!(
        "pane.split.vertical",
        "New Pane Below",
        "Split the focused column below the active pane.",
        Panes,
        ACTION_SPLIT_PANE_BELOW,
        Some("command+shift+d")
    ),
    command!(
        "pane.arrange.width.full",
        "Arrange Width: Full Width",
        "Make each column full width.",
        Panes,
        ACTION_ARRANGE_WIDTH_FULL,
        Some("command+1")
    ),
    command!(
        "pane.arrange.width.halves",
        "Arrange Width: Half Width",
        "Arrange columns at half width.",
        Panes,
        ACTION_ARRANGE_WIDTH_HALF,
        Some("command+2")
    ),
    command!(
        "pane.arrange.width.thirds",
        "Arrange Width: Thirds",
        "Arrange columns in thirds.",
        Panes,
        ACTION_ARRANGE_WIDTH_THIRDS,
        Some("command+3")
    ),
    command!(
        "pane.arrange.width.quarters",
        "Arrange Width: Quarters",
        "Arrange columns in quarters.",
        Panes,
        ACTION_ARRANGE_WIDTH_QUARTERS,
        Some("command+4")
    ),
    command!(
        "pane.arrange.height.full",
        "Arrange Height: Full Height",
        "Make panes full column height.",
        Panes,
        ACTION_ARRANGE_HEIGHT_FULL,
        Some("command+option+1")
    ),
    command!(
        "pane.arrange.height.two_per_column",
        "Arrange Height: 2 Per Column",
        "Arrange two panes per column.",
        Panes,
        ACTION_ARRANGE_HEIGHT_TWO,
        Some("command+option+2")
    ),
    command!(
        "pane.arrange.height.three_per_column",
        "Arrange Height: 3 Per Column",
        "Arrange three panes per column.",
        Panes,
        ACTION_ARRANGE_HEIGHT_THREE,
        Some("command+option+3")
    ),
    command!(
        "pane.arrange.height.four_per_column",
        "Arrange Height: 4 Per Column",
        "Arrange four panes per column.",
        Panes,
        ACTION_ARRANGE_HEIGHT_FOUR,
        Some("command+option+4")
    ),
    command!(
        "pane.arrange.width.golden_focus_wide",
        "Arrange Width: Golden — Focus Wide",
        "Give the focused column the wide golden-ratio share.",
        Panes,
        ACTION_ARRANGE_GOLDEN_WIDE,
        Some("command+control+g")
    ),
    command!(
        "pane.arrange.width.golden_focus_narrow",
        "Arrange Width: Golden — Focus Narrow",
        "Give the focused column the narrow golden-ratio share.",
        Panes,
        ACTION_ARRANGE_GOLDEN_NARROW,
        Some("command+control+option+g")
    ),
    command!(
        "pane.arrange.height.golden_focus_tall",
        "Arrange Height: Golden — Focus Tall",
        "Give the focused pane the tall golden-ratio share.",
        Panes,
        ACTION_ARRANGE_GOLDEN_TALL,
        Some("command+control+shift+g")
    ),
    command!(
        "pane.arrange.height.golden_focus_short",
        "Arrange Height: Golden — Focus Short",
        "Give the focused pane the short golden-ratio share.",
        Panes,
        ACTION_ARRANGE_GOLDEN_SHORT,
        Some("command+control+option+shift+g")
    ),
    command!(
        "pane.close_focused",
        "Close Pane",
        "Close the focused pane.",
        Panes,
        ACTION_CLOSE_PANE,
        Some("command+w")
    ),
    command!(
        "pane.restore_closed",
        "Undo Close Pane",
        "Restore the most recently closed pane.",
        Panes,
        ACTION_RESTORE_CLOSED_PANE,
        Some("command+shift+t")
    ),
    command!(
        "pane.focus.previous",
        "Focus Previous Pane",
        "Focus the previous pane in traversal order.",
        Panes,
        ACTION_PREVIOUS_PANE,
        Some("command+option+up")
    ),
    command!(
        "pane.focus.next",
        "Focus Next Pane",
        "Focus the next pane in traversal order.",
        Panes,
        ACTION_NEXT_PANE,
        Some("command+option+down")
    ),
    command!(
        "pane.focus.left",
        "Focus Left Pane",
        "Focus the pane to the left.",
        Panes,
        ACTION_FOCUS_PANE_LEFT,
        Some("command+left")
    ),
    command!(
        "pane.focus.right",
        "Focus Right Pane",
        "Focus the pane to the right.",
        Panes,
        ACTION_FOCUS_PANE_RIGHT,
        Some("command+right")
    ),
    command!(
        "pane.focus.up",
        "Focus Up In Column",
        "Focus the pane above.",
        Panes,
        ACTION_FOCUS_PANE_UP,
        Some("command+up")
    ),
    command!(
        "pane.focus.down",
        "Focus Down In Column",
        "Focus the pane below.",
        Panes,
        ACTION_FOCUS_PANE_DOWN,
        Some("command+down")
    ),
    command!(
        "pane.move.left",
        "Move Pane Left",
        "Move the focused pane left.",
        Panes,
        ACTION_MOVE_PANE_LEFT,
        Some("command+control+option+left")
    ),
    command!(
        "pane.move.right",
        "Move Pane Right",
        "Move the focused pane right.",
        Panes,
        ACTION_MOVE_PANE_RIGHT,
        Some("command+control+option+right")
    ),
    command!(
        "pane.move.up",
        "Move Pane Up",
        "Move the focused pane up.",
        Panes,
        ACTION_MOVE_PANE_UP,
        Some("command+control+option+up")
    ),
    command!(
        "pane.move.down",
        "Move Pane Down",
        "Move the focused pane down.",
        Panes,
        ACTION_MOVE_PANE_DOWN,
        Some("command+control+option+down")
    ),
    command!(
        "pane.resize.left",
        "Resize Pane Left",
        "Resize the focused pane left by one terminal cell.",
        Panes,
        ACTION_RESIZE_PANE_LEFT,
        Some("command+option+shift+left")
    ),
    command!(
        "pane.resize.right",
        "Resize Pane Right",
        "Resize the focused pane right by one terminal cell.",
        Panes,
        ACTION_RESIZE_PANE_RIGHT,
        Some("command+option+shift+right")
    ),
    command!(
        "pane.resize.up",
        "Resize Pane Up",
        "Resize the focused pane up by one terminal cell.",
        Panes,
        ACTION_RESIZE_PANE_UP,
        Some("command+option+shift+up")
    ),
    command!(
        "pane.resize.down",
        "Resize Pane Down",
        "Resize the focused pane down by one terminal cell.",
        Panes,
        ACTION_RESIZE_PANE_DOWN,
        Some("command+option+shift+down")
    ),
    command!(
        "pane.reset_layout",
        "Reset Pane Layout",
        "Reset pane sizes to the current layout policy.",
        Panes,
        ACTION_RESET_PANE_LAYOUT,
        Some("command+control+option+0")
    ),
    command!(
        "task_manager.show",
        "Task Manager",
        "Show running pane process trees.",
        General,
        ACTION_SHOW_TASK_MANAGER,
        None
    ),
    command!(
        "branch.open_remote",
        "Open Branch on Remote",
        "Open the current branch on its Git host.",
        General,
        ACTION_OPEN_BRANCH_REMOTE,
        None
    ),
    command!(
        "review.refresh_status",
        "Refresh PR Status",
        "Refresh pull-request status for the focused project.",
        General,
        ACTION_REFRESH_REVIEW_STATUS,
        None
    ),
    command!(
        "app.new_window",
        "New Window",
        "Create a new Zentty window.",
        General,
        ACTION_NEW_WINDOW,
        Some("command+shift+n")
    ),
    command!(
        "app.close_window",
        "Close Window",
        "Close the active Zentty window.",
        General,
        ACTION_CLOSE_WINDOW,
        Some("command+shift+w")
    ),
];

pub(crate) fn definitions() -> Vec<ShortcutDefinition> {
    COMMANDS
        .iter()
        .map(|command| ShortcutDefinition {
            command_id: command.command_id,
            default_shortcut: command.default.and_then(KeyboardShortcut::parse),
        })
        .collect()
}

pub(crate) fn validate() -> Result<(), String> {
    let action_specs = ACTION_SPECS
        .iter()
        .map(|spec| spec.name)
        .collect::<HashSet<_>>();
    let mut command_ids = HashSet::new();
    let mut actions = HashSet::new();
    for command in COMMANDS {
        if !command_ids.insert(command.command_id) {
            return Err(format!(
                "duplicate shortcut command ID: {}",
                command.command_id
            ));
        }
        if !action_specs.contains(command.action) {
            return Err(format!(
                "shortcut command {} references unknown action {}",
                command.command_id, command.action
            ));
        }
        if command
            .default
            .is_some_and(|value| KeyboardShortcut::parse(value).is_none())
        {
            return Err(format!(
                "invalid default shortcut for {}",
                command.command_id
            ));
        }
        actions.insert(command.action);
    }
    zentty_core::ShortcutManager::new(&definitions(), &[]).map(|_| ())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_ids_defaults_and_action_references_are_valid() {
        validate().unwrap();
    }

    #[test]
    fn source_categories_remain_complete_and_ordered() {
        for category in ShortcutCategory::ALL {
            assert!(
                COMMANDS.iter().any(|command| command.category == category)
                    || category == ShortcutCategory::Notifications
            );
        }
    }
}
