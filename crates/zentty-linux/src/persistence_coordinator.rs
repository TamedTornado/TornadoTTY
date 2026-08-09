use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

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
    pub(crate) warning: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct LiveSnapshotContent {
    window: WindowRecipe,
    restored_drafts: Vec<PaneRestoreDraft>,
    default_working_directory: String,
}

#[derive(Clone, Debug)]
struct PendingLiveSnapshot {
    content: LiveSnapshotContent,
    deadline: Duration,
}

enum PersistenceWorkerRequest {
    Persist {
        request: PersistenceRequest,
        generation: u64,
    },
    PersistSynchronously {
        request: PersistenceRequest,
        generation: u64,
        clean_exit_timestamp: Option<f64>,
        response: mpsc::SyncSender<Result<bool, String>>,
    },
    #[cfg(test)]
    Synchronize(mpsc::SyncSender<()>),
    Shutdown,
}

struct PersistenceWorker {
    requests: mpsc::Sender<PersistenceWorkerRequest>,
    results: mpsc::Receiver<Result<bool, String>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl PersistenceWorker {
    fn spawn(store: SessionRestoreStore) -> Result<Self, String> {
        let (request_sender, request_receiver) = mpsc::channel();
        let (result_sender, result_receiver) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("zentty-session-persistence".to_owned())
            .spawn(move || {
                let mut persistence = SnapshotPersistence::new(store);
                while let Ok(request) = request_receiver.recv() {
                    match request {
                        PersistenceWorkerRequest::Persist {
                            request,
                            generation,
                        } => {
                            let result = persistence
                                .persist(request, generation)
                                .map_err(|error| error.to_string());
                            let _ = result_sender.send(result);
                        }
                        PersistenceWorkerRequest::PersistSynchronously {
                            request,
                            generation,
                            clean_exit_timestamp,
                            response,
                        } => {
                            let result = persistence
                                .persist(request, generation)
                                .map_err(|error| error.to_string())
                                .and_then(|accepted| {
                                    if !accepted {
                                        return Ok(false);
                                    }
                                    if let Some(updated_at) = clean_exit_timestamp {
                                        persistence
                                            .store()
                                            .mark_clean_exit(updated_at)
                                            .map_err(|error| error.to_string())?;
                                    }
                                    Ok(accepted)
                                });
                            let _ = response.send(result);
                        }
                        #[cfg(test)]
                        PersistenceWorkerRequest::Synchronize(response) => {
                            let _ = response.send(());
                        }
                        PersistenceWorkerRequest::Shutdown => break,
                    }
                }
            })
            .map_err(|error| format!("could not start session-persistence worker: {error}"))?;
        Ok(Self {
            requests: request_sender,
            results: result_receiver,
            thread: Some(thread),
        })
    }

    fn persist(&self, request: PersistenceRequest, generation: u64) -> Result<(), String> {
        self.requests
            .send(PersistenceWorkerRequest::Persist {
                request,
                generation,
            })
            .map_err(|_| "session-persistence worker stopped before accepting a save".to_owned())
    }

    fn persist_synchronously(
        &self,
        request: PersistenceRequest,
        generation: u64,
        clean_exit_timestamp: Option<f64>,
    ) -> Result<bool, String> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.requests
            .send(PersistenceWorkerRequest::PersistSynchronously {
                request,
                generation,
                clean_exit_timestamp,
                response: sender,
            })
            .map_err(|_| {
                "session-persistence worker stopped before accepting a synchronous save".to_owned()
            })?;
        receiver.recv().map_err(|_| {
            "session-persistence worker stopped before completing a synchronous save".to_owned()
        })?
    }

    fn drain_errors(&self) -> Vec<String> {
        self.results.try_iter().filter_map(Result::err).collect()
    }

    #[cfg(test)]
    fn synchronize(&self) {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.requests
            .send(PersistenceWorkerRequest::Synchronize(sender))
            .unwrap();
        receiver.recv().unwrap();
    }
}

impl Drop for PersistenceWorker {
    fn drop(&mut self) {
        let _ = self.requests.send(PersistenceWorkerRequest::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(crate) struct PersistenceCoordinator {
    worker: PersistenceWorker,
    next_generation: u64,
    phase: PersistencePhase,
    normal_restore_enabled: bool,
    prepared_restore: bool,
    last_observed_live_snapshot: Option<LiveSnapshotContent>,
    pending_live_snapshot: Option<PendingLiveSnapshot>,
}

impl PersistenceCoordinator {
    const LIVE_SNAPSHOT_DEBOUNCE: Duration = Duration::from_millis(350);

    pub(crate) fn start(
        state_directory: &Path,
        restore_enabled: bool,
        now: f64,
    ) -> Result<(Self, LaunchProjection), String> {
        let store = SessionRestoreStore::new(
            state_directory.join("restore-snapshot.json"),
            state_directory.join("restore-lifecycle.json"),
        );
        let (launch_decision, mut warning) = match store.prepare_for_launch(restore_enabled) {
            Ok(decision) => (decision, None),
            Err(error) => (
                None,
                Some(format!("Failed to prepare restore launch: {error}")),
            ),
        };
        let restored_window = match launch_decision.as_ref() {
            Some(decision) => match select_restored_window(&decision.envelope.workspace) {
                Ok(window) => Some(window),
                Err(error) => {
                    store.consume_snapshot().map_err(|delete_error| {
                        format!(
                            "Prepared restore snapshot was unusable ({error}) and could not be deleted: {delete_error}"
                        )
                    })?;
                    warning = Some(format!(
                        "Prepared restore snapshot was unusable and was deleted: {error}"
                    ));
                    None
                }
            },
            None => None,
        };
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
                worker: PersistenceWorker::spawn(store)?,
                next_generation: 0,
                phase: PersistencePhase::Running,
                normal_restore_enabled: restore_enabled,
                prepared_restore: restored_window.is_some(),
                last_observed_live_snapshot: None,
                pending_live_snapshot: None,
            },
            LaunchProjection {
                restored_window,
                restored_drafts,
                warning,
            },
        ))
    }

    pub(crate) fn complete_launch(&mut self) -> Result<(), String> {
        if !self.prepared_restore {
            return Ok(());
        }
        self.next_generation = self.next_generation.wrapping_add(1);
        let accepted = self.worker.persist_synchronously(
            PersistenceRequest::DeleteSnapshot,
            self.next_generation,
            None,
        )?;
        if !accepted {
            return Err("restore snapshot consumption rejected a current generation".to_owned());
        }
        self.prepared_restore = false;
        Ok(())
    }

    pub(crate) fn observe_live_snapshot(
        &mut self,
        window: WindowRecipe,
        restored_drafts: Vec<PaneRestoreDraft>,
        default_working_directory: &str,
        observed_at: Duration,
    ) -> bool {
        let content = LiveSnapshotContent {
            window,
            restored_drafts,
            default_working_directory: default_working_directory.to_owned(),
        };
        if self.last_observed_live_snapshot.as_ref() == Some(&content) {
            return false;
        }
        self.last_observed_live_snapshot = Some(content.clone());
        self.pending_live_snapshot = Some(PendingLiveSnapshot {
            content,
            deadline: observed_at.saturating_add(Self::LIVE_SNAPSHOT_DEBOUNCE),
        });
        true
    }

    pub(crate) fn flush_live_snapshot_if_due(
        &mut self,
        now: Duration,
        saved_at: f64,
    ) -> Result<bool, String> {
        let Some(pending) = self.pending_live_snapshot.as_ref() else {
            return Ok(false);
        };
        if now < pending.deadline {
            return Ok(false);
        }
        let Some(pending) = self.pending_live_snapshot.take() else {
            return Ok(false);
        };
        let request = snapshot_request(
            pending.content,
            self.normal_restore_enabled,
            saved_at,
            SaveReason::LiveSnapshot,
        );
        self.next_generation = self.next_generation.wrapping_add(1);
        self.worker.persist(request, self.next_generation)?;
        Ok(true)
    }

    pub(crate) fn drain_live_snapshot_errors(&self) -> Vec<String> {
        self.worker.drain_errors()
    }

    pub(crate) fn save_clean_exit(
        &mut self,
        window: WindowRecipe,
        restored_drafts: Vec<PaneRestoreDraft>,
        default_working_directory: &str,
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
        let request = snapshot_request(
            LiveSnapshotContent {
                window,
                restored_drafts,
                default_working_directory: default_working_directory.to_owned(),
            },
            self.normal_restore_enabled,
            now,
            SaveReason::CleanExit,
        );
        self.next_generation = self.next_generation.wrapping_add(1);
        let result = self
            .worker
            .persist_synchronously(request, self.next_generation, Some(now))
            .and_then(|accepted| {
                accepted.then_some(()).ok_or_else(|| {
                    "clean-exit persistence rejected a current generation".to_owned()
                })
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

fn snapshot_request(
    content: LiveSnapshotContent,
    normal_restore_enabled: bool,
    saved_at: f64,
    reason: SaveReason,
) -> PersistenceRequest {
    let has_restore_drafts = !content.restored_drafts.is_empty();
    let window_id = content.window.id.clone();
    let workspace = WorkspaceRecipe {
        schema_version: Some(WorkspaceRecipe::CURRENT_SCHEMA_VERSION),
        active_window_id: Some(window_id.clone()),
        windows: vec![content.window],
    };
    if !has_restore_drafts
        && (!normal_restore_enabled || !workspace.is_meaningful(&content.default_working_directory))
    {
        return PersistenceRequest::DeleteSnapshot;
    }
    let restore_draft_windows = if content.restored_drafts.is_empty() {
        Vec::new()
    } else {
        vec![SessionRestoreDraftWindow {
            window_id,
            pane_drafts: content.restored_drafts,
        }]
    };
    PersistenceRequest::SaveSnapshot(SessionRestoreEnvelope {
        schema_version: 1,
        saved_at,
        reason,
        workspace,
        restore_draft_windows,
    })
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
    use super::{
        CleanExitDecision, LiveSnapshotContent, PersistenceCoordinator, PersistencePhase,
        clean_exit_decision, snapshot_request,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;
    use zentty_core::{
        PersistenceRequest, SaveReason, SessionRestoreEnvelope, SessionRestoreStore,
    };

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

        let (mut coordinator, launch) =
            PersistenceCoordinator::start(&directory.0, true, 2.0).unwrap();
        assert_eq!(coordinator.phase, PersistencePhase::Running);
        assert_eq!(launch.restored_window.unwrap().id, "window-main");
        assert_eq!(launch.restored_drafts.len(), 1);
        assert!(launch.warning.is_none());
        assert!(store.snapshot_path().is_file());
        coordinator.complete_launch().unwrap();
        assert!(!store.snapshot_path().exists());
        assert_eq!(store.prepare_for_launch(false).unwrap(), None,);
    }

    #[test]
    fn unreadable_snapshot_is_reported_and_allows_a_fresh_launch() {
        let directory = TestDirectory::new("unreadable");
        let store = directory.store();
        fs::write(store.snapshot_path(), b"not valid JSON").unwrap();

        let (coordinator, launch) = PersistenceCoordinator::start(&directory.0, true, 2.0).unwrap();

        assert!(launch.restored_window.is_none());
        assert!(launch.restored_drafts.is_empty());
        assert!(
            launch
                .warning
                .as_deref()
                .unwrap()
                .contains("Failed to prepare restore launch")
        );
        assert_eq!(coordinator.phase, PersistencePhase::Running);
        assert!(store.prepare_for_launch(true).is_err());
        assert!(store.prepare_for_launch(false).is_err());
    }

    #[test]
    fn live_snapshot_waits_for_quiet_time_and_resets_the_deadline_on_change() {
        let directory = TestDirectory::new("live-debounce");
        let (mut coordinator, _) = PersistenceCoordinator::start(&directory.0, true, 1.0).unwrap();
        let source = envelope();
        let mut window = source.workspace.windows[0].clone();
        let drafts = source.restore_draft_windows[0].pane_drafts.clone();

        assert!(coordinator.observe_live_snapshot(
            window.clone(),
            drafts.clone(),
            "/tmp",
            Duration::ZERO,
        ));
        assert!(
            !coordinator
                .flush_live_snapshot_if_due(Duration::from_millis(349), 2.0)
                .unwrap()
        );
        window.worklanes[0].title = Some("Changed during debounce".to_owned());
        assert!(coordinator.observe_live_snapshot(
            window,
            drafts,
            "/tmp",
            Duration::from_millis(300),
        ));
        assert!(
            !coordinator
                .flush_live_snapshot_if_due(Duration::from_millis(649), 3.0)
                .unwrap()
        );
        assert!(
            coordinator
                .flush_live_snapshot_if_due(Duration::from_millis(650), 4.0)
                .unwrap()
        );
        coordinator.worker.synchronize();

        let restored = directory
            .store()
            .prepare_for_launch(false)
            .unwrap()
            .unwrap();
        assert_eq!(restored.envelope.reason, SaveReason::LiveSnapshot);
        assert_eq!(
            restored.envelope.workspace.windows[0].worklanes[0]
                .title
                .as_deref(),
            Some("Changed during debounce")
        );
    }

    #[test]
    fn unchanged_observations_do_not_postpone_or_repeat_a_live_snapshot() {
        let directory = TestDirectory::new("live-unchanged");
        let (mut coordinator, _) = PersistenceCoordinator::start(&directory.0, true, 1.0).unwrap();
        let source = envelope();
        let window = source.workspace.windows[0].clone();
        let drafts = source.restore_draft_windows[0].pane_drafts.clone();

        assert!(coordinator.observe_live_snapshot(
            window.clone(),
            drafts.clone(),
            "/tmp",
            Duration::ZERO,
        ));
        assert!(!coordinator.observe_live_snapshot(
            window,
            drafts,
            "/tmp",
            Duration::from_millis(300),
        ));
        assert!(
            coordinator
                .flush_live_snapshot_if_due(Duration::from_millis(350), 2.0)
                .unwrap()
        );
        coordinator.worker.synchronize();
        assert!(
            !coordinator
                .flush_live_snapshot_if_due(Duration::from_secs(1), 3.0)
                .unwrap()
        );
    }

    #[test]
    fn asynchronous_live_snapshot_failure_is_reported_back_to_the_ui_owner() {
        let directory = TestDirectory::new("live-failure");
        let (mut coordinator, _) = PersistenceCoordinator::start(&directory.0, true, 1.0).unwrap();
        let source = envelope();
        assert!(coordinator.observe_live_snapshot(
            source.workspace.windows[0].clone(),
            source.restore_draft_windows[0].pane_drafts.clone(),
            "/tmp",
            Duration::ZERO,
        ));
        fs::remove_dir_all(&directory.0).unwrap();
        fs::write(&directory.0, "not a directory").unwrap();

        assert!(
            coordinator
                .flush_live_snapshot_if_due(Duration::from_millis(350), 2.0)
                .unwrap()
        );
        coordinator.worker.synchronize();
        let errors = coordinator.drain_live_snapshot_errors();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("persist JSON failed"));
        assert!(coordinator.drain_live_snapshot_errors().is_empty());
    }

    #[test]
    fn pristine_default_and_disabled_restore_delete_instead_of_retaining_snapshots() {
        let source = envelope();
        let mut window = source.workspace.windows[0].clone();
        window.worklanes.truncate(1);
        window.worklanes[0].title = None;
        window.worklanes[0].next_pane_number = 2;
        window.worklanes[0].columns.truncate(1);
        window.worklanes[0].columns[0].panes.truncate(1);
        let worklane_id = window.worklanes[0].id.clone();
        let column_id = window.worklanes[0].columns[0].id.clone();
        let pane_id = window.worklanes[0].columns[0].panes[0].id.clone();
        window.active_worklane_id = Some(worklane_id);
        window.worklanes[0].focused_column_id = Some(column_id);
        let column = &mut window.worklanes[0].columns[0];
        column.focused_pane_id = Some(pane_id.clone());
        column.last_focused_pane_id = Some(pane_id);
        let pane = &mut column.panes[0];
        pane.custom_title = None;
        pane.title_seed = Some("shell".to_owned());
        pane.working_directory = Some("/tmp".to_owned());
        pane.last_activity_title = None;
        pane.last_run_command = None;
        let content = LiveSnapshotContent {
            window: window.clone(),
            restored_drafts: Vec::new(),
            default_working_directory: "/tmp".to_owned(),
        };

        assert_eq!(
            snapshot_request(content.clone(), true, 1.0, SaveReason::CleanExit),
            PersistenceRequest::DeleteSnapshot
        );
        assert!(matches!(
            snapshot_request(
                LiveSnapshotContent {
                    restored_drafts: source.restore_draft_windows[0].pane_drafts.clone(),
                    ..content
                },
                true,
                1.5,
                SaveReason::CleanExit,
            ),
            PersistenceRequest::SaveSnapshot(_)
        ));
        assert!(matches!(
            snapshot_request(
                LiveSnapshotContent {
                    window: window.clone(),
                    restored_drafts: source.restore_draft_windows[0].pane_drafts.clone(),
                    default_working_directory: "/tmp".to_owned(),
                },
                false,
                1.75,
                SaveReason::CleanExit,
            ),
            PersistenceRequest::SaveSnapshot(_)
        ));
        window.worklanes.push(window.worklanes[0].clone());
        assert!(matches!(
            snapshot_request(
                LiveSnapshotContent {
                    window,
                    restored_drafts: Vec::new(),
                    default_working_directory: "/tmp".to_owned(),
                },
                false,
                2.0,
                SaveReason::CleanExit,
            ),
            PersistenceRequest::DeleteSnapshot
        ));
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

        coordinator
            .save_clean_exit(window, drafts, "/tmp", 2.0)
            .unwrap();
        assert_eq!(coordinator.phase, PersistencePhase::Complete);
        assert!(
            coordinator
                .save_clean_exit(source.workspace.windows[0].clone(), Vec::new(), "/tmp", 3.0,)
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
                .save_clean_exit(window.clone(), Vec::new(), "/tmp", 2.0)
                .is_err()
        );
        assert_eq!(coordinator.phase, PersistencePhase::Failed);
        assert!(
            coordinator
                .save_clean_exit(window, Vec::new(), "/tmp", 3.0)
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
                .save_clean_exit(window, Vec::new(), "/tmp", 2.0)
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
