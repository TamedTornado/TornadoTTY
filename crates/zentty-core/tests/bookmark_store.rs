use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    sync::Arc,
    thread,
};

use zentty_core::{
    AtomicFileStoreError, BookmarkStore, BookmarkStoreError, TemplateKind, WorkspaceTemplate,
    WorkspaceTemplateColumn, WorkspaceTemplateExportEnvelope, WorkspaceTemplatePane,
};

fn fixture_root(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "zentty-bookmark-store-{label}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn template(id: &str, name: &str, kind: TemplateKind) -> WorkspaceTemplate {
    WorkspaceTemplate {
        schema_version: WorkspaceTemplate::CURRENT_SCHEMA_VERSION,
        id: id.into(),
        name: name.into(),
        kind,
        title: None,
        color: None,
        project_root: (kind == TemplateKind::Bookmark).then(|| "/tmp/project".into()),
        captured_readable_width: Some(800.0),
        next_pane_number: 2,
        focused_column_id: Some("column".into()),
        columns: vec![WorkspaceTemplateColumn {
            id: "column".into(),
            width: 800.0,
            focused_pane_id: Some("pane".into()),
            last_focused_pane_id: Some("pane".into()),
            pane_heights: vec![1.0],
            panes: vec![WorkspaceTemplatePane {
                id: "pane".into(),
                custom_title: None,
                title_seed: Some("shell".into()),
                working_directory: (kind == TemplateKind::Bookmark).then(|| "/tmp/project".into()),
                command: Some("cargo test".into()),
                environment: BTreeMap::from([
                    ("NODE_ENV".into(), "test".into()),
                    ("ZENTTY_PANE_TOKEN".into(), "secret".into()),
                ]),
                was_user_edited: false,
            }],
        }],
        pinned: false,
        created_at: "2026-08-11T10:00:00Z".into(),
        updated_at: "2026-08-11T10:00:00Z".into(),
        last_used_at: None,
    }
}

#[test]
fn store_mutations_reload_latest_locked_state_and_preserve_deterministic_order() {
    let root = fixture_root("mutations");
    let store = Arc::new(BookmarkStore::new(root.join("bookmarks.json")));
    let first = Arc::clone(&store);
    let second = Arc::clone(&store);
    let a = thread::spawn(move || {
        first
            .upsert(
                template("a", "Zulu", TemplateKind::Bookmark),
                "2026-08-11T11:00:00Z",
            )
            .unwrap();
    });
    let b = thread::spawn(move || {
        second
            .upsert(
                template("b", "Alpha", TemplateKind::Preset),
                "2026-08-11T12:00:00Z",
            )
            .unwrap();
    });
    a.join().unwrap();
    b.join().unwrap();
    store.set_pinned("a", true, "2026-08-11T13:00:00Z").unwrap();
    store.record_use("b", "2026-08-11T14:00:00Z").unwrap();

    let loaded = store.load().unwrap();
    assert_eq!(loaded.templates.len(), 2);
    assert_eq!(
        loaded
            .sorted_templates()
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["a", "b"]
    );
    assert_eq!(
        loaded.template("b").unwrap().last_used_at.as_deref(),
        Some("2026-08-11T14:00:00Z")
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn duplicate_rename_delete_and_missing_ids_match_source_no_op_contracts() {
    let root = fixture_root("editing");
    let store = BookmarkStore::new(root.join("bookmarks.json"));
    store
        .upsert(
            template("a", "Demo", TemplateKind::Preset),
            "2026-08-11T11:00:00Z",
        )
        .unwrap();
    let first = store
        .duplicate("a", "copy-1", "2026-08-11T12:00:00Z")
        .unwrap()
        .unwrap();
    let second = store
        .duplicate("a", "copy-2", "2026-08-11T13:00:00Z")
        .unwrap()
        .unwrap();
    assert_eq!(first.name, "Demo copy");
    assert_eq!(second.name, "Demo copy 2");
    let third = store
        .duplicate("a", "copy-3", "2026-08-11T13:30:00Z")
        .unwrap()
        .unwrap();
    assert_eq!(third.name, "Demo copy 3");
    assert!(!first.pinned);
    assert_eq!(first.last_used_at, None);
    let before_no_ops = fs::read(store.path()).unwrap();
    assert!(
        !store
            .rename("missing", "Ignored", "2026-08-11T14:00:00Z")
            .unwrap()
    );
    assert!(!store.rename("a", "   ", "2026-08-11T14:00:00Z").unwrap());
    assert!(
        !store
            .set_pinned("missing", true, "2026-08-11T14:00:00Z")
            .unwrap()
    );
    assert!(!store.record_use("missing", "2026-08-11T14:00:00Z").unwrap());
    assert_eq!(
        store
            .duplicate("missing", "unused", "2026-08-11T14:00:00Z")
            .unwrap(),
        None
    );
    assert!(!store.delete("missing", "2026-08-11T15:00:00Z").unwrap());
    assert_eq!(fs::read(store.path()).unwrap(), before_no_ops);
    assert!(
        store
            .rename("a", " Renamed ", "2026-08-11T14:00:00Z")
            .unwrap()
    );
    assert!(store.delete("copy-1", "2026-08-11T15:00:00Z").unwrap());
    assert_eq!(store.load().unwrap().template("a").unwrap().name, "Renamed");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn store_rejects_every_invalid_template_boundary_and_preserves_error_sources() {
    let root = fixture_root("validation");
    let store = BookmarkStore::new(root.join("bookmarks.json"));
    for invalid in [
        {
            let mut value = template("valid", "Name", TemplateKind::Preset);
            value.schema_version = WorkspaceTemplate::CURRENT_SCHEMA_VERSION + 1;
            value
        },
        template("  ", "Name", TemplateKind::Preset),
        template("valid", "  ", TemplateKind::Preset),
    ] {
        assert!(store.upsert(invalid, "2026-08-11T12:00:00Z").is_err());
    }
    let storage = BookmarkStoreError::Storage(AtomicFileStoreError::Transaction("cause".into()));
    assert_eq!(storage.to_string(), "cause");
    assert!(storage.source().is_some());
    let invalid = BookmarkStoreError::InvalidTemplate("reason".into());
    assert_eq!(invalid.to_string(), "invalid template: reason");
    assert!(invalid.source().is_none());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn corrupt_input_is_quarantined_future_input_is_preserved_and_symlink_is_rejected() {
    let root = fixture_root("boundaries");
    let path = root.join("bookmarks.json");
    fs::write(&path, b"not json").unwrap();
    let store = BookmarkStore::new(&path);
    let loaded = store.load().unwrap();
    assert!(loaded.templates.is_empty());
    let quarantine = loaded.quarantined_path.unwrap();
    assert_eq!(fs::read(quarantine).unwrap(), b"not json");

    let future = format!(
        "{{\"schemaVersion\":{},\"savedAt\":\"later\",\"templates\":[]}}",
        zentty_core::WorkspaceTemplateBundle::CURRENT_SCHEMA_VERSION + 1
    );
    fs::write(&path, &future).unwrap();
    assert!(matches!(
        store.load(),
        Err(BookmarkStoreError::FutureSchema { .. })
    ));
    assert_eq!(fs::read_to_string(&path).unwrap(), future);

    let mut future_template = template("future-template", "Future", TemplateKind::Preset);
    future_template.schema_version = WorkspaceTemplate::CURRENT_SCHEMA_VERSION + 1;
    let future_template_bundle = serde_json::json!({
        "schemaVersion": zentty_core::WorkspaceTemplateBundle::CURRENT_SCHEMA_VERSION,
        "savedAt": "later",
        "templates": [future_template],
    });
    fs::write(&path, serde_json::to_vec(&future_template_bundle).unwrap()).unwrap();
    assert!(matches!(
        store.load(),
        Err(BookmarkStoreError::FutureSchema { .. })
    ));

    fs::remove_file(&path).unwrap();
    let target = root.join("dotfiles.json");
    fs::write(&target, b"{}").unwrap();
    symlink(&target, &path).unwrap();
    assert!(matches!(
        store.load(),
        Err(BookmarkStoreError::UnsafePath(_))
    ));
    assert!(path.is_symlink());
    let linked = BookmarkStore::new_resolving_final_symlink(&path).unwrap();
    linked
        .upsert(
            template("linked", "Linked", TemplateKind::Bookmark),
            "2026-08-11T16:00:00Z",
        )
        .unwrap();
    assert!(path.is_symlink());
    assert_eq!(linked.load().unwrap().templates[0].id, "linked");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn final_symlink_resolution_rejects_dangling_directory_and_uninspectable_paths() {
    let root = fixture_root("symlink-errors");
    let missing = root.join("new-bookmarks.json");
    let new_store = BookmarkStore::new_resolving_final_symlink(&missing).unwrap();
    assert_eq!(new_store.path(), missing);

    let dangling = root.join("dangling.json");
    symlink(root.join("absent.json"), &dangling).unwrap();
    assert!(BookmarkStore::new_resolving_final_symlink(&dangling).is_err());

    let directory = root.join("directory");
    fs::create_dir(&directory).unwrap();
    let directory_link = root.join("directory-link.json");
    symlink(&directory, &directory_link).unwrap();
    assert!(BookmarkStore::new_resolving_final_symlink(&directory_link).is_err());

    let real_parent = root.join("real-parent");
    fs::create_dir(&real_parent).unwrap();
    fs::write(real_parent.join("bookmarks.json"), b"{}").unwrap();
    let linked_parent = root.join("linked-parent");
    symlink(&real_parent, &linked_parent).unwrap();
    let through_ancestor =
        BookmarkStore::new_resolving_final_symlink(linked_parent.join("bookmarks.json")).unwrap();
    assert!(matches!(
        through_ancestor.load(),
        Err(BookmarkStoreError::UnsafePath(_))
    ));

    let inaccessible = root.join("inaccessible");
    fs::create_dir(&inaccessible).unwrap();
    fs::set_permissions(&inaccessible, fs::Permissions::from_mode(0o000)).unwrap();
    let result = BookmarkStore::new_resolving_final_symlink(inaccessible.join("bookmarks.json"));
    fs::set_permissions(&inaccessible, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn portable_export_and_import_are_bounded_reset_identity_and_reject_future_schema() {
    let bookmark = template("source", "Project", TemplateKind::Bookmark);
    let bytes = WorkspaceTemplateExportEnvelope::export(bookmark, "2026-08-11T12:00:00Z").unwrap();
    let imported =
        WorkspaceTemplateExportEnvelope::import(&bytes, "fresh", "2026-08-11T13:00:00Z").unwrap();
    assert_eq!(imported.id, "fresh");
    assert_eq!(imported.kind, TemplateKind::Preset);
    assert_eq!(imported.project_root, None);
    assert!(
        imported
            .all_panes()
            .all(|pane| pane.working_directory.is_none())
    );
    assert_eq!(
        imported.columns[0].panes[0].environment,
        BTreeMap::from([("NODE_ENV".into(), "test".into())])
    );
    assert!(!imported.pinned);
    assert_eq!(imported.last_used_at, None);

    let future = format!(
        "{{\"schemaVersion\":{},\"exportedAt\":\"later\",\"template\":{}}}",
        WorkspaceTemplateExportEnvelope::CURRENT_SCHEMA_VERSION + 1,
        serde_json::to_string(&template("source", "Project", TemplateKind::Preset)).unwrap()
    );
    assert!(matches!(
        WorkspaceTemplateExportEnvelope::import(future.as_bytes(), "fresh", "2026-08-11T13:00:00Z"),
        Err(BookmarkStoreError::FutureExportSchema { .. })
    ));
    assert!(matches!(
        WorkspaceTemplateExportEnvelope::import(
            &vec![b'x'; BookmarkStore::MAX_FILE_BYTES + 1],
            "fresh",
            "2026-08-11T13:00:00Z"
        ),
        Err(BookmarkStoreError::ImportTooLarge { .. })
    ));

    let mut future_template = template("source", "Project", TemplateKind::Preset);
    future_template.schema_version = WorkspaceTemplate::CURRENT_SCHEMA_VERSION + 1;
    let future_template = serde_json::json!({
        "schemaVersion": WorkspaceTemplateExportEnvelope::CURRENT_SCHEMA_VERSION,
        "exportedAt": "later",
        "template": future_template,
    });
    assert!(matches!(
        WorkspaceTemplateExportEnvelope::import(
            serde_json::to_string(&future_template).unwrap().as_bytes(),
            "fresh",
            "2026-08-11T13:00:00Z"
        ),
        Err(BookmarkStoreError::FutureTemplateSchema { .. })
    ));

    let valid = WorkspaceTemplateExportEnvelope::export(
        template("exact", "Exact", TemplateKind::Preset),
        "2026-08-11T12:00:00Z",
    )
    .unwrap();
    let mut exact_limit = valid;
    exact_limit.resize(BookmarkStore::MAX_FILE_BYTES, b' ');
    assert!(
        WorkspaceTemplateExportEnvelope::import(
            &exact_limit,
            "fresh-exact",
            "2026-08-11T13:00:00Z"
        )
        .is_ok()
    );
}
