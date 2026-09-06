use std::collections::VecDeque;
use std::path::PathBuf;
use zentty_core::{CodexTranscriptCacheKey, CodexTranscriptQuestion};

const MAX_QUESTIONS: usize = 64;
const MAX_PATHS: usize = 32;

#[derive(Default)]
pub(super) struct QuestionCache(VecDeque<(CodexTranscriptCacheKey, CodexTranscriptQuestion)>);

impl QuestionCache {
    pub(super) fn get(&self, key: &CodexTranscriptCacheKey) -> Option<&CodexTranscriptQuestion> {
        self.0
            .iter()
            .find(|(stored, _)| stored == key)
            .map(|(_, question)| question)
    }

    pub(super) fn insert(
        &mut self,
        key: CodexTranscriptCacheKey,
        question: CodexTranscriptQuestion,
    ) {
        // A growing transcript must not leave every previous file version cached.
        self.0.retain(|(stored, _)| stored.path != key.path);
        if self.0.len() >= MAX_QUESTIONS {
            self.0.pop_front();
        }
        self.0.push_back((key, question));
    }
}

#[derive(Default)]
pub(super) struct TranscriptPaths(VecDeque<(String, String, PathBuf)>);

impl TranscriptPaths {
    pub(super) fn get(&self, pane: &str, session: &str) -> Option<&PathBuf> {
        self.0
            .iter()
            .find(|(stored_pane, stored_session, _)| {
                stored_pane == pane && stored_session == session
            })
            .map(|(_, _, path)| path)
    }

    pub(super) fn insert(&mut self, pane: String, session: String, path: PathBuf) {
        self.remove_pane(&pane);
        if self.0.len() >= MAX_PATHS {
            self.0.pop_front();
        }
        self.0.push_back((pane, session, path));
    }

    pub(super) fn remove_pane(&mut self, pane: &str) {
        self.0.retain(|(stored, _, _)| stored != pane);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zentty_core::AgentInteractionKind;

    fn key(path: &str, file_size: u64) -> CodexTranscriptCacheKey {
        CodexTranscriptCacheKey {
            path: path.into(),
            file_size,
            modification_time: None,
        }
    }

    fn question(text: &str) -> CodexTranscriptQuestion {
        CodexTranscriptQuestion {
            text: text.to_owned(),
            interaction: AgentInteractionKind::Question,
        }
    }

    #[test]
    fn changed_transcript_replaces_old_version_without_returning_stale_text() {
        let mut cache = QuestionCache::default();
        cache.insert(key("a", 1), question("old"));
        cache.insert(key("b", 1), question("other"));
        cache.insert(key("a", 2), question("current"));
        assert!(cache.get(&key("a", 1)).is_none());
        assert_eq!(cache.get(&key("a", 2)), Some(&question("current")));
        assert_eq!(cache.get(&key("b", 1)), Some(&question("other")));
        assert_eq!(cache.0.len(), 2);
    }

    #[test]
    fn question_cache_evicts_oldest_at_capacity() {
        let mut cache = QuestionCache::default();
        for index in 0..=MAX_QUESTIONS {
            cache.insert(key(&index.to_string(), 1), question(&index.to_string()));
        }
        assert_eq!(cache.0.len(), MAX_QUESTIONS);
        assert!(cache.get(&key("0", 1)).is_none());
        assert_eq!(cache.get(&key("1", 1)), Some(&question("1")));
        assert!(cache.get(&key(&MAX_QUESTIONS.to_string(), 1)).is_some());
    }

    #[test]
    fn path_hints_are_bounded_and_never_cross_sessions_or_survive_pane_close() {
        let mut paths = TranscriptPaths::default();
        for index in 0..=MAX_PATHS {
            paths.insert(
                index.to_string(),
                "old".to_owned(),
                PathBuf::from(index.to_string()),
            );
        }
        assert_eq!(paths.0.len(), MAX_PATHS);
        assert!(paths.get("0", "old").is_none());
        assert_eq!(paths.get("1", "old"), Some(&PathBuf::from("1")));
        paths.insert("1".to_owned(), "new".to_owned(), "new-path".into());
        assert!(paths.get("1", "old").is_none());
        assert!(paths.get("2", "new").is_none());
        assert_eq!(paths.get("1", "new"), Some(&PathBuf::from("new-path")));
        paths.remove_pane("1");
        assert!(paths.get("1", "new").is_none());
        assert!(paths.get("2", "old").is_some());
    }
}
