use std::collections::BTreeMap;

use zentty_core::{
    ColumnRecipe, PaneRecipe, TemplateKind, TemplateRestoreFallback, WindowRecipe,
    WorkspaceTemplate, WorkspaceTemplateCaptureContext,
};

fn pane(id: &str, cwd: Option<&str>) -> PaneRecipe {
    PaneRecipe {
        id: id.into(),
        custom_title: None,
        title_seed: Some(format!("{id} title")),
        working_directory: cwd.map(str::to_owned),
        last_activity_title: None,
        last_run_command: None,
    }
}

fn worklane_fixture(first_cwd: &str, second_cwd: &str) -> zentty_core::WorklaneRecipe {
    zentty_core::WorklaneRecipe {
        id: "worklane-source".into(),
        title: Some("  Demo  ".into()),
        next_pane_number: 9,
        focused_column_id: Some("right".into()),
        columns: vec![
            ColumnRecipe {
                id: "left".into(),
                width: 200.0,
                focused_pane_id: Some("pane-a".into()),
                last_focused_pane_id: Some("pane-a".into()),
                pane_heights: vec![1.0],
                panes: vec![pane("pane-a", Some(first_cwd))],
            },
            ColumnRecipe {
                id: "right".into(),
                width: 400.0,
                focused_pane_id: Some("pane-c".into()),
                last_focused_pane_id: Some("pane-b".into()),
                pane_heights: vec![0.4, 0.6],
                panes: vec![
                    pane("pane-b", Some(second_cwd)),
                    pane("pane-c", Some(second_cwd)),
                ],
            },
        ],
        color: Some("blue".into()),
        bookmark_origin_id: None,
    }
}

#[test]
fn bookmark_capture_ports_topology_context_commands_and_safe_environment() {
    let recipe = worklane_fixture("/srv/project/api", "/srv/project/web");
    let commands = BTreeMap::from([
        ("pane-a".into(), "cargo test".into()),
        ("pane-b".into(), "bash".into()),
    ]);
    let environments = BTreeMap::from([(
        "pane-a".into(),
        BTreeMap::from([
            ("NODE_ENV".into(), "test".into()),
            ("PATH".into(), "/fixture".into()),
            ("ZENTTY_PANE_TOKEN".into(), "secret".into()),
        ]),
    )]);
    let template = WorkspaceTemplate::capture(
        &recipe,
        TemplateKind::Bookmark,
        "  Project setup  ",
        WorkspaceTemplateCaptureContext {
            id: "template-1",
            now: "2026-08-11T12:00:00Z",
            captured_readable_width: Some(600.0),
            commands: &commands,
            environments: &environments,
        },
    );

    assert_eq!(template.name, "Project setup");
    assert_eq!(template.title.as_deref(), Some("Demo"));
    assert_eq!(template.project_root.as_deref(), Some("/srv/project"));
    assert_eq!(template.color.as_deref(), Some("blue"));
    assert_eq!(template.focused_column_id.as_deref(), Some("right"));
    assert_eq!(template.columns[1].pane_heights, [0.4, 0.6]);
    assert_eq!(
        template.columns[0].panes[0].command.as_deref(),
        Some("cargo test")
    );
    assert_eq!(template.columns[1].panes[0].command, None);
    assert_eq!(
        template.columns[0].panes[0].environment,
        BTreeMap::from([("NODE_ENV".into(), "test".into())])
    );
}

#[test]
fn capture_filters_absolute_and_login_shell_commands_from_real_proc_shapes() {
    let recipe = worklane_fixture("/srv/project/api", "/srv/project/web");
    let commands = BTreeMap::from([
        ("pane-a".into(), "/bin/bash --login".into()),
        ("pane-b".into(), "-zsh".into()),
    ]);
    let template = WorkspaceTemplate::capture(
        &recipe,
        TemplateKind::Bookmark,
        "Shells",
        WorkspaceTemplateCaptureContext {
            id: "shell-template",
            now: "2026-08-11T10:00:00Z",
            captured_readable_width: Some(800.0),
            commands: &commands,
            environments: &BTreeMap::new(),
        },
    );
    assert!(template.all_panes().all(|pane| pane.command.is_none()));
}

#[test]
fn preset_capture_and_portable_export_strip_all_location_and_runtime_context() {
    let recipe = worklane_fixture("/srv/project/api", "/srv/project/web");
    let commands = BTreeMap::from([("pane-a".into(), "cargo test".into())]);
    let environments = BTreeMap::from([(
        "pane-a".into(),
        BTreeMap::from([
            ("TERM".into(), "xterm".into()),
            ("CUSTOM".into(), "kept".into()),
        ]),
    )]);
    let captured = WorkspaceTemplate::capture(
        &recipe,
        TemplateKind::Bookmark,
        "Demo",
        WorkspaceTemplateCaptureContext {
            id: "template-1",
            now: "2026-08-11T12:00:00Z",
            captured_readable_width: Some(600.0),
            commands: &commands,
            environments: &environments,
        },
    );
    let portable = captured.into_portable_preset("2026-08-11T13:00:00Z");

    assert_eq!(portable.kind, TemplateKind::Preset);
    assert_eq!(portable.project_root, None);
    assert!(
        portable
            .all_panes()
            .all(|pane| pane.working_directory.is_none())
    );
    assert_eq!(
        portable.columns[0].panes[0].environment,
        BTreeMap::from([
            ("CUSTOM".into(), "kept".into()),
            ("TERM".into(), "xterm".into()),
        ])
    );
    assert_eq!(portable.updated_at, "2026-08-11T13:00:00Z");
}

#[test]
fn restore_allocates_fresh_ids_remaps_focus_scales_widths_and_reports_fallbacks() {
    let root = std::env::temp_dir().join(format!("zentty-template-restore-{}", std::process::id()));
    let valid = root.join("valid");
    std::fs::create_dir_all(&valid).unwrap();
    let missing = root.join("missing");
    let mut recipe = worklane_fixture(valid.to_str().unwrap(), missing.to_str().unwrap());
    recipe.columns[1].panes[1].working_directory = Some(valid.to_string_lossy().into_owned());
    let commands = BTreeMap::from([
        ("pane-a".into(), "cargo test".into()),
        ("pane-b".into(), "missing-command --watch".into()),
    ]);
    let template = WorkspaceTemplate::capture(
        &recipe,
        TemplateKind::Bookmark,
        "Demo",
        WorkspaceTemplateCaptureContext {
            id: "template-1",
            now: "2026-08-11T12:00:00Z",
            captured_readable_width: Some(600.0),
            commands: &commands,
            environments: &BTreeMap::new(),
        },
    );
    let mut ids = [
        "column-new-left",
        "pane-new-a",
        "column-new-right",
        "pane-new-b",
        "pane-new-c",
    ]
    .into_iter()
    .map(str::to_owned);
    let restored = template
        .restore(
            "worklane-new",
            &mut ids,
            valid.to_str().unwrap(),
            1200.0,
            1200.0,
            |command| command == "cargo test",
        )
        .unwrap();

    assert_eq!(restored.recipe.id, "worklane-new");
    assert_eq!(
        restored.recipe.bookmark_origin_id.as_deref(),
        Some("template-1")
    );
    assert_eq!(
        restored.recipe.focused_column_id.as_deref(),
        Some("column-new-right")
    );
    assert!((restored.recipe.columns[0].width - 400.0).abs() < f64::EPSILON);
    assert!((restored.recipe.columns[1].width - 800.0).abs() < f64::EPSILON);
    assert_eq!(
        restored.recipe.columns[1].focused_pane_id.as_deref(),
        Some("pane-new-c")
    );
    assert_eq!(
        restored.recipe.columns[1].last_focused_pane_id.as_deref(),
        Some("pane-new-b")
    );
    assert_eq!(restored.recipe.columns[1].pane_heights, [0.4, 0.6]);
    assert_eq!(
        restored.launches["pane-new-a"].command.as_deref(),
        Some("cargo test")
    );
    assert_eq!(
        restored.launches["pane-new-b"].prefill.as_deref(),
        Some("missing-command --watch")
    );
    assert_eq!(
        restored.recipe.columns[1].panes[0]
            .working_directory
            .as_deref(),
        valid.to_str()
    );
    assert_eq!(restored.fallbacks.len(), 2);
    assert!(restored.fallbacks.iter().any(|fallback| matches!(
        fallback,
        TemplateRestoreFallback::MissingDirectory { pane_id, requested, .. }
            if pane_id == "pane-new-b" && requested == missing.to_str().unwrap()
    )));
    assert!(restored.fallbacks.iter().any(|fallback| matches!(
        fallback,
        TemplateRestoreFallback::MissingCommand { pane_id, command }
            if pane_id == "pane-new-b" && command == "missing-command --watch"
    )));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn restored_worklane_recipe_imports_through_the_only_live_workspace_model() {
    let root = std::env::temp_dir().join(format!("zentty-template-model-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let recipe = worklane_fixture(root.to_str().unwrap(), root.to_str().unwrap());
    let template = WorkspaceTemplate::capture(
        &recipe,
        TemplateKind::Preset,
        "Demo",
        WorkspaceTemplateCaptureContext {
            id: "template-1",
            now: "2026-08-11T12:00:00Z",
            captured_readable_width: Some(600.0),
            commands: &BTreeMap::new(),
            environments: &BTreeMap::new(),
        },
    );
    let mut ids = [
        "column-restored-a",
        "pane-restored-a",
        "column-restored-b",
        "pane-restored-b",
        "pane-restored-c",
    ]
    .into_iter()
    .map(str::to_owned);
    let restored = template
        .restore(
            "worklane-new",
            &mut ids,
            root.to_str().unwrap(),
            600.0,
            600.0,
            |_| true,
        )
        .unwrap();
    let restored_window = WindowRecipe {
        id: "window".into(),
        frame: None,
        worklanes: vec![restored.recipe],
        active_worklane_id: Some("worklane-new".into()),
    };
    let imported = zentty_core::WorkspaceState::from_window_recipe(&restored_window).unwrap();
    assert_eq!(imported.active_worklane_id(), "worklane-new");
    assert_eq!(imported.worklanes().len(), 1);

    let base_window = WindowRecipe {
        id: "window".into(),
        frame: None,
        worklanes: vec![worklane_fixture(
            root.to_str().unwrap(),
            root.to_str().unwrap(),
        )],
        active_worklane_id: Some("worklane-source".into()),
    };
    let mut state = zentty_core::WorkspaceState::from_window_recipe(&base_window).unwrap();
    state
        .insert_worklane_recipe(restored_window.worklanes[0].clone())
        .unwrap();
    assert_eq!(state.active_worklane_id(), "worklane-new");
    assert_eq!(state.worklanes().len(), 2);
    assert_eq!(
        state.active_worklane().bookmark_origin_id.as_deref(),
        Some("template-1")
    );
    assert!(state.set_bookmark_origin("worklane-new", None));
    assert_eq!(state.active_worklane().bookmark_origin_id, None);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn pane_iterators_and_restore_geometry_reject_every_invalid_boundary() {
    let root =
        std::env::temp_dir().join(format!("zentty-template-geometry-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let recipe = worklane_fixture(root.to_str().unwrap(), root.to_str().unwrap());
    let mut template = WorkspaceTemplate::capture(
        &recipe,
        TemplateKind::Bookmark,
        "Geometry",
        WorkspaceTemplateCaptureContext {
            id: "geometry",
            now: "2026-08-11T12:00:00Z",
            captured_readable_width: Some(600.0),
            commands: &BTreeMap::new(),
            environments: &BTreeMap::new(),
        },
    );
    assert_eq!(template.all_panes().count(), 3);
    for pane in template.all_panes_mut() {
        pane.was_user_edited = true;
    }
    assert!(template.all_panes().all(|pane| pane.was_user_edited));

    for invalid_width in [0.0, f64::NAN, f64::INFINITY] {
        let mut invalid = template.clone();
        invalid.captured_readable_width = Some(invalid_width);
        let mut ids = (0..5).map(|index| format!("invalid-{invalid_width:?}-{index}"));
        let restored = invalid
            .restore(
                "invalid-width",
                &mut ids,
                root.to_str().unwrap(),
                1200.0,
                777.0,
                |_| true,
            )
            .unwrap();
        assert!((restored.recipe.columns[0].width - 200.0).abs() < f64::EPSILON);
        assert!((restored.recipe.columns[1].width - 400.0).abs() < f64::EPSILON);
    }

    let invalid_heights = [
        vec![1.0],
        vec![0.0, 1.0],
        vec![-1.0, 1.0],
        vec![f64::NAN, 1.0],
        vec![f64::INFINITY, 1.0],
    ];
    for (case, heights) in invalid_heights.into_iter().enumerate() {
        let mut invalid = template.clone();
        invalid.columns[1].pane_heights = heights;
        let mut ids = (0..5).map(|index| format!("height-{case}-{index}"));
        let restored = invalid
            .restore(
                "invalid-height",
                &mut ids,
                root.to_str().unwrap(),
                600.0,
                777.0,
                |_| true,
            )
            .unwrap();
        assert_eq!(restored.recipe.columns[1].pane_heights, [1.0, 1.0]);
    }

    let mut single = template.clone();
    single.columns.truncate(1);
    let mut ids = ["single-column", "single-pane"]
        .into_iter()
        .map(str::to_owned);
    let restored = single
        .restore(
            "single",
            &mut ids,
            root.to_str().unwrap(),
            1200.0,
            777.0,
            |_| true,
        )
        .unwrap();
    assert!((restored.recipe.columns[0].width - 777.0).abs() < f64::EPSILON);
    std::fs::remove_dir_all(root).unwrap();
}
