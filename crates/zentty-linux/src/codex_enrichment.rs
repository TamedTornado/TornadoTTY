use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;
use zentty_core::{
    CodexTranscriptCacheKey, CodexTranscriptEnrichmentCandidate, CodexTranscriptQuestion,
    codex_question_from_transcript_path, codex_transcript_cache_key,
    locate_recent_codex_transcript_path,
};

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
}

struct PendingWorker {
    candidate: CodexTranscriptEnrichmentCandidate,
    generation: u64,
    cancellation: Arc<AtomicBool>,
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
    transcript_by_session: BTreeMap<(String, String), PathBuf>,
    cache: Arc<Mutex<HashMap<CodexTranscriptCacheKey, CodexTranscriptQuestion>>>,
    sender: mpsc::Sender<WorkerResult>,
    receiver: mpsc::Receiver<WorkerResult>,
    next_generation: u64,
    shutting_down: bool,
}

impl CodexTranscriptEnricher {
    pub(crate) fn new(codex_home: PathBuf) -> Self {
        Self::with_delays(codex_home, RETRY_DELAYS.to_vec())
    }

    fn with_delays(codex_home: PathBuf, delays: Vec<Duration>) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            codex_home,
            delays,
            pending_by_pane: BTreeMap::new(),
            transcript_by_session: BTreeMap::new(),
            cache: Arc::new(Mutex::new(HashMap::new())),
            sender,
            receiver,
            next_generation: 0,
            shutting_down: false,
        }
    }

    pub(crate) fn schedule(&mut self, candidate: CodexTranscriptEnrichmentCandidate) -> bool {
        if self.shutting_down {
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
        self.pending_by_pane.insert(
            candidate.pane_id.clone(),
            PendingWorker {
                candidate: candidate.clone(),
                generation,
                cancellation: Arc::clone(&cancellation),
            },
        );
        let preferred_path = candidate
            .transcript_path
            .as_ref()
            .map(PathBuf::from)
            .or_else(|| {
                self.transcript_by_session
                    .get(&(candidate.pane_id.clone(), candidate.session_id.clone()))
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
                let (transcript_path, question) = resolve_with_retries(
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
                });
            });
        if spawn.is_err() {
            self.pending_by_pane.remove(&pane_id);
            return false;
        }
        true
    }

    pub(crate) fn drain(&mut self) -> Vec<CodexTranscriptEnrichment> {
        let mut enrichments = Vec::new();
        for result in self.receiver.try_iter() {
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
                (
                    result.candidate.pane_id.clone(),
                    result.candidate.session_id.clone(),
                ),
                path,
            );
            enrichments.push(CodexTranscriptEnrichment {
                candidate: result.candidate,
                question,
            });
        }
        enrichments
    }

    pub(crate) fn cancel_pane(&mut self, pane_id: &str) -> bool {
        let Some(pending) = self.pending_by_pane.remove(pane_id) else {
            return false;
        };
        pending.cancellation.store(true, Ordering::Release);
        true
    }

    pub(crate) fn shutdown(&mut self) {
        self.shutting_down = true;
        for (_, pending) in std::mem::take(&mut self.pending_by_pane) {
            pending.cancellation.store(true, Ordering::Release);
        }
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
    cache: &Mutex<HashMap<CodexTranscriptCacheKey, CodexTranscriptQuestion>>,
    cancellation: &AtomicBool,
) -> (Option<PathBuf>, Option<CodexTranscriptQuestion>) {
    for delay in delays {
        if cancellation.load(Ordering::Acquire) {
            break;
        }
        std::thread::sleep(*delay);
        if cancellation.load(Ordering::Acquire) {
            break;
        }
        let path = preferred_path
            .filter(|path| codex_transcript_cache_key(path).is_some())
            .map(Path::to_path_buf)
            .or_else(|| {
                working_directory.and_then(|working_directory| {
                    locate_recent_codex_transcript_path(codex_home, working_directory)
                })
            });
        let Some(path) = path else {
            continue;
        };
        let Some(key) = codex_transcript_cache_key(&path) else {
            continue;
        };
        if let Some(question) = cache.lock().ok().and_then(|cache| cache.get(&key).cloned()) {
            return (Some(path), Some(question));
        }
        let Some(question) = codex_question_from_transcript_path(&path) else {
            continue;
        };
        if let Ok(mut cache) = cache.lock() {
            cache.insert(key, question.clone());
        }
        return (Some(path), Some(question));
    }
    (None, None)
}

#[cfg(test)]
mod tests {
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
