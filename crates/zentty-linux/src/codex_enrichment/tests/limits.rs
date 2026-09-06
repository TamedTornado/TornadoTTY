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
