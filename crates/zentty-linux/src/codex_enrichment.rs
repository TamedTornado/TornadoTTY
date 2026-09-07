use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
use zentty_core::{
    CodexTranscriptEnrichmentCandidate, CodexTranscriptQuestion,
    codex_question_from_transcript_path, codex_transcript_cache_key,
    discover_recent_codex_transcript_path,
};

mod cache;
mod worker_budget;
use cache::{QuestionCache, TranscriptPaths};
use worker_budget::{WorkerBudget, WorkerPermit};

// Process-wide: include detached, stale and cancelled threads until they exit.
const MAX_WORKERS: usize = 4;
static WORKER_BUDGET: std::sync::LazyLock<Arc<WorkerBudget>> =
    std::sync::LazyLock::new(|| Arc::new(WorkerBudget::default()));
const MAX_PENDING: usize = 32;
const MAX_RESULTS_PER_TICK: usize = 4;

const RETRY_DELAYS: [Duration; 5] = [
    Duration::ZERO,
    Duration::from_millis(100),
    Duration::from_millis(200),
    Duration::from_millis(400),
    Duration::from_millis(600),
];

#[derive(Debug)]
pub(crate) struct CodexTranscriptEnrichment {
    pub(crate) candidate: CodexTranscriptEnrichmentCandidate,
    pub(crate) question: CodexTranscriptQuestion,
}

#[derive(Debug)]
struct WorkerResult {
    candidate: CodexTranscriptEnrichmentCandidate,
    generation: u64,
    transcript_path: Option<PathBuf>,
    question: Option<CodexTranscriptQuestion>,
    discovery_limited: bool,
}

struct PendingWorker {
    candidate: CodexTranscriptEnrichmentCandidate,
    generation: u64,
    cancellation: Arc<AtomicBool>,
}

struct ActiveWorker {
    pane_id: String,
    generation: u64,
    thread: std::thread::JoinHandle<()>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum CompletionDecision {
    Apply,
    #[default]
    IgnoreStale,
    IgnoreDuringShutdown,
}

fn completion_decision(
    shutting_down: bool,
    pending_generation: Option<u64>,
    result_generation: u64,
    candidate_matches: bool,
) -> CompletionDecision {
    if shutting_down {
        CompletionDecision::IgnoreDuringShutdown
    } else if pending_generation == Some(result_generation) && candidate_matches {
        CompletionDecision::Apply
    } else {
        CompletionDecision::IgnoreStale
    }
}

/// Owns only background transcript file discovery and parsing. Canonical
/// agent state remains in `WorkspaceState`; stale results are validated there
/// again before application.
pub(crate) struct CodexTranscriptEnricher {
    codex_home: PathBuf,
    delays: Vec<Duration>,
    pending_by_pane: BTreeMap<String, PendingWorker>,
    queued: VecDeque<String>,
    active: Vec<ActiveWorker>,
    worker_budget: Arc<WorkerBudget>,
    transcript_by_session: TranscriptPaths,
    cache: Arc<Mutex<QuestionCache>>,
    sender: mpsc::SyncSender<WorkerResult>,
    receiver: mpsc::Receiver<WorkerResult>,
    next_generation: u64,
    shutting_down: bool,
    rejected: u64,
    discovery_limited: u64,
    last_pressure_report: Option<Instant>,
}

impl CodexTranscriptEnricher {
    pub(crate) fn new(codex_home: PathBuf) -> Self {
        Self::with_budget(
            codex_home,
            RETRY_DELAYS.to_vec(),
            Arc::clone(&WORKER_BUDGET),
        )
    }

    #[cfg(test)]
    fn with_delays(codex_home: PathBuf, delays: Vec<Duration>) -> Self {
        Self::with_budget(codex_home, delays, Arc::new(WorkerBudget::default()))
    }

    fn with_budget(
        codex_home: PathBuf,
        delays: Vec<Duration>,
        worker_budget: Arc<WorkerBudget>,
    ) -> Self {
        let (sender, receiver) = mpsc::sync_channel(MAX_RESULTS_PER_TICK);
        Self {
            codex_home,
            delays,
            pending_by_pane: BTreeMap::new(),
            queued: VecDeque::new(),
            active: Vec::new(),
            worker_budget,
            transcript_by_session: TranscriptPaths::default(),
            cache: Arc::new(Mutex::new(QuestionCache::default())),
            sender,
            receiver,
            next_generation: 0,
            shutting_down: false,
            rejected: 0,
            discovery_limited: 0,
            last_pressure_report: None,
        }
    }

    pub(crate) fn schedule(&mut self, candidate: CodexTranscriptEnrichmentCandidate) -> bool {
        if self.shutting_down {
            return false;
        }
        if !self.pending_by_pane.contains_key(&candidate.pane_id)
            && self.pending_by_pane.len() >= MAX_PENDING
        {
            self.rejected = self.rejected.saturating_add(1);
            return false;
        }
        if let Some(pending) = self.pending_by_pane.get(&candidate.pane_id) {
            if pending.candidate == candidate {
                return false;
            }
            pending.cancellation.store(true, Ordering::Release);
        }
        self.next_generation = self.next_generation.wrapping_add(1);
        let generation = self.next_generation;
        let cancellation = Arc::new(AtomicBool::new(false));
        if !self.queued.contains(&candidate.pane_id) {
            self.queued.push_back(candidate.pane_id.clone());
        }
        self.pending_by_pane.insert(
            candidate.pane_id.clone(),
            PendingWorker {
                candidate,
                generation,
                cancellation,
            },
        );
        self.dispatch();
        true
    }

    fn dispatch(&mut self) {
        self.reap_finished();
        while self.active.len() < MAX_WORKERS {
            // A cancelled read still occupies its pane's slot until it exits.
            // Skip that pane so replacements cannot monopolize all workers.
            let Some(index) = self
                .queued
                .iter()
                .position(|pane| !self.active.iter().any(|worker| &worker.pane_id == pane))
            else {
                break;
            };
            let Some(permit) = self.worker_budget.acquire() else {
                break;
            };
            let pane_id = self
                .queued
                .remove(index)
                .expect("selected queued pane exists");
            let pending = self
                .pending_by_pane
                .get(&pane_id)
                .expect("queued pane owns a candidate");
            let candidate = pending.candidate.clone();
            let generation = pending.generation;
            let cancellation = Arc::clone(&pending.cancellation);
            self.spawn(candidate, generation, cancellation, permit);
        }
    }

    fn reap_finished(&mut self) {
        for index in (0..self.active.len()).rev() {
            if !self.active[index].thread.is_finished() {
                continue;
            }
            let worker = self.active.swap_remove(index);
            if worker.thread.join().is_err() {
                if self
                    .pending_by_pane
                    .get(&worker.pane_id)
                    .is_some_and(|pending| pending.generation == worker.generation)
                {
                    self.pending_by_pane.remove(&worker.pane_id);
                }
                eprintln!(
                    "zentty-linux: codex-transcript-worker-failed pane={} reason=panic",
                    worker.pane_id
                );
            }
        }
    }

    fn spawn(
        &mut self,
        candidate: CodexTranscriptEnrichmentCandidate,
        generation: u64,
        cancellation: Arc<AtomicBool>,
        permit: WorkerPermit,
    ) {
        let preferred_path = candidate
            .transcript_path
            .as_ref()
            .map(PathBuf::from)
            .or_else(|| {
                self.transcript_by_session
                    .get(&candidate.pane_id, &candidate.session_id)
                    .cloned()
            });
        let codex_home = self.codex_home.clone();
        let delays = self.delays.clone();
        let cache = Arc::clone(&self.cache);
        let sender = self.sender.clone();
        let pane_id = candidate.pane_id.clone();
        let spawn = std::thread::Builder::new()
            .name("zentty-codex-transcript".to_owned())
            .spawn(move || {
                let _permit = permit;
                let (transcript_path, question, discovery_limited) = resolve_with_retries(
                    &codex_home,
                    candidate.working_directory.as_deref(),
                    preferred_path.as_deref(),
                    &delays,
                    &cache,
                    &cancellation,
                );
                let _ = sender.send(WorkerResult {
                    candidate,
                    generation,
                    transcript_path,
                    question,
                    discovery_limited,
                });
            });
        match spawn {
            Ok(thread) => self.active.push(ActiveWorker {
                pane_id,
                generation,
                thread,
            }),
            Err(error) => {
                self.pending_by_pane.remove(&pane_id);
                eprintln!(
                    "zentty-linux: codex-transcript-worker-failed pane={pane_id} reason=spawn error={error}"
                );
            }
        }
    }

    pub(crate) fn drain(&mut self) -> Vec<CodexTranscriptEnrichment> {
        let mut enrichments = Vec::new();
        if self.shutting_down {
            return enrichments;
        }
        if let Some(rejected) = self.take_pressure(Instant::now()) {
            let discovery_limited = std::mem::take(&mut self.discovery_limited);
            eprintln!(
                "zentty-linux: codex-transcript-backpressure boundary=enrichment rejected={rejected} discovery-limited={discovery_limited} pending-capacity={MAX_PENDING} process-worker-capacity={MAX_WORKERS}"
            );
        }
        for result in self.receiver.try_iter().take(MAX_RESULTS_PER_TICK) {
            if result.discovery_limited {
                // Aggregate using the owner's existing pressure reporting,
                // never print paths or emit one record per rejected file.
                self.rejected = self.rejected.saturating_add(1);
                self.discovery_limited = self.discovery_limited.saturating_add(1);
            }
            let pending = self.pending_by_pane.get(&result.candidate.pane_id);
            if completion_decision(
                self.shutting_down,
                pending.map(|pending| pending.generation),
                result.generation,
                pending.is_some_and(|pending| pending.candidate == result.candidate),
            ) != CompletionDecision::Apply
            {
                continue;
            }
            self.pending_by_pane.remove(&result.candidate.pane_id);
            let (Some(path), Some(question)) = (result.transcript_path, result.question) else {
                continue;
            };
            self.transcript_by_session.insert(
                result.candidate.pane_id.clone(),
                result.candidate.session_id.clone(),
                path,
            );
            enrichments.push(CodexTranscriptEnrichment {
                candidate: result.candidate,
                question,
            });
        }
        self.dispatch();
        enrichments
    }

    fn take_pressure(&mut self, now: Instant) -> Option<u64> {
        if self.rejected == 0
            || self
                .last_pressure_report
                .is_some_and(|last| now.saturating_duration_since(last) < Duration::from_secs(5))
        {
            return None;
        }
        self.last_pressure_report = Some(now);
        Some(std::mem::take(&mut self.rejected))
    }

    pub(crate) fn cancel_pane(&mut self, pane_id: &str) -> bool {
        self.queued.retain(|queued| queued != pane_id);
        self.transcript_by_session.remove_pane(pane_id);
        let Some(pending) = self.pending_by_pane.remove(pane_id) else {
            return false;
        };
        pending.cancellation.store(true, Ordering::Release);
        true
    }

    pub(crate) fn shutdown(&mut self) {
        self.shutting_down = true;
        self.queued.clear();
        for (_, pending) in std::mem::take(&mut self.pending_by_pane) {
            pending.cancellation.store(true, Ordering::Release);
        }
        // Free result capacity without waiting for file I/O or joining live threads.
        for _ in self.receiver.try_iter().take(MAX_RESULTS_PER_TICK) {}
    }

    #[cfg(test)]
    fn pending_count(&self) -> usize {
        self.pending_by_pane.len()
    }
}

impl Drop for CodexTranscriptEnricher {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn resolve_with_retries(
    codex_home: &Path,
    working_directory: Option<&str>,
    preferred_path: Option<&Path>,
    delays: &[Duration],
    cache: &Mutex<QuestionCache>,
    cancellation: &AtomicBool,
) -> (Option<PathBuf>, Option<CodexTranscriptQuestion>, bool) {
    for delay in delays {
        if cancellation.load(Ordering::Acquire) {
            break;
        }
        std::thread::sleep(*delay);
        if cancellation.load(Ordering::Acquire) {
            break;
        }
        let mut discovery_limited = false;
        let path = preferred_path
            .filter(|path| codex_transcript_cache_key(path).is_some())
            .map(Path::to_path_buf)
            .or_else(|| {
                working_directory.and_then(|working_directory| {
                    if let Ok(path) =
                        discover_recent_codex_transcript_path(codex_home, working_directory)
                    {
                        path
                    } else {
                        discovery_limited = true;
                        None
                    }
                })
            });
        let Some(path) = path else {
            if discovery_limited {
                return (None, None, true);
            }
            continue;
        };
        let Some(key) = codex_transcript_cache_key(&path) else {
            continue;
        };
        if let Some(question) = cache.lock().ok().and_then(|cache| cache.get(&key).cloned()) {
            return (Some(path), Some(question), false);
        }
        let Some(question) = codex_question_from_transcript_path(&path) else {
            continue;
        };
        if let Ok(mut cache) = cache.lock() {
            cache.insert(key, question.clone());
        }
        return (Some(path), Some(question), false);
    }
    (None, None, false)
}

#[cfg(test)]
mod tests {
    mod limits;

    use super::{CodexTranscriptEnricher, CompletionDecision, completion_decision};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
    use zentty_core::{AgentInteractionKind, CodexTranscriptEnrichmentCandidate};

    fn temporary_directory(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "zentty-codex-enrichment-{name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn transcript_path(codex_home: &Path) -> PathBuf {
        let day = codex_home.join("sessions/2026/08/07");
        fs::create_dir_all(&day).unwrap();
        day.join("rollout.jsonl")
    }

    fn wait_for_result(enricher: &mut CodexTranscriptEnricher) -> super::CodexTranscriptEnrichment {
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            if let Some(result) = enricher.drain().into_iter().next() {
                return result;
            }
            assert!(Instant::now() < deadline, "transcript enrichment timed out");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn admission_is_bounded_before_the_ui_drains_completions() {
        let root = temporary_directory("admission");
        let mut enricher =
            CodexTranscriptEnricher::with_delays(root.clone(), vec![Duration::from_millis(100)]);
        for index in 0..32 {
            assert!(enricher.schedule(CodexTranscriptEnrichmentCandidate {
                pane_id: format!("pane-{index}"),
                session_id: "session".to_owned(),
                working_directory: None,
                transcript_path: None,
            }));
        }
        assert!(!enricher.schedule(CodexTranscriptEnrichmentCandidate {
            pane_id: "overflow".to_owned(),
            session_id: "session".to_owned(),
            working_directory: None,
            transcript_path: None,
        }));
        assert!(
            enricher.schedule(CodexTranscriptEnrichmentCandidate {
                pane_id: "pane-31".to_owned(),
                session_id: "replacement".to_owned(),
                working_directory: None,
                transcript_path: None,
            }),
            "existing panes must still replace queued work at capacity"
        );
        assert!(enricher.cancel_pane("pane-31"));
        assert!(!enricher.queued.iter().any(|pane| pane == "pane-31"));
        assert!(enricher.queued.iter().any(|pane| pane == "pane-30"));
        assert!(enricher.schedule(CodexTranscriptEnrichmentCandidate {
            pane_id: "newly-admitted".to_owned(),
            session_id: "session".to_owned(),
            working_directory: None,
            transcript_path: None,
        }));
        enricher.shutdown();
        assert!(enricher.queued.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stalled_workers_keep_replacements_bounded_and_service_the_quiet_pane() {
        let root = temporary_directory("stalled");
        let transcript = transcript_path(&root);
        fs::write(&transcript, r#"{"type":"function_call","name":"request_user_input","arguments":{"question":"Current question?"}}"#).unwrap();
        let mut enricher = CodexTranscriptEnricher::with_delays(root.clone(), vec![Duration::ZERO]);
        let cache = std::sync::Arc::clone(&enricher.cache);
        // Hold the real cache boundary, not a substitute resolver. All four
        // workers remain alive; cancellation must not pretend they have exited.
        let gate = cache.lock().unwrap();
        let candidate = |pane: &str, session: &str| CodexTranscriptEnrichmentCandidate {
            pane_id: pane.to_owned(),
            session_id: session.to_owned(),
            working_directory: None,
            transcript_path: Some(transcript.to_string_lossy().into_owned()),
        };
        for index in 0..super::MAX_WORKERS {
            assert!(enricher.schedule(candidate(&format!("pane-{index}"), "original")));
        }
        assert!(enricher.schedule(candidate("quiet", "quiet-session")));
        for index in 0..1000 {
            assert!(enricher.schedule(candidate("pane-0", &format!("replacement-{index}"))));
            assert_eq!(enricher.active.len(), super::MAX_WORKERS);
            assert!(enricher.queued.len() <= 2);
            assert_eq!(enricher.pending_count(), super::MAX_WORKERS + 1);
        }
        drop(gate);
        let mut results = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(3);
        while results.len() < super::MAX_WORKERS + 1 {
            let batch = enricher.drain();
            assert!(batch.len() <= super::MAX_RESULTS_PER_TICK);
            assert!(enricher.active.len() <= super::MAX_WORKERS);
            results.extend(batch);
            assert!(
                Instant::now() < deadline,
                "quiet or current pane failed to complete"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        let replaced: Vec<_> = results
            .iter()
            .filter(|result| result.candidate.pane_id == "pane-0")
            .collect();
        assert_eq!(replaced.len(), 1);
        assert_eq!(replaced[0].candidate.session_id, "replacement-999");
        assert!(
            results
                .iter()
                .any(|result| result.candidate.session_id == "quiet-session")
        );
        assert!(
            results
                .iter()
                .all(|result| result.question.text == "Current question?")
        );
        enricher.cancel_pane("quiet");
        assert!(
            enricher
                .transcript_by_session
                .get("quiet", "quiet-session")
                .is_none()
        );
        enricher.shutdown();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn one_stalled_pane_cannot_consume_other_worker_slots() {
        let root = temporary_directory("one-stalled");
        let transcript = transcript_path(&root);
        fs::write(&transcript, "{}").unwrap();
        let mut enricher = CodexTranscriptEnricher::with_delays(root.clone(), vec![Duration::ZERO]);
        let cache = std::sync::Arc::clone(&enricher.cache);
        let gate = cache.lock().unwrap();
        let candidate = |pane: &str, session: usize| CodexTranscriptEnrichmentCandidate {
            pane_id: pane.to_owned(),
            session_id: session.to_string(),
            working_directory: None,
            transcript_path: Some(transcript.to_string_lossy().into_owned()),
        };
        for session in 0..1000 {
            assert!(enricher.schedule(candidate("noisy", session)));
            assert_eq!(enricher.active.len(), 1);
        }
        assert!(enricher.schedule(candidate("quiet", 0)));
        assert_eq!(enricher.active.len(), 2);
        assert_eq!(enricher.active[1].pane_id, "quiet");
        drop(gate);
        enricher.shutdown();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn completion_requires_the_current_generation_candidate_and_live_coordinator() {
        assert_eq!(
            completion_decision(false, Some(7), 7, true),
            CompletionDecision::Apply
        );
        for decision in [
            completion_decision(false, None, 7, true),
            completion_decision(false, Some(6), 7, true),
            completion_decision(false, Some(7), 7, false),
        ] {
            assert_eq!(decision, CompletionDecision::IgnoreStale);
        }
        assert_eq!(
            completion_decision(true, Some(7), 7, true),
            CompletionDecision::IgnoreDuringShutdown
        );
    }

    #[test]
    fn real_file_worker_retries_deduplicates_and_extracts_the_flushed_question() {
        let root = temporary_directory("retry");
        let codex_home = root.join(".codex");
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        let transcript = transcript_path(&codex_home);
        fs::write(&transcript, format!("{{\"cwd\":{project:?}}}\n")).unwrap();
        let mut enricher = CodexTranscriptEnricher::with_delays(
            codex_home,
            vec![
                Duration::ZERO,
                Duration::from_millis(10),
                Duration::from_millis(20),
                Duration::from_millis(40),
            ],
        );
        let candidate = CodexTranscriptEnrichmentCandidate {
            pane_id: "pane-a".to_owned(),
            session_id: "session-a".to_owned(),
            working_directory: Some(project.to_string_lossy().into_owned()),
            transcript_path: None,
        };
        assert!(enricher.schedule(candidate.clone()));
        assert!(!enricher.schedule(candidate.clone()));
        std::thread::sleep(Duration::from_millis(15));
        fs::write(
            &transcript,
            format!(
                "{{\"cwd\":{project:?}}}\n{{\"type\":\"function_call\",\"name\":\"request_user_input\",\"arguments\":{{\"questions\":[{{\"question\":\"Choose the scope\",\"options\":[{{\"label\":\"Minimal\"}},{{\"label\":\"Complete\"}}]}}]}}}}\n"
            ),
        )
        .unwrap();

        let result = wait_for_result(&mut enricher);
        assert_eq!(
            result.question.text,
            "Choose the scope\n[Minimal] [Complete]"
        );
        assert_eq!(result.question.interaction, AgentInteractionKind::Decision);

        fs::set_permissions(&transcript, fs::Permissions::from_mode(0o000)).unwrap();
        assert!(enricher.schedule(candidate));
        let cached = wait_for_result(&mut enricher);
        assert_eq!(cached.question, result.question);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn replaced_request_discards_the_old_worker_result() {
        let root = temporary_directory("stale");
        let codex_home = root.join(".codex");
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        let transcript = transcript_path(&codex_home);
        fs::write(
            transcript,
            format!(
                "{{\"cwd\":{project:?}}}\n{{\"type\":\"function_call\",\"name\":\"request_user_input\",\"arguments\":{{\"question\":\"Proceed?\"}}}}\n"
            ),
        )
        .unwrap();
        let mut enricher =
            CodexTranscriptEnricher::with_delays(codex_home, vec![Duration::from_millis(15)]);
        let candidate = |session: &str| CodexTranscriptEnrichmentCandidate {
            pane_id: "pane-a".to_owned(),
            session_id: session.to_owned(),
            working_directory: Some(project.to_string_lossy().into_owned()),
            transcript_path: None,
        };
        assert!(enricher.schedule(candidate("old")));
        assert!(enricher.schedule(candidate("current")));
        let result = wait_for_result(&mut enricher);
        assert_eq!(result.candidate.session_id, "current");
        std::thread::sleep(Duration::from_millis(30));
        assert!(enricher.drain().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pane_removal_cancels_pending_enrichment_and_rejects_late_completion() {
        let root = temporary_directory("pane-cancel");
        let codex_home = root.join(".codex");
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        let transcript = transcript_path(&codex_home);
        fs::write(
            transcript,
            format!(
                "{{\"cwd\":{project:?}}}\n{{\"type\":\"function_call\",\"name\":\"request_user_input\",\"arguments\":{{\"question\":\"Must not arrive\"}}}}\n"
            ),
        )
        .unwrap();
        let mut enricher =
            CodexTranscriptEnricher::with_delays(codex_home, vec![Duration::from_millis(20)]);
        let candidate = CodexTranscriptEnrichmentCandidate {
            pane_id: "pane-a".to_owned(),
            session_id: "session-a".to_owned(),
            working_directory: Some(project.to_string_lossy().into_owned()),
            transcript_path: None,
        };

        assert!(enricher.schedule(candidate));
        assert_eq!(enricher.pending_count(), 1);
        assert!(enricher.cancel_pane("pane-a"));
        assert_eq!(enricher.pending_count(), 0);
        std::thread::sleep(Duration::from_millis(30));
        assert!(enricher.drain().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn shutdown_cancels_every_generation_and_rejects_work_in_flight() {
        let root = temporary_directory("shutdown");
        let codex_home = root.join(".codex");
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        let mut enricher =
            CodexTranscriptEnricher::with_delays(codex_home, vec![Duration::from_millis(20)]);
        for pane_id in ["pane-a", "pane-b"] {
            assert!(enricher.schedule(CodexTranscriptEnrichmentCandidate {
                pane_id: pane_id.to_owned(),
                session_id: format!("session-{pane_id}"),
                working_directory: Some(project.to_string_lossy().into_owned()),
                transcript_path: None,
            }));
        }

        assert_eq!(enricher.pending_count(), 2);
        enricher.shutdown();
        assert_eq!(enricher.pending_count(), 0);
        assert!(!enricher.schedule(CodexTranscriptEnrichmentCandidate {
            pane_id: "pane-c".to_owned(),
            session_id: "session-c".to_owned(),
            working_directory: None,
            transcript_path: None,
        }));
        std::thread::sleep(Duration::from_millis(30));
        assert!(enricher.drain().is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
