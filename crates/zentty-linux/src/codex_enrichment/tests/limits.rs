use super::super::{ActiveWorker, CodexTranscriptEnricher, PendingWorker};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use zentty_core::CodexTranscriptEnrichmentCandidate;

fn candidate() -> CodexTranscriptEnrichmentCandidate {
    CodexTranscriptEnrichmentCandidate {
        pane_id: "pane".to_owned(),
        session_id: "session".to_owned(),
        working_directory: None,
        transcript_path: None,
    }
}

#[test]
fn closed_windows_cannot_release_still_running_workers_from_the_shared_budget() {
    let root = super::temporary_directory("cross-window-workers");
    let path = super::transcript_path(&root);
    std::fs::write(&path, r#"{"type":"function_call","name":"request_user_input","arguments":{"question":"Still bounded?"}}"#).unwrap();
    let mut first = CodexTranscriptEnricher::new(root.clone());
    let mut second = CodexTranscriptEnricher::new(root.clone());
    first.delays = vec![Duration::ZERO];
    second.delays = vec![Duration::ZERO];
    let mut requests = Vec::new();
    // Hold actual file-worker cache access, not a substituted resolver.
    let cache = Arc::clone(&first.cache);
    let guard = cache.lock().unwrap();
    for index in 0..super::super::MAX_WORKERS {
        let mut request = candidate();
        request.pane_id = format!("first-{index}");
        request.transcript_path = Some(path.to_string_lossy().into_owned());
        assert!(first.schedule(request));
    }
    for index in 0..super::super::MAX_WORKERS {
        let mut request = candidate();
        request.pane_id = format!("second-{index}");
        request.transcript_path = Some(path.to_string_lossy().into_owned());
        requests.push(request.clone());
        assert!(second.schedule(request));
    }
    assert!(
        second.active.is_empty(),
        "a second window must share the actual worker bound"
    );
    drop(first);
    // Thread-owned permits are checked separately with a deterministic blocked
    // worker. Here cancellation may already have let an unstarted read exit.
    drop(guard);
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut completed = 0;
    while completed < requests.len() {
        completed += second.drain().len();
        assert!(
            Instant::now() < deadline,
            "other window did not recover after workers exited"
        );
        std::thread::yield_now();
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_panicking_worker_removes_only_its_own_generation() {
    for generation in [1, 2] {
        let mut enricher = CodexTranscriptEnricher::with_delays(PathBuf::new(), Vec::new());
        enricher.pending_by_pane.insert(
            "pane".to_owned(),
            PendingWorker {
                candidate: candidate(),
                generation: 2,
                cancellation: Arc::new(AtomicBool::new(false)),
            },
        );
        let thread = std::thread::spawn(|| panic!("controlled transcript worker failure"));
        let deadline = Instant::now() + Duration::from_secs(1);
        while !thread.is_finished() {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        enricher.active.push(ActiveWorker {
            pane_id: "pane".to_owned(),
            generation,
            thread,
        });
        enricher.reap_finished();
        assert!(enricher.active.is_empty());
        assert_eq!(
            enricher.pending_by_pane.contains_key("pane"),
            generation != 2
        );
    }
}

#[test]
fn dropping_the_owner_cancels_a_real_background_job() {
    let mut enricher =
        CodexTranscriptEnricher::with_delays(PathBuf::new(), vec![Duration::from_millis(100)]);
    assert!(enricher.schedule(candidate()));
    let cancellation = Arc::clone(&enricher.pending_by_pane["pane"].cancellation);
    drop(enricher);
    assert!(cancellation.load(Ordering::Acquire));
}

#[test]
fn pressure_reports_are_rate_limited_without_losing_counts() {
    let mut enricher = CodexTranscriptEnricher::with_delays(PathBuf::new(), Vec::new());
    let now = Instant::now();
    assert_eq!(enricher.take_pressure(now), None);
    enricher.rejected = 1;
    assert_eq!(enricher.take_pressure(now), Some(1));
    enricher.rejected = 2;
    assert_eq!(enricher.take_pressure(now + Duration::from_secs(4)), None);
    assert_eq!(enricher.rejected, 2);
    enricher.rejected += 1;
    assert_eq!(
        enricher.take_pressure(now + Duration::from_secs(5)),
        Some(3)
    );
    assert_eq!(enricher.take_pressure(now + Duration::from_secs(10)), None);
}
