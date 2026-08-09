//! User-facing command vocabulary owned by the checked-in Zentty source.
//!
//! Linux-native presentation may differ, but these labels must not be
//! generalized: several names distinguish materially different behaviors.

pub(crate) const TOGGLE_SIDEBAR: &str = "Toggle sidebar";
pub(crate) const ARRANGE_PANES: &str = "Arrange panes";
pub(crate) const NAVIGATE_BACK: &str = "Navigate Back";
pub(crate) const NAVIGATE_FORWARD: &str = "Navigate Forward";
pub(crate) const NOTIFICATIONS: &str = "Notifications";

pub(crate) const NEW_WORKLANE: &str = "New worklane";
pub(crate) const RENAME_WORKLANE: &str = "Rename Worklane…";
pub(crate) const CLOSE_WORKLANE: &str = "Close Worklane";
pub(crate) const MOVE_WORKLANE_UP: &str = "Move Worklane Up";
pub(crate) const MOVE_WORKLANE_DOWN: &str = "Move Worklane Down";
pub(crate) const WORKLANE_COLOR: &str = "Worklane Color";

pub(crate) const RENAME_PANE: &str = "Rename Pane…";
pub(crate) const SPLIT_RIGHT: &str = "Split Right";
pub(crate) const ADD_PANE_RIGHT: &str = "Add Pane Right";
pub(crate) const ADD_PANE_LEFT: &str = "Add Pane Left";
pub(crate) const NEW_PANE_BELOW: &str = "New Pane Below";
pub(crate) const MOVE_PANE_LEFT: &str = "Move Pane Left";
pub(crate) const MOVE_PANE_RIGHT: &str = "Move Pane Right";
pub(crate) const MOVE_PANE_UP: &str = "Move Pane Up";
pub(crate) const MOVE_PANE_DOWN: &str = "Move Pane Down";
pub(crate) const RESIZE_PANE_LEFT: &str = "Resize Pane Left";
pub(crate) const RESIZE_PANE_RIGHT: &str = "Resize Pane Right";
pub(crate) const RESIZE_PANE_UP: &str = "Resize Pane Up";
pub(crate) const RESIZE_PANE_DOWN: &str = "Resize Pane Down";
pub(crate) const CLOSE_PANE: &str = "Close Pane";
pub(crate) const UNDO_CLOSE_PANE: &str = "Undo Close Pane";
pub(crate) const ARRANGE_WIDTH_FULL: &str = "Arrange Width: Full Width";
pub(crate) const ARRANGE_WIDTH_HALF: &str = "Arrange Width: Half Width";
pub(crate) const ARRANGE_WIDTH_THIRDS: &str = "Arrange Width: Thirds";
pub(crate) const ARRANGE_WIDTH_QUARTERS: &str = "Arrange Width: Quarters";
pub(crate) const ARRANGE_HEIGHT_FULL: &str = "Arrange Height: Full Height";
pub(crate) const ARRANGE_HEIGHT_TWO: &str = "Arrange Height: 2 Per Column";
pub(crate) const ARRANGE_HEIGHT_THREE: &str = "Arrange Height: 3 Per Column";
pub(crate) const ARRANGE_HEIGHT_FOUR: &str = "Arrange Height: 4 Per Column";
pub(crate) const ARRANGE_GOLDEN_WIDE: &str = "Arrange Width: Golden — Focus Wide";
pub(crate) const ARRANGE_GOLDEN_NARROW: &str = "Arrange Width: Golden — Focus Narrow";
pub(crate) const ARRANGE_GOLDEN_TALL: &str = "Arrange Height: Golden — Focus Tall";
pub(crate) const ARRANGE_GOLDEN_SHORT: &str = "Arrange Height: Golden — Focus Short";
pub(crate) const RESET_PANE_LAYOUT: &str = "Reset Pane Layout";

#[cfg(test)]
mod tests {
    use super::*;

    const SHORTCUT_SOURCE: &str =
        include_str!("../../../Zentty/Input/KeyboardShortcutResolver.swift");
    const LAYOUT_SOURCE: &str = include_str!("../../../Zentty/Layout/PaneLayoutPreferences.swift");
    const SIDEBAR_SOURCE: &str =
        include_str!("../../../Zentty/UI/Sidebar/SidebarPaneRowButton.swift");
    const CHROME_SOURCE: &str =
        include_str!("../../../Zentty/UI/Chrome/PaneNavigationButtons.swift");
    const SIDEBAR_TOGGLE_SOURCE: &str =
        include_str!("../../../Zentty/UI/Sidebar/SidebarToggleButton.swift");
    const CREATE_WORKLANE_SOURCE: &str =
        include_str!("../../../Zentty/UI/Sidebar/SidebarCreateWorklaneButton.swift");
    const LINUX_SIDEBAR: &str = include_str!("sidebar.rs");
    const LINUX_CHROME: &str = include_str!("window_chrome.rs");

    #[test]
    fn linux_command_vocabulary_is_present_in_the_checked_in_zentty_source() {
        for label in [
            NAVIGATE_BACK,
            NAVIGATE_FORWARD,
            MOVE_WORKLANE_UP,
            MOVE_WORKLANE_DOWN,
            NEW_PANE_BELOW,
            MOVE_PANE_LEFT,
            MOVE_PANE_RIGHT,
            MOVE_PANE_UP,
            MOVE_PANE_DOWN,
            RESIZE_PANE_LEFT,
            RESIZE_PANE_RIGHT,
            RESIZE_PANE_UP,
            RESIZE_PANE_DOWN,
            CLOSE_PANE,
            UNDO_CLOSE_PANE,
            ARRANGE_WIDTH_FULL,
            ARRANGE_WIDTH_HALF,
            ARRANGE_WIDTH_THIRDS,
            ARRANGE_WIDTH_QUARTERS,
            ARRANGE_HEIGHT_FULL,
            ARRANGE_HEIGHT_TWO,
            ARRANGE_HEIGHT_THREE,
            ARRANGE_HEIGHT_FOUR,
            ARRANGE_GOLDEN_WIDE,
            ARRANGE_GOLDEN_NARROW,
            ARRANGE_GOLDEN_TALL,
            ARRANGE_GOLDEN_SHORT,
            RESET_PANE_LAYOUT,
        ] {
            assert!(
                SHORTCUT_SOURCE.contains(label),
                "missing source command {label:?}"
            );
        }
        for label in [CLOSE_WORKLANE, WORKLANE_COLOR] {
            assert!(
                SIDEBAR_SOURCE.contains(label),
                "missing source sidebar action {label:?}"
            );
        }
        assert!(SIDEBAR_SOURCE.contains("Rename Pane\\u{2026}"));
        assert!(SIDEBAR_SOURCE.contains("Rename Worklane\\u{2026}"));
        assert_eq!(RENAME_PANE, "Rename Pane…");
        assert_eq!(RENAME_WORKLANE, "Rename Worklane…");
        assert!(LAYOUT_SOURCE.contains(SPLIT_RIGHT));
        assert!(LAYOUT_SOURCE.contains(ADD_PANE_RIGHT));
        let pane_container = include_str!("../../../Zentty/UI/PaneStrip/PaneContainerView.swift");
        assert!(pane_container.contains(ADD_PANE_LEFT));
        assert!(SIDEBAR_TOGGLE_SOURCE.contains(TOGGLE_SIDEBAR));
        assert!(CHROME_SOURCE.contains(ARRANGE_PANES));
        assert!(CREATE_WORKLANE_SOURCE.contains(NEW_WORKLANE));
        assert!(SHORTCUT_SOURCE.contains(NOTIFICATIONS));
    }

    #[test]
    fn distinct_rightward_behaviors_are_not_conflated() {
        assert!(LAYOUT_SOURCE.contains("case .visibleSplit"));
        assert!(LAYOUT_SOURCE.contains("case .worklaneAdd"));
        assert!(LAYOUT_SOURCE.contains("\"Split Right\""));
        assert!(LAYOUT_SOURCE.contains("\"Add Pane Right\""));
        assert_ne!(SPLIT_RIGHT, "Add Pane Right");
        assert_ne!(SPLIT_RIGHT, "New Pane Right");
    }

    #[test]
    fn linux_action_surfaces_do_not_reintroduce_rejected_aliases() {
        assert!(!LINUX_SIDEBAR.contains("New Pane Right"));
        assert!(!LINUX_CHROME.contains("New Pane Right"));
        assert!(!LINUX_CHROME.contains("label: \"Go back\""));
        assert!(!LINUX_CHROME.contains("label: \"Go forward\""));
    }
}
