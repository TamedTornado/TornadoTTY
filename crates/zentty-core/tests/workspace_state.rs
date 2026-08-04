use zentty_core::{
    ClosePaneOutcome, PaneRecipe, SessionRestoreEnvelope, WorklaneColor, WorkspaceState,
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
fn closing_a_named_worklane_preserves_unrelated_active_selection() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    assert!(state.create_worklane("worklane-b", "pane-b"));
    assert!(state.create_worklane("worklane-c", "pane-c"));
    assert!(state.select_worklane("worklane-b"));

    assert!(state.close_worklane("worklane-a"));
    assert_eq!(state.worklane_ids(), ["worklane-b", "worklane-c"]);
    assert_eq!(state.active_worklane_id(), "worklane-b");

    assert!(state.close_worklane("worklane-b"));
    assert_eq!(state.worklane_ids(), ["worklane-c"]);
    assert_eq!(state.active_worklane_id(), "worklane-c");
    assert!(!state.close_worklane("worklane-c"));
    assert!(!state.close_worklane("missing"));
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
fn closing_an_unfocused_pane_does_not_steal_focus() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    assert!(state.split_focused_pane_right("pane-b"));
    assert_eq!(state.focused_pane_id(), Some("pane-b"));

    assert_eq!(state.close_pane("pane-a"), ClosePaneOutcome::Closed);
    assert_eq!(state.active_pane_ids(), ["pane-b"]);
    assert_eq!(state.focused_pane_id(), Some("pane-b"));

    assert!(state.split_focused_pane_below("pane-c"));
    assert_eq!(state.close_pane("pane-b"), ClosePaneOutcome::Closed);
    assert_eq!(state.active_pane_ids(), ["pane-c"]);
    assert_eq!(state.focused_pane_id(), Some("pane-c"));
}

#[test]
fn vertical_pane_commands_preserve_column_identity_and_geometry() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");

    assert!(state.split_focused_pane_below("pane-b"));
    assert_eq!(state.active_columns().len(), 1);
    assert_eq!(state.active_pane_ids(), ["pane-a", "pane-b"]);
    assert_eq!(state.active_columns()[0].pane_heights, [0.5, 0.5]);
    assert!(state.move_focused_pane_up());
    assert_eq!(state.active_pane_ids(), ["pane-b", "pane-a"]);
    assert!(!state.move_focused_pane_up());
    assert!(state.move_focused_pane_down());
    assert_eq!(state.active_pane_ids(), ["pane-a", "pane-b"]);
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
    assert!(state.move_focused_pane_left());
    assert_eq!(state.active_columns().len(), 3);
    assert!(!state.move_focused_pane_left());
    assert!(state.move_focused_pane_right());
    assert_eq!(state.active_pane_ids(), ["pane-c", "pane-a", "pane-b"]);
    assert_eq!(state.active_columns().len(), 2);
}

#[test]
fn focused_pane_transfers_to_existing_worklane_as_a_focused_column() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    assert!(state.split_focused_pane_below("pane-b"));
    assert!(state.set_pane_title("pane-b", "agent review"));
    assert!(state.create_worklane("worklane-b", "pane-c"));
    assert!(state.select_worklane("worklane-a"));

    assert!(state.transfer_focused_pane_to_worklane("worklane-b"));
    assert_eq!(state.worklane_ids(), ["worklane-a", "worklane-b"]);
    assert_eq!(state.active_worklane_id(), "worklane-b");
    assert_eq!(state.active_pane_ids(), ["pane-c", "pane-b"]);
    assert_eq!(state.focused_pane_id(), Some("pane-b"));
    assert_eq!(state.active_columns().len(), 2);
    assert_eq!(state.active_columns()[1].id, "column-pane-b");
    assert_eq!(
        state.active_columns()[1].panes[0].live_title,
        "agent review"
    );
    assert_eq!(state.worklanes()[0].columns[0].pane_heights, [1.0]);
}

#[test]
fn cross_worklane_transfer_removes_an_empty_source_and_rejects_invalid_targets() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    assert!(state.create_worklane("worklane-b", "pane-b"));
    assert!(state.select_worklane("worklane-a"));

    let unchanged = state.clone();
    assert!(!state.transfer_focused_pane_to_worklane("worklane-a"));
    assert!(!state.transfer_focused_pane_to_worklane("missing"));
    assert_eq!(state, unchanged);

    assert!(state.transfer_focused_pane_to_worklane("worklane-b"));
    assert_eq!(state.worklane_ids(), ["worklane-b"]);
    assert_eq!(state.active_worklane_id(), "worklane-b");
    assert_eq!(state.active_pane_ids(), ["pane-b", "pane-a"]);
    assert_eq!(state.focused_pane_id(), Some("pane-a"));
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
fn pane_custom_identity_survives_runtime_titles_and_clears_to_live_fallback() {
    let envelope = SessionRestoreEnvelope::from_json(V3_ENVELOPE).unwrap();
    let template = &envelope.workspace.windows[0];
    let mut state = WorkspaceState::from_window_recipe(template).unwrap();

    assert_eq!(
        state.sidebar_summaries()[0].pane_rows[0].primary_text,
        "Agent"
    );
    assert!(state.set_pane_title("pane-agent", "  compiling  "));
    assert_eq!(
        state.sidebar_summaries()[0].pane_rows[0].primary_text,
        "Agent"
    );
    assert!(state.set_pane_custom_title("pane-agent", Some("  Review Agent  ")));
    assert_eq!(
        state.sidebar_summaries()[0].pane_rows[0].primary_text,
        "Review Agent"
    );
    assert!(state.set_pane_custom_title("pane-agent", Some("   ")));
    assert_eq!(
        state.sidebar_summaries()[0].pane_rows[0].primary_text,
        "compiling"
    );

    let projected = state.to_window_recipe(template);
    let pane = &projected.worklanes[0].columns[0].panes[0];
    assert_eq!(pane.custom_title, None);
    assert_eq!(pane.last_activity_title.as_deref(), Some("compiling"));
    assert_eq!(pane.title_seed.as_deref(), Some("Codex"));
}

#[test]
fn pane_navigation_history_crosses_worklanes_and_preserves_browser_semantics() {
    let mut state = WorkspaceState::new("lane-1", "pane-1");
    assert!(state.split_focused_pane_right("pane-2"));
    assert!(state.create_worklane("lane-2", "pane-3"));

    assert!(state.can_navigate_back());
    assert!(!state.can_navigate_forward());
    assert!(state.navigate_back());
    assert_eq!(state.active_worklane_id(), "lane-1");
    assert_eq!(state.focused_pane_id(), Some("pane-2"));
    assert!(state.navigate_back());
    assert_eq!(state.focused_pane_id(), Some("pane-1"));
    assert!(state.can_navigate_forward());
    assert!(state.navigate_forward());
    assert_eq!(state.focused_pane_id(), Some("pane-2"));

    assert!(state.select_worklane_and_pane("lane-2", "pane-3"));
    assert!(!state.can_navigate_forward());
    assert!(state.navigate_back());
    assert_eq!(state.active_worklane_id(), "lane-1");
    assert_eq!(state.focused_pane_id(), Some("pane-2"));
}

#[test]
fn adjacent_pane_traversal_follows_sidebar_order_and_wraps_across_worklanes() {
    const SOURCE: &str =
        include_str!("../../../Zentty/UI/WorklanePeek/WorklanePeekSelectionState.swift");

    let mut state = WorkspaceState::new("lane-1", "pane-1");
    assert!(state.split_focused_pane_below("pane-2"));
    assert!(state.create_worklane("lane-2", "pane-3"));

    assert!(state.select_adjacent_pane(true));
    assert_eq!(state.active_worklane_id(), "lane-1");
    assert_eq!(state.focused_pane_id(), Some("pane-1"));
    assert!(state.select_adjacent_pane(true));
    assert_eq!(state.focused_pane_id(), Some("pane-2"));
    assert!(state.select_adjacent_pane(true));
    assert_eq!(state.active_worklane_id(), "lane-2");
    assert_eq!(state.focused_pane_id(), Some("pane-3"));
    assert!(state.select_adjacent_pane(false));
    assert_eq!(state.active_worklane_id(), "lane-1");
    assert_eq!(state.focused_pane_id(), Some("pane-2"));

    assert!(SOURCE.contains("paneStripState.panes.map"));
    assert!(SOURCE.contains("(currentIndex + direction.offset + count) % count"));
}

#[test]
fn adjacent_worklane_traversal_wraps_without_changing_each_lanes_focused_pane() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    assert!(state.create_worklane("worklane-b", "pane-b"));
    assert!(state.select_worklane("worklane-a"));
    let first_pane = state.focused_pane_id().map(str::to_owned);

    assert!(state.select_adjacent_worklane(true));
    let second_id = state.active_worklane_id().to_owned();
    let second_pane = state.focused_pane_id().map(str::to_owned);
    assert!(state.select_adjacent_worklane(true));
    assert_eq!(state.focused_pane_id(), first_pane.as_deref());
    assert!(state.select_adjacent_worklane(false));
    assert_eq!(state.active_worklane_id(), second_id);
    assert_eq!(state.focused_pane_id(), second_pane.as_deref());
}

#[test]
fn right_insertion_commands_preserve_their_distinct_width_contracts() {
    let mut added = WorkspaceState::new("lane-1", "pane-1");
    assert!(added.add_pane_right_without_resizing("pane-2", 719.0));
    assert!(
        added
            .active_columns()
            .iter()
            .all(|column| (column.width - 719.0).abs() < f64::EPSILON)
    );

    let mut split = WorkspaceState::new("lane-1", "pane-1");
    assert!(split.split_focused_pane_right_visibly("pane-2", 359.0));
    assert!(
        split
            .active_columns()
            .iter()
            .all(|column| (column.width - 359.0).abs() < f64::EPSILON)
    );
}

#[test]
fn multi_column_recipe_round_trip_preserves_source_topology() {
    let envelope = SessionRestoreEnvelope::from_json(V3_ENVELOPE).unwrap();
    let mut window = envelope.workspace.windows[0].clone();
    window.worklanes[0].columns.push(zentty_core::ColumnRecipe {
        id: "column-right".to_owned(),
        width: 320.0,
        focused_pane_id: Some("pane-review".to_owned()),
        last_focused_pane_id: Some("pane-review".to_owned()),
        pane_heights: vec![700.0],
        panes: vec![PaneRecipe {
            id: "pane-review".to_owned(),
            custom_title: Some("Review".to_owned()),
            title_seed: None,
            working_directory: Some("/tmp/project".to_owned()),
            last_activity_title: None,
            last_run_command: None,
        }],
    });
    window.worklanes[0].focused_column_id = Some("column-right".to_owned());

    let state = WorkspaceState::from_window_recipe(&window).unwrap();
    assert_eq!(state.active_columns().len(), 2);
    assert_eq!(
        state.active_pane_ids(),
        ["pane-agent", "pane-shell", "pane-review"]
    );
    assert_eq!(state.focused_pane_id(), Some("pane-review"));
    let projected = state.to_window_recipe(&window);
    assert_eq!(projected.worklanes[0].columns, window.worklanes[0].columns);
}

#[test]
fn cross_column_move_matches_source_height_reconciliation() {
    let envelope = SessionRestoreEnvelope::from_json(V3_ENVELOPE).unwrap();
    let mut window = envelope.workspace.windows[0].clone();
    window.worklanes[0].columns.push(zentty_core::ColumnRecipe {
        id: "column-right".to_owned(),
        width: 320.0,
        focused_pane_id: Some("pane-review".to_owned()),
        last_focused_pane_id: Some("pane-review".to_owned()),
        pane_heights: vec![700.0],
        panes: vec![PaneRecipe {
            id: "pane-review".to_owned(),
            custom_title: Some("Review".to_owned()),
            title_seed: None,
            working_directory: Some("/tmp/project".to_owned()),
            last_activity_title: None,
            last_run_command: None,
        }],
    });

    let mut state = WorkspaceState::from_window_recipe(&window).unwrap();
    assert!(state.move_focused_pane_right());
    assert_eq!(state.active_columns()[0].pane_heights, [700.0]);
    assert_eq!(state.active_columns()[1].pane_heights, [1.0, 1.0]);
    assert_eq!(
        state.active_pane_ids(),
        ["pane-shell", "pane-agent", "pane-review"]
    );
    assert_eq!(state.focused_pane_id(), Some("pane-agent"));
}

#[test]
fn duplicate_column_identity_is_rejected() {
    let envelope = SessionRestoreEnvelope::from_json(V3_ENVELOPE).unwrap();
    let mut window = envelope.workspace.windows[0].clone();
    let duplicate = window.worklanes[0].columns[0].clone();
    window.worklanes[0].columns.push(duplicate);

    assert_eq!(
        WorkspaceState::from_window_recipe(&window),
        Err(WorkspaceStateImportError::DuplicateColumn(
            "column-left".to_owned()
        ))
    );
}
