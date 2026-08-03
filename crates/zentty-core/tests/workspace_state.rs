use zentty_core::{ClosePaneOutcome, WorklaneColor, WorkspaceState};

#[test]
fn worklane_commands_preserve_order_selection_and_source_title_semantics() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");

    assert!(state.create_worklane("worklane-b", "pane-b"));
    assert_eq!(state.worklane_ids(), ["worklane-a", "worklane-b"]);
    assert_eq!(state.active_worklane_id(), "worklane-b");

    assert!(state.set_worklane_title("worklane-a", Some("  API work  ")));
    assert_eq!(state.worklanes()[0].title.as_deref(), Some("API work"));
    assert!(state.set_worklane_title("worklane-a", Some("   ")));
    assert_eq!(state.worklanes()[0].title, None);
    assert!(!state.set_worklane_title("missing", Some("ignored")));

    assert!(state.set_worklane_color("worklane-a", Some(WorklaneColor::Blue)));
    assert_eq!(state.worklanes()[0].color, Some(WorklaneColor::Blue));
    assert!(state.move_worklane("worklane-a", 1));
    assert_eq!(state.worklane_ids(), ["worklane-b", "worklane-a"]);
    assert_eq!(state.active_worklane_id(), "worklane-b");

    assert!(state.select_worklane("worklane-a"));
    assert!(!state.select_worklane("missing"));
    assert!(state.close_active_worklane());
    assert_eq!(state.worklane_ids(), ["worklane-b"]);
    assert_eq!(state.active_worklane_id(), "worklane-b");
    assert!(!state.close_active_worklane());
}

#[test]
fn pane_commands_keep_real_terminal_identity_attached_to_stable_panes() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");

    assert!(state.split_focused_pane_right("pane-b"));
    assert_eq!(state.active_pane_ids(), ["pane-a", "pane-b"]);
    assert_eq!(state.focused_pane_id(), Some("pane-b"));

    assert!(state.select_pane("pane-a"));
    assert_eq!(state.focused_pane_id(), Some("pane-a"));
    assert_eq!(state.close_focused_pane(), ClosePaneOutcome::Closed);
    assert_eq!(state.active_pane_ids(), ["pane-b"]);
    assert_eq!(state.focused_pane_id(), Some("pane-b"));
    assert_eq!(state.close_focused_pane(), ClosePaneOutcome::CloseWindow);
    assert_eq!(state.active_pane_ids(), ["pane-b"]);
}

#[test]
fn duplicate_ids_and_invalid_reorders_are_rejected_without_mutation() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    assert!(!state.create_worklane("worklane-a", "pane-b"));
    assert!(!state.create_worklane("worklane-b", "pane-a"));
    assert!(!state.split_focused_pane_right("pane-a"));
    assert!(!state.move_worklane("worklane-a", 1));
    assert_eq!(state.worklane_ids(), ["worklane-a"]);
    assert_eq!(state.active_pane_ids(), ["pane-a"]);
}

#[test]
fn closing_a_specific_inactive_pane_removes_its_lane_without_changing_selection() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    assert!(state.create_worklane("worklane-b", "pane-b"));
    assert!(state.select_worklane("worklane-a"));

    assert_eq!(state.close_pane("pane-b"), ClosePaneOutcome::Closed);
    assert_eq!(state.worklane_ids(), ["worklane-a"]);
    assert_eq!(state.active_worklane_id(), "worklane-a");
    assert_eq!(state.close_pane("missing"), ClosePaneOutcome::NotFound);
}
