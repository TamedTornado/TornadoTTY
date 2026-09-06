use super::super::tests::{TestDirectory, envelope};
use super::*;
use zentty_core::SaveReason;

// Hold scheduling, not persistence: the worker is started explicitly after
// submissions. Every accepted save still executes against the real file store.
fn parked_worker() -> PersistenceWorker {
    PersistenceWorker {
        shared: Arc::new(Shared::default()),
        thread: None,
    }
}

fn start(worker: &mut PersistenceWorker, store: SessionRestoreStore) {
    let shared = Arc::clone(&worker.shared);
    worker.thread = Some(thread::spawn(move || run(&shared, store)));
}

#[test]
fn burst_retains_only_latest_snapshot_and_rejects_stale_replacement() {
    let directory = TestDirectory::new("worker-burst");
    let store = directory.store();
    let mut worker = parked_worker();
    assert_eq!(worker.take_pressure(), None);
    for generation in 1..=1000 {
        let mut snapshot = envelope();
        snapshot.workspace.windows[0].worklanes[0].title = Some(format!("revision-{generation}"));
        worker
            .persist(PersistenceRequest::SaveSnapshot(snapshot), generation)
            .unwrap();
    }
    assert!(
        worker
            .persist(PersistenceRequest::DeleteSnapshot, 999)
            .is_err()
    );
    {
        let mailbox = worker.shared.mailbox.lock().unwrap();
        assert_eq!(mailbox.pending.as_ref().unwrap().generation, 1000);
        assert_eq!(mailbox.coalesced, 999);
        assert!(mailbox.control.is_none());
    }
    assert_eq!(worker.take_pressure(), Some(999));
    assert_eq!(worker.take_pressure(), None);
    start(&mut worker, store.clone());
    worker.synchronize();
    let restored = store.prepare_for_launch(true).unwrap().unwrap().envelope;
    assert_eq!(
        restored.workspace.windows[0].worklanes[0].title.as_deref(),
        Some("revision-1000")
    );
    assert_eq!(
        restored.restore_draft_windows,
        envelope().restore_draft_windows
    );
}

#[test]
fn final_save_merges_pending_drafts_and_prevents_later_writes() {
    let directory = TestDirectory::new("worker-final");
    let store = directory.store();
    store.mark_launch_started(1.0).unwrap();
    let mut worker = parked_worker();
    worker
        .persist(PersistenceRequest::SaveSnapshot(envelope()), 1)
        .unwrap();
    let mut final_snapshot = envelope();
    final_snapshot.reason = SaveReason::CleanExit;
    final_snapshot.restore_draft_windows.clear();
    let (response, receiver) = mpsc::sync_channel(1);
    worker
        .control(Control::Save {
            save: Save {
                request: PersistenceRequest::SaveSnapshot(final_snapshot),
                generation: 2,
            },
            clean_exit_timestamp: Some(3.0),
            response,
        })
        .unwrap();
    assert!(
        worker
            .persist(PersistenceRequest::DeleteSnapshot, 3)
            .is_err()
    );
    let (duplicate, _) = mpsc::sync_channel(1);
    assert!(worker.control(Control::Flush(duplicate)).is_err());
    start(&mut worker, store.clone());
    assert_eq!(receiver.recv().unwrap(), Ok(true));
    assert!(
        worker
            .persist(PersistenceRequest::DeleteSnapshot, 4)
            .is_err()
    );
    assert!(
        store.prepare_for_launch(false).unwrap().is_none(),
        "clean exit must obey disabled restore"
    );
    let restored = store.prepare_for_launch(true).unwrap().unwrap().envelope;
    assert_eq!(restored.reason, SaveReason::CleanExit);
    assert_eq!(
        restored.restore_draft_windows,
        envelope().restore_draft_windows
    );
}

#[test]
fn failure_summary_is_bounded_and_survives_a_later_success() {
    let directory = TestDirectory::new("worker-errors");
    let store = directory.store();
    let worker = PersistenceWorker::spawn(store.clone()).unwrap();
    std::fs::remove_dir(&directory.0).unwrap();
    std::fs::write(&directory.0, "not a directory").unwrap();
    for generation in 1..=10 {
        worker
            .persist(PersistenceRequest::SaveSnapshot(envelope()), generation)
            .unwrap();
        worker.synchronize();
    }
    assert_eq!(
        worker
            .shared
            .mailbox
            .lock()
            .unwrap()
            .failure
            .as_ref()
            .unwrap()
            .0,
        10
    );
    std::fs::remove_file(&directory.0).unwrap();
    std::fs::create_dir(&directory.0).unwrap();
    worker
        .persist(PersistenceRequest::SaveSnapshot(envelope()), 11)
        .unwrap();
    worker.synchronize();
    assert!(store.prepare_for_launch(true).unwrap().is_some());
    let errors = worker.drain_errors();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("10 failed operation(s)"));
    assert!(errors[0].contains("persist JSON failed"));
    assert!(worker.drain_errors().is_empty());
}

#[test]
fn drop_drains_latest_accepted_save() {
    let directory = TestDirectory::new("worker-drop");
    let store = directory.store();
    let mut worker = parked_worker();
    worker
        .persist(PersistenceRequest::SaveSnapshot(envelope()), 1)
        .unwrap();
    start(&mut worker, store.clone());
    // Observe worker termination as well as disk state: a shutdown panic
    // after a successful write is still a failed shutdown.
    let thread = worker.thread.take().unwrap();
    drop(worker);
    thread.join().unwrap();
    assert_eq!(
        store.prepare_for_launch(true).unwrap().unwrap().envelope,
        envelope()
    );
}

#[test]
fn drop_completes_an_accepted_final_barrier() {
    let directory = TestDirectory::new("worker-drop-final");
    let store = directory.store();
    store.mark_launch_started(1.0).unwrap();
    let mut worker = parked_worker();
    let (response, receiver) = mpsc::sync_channel(1);
    worker
        .control(Control::Save {
            save: Save {
                request: PersistenceRequest::SaveSnapshot(envelope()),
                generation: 1,
            },
            clean_exit_timestamp: Some(2.0),
            response,
        })
        .unwrap();
    // Deterministically enter shutdown before the worker takes its barrier.
    worker.shared.mailbox.lock().unwrap().phase = WorkerPhase::Closing;
    start(&mut worker, store.clone());
    drop(worker);
    assert_eq!(receiver.recv().unwrap(), Ok(true));
    assert!(store.prepare_for_launch(true).unwrap().is_some());
    assert!(store.prepare_for_launch(false).unwrap().is_none());
}

#[test]
fn stale_final_save_never_marks_clean() {
    let directory = TestDirectory::new("worker-stale-final");
    let store = directory.store();
    store.mark_launch_started(1.0).unwrap();
    let worker = PersistenceWorker::spawn(store.clone()).unwrap();
    worker
        .persist(PersistenceRequest::SaveSnapshot(envelope()), 2)
        .unwrap();
    assert!(
        !worker
            .persist_synchronously(PersistenceRequest::DeleteSnapshot, 1, Some(3.0))
            .unwrap()
    );
    assert!(
        store.prepare_for_launch(false).unwrap().is_some(),
        "stale save must leave crash recovery available"
    );
}

#[test]
fn worker_exit_disconnects_queued_barriers_and_rejects_new_work() {
    let worker = parked_worker();
    let (response, receiver) = mpsc::sync_channel(1);
    worker.control(Control::Flush(response)).unwrap();
    drop(WorkerExit(Arc::clone(&worker.shared)));
    assert!(matches!(
        receiver.try_recv(),
        Err(mpsc::TryRecvError::Disconnected)
    ));
    assert!(worker.persist(PersistenceRequest::None, 1).is_err());
}
