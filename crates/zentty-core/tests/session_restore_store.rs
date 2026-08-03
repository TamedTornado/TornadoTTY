use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use zentty_core::{
    AgentLaunchSnapshot, ColumnRecipe, LaunchReason, PaneRecipe, PaneRestoreDraft,
    PersistenceRequest, RestoreDraftKind, SaveReason, SessionRestoreDraftWindow,
    SessionRestoreEnvelope, SessionRestoreStore, SnapshotPersistence, WindowRecipe, WorklaneRecipe,
    WorkspaceRecipe,
};

static DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let sequence = DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "zentty-session-store-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn envelope(window_id: &str, reason: SaveReason) -> SessionRestoreEnvelope {
    SessionRestoreEnvelope {
        schema_version: 1,
        saved_at: 807_321_600.0,
        reason,
        workspace: WorkspaceRecipe {
            schema_version: Some(3),
            windows: vec![WindowRecipe {
                id: window_id.to_owned(),
                frame: None,
                worklanes: vec![],
                active_worklane_id: None,
            }],
            active_window_id: Some(window_id.to_owned()),
        },
        restore_draft_windows: vec![],
    }
}

fn store(directory: &TestDirectory) -> SessionRestoreStore {
    SessionRestoreStore::new(
        directory.0.join("restore-snapshot.json"),
        directory.0.join("restore-lifecycle.json"),
    )
}

#[test]
fn launch_decision_matches_clean_crash_and_preference_behavior() {
    let directory = TestDirectory::new("launch-decision");
    let store = store(&directory);
    assert_eq!(store.prepare_for_launch(true).unwrap(), None);

    store
        .save_snapshot(&envelope("window-main", SaveReason::LiveSnapshot))
        .unwrap();
    store.mark_launch_started(1.0).unwrap();
    assert_eq!(
        store.prepare_for_launch(false).unwrap().unwrap().reason,
        LaunchReason::CrashRecovery
    );

    store.mark_clean_exit(2.0).unwrap();
    assert_eq!(store.prepare_for_launch(false).unwrap(), None);
    let normal = store.prepare_for_launch(true).unwrap().unwrap();
    assert_eq!(normal.reason, LaunchReason::NormalRestore);
    assert_eq!(normal.envelope.workspace.windows[0].id, "window-main");
}

#[test]
fn snapshot_is_atomic_consumable_and_corruption_is_preserved() {
    let directory = TestDirectory::new("atomic");
    let store = store(&directory);
    store
        .save_snapshot(&envelope("window-main", SaveReason::LiveSnapshot))
        .unwrap();
    assert!(store.snapshot_path().is_file());
    assert!(!directory.0.join("restore-snapshot.json.bak").exists());
    assert!(fs::read_dir(&directory.0).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".tmp.")
    }));

    store.consume_snapshot().unwrap();
    assert!(!store.snapshot_path().exists());

    let corrupt = b"not valid json";
    fs::write(store.snapshot_path(), corrupt).unwrap();
    assert!(store.prepare_for_launch(true).is_err());
    assert_eq!(fs::read(store.snapshot_path()).unwrap(), corrupt);
}

#[test]
fn clean_exit_keeps_missing_live_restore_drafts_for_still_present_panes() {
    let directory = TestDirectory::new("merge-drafts");
    let store = store(&directory);
    let mut live = envelope_with_panes(SaveReason::LiveSnapshot, &["pane-keep", "pane-removed"]);
    live.restore_draft_windows = vec![draft_window(&["pane-keep", "pane-removed"])];
    store.save_snapshot(&live).unwrap();

    let clean = envelope_with_panes(SaveReason::CleanExit, &["pane-keep"]);
    store.save_snapshot(&clean).unwrap();
    let restored = store.prepare_for_launch(true).unwrap().unwrap().envelope;

    assert_eq!(restored.reason, SaveReason::CleanExit);
    assert_eq!(restored.restore_draft_windows.len(), 1);
    assert_eq!(restored.restore_draft_windows[0].pane_drafts.len(), 1);
    assert_eq!(
        restored.restore_draft_windows[0].pane_drafts[0].pane_id,
        "pane-keep"
    );
}

#[test]
fn persistence_generation_refuses_stale_requests() {
    let directory = TestDirectory::new("generation");
    let mut persistence = SnapshotPersistence::new(store(&directory));

    assert!(
        persistence
            .persist(
                PersistenceRequest::SaveSnapshot(envelope("window-current", SaveReason::CleanExit)),
                2,
            )
            .unwrap()
    );
    assert!(
        !persistence
            .persist(
                PersistenceRequest::SaveSnapshot(envelope(
                    "window-stale",
                    SaveReason::LiveSnapshot
                )),
                1,
            )
            .unwrap()
    );

    let decision = persistence
        .store()
        .prepare_for_launch(true)
        .unwrap()
        .unwrap();
    assert_eq!(decision.envelope.workspace.windows[0].id, "window-current");
}

fn envelope_with_panes(reason: SaveReason, pane_ids: &[&str]) -> SessionRestoreEnvelope {
    let mut envelope = envelope("window-main", reason);
    envelope.workspace.windows[0].worklanes = vec![WorklaneRecipe {
        id: "worklane-main".to_owned(),
        title: None,
        next_pane_number: 3,
        focused_column_id: Some("column-main".to_owned()),
        columns: vec![ColumnRecipe {
            id: "column-main".to_owned(),
            width: 640.0,
            focused_pane_id: pane_ids.first().map(ToString::to_string),
            last_focused_pane_id: None,
            pane_heights: pane_ids.iter().map(|_| 320.0).collect(),
            panes: pane_ids
                .iter()
                .map(|id| PaneRecipe {
                    id: (*id).to_owned(),
                    custom_title: None,
                    title_seed: None,
                    working_directory: Some("/tmp/project".to_owned()),
                    last_activity_title: None,
                    last_run_command: None,
                })
                .collect(),
        }],
        color: None,
        bookmark_origin_id: None,
    }];
    envelope.workspace.windows[0].active_worklane_id = Some("worklane-main".to_owned());
    envelope
}

fn draft_window(pane_ids: &[&str]) -> SessionRestoreDraftWindow {
    SessionRestoreDraftWindow {
        window_id: "window-main".to_owned(),
        pane_drafts: pane_ids
            .iter()
            .map(|pane_id| PaneRestoreDraft {
                pane_id: (*pane_id).to_owned(),
                kind: RestoreDraftKind::AgentResume,
                tool_name: "Codex".to_owned(),
                session_id: format!("session-{pane_id}"),
                working_directory: Some("/tmp/project".to_owned()),
                tracked_pid: 4242,
                agent_launch_snapshot: Some(AgentLaunchSnapshot {
                    arguments: vec!["codex".to_owned()],
                    environment: None,
                }),
            })
            .collect(),
    }
}
