use zentty_core::{
    ClosePaneOutcome, SessionRestoreEnvelope, WorklaneColor, WorkspaceState,
    WorkspaceStateImportError,
};

const V3_ENVELOPE: &[u8] = include_bytes!("fixtures/session-restore-v3.json");

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

#[test]
fn focused_pane_moves_within_the_active_worklane_without_losing_focus() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    assert!(state.split_focused_pane_right("pane-b"));
    assert!(state.split_focused_pane_right("pane-c"));
    assert_eq!(state.active_pane_ids(), ["pane-a", "pane-b", "pane-c"]);

    assert!(state.move_focused_pane_left());
    assert_eq!(state.active_pane_ids(), ["pane-a", "pane-c", "pane-b"]);
    assert_eq!(state.focused_pane_id(), Some("pane-c"));
    assert!(state.move_focused_pane_left());
    assert_eq!(state.active_pane_ids(), ["pane-c", "pane-a", "pane-b"]);
    assert!(!state.move_focused_pane_left());
    assert!(state.move_focused_pane_right());
    assert_eq!(state.active_pane_ids(), ["pane-a", "pane-c", "pane-b"]);
}

#[test]
fn sidebar_summaries_are_compound_worklane_and_pane_presentations() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    assert!(state.split_focused_pane_right("pane-b"));
    assert!(state.set_pane_title("pane-a", "project shell"));
    assert!(state.set_pane_title("pane-b", "cargo test"));
    assert!(state.set_worklane_title("worklane-a", Some("  Nimbu API  ")));
    assert!(state.set_worklane_color("worklane-a", Some(WorklaneColor::Blue)));

    let summaries = state.sidebar_summaries();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].top_label.as_deref(), Some("Nimbu API"));
    assert_eq!(summaries[0].primary_text, "cargo test");
    assert_eq!(summaries[0].color, Some(WorklaneColor::Blue));
    assert!(summaries[0].is_active);
    assert_eq!(summaries[0].pane_rows.len(), 2);
    assert_eq!(summaries[0].pane_rows[0].primary_text, "project shell");
    assert!(!summaries[0].pane_rows[0].is_focused);
    assert_eq!(summaries[0].pane_rows[1].primary_text, "cargo test");
    assert!(summaries[0].pane_rows[1].is_focused);
}

#[test]
fn source_window_round_trip_preserves_metadata_while_applying_product_state() {
    let envelope = SessionRestoreEnvelope::from_json(V3_ENVELOPE).unwrap();
    let template = &envelope.workspace.windows[0];
    let mut state = WorkspaceState::from_window_recipe(template).unwrap();

    assert_eq!(state.worklane_ids(), ["worklane-main"]);
    assert_eq!(state.active_pane_ids(), ["pane-agent", "pane-shell"]);
    assert_eq!(state.focused_pane_id(), Some("pane-agent"));
    assert!(state.set_worklane_title("worklane-main", Some("  Linux port  ")));
    assert!(state.set_worklane_color("worklane-main", Some(WorklaneColor::Purple)));
    assert!(state.select_pane("pane-shell"));

    let projected = state.to_window_recipe(template);
    assert_eq!(projected.worklanes[0].title.as_deref(), Some("Linux port"));
    assert_eq!(projected.worklanes[0].color.as_deref(), Some("purple"));
    assert_eq!(
        projected.worklanes[0].columns[0].focused_pane_id.as_deref(),
        Some("pane-shell")
    );
    assert_eq!(
        projected.worklanes[0].columns[0].pane_heights,
        [420.0, 280.0]
    );
    assert_eq!(
        projected.worklanes[0].columns[0].panes[0]
            .last_run_command
            .as_deref(),
        Some("cargo test")
    );
    assert_eq!(
        projected.worklanes[0].bookmark_origin_id.as_deref(),
        Some("bookmark-main")
    );
}

#[test]
fn unsupported_multi_column_recipe_fails_instead_of_flattening() {
    let envelope = SessionRestoreEnvelope::from_json(V3_ENVELOPE).unwrap();
    let mut window = envelope.workspace.windows[0].clone();
    let duplicate_column = window.worklanes[0].columns[0].clone();
    window.worklanes[0].columns.push(duplicate_column);

    assert_eq!(
        WorkspaceState::from_window_recipe(&window),
        Err(WorkspaceStateImportError::UnsupportedColumnCount {
            worklane_id: "worklane-main".to_owned(),
            count: 2,
        })
    );
}
