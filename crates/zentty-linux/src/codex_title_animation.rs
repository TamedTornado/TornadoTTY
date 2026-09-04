use std::collections::BTreeMap;
use zentty_core::AgentPhase;

#[derive(Default)]
pub(crate) struct TerminalTitleEventGate {
    last_semantic_title: Option<String>,
}

impl TerminalTitleEventGate {
    pub(crate) fn accepts(&mut self, title: &str) -> bool {
        let semantic_title =
            zentty_core::stable_codex_terminal_title(title).unwrap_or_else(|| title.to_owned());
        if self.last_semantic_title.as_deref() == Some(&semantic_title) {
            return false;
        }
        self.last_semantic_title = Some(semantic_title);
        true
    }
}

pub(crate) fn is_eligible(
    title: &str,
    agent_name: Option<&str>,
    phase: Option<AgentPhase>,
    has_custom_title: bool,
    is_remote: bool,
) -> bool {
    !has_custom_title
        && !is_remote
        && agent_name.is_some_and(|name| name.eq_ignore_ascii_case("codex"))
        && matches!(phase, Some(AgentPhase::Starting | AgentPhase::Running))
        && zentty_core::codex_activity_spinner_range(title).is_some()
}

#[derive(Default)]
pub(crate) struct CodexTitleAnimation {
    titles: BTreeMap<String, String>,
    last_frame: Option<usize>,
}

impl CodexTitleAnimation {
    pub(crate) fn reconcile(&mut self, pane_id: &str, title: &str, eligible: bool) -> bool {
        if eligible {
            // Codex advances the same Braille spinner in its terminal title.
            // Keep one canonical template so those source frames do not
            // restart TornadoTTY's frame-clock animation.
            let canonical = zentty_core::codex_activity_title_frame(title, 0)
                .unwrap_or_else(|| title.to_owned());
            if self
                .titles
                .get(pane_id)
                .is_some_and(|current| current == &canonical)
            {
                return false;
            }
            self.titles.insert(pane_id.to_owned(), canonical);
            self.last_frame = None;
            true
        } else {
            self.remove(pane_id)
        }
    }

    pub(crate) fn remove(&mut self, pane_id: &str) -> bool {
        let removed = self.titles.remove(pane_id).is_some();
        if self.titles.is_empty() {
            self.last_frame = None;
        }
        removed
    }

    pub(crate) fn retain(&mut self, mut eligible: impl FnMut(&str, &str) -> bool) {
        self.titles
            .retain(|pane_id, title| eligible(pane_id, title));
        if self.titles.is_empty() {
            self.last_frame = None;
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.titles.is_empty()
    }

    pub(crate) fn snapshot(&self) -> Vec<(String, String)> {
        self.titles
            .iter()
            .map(|(pane_id, title)| (pane_id.clone(), title.clone()))
            .collect()
    }

    pub(crate) fn frame_is_due(&self, frame: usize, reduced_motion: bool) -> bool {
        let frame = if reduced_motion { 0 } else { frame };
        self.last_frame != Some(frame)
    }

    pub(crate) fn render_frame(
        &mut self,
        frame: usize,
        reduced_motion: bool,
    ) -> Option<BTreeMap<String, String>> {
        let frame = if reduced_motion { 0 } else { frame };
        if self.last_frame == Some(frame) {
            return None;
        }
        self.last_frame = Some(frame);
        Some(
            self.titles
                .iter()
                .filter_map(|(pane_id, title)| {
                    zentty_core::codex_activity_title_frame(title, frame)
                        .map(|rendered| (pane_id.clone(), rendered))
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{CodexTitleAnimation, TerminalTitleEventGate, is_eligible};
    use zentty_core::AgentPhase;

    #[test]
    fn eligibility_requires_live_local_recognized_codex_title_ownership() {
        assert!(is_eligible(
            "Working ⠋ zentty",
            Some("Codex"),
            Some(AgentPhase::Running),
            false,
            false,
        ));
        for (title, agent, phase, custom, remote) in [
            (
                "Working on ⠋ literal",
                Some("Codex"),
                Some(AgentPhase::Running),
                false,
                false,
            ),
            (
                "Working ⠋ zentty",
                Some("Claude Code"),
                Some(AgentPhase::Running),
                false,
                false,
            ),
            (
                "Working ⠋ zentty",
                Some("Codex"),
                Some(AgentPhase::Idle),
                false,
                false,
            ),
            (
                "Working ⠋ zentty",
                Some("Codex"),
                Some(AgentPhase::Running),
                true,
                false,
            ),
            (
                "Working ⠋ zentty",
                Some("Codex"),
                Some(AgentPhase::Running),
                false,
                true,
            ),
        ] {
            assert!(!is_eligible(title, agent, phase, custom, remote), "{title}");
        }
    }

    #[test]
    fn reconciliation_stops_noneligible_and_removed_panes_without_stale_frames() {
        let mut animation = CodexTitleAnimation::default();
        assert!(animation.reconcile("pane-1", "Working ⠋ zentty", true));
        assert_eq!(
            animation.render_frame(1, false).unwrap()["pane-1"],
            "Working ⠙ zentty"
        );
        assert!(!animation.reconcile("pane-1", "Working ⠋ zentty", true));
        assert!(animation.reconcile("pane-1", "Ready | zentty", false));
        assert!(animation.is_empty());
        assert!(animation.render_frame(2, false).unwrap().is_empty());
    }

    #[test]
    fn source_spinner_frames_are_one_semantic_animation_not_restarts() {
        let mut animation = CodexTitleAnimation::default();
        assert!(animation.reconcile("pane-1", "Working ⠋ zentty", true));
        assert_eq!(
            animation.render_frame(4, false).unwrap()["pane-1"],
            "Working ⠼ zentty"
        );

        // Codex also advances the spinner in its terminal title. Receiving
        // that next frame must not reset our frame-clock animation or replace
        // the semantic title retained for the pane.
        assert!(!animation.reconcile("pane-1", "Working ⠙ zentty", true));
        assert!(animation.render_frame(4, false).is_none());
        assert_eq!(
            animation.render_frame(5, false).unwrap()["pane-1"],
            "Working ⠴ zentty"
        );
    }

    #[test]
    fn title_event_gate_drops_frames_but_preserves_semantic_transitions() {
        let mut gate = TerminalTitleEventGate::default();
        assert!(gate.accepts("Working ⠋ zentty | Tasks 1/5"));
        assert!(!gate.accepts("Working ⠙ zentty | Tasks 1/5"));
        assert!(!gate.accepts("Working ⠹ zentty | Tasks 1/5"));
        assert!(gate.accepts("Working ⠸ zentty | Tasks 2/5"));
        assert!(gate.accepts("ordinary shell title"));
        assert!(!gate.accepts("ordinary shell title"));
        assert!(gate.accepts("different shell title"));
    }

    #[test]
    fn reduced_motion_holds_a_deterministic_static_frame() {
        let mut animation = CodexTitleAnimation::default();
        animation.reconcile("pane-1", "Thinking ⠹ zentty", true);
        assert_eq!(
            animation.render_frame(9, true).unwrap()["pane-1"],
            "Thinking ⠋ zentty"
        );
        assert!(animation.render_frame(10, true).is_none());
        assert!(!animation.frame_is_due(99, true));
    }

    #[test]
    fn retain_tears_down_only_titles_that_lost_ownership() {
        let mut animation = CodexTitleAnimation::default();
        animation.reconcile("local", "Working ⠋ local", true);
        animation.reconcile("remote", "Working ⠋ remote", true);
        animation.retain(|pane_id, _| pane_id == "local");
        let rendered = animation.render_frame(2, false).unwrap();
        assert_eq!(rendered.len(), 1);
        assert!(rendered.contains_key("local"));
    }
}
