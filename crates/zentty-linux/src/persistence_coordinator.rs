use std::path::{Path, PathBuf};

use zentty_core::{
    PaneRestoreDraft, PersistenceRequest, SaveReason, SessionRestoreDraftWindow,
    SessionRestoreEnvelope, SessionRestoreStore, SnapshotPersistence, WindowRecipe,
    WorkspaceRecipe, WorkspaceState,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum PersistencePhase {
    #[default]
    Running,
    Saving,
    Complete,
    Failed,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum CleanExitDecision {
    Begin,
    #[default]
    RejectInFlight,
    RejectComplete,
    RejectFailed,
}

fn clean_exit_decision(phase: PersistencePhase) -> CleanExitDecision {
    match phase {
        PersistencePhase::Running => CleanExitDecision::Begin,
        PersistencePhase::Saving => CleanExitDecision::RejectInFlight,
        PersistencePhase::Complete => CleanExitDecision::RejectComplete,
        PersistencePhase::Failed => CleanExitDecision::RejectFailed,
    }
}

pub(crate) struct LaunchProjection {
    pub(crate) restored_window: Option<WindowRecipe>,
    pub(crate) restored_drafts: Vec<PaneRestoreDraft>,
}

pub(crate) struct PersistenceCoordinator {
    persistence: SnapshotPersistence,
    next_generation: u64,
    phase: PersistencePhase,
}

impl PersistenceCoordinator {
    pub(crate) fn start(
        state_directory: &Path,
        restore_enabled: bool,
        now: f64,
    ) -> Result<(Self, LaunchProjection), String> {
        let store = SessionRestoreStore::new(
            state_directory.join("restore-snapshot.json"),
            state_directory.join("restore-lifecycle.json"),
        );
        let launch_decision = store
            .prepare_for_launch(restore_enabled)
            .map_err(|error| error.to_string())?;
        let restored_window = launch_decision
            .as_ref()
            .map(|decision| select_restored_window(&decision.envelope.workspace))
            .transpose()?;
        let restored_drafts = restored_window.as_ref().map_or_else(Vec::new, |window| {
            launch_decision
                .as_ref()
                .and_then(|decision| {
                    decision
                        .envelope
                        .restore_draft_windows
                        .iter()
                        .find(|drafts| drafts.window_id == window.id)
                })
                .map_or_else(Vec::new, |drafts| drafts.pane_drafts.clone())
        });
        store
            .mark_launch_started(now)
            .map_err(|error| error.to_string())?;
        Ok((
            Self {
                persistence: SnapshotPersistence::new(store),
                next_generation: 0,
                phase: PersistencePhase::Running,
            },
            LaunchProjection {
                restored_window,
                restored_drafts,
            },
        ))
    }

    pub(crate) fn save_clean_exit(
        &mut self,
        window: WindowRecipe,
        restored_drafts: Vec<PaneRestoreDraft>,
        now: f64,
    ) -> Result<(), String> {
        match clean_exit_decision(self.phase) {
            CleanExitDecision::Begin => self.phase = PersistencePhase::Saving,
            decision => return Err(format!("clean-exit persistence rejected: {decision:?}")),
        }
        if let Err(error) = WorkspaceState::from_window_recipe(&window) {
            self.phase = PersistencePhase::Failed;
            return Err(format!("clean-exit workspace validation failed: {error}"));
        }
        let window_id = window.id.clone();
        let restore_draft_windows = if restored_drafts.is_empty() {
            Vec::new()
        } else {
            vec![SessionRestoreDraftWindow {
                window_id: window_id.clone(),
                pane_drafts: restored_drafts,
            }]
        };
        let envelope = SessionRestoreEnvelope {
            schema_version: 1,
            saved_at: now,
            reason: SaveReason::CleanExit,
            workspace: WorkspaceRecipe {
                schema_version: Some(WorkspaceRecipe::CURRENT_SCHEMA_VERSION),
                active_window_id: Some(window_id),
                windows: vec![window],
            },
            restore_draft_windows,
        };
        self.next_generation = self.next_generation.wrapping_add(1);
        let result = self
            .persistence
            .persist(
                PersistenceRequest::SaveSnapshot(envelope),
                self.next_generation,
            )
            .map_err(|error| error.to_string())
            .and_then(|accepted| {
                accepted.then_some(()).ok_or_else(|| {
                    "clean-exit persistence rejected a current generation".to_owned()
                })
            })
            .and_then(|()| {
                self.persistence
                    .store()
                    .mark_clean_exit(now)
                    .map_err(|error| error.to_string())
            });
        match result {
            Ok(()) => {
                self.phase = PersistencePhase::Complete;
                Ok(())
            }
            Err(error) => {
                self.phase = PersistencePhase::Failed;
                Err(error)
            }
        }
    }
}

fn select_restored_window(workspace: &WorkspaceRecipe) -> Result<WindowRecipe, String> {
    if workspace.windows.len() != 1 {
        return Err(format!(
            "workspace restore has {} windows; Linux currently requires exactly one",
            workspace.windows.len()
        ));
    }
    let window = &workspace.windows[0];
    if workspace
        .active_window_id
        .as_deref()
        .is_some_and(|id| id != window.id)
    {
        return Err("workspace active window does not exist".to_owned());
    }
    Ok(window.clone())
}

pub(crate) fn default_state_directory() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("XDG_STATE_HOME") {
        return Ok(PathBuf::from(path).join("zentty"));
    }
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".local/state/zentty"))
        .ok_or_else(|| "neither XDG_STATE_HOME nor HOME is set".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{CleanExitDecision, PersistenceCoordinator, PersistencePhase, clean_exit_decision};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use zentty_core::{SaveReason, SessionRestoreEnvelope, SessionRestoreStore};

    const V3_ENVELOPE: &[u8] =
        include_bytes!("../../zentty-core/tests/fixtures/session-restore-v3.json");
    static DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let sequence = DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "zentty-persistence-coordinator-{label}-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn store(&self) -> SessionRestoreStore {
            SessionRestoreStore::new(
                self.0.join("restore-snapshot.json"),
                self.0.join("restore-lifecycle.json"),
            )
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            if self.0.is_dir() {
                fs::remove_dir_all(&self.0).unwrap();
            } else if self.0.exists() {
                fs::remove_file(&self.0).unwrap();
            }
        }
    }

    fn envelope() -> SessionRestoreEnvelope {
        SessionRestoreEnvelope::from_json(V3_ENVELOPE).unwrap()
    }

    #[test]
    fn clean_exit_is_single_flight_and_terminal_after_success_or_failure() {
        assert_eq!(
            clean_exit_decision(PersistencePhase::Running),
            CleanExitDecision::Begin
        );
        assert_eq!(
            clean_exit_decision(PersistencePhase::Saving),
            CleanExitDecision::RejectInFlight
        );
        assert_eq!(
            clean_exit_decision(PersistencePhase::Complete),
            CleanExitDecision::RejectComplete
        );
        assert_eq!(
            clean_exit_decision(PersistencePhase::Failed),
            CleanExitDecision::RejectFailed
        );
    }

    #[test]
    fn startup_projects_the_existing_snapshot_and_marks_the_launch_unclean() {
        let directory = TestDirectory::new("startup");
        let store = directory.store();
        store.save_snapshot(&envelope()).unwrap();
        store.mark_clean_exit(1.0).unwrap();

        let (coordinator, launch) = PersistenceCoordinator::start(&directory.0, true, 2.0).unwrap();
        assert_eq!(coordinator.phase, PersistencePhase::Running);
        assert_eq!(launch.restored_window.unwrap().id, "window-main");
        assert_eq!(launch.restored_drafts.len(), 1);
        assert_eq!(
            store
                .prepare_for_launch(false)
                .unwrap()
                .unwrap()
                .envelope
                .reason,
            SaveReason::LiveSnapshot
        );
    }

    #[test]
    fn clean_shutdown_saves_once_then_marks_clean() {
        let directory = TestDirectory::new("clean");
        let (mut coordinator, launch) =
            PersistenceCoordinator::start(&directory.0, true, 1.0).unwrap();
        assert!(launch.restored_window.is_none());
        let source = envelope();
        let window = source.workspace.windows[0].clone();
        let drafts = source.restore_draft_windows[0].pane_drafts.clone();

        coordinator.save_clean_exit(window, drafts, 2.0).unwrap();
        assert_eq!(coordinator.phase, PersistencePhase::Complete);
        assert!(
            coordinator
                .save_clean_exit(source.workspace.windows[0].clone(), Vec::new(), 3.0)
                .is_err()
        );
        let store = directory.store();
        assert!(store.prepare_for_launch(false).unwrap().is_none());
        let restored = store.prepare_for_launch(true).unwrap().unwrap();
        assert_eq!(restored.envelope.reason, SaveReason::CleanExit);
        assert_eq!(restored.envelope.restore_draft_windows.len(), 1);
    }

    #[test]
    fn save_failure_is_terminal_and_never_marks_the_lifecycle_clean() {
        let directory = TestDirectory::new("failure");
        let (mut coordinator, _) = PersistenceCoordinator::start(&directory.0, false, 1.0).unwrap();
        fs::remove_dir_all(&directory.0).unwrap();
        fs::write(&directory.0, "not a directory").unwrap();
        let window = envelope().workspace.windows[0].clone();

        assert!(
            coordinator
                .save_clean_exit(window.clone(), Vec::new(), 2.0)
                .is_err()
        );
        assert_eq!(coordinator.phase, PersistencePhase::Failed);
        assert!(
            coordinator
                .save_clean_exit(window, Vec::new(), 3.0)
                .is_err()
        );
    }

    #[test]
    fn invalid_frozen_workspace_is_rejected_before_snapshot_publication() {
        let directory = TestDirectory::new("invalid");
        let (mut coordinator, _) = PersistenceCoordinator::start(&directory.0, false, 1.0).unwrap();
        let mut window = envelope().workspace.windows[0].clone();
        window.worklanes.clear();

        assert!(
            coordinator
                .save_clean_exit(window, Vec::new(), 2.0)
                .is_err()
        );
        assert_eq!(coordinator.phase, PersistencePhase::Failed);
        assert!(!directory.store().snapshot_path().exists());
        assert!(
            directory
                .store()
                .prepare_for_launch(false)
                .unwrap()
                .is_none(),
            "no snapshot exists to restore despite the unclean lifecycle marker"
        );
    }
}
