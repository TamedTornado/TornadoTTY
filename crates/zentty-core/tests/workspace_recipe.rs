use zentty_core::{SaveReason, SessionRestoreEnvelope, WorkspaceRecipe};

const V3_ENVELOPE: &[u8] = include_bytes!("fixtures/session-restore-v3.json");
const UNVERSIONED_RECIPE: &[u8] = include_bytes!("fixtures/workspace-recipe-unversioned.json");
const FUTURE_RECIPE: &[u8] = include_bytes!("fixtures/workspace-recipe-future.json");

#[test]
fn source_compatible_v3_envelope_preserves_recipe_and_separate_agent_drafts() {
    let envelope = SessionRestoreEnvelope::from_json(V3_ENVELOPE).unwrap();

    assert_eq!(envelope.schema_version, 1);
    assert_eq!(envelope.reason, SaveReason::LiveSnapshot);
    assert_eq!(envelope.workspace.schema_version, Some(3));
    assert_eq!(
        envelope.workspace.active_window_id.as_deref(),
        Some("window-main")
    );
    let window = &envelope.workspace.windows[0];
    assert!((window.frame.as_ref().unwrap().x - 1721.0).abs() < f64::EPSILON);
    let worklane = &window.worklanes[0];
    assert_eq!(worklane.color.as_deref(), Some("blue"));
    assert_eq!(worklane.focused_column_id.as_deref(), Some("column-left"));
    assert_eq!(worklane.columns[0].pane_heights, [420.0, 280.0]);
    assert_eq!(
        worklane.columns[0].panes[0].last_run_command.as_deref(),
        Some("cargo test")
    );
    assert_eq!(
        envelope.restore_draft_windows[0].pane_drafts[0].pane_id,
        "pane-agent"
    );
    assert_eq!(
        envelope.restore_draft_windows[0].pane_drafts[0]
            .agent_launch_snapshot
            .as_ref()
            .unwrap()
            .arguments,
        ["codex", "resume", "session-codex"]
    );

    let round_trip = SessionRestoreEnvelope::from_json(&envelope.to_json().unwrap()).unwrap();
    assert_eq!(round_trip, envelope);
}

#[test]
fn unversioned_migration_only_sanitizes_legacy_generated_titles() {
    let recipe = WorkspaceRecipe::from_json(UNVERSIONED_RECIPE).unwrap();
    assert_eq!(recipe.schema_version, None);

    let migrated = recipe.migrated();
    assert_eq!(
        migrated.schema_version,
        Some(WorkspaceRecipe::CURRENT_SCHEMA_VERSION)
    );
    assert_eq!(
        migrated.windows[0]
            .worklanes
            .iter()
            .map(|worklane| worklane.title.as_deref())
            .collect::<Vec<_>>(),
        [None, None, Some("Nimbu support")]
    );
}

#[test]
fn versioned_future_recipe_matches_current_forward_compatibility_behavior() {
    let recipe = WorkspaceRecipe::from_json(FUTURE_RECIPE).unwrap();
    assert_eq!(recipe.schema_version, Some(99));

    let migrated = recipe.migrated();
    assert_eq!(
        migrated.schema_version,
        Some(WorkspaceRecipe::CURRENT_SCHEMA_VERSION)
    );
    assert_eq!(
        migrated.windows[0].worklanes[0].title.as_deref(),
        Some("MAIN")
    );
}

#[test]
fn meaningfulness_matches_default_legacy_and_user_modified_recipes() {
    let legacy_default = WorkspaceRecipe::from_json(UNVERSIONED_RECIPE).unwrap();
    let mut single = legacy_default;
    single.windows[0].worklanes.truncate(1);
    single.windows[0].worklanes[0].columns = vec![zentty_core::ColumnRecipe {
        id: "column-main".to_owned(),
        width: 640.0,
        focused_pane_id: Some("pane-main".to_owned()),
        last_focused_pane_id: Some("pane-main".to_owned()),
        pane_heights: vec![700.0],
        panes: vec![zentty_core::PaneRecipe {
            id: "pane-main".to_owned(),
            custom_title: None,
            title_seed: Some("shell".to_owned()),
            working_directory: Some("/Users/peter/project/..".to_owned()),
            last_activity_title: None,
            last_run_command: None,
        }],
    }];
    single.windows[0].worklanes[0].focused_column_id = Some("column-main".to_owned());
    single.windows[0].worklanes[0].next_pane_number = 2;
    single.windows[0].active_worklane_id = Some("legacy-main".to_owned());

    assert!(!single.clone().is_meaningful("/Users/peter"));

    let mut versioned_title = single.clone();
    versioned_title.schema_version = Some(3);
    assert!(versioned_title.is_meaningful("/Users/peter"));

    let mut custom_title = single.clone();
    custom_title.windows[0].worklanes[0].columns[0].panes[0].custom_title =
        Some("Nimbu API".to_owned());
    assert!(custom_title.is_meaningful("/Users/peter"));

    let mut different_cwd = single;
    different_cwd.windows[0].worklanes[0].columns[0].panes[0].working_directory =
        Some("/tmp/project".to_owned());
    assert!(different_cwd.is_meaningful("/Users/peter"));
}
