use crate::{
    AgentInteractionKind, AgentTarget, AuthenticatedAgentEvent, CodexTitlePhase,
    classify_codex_terminal_title,
};
use std::collections::{HashMap, HashSet};

const CODEX_TITLE_IDLE_SUPPRESSION_MS: u64 = 1_000;
const CODEX_INPUT_SUBMIT_STABILIZATION_MS: u64 = 350;
const CODEX_INTERRUPT_SUPPRESSION_MS: u64 = 3_000;
const CLAUDE_POST_STOP_NEEDS_INPUT_GRACE_MS: u64 = 5_000;

#[derive(Clone, Debug, Eq, PartialEq)]
struct CodexInterruptSuppression {
    until: u64,
    session_ids: HashSet<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentPhase {
    Starting,
    Running,
    NeedsInput,
    Idle,
    UnresolvedStop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalProgressState {
    Remove,
    Set,
    Error,
    Indeterminate,
    Pause,
}

impl TerminalProgressState {
    #[must_use]
    pub fn indicates_activity(self) -> bool {
        self != Self::Remove
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentProgress {
    pub done: u64,
    pub total: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneAgentStatus {
    pub session_id: String,
    pub parent_session_id: Option<String>,
    pub agent_name: String,
    pub phase: AgentPhase,
    pub text: Option<String>,
    pub interaction: AgentInteractionKind,
    pub progress: Option<AgentProgress>,
    pub tracked_pid: Option<i32>,
    pub transcript_path: Option<String>,
    pub updated_at: u64,
}

impl PaneAgentStatus {
    #[must_use]
    pub fn requires_attention(&self) -> bool {
        self.phase == AgentPhase::NeedsInput && self.interaction != AgentInteractionKind::None
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AgentStatusStore {
    panes: HashMap<String, HashMap<String, PaneAgentStatus>>,
    codex_title_inferred: HashSet<(String, String)>,
    codex_idle_suppression_until: HashMap<(String, String), u64>,
    codex_observed_running: HashSet<(String, String)>,
    codex_interrupt_suppression: HashMap<String, CodexInterruptSuppression>,
}

impl AgentStatusStore {
    /// Reconciles Ghostty's OSC 9;4 activity report without treating its
    /// optional percentage as task completion. Explicit attention remains
    /// authoritative. An interrupted pane has no Codex status to promote, so
    /// an unauthenticated progress report cannot recreate its session.
    pub fn apply_terminal_progress(
        &mut self,
        pane_id: &str,
        state: TerminalProgressState,
        now: u64,
    ) -> bool {
        if !state.indicates_activity() {
            return false;
        }
        let Some(status) = self.panes.get_mut(pane_id).and_then(|sessions| {
            sessions
                .values_mut()
                .filter(|status| status.agent_name.eq_ignore_ascii_case("codex"))
                .max_by_key(|status| (status_priority(status), status.updated_at))
        }) else {
            return false;
        };
        if status.requires_attention() || status.phase == AgentPhase::Running {
            return false;
        }
        status.phase = AgentPhase::Running;
        status.interaction = AgentInteractionKind::None;
        status.text = None;
        status.updated_at = now;
        let key = (pane_id.to_owned(), status.session_id.clone());
        self.codex_title_inferred.remove(&key);
        self.codex_idle_suppression_until.remove(&key);
        self.codex_observed_running.insert(key);
        true
    }

    pub(crate) fn codex_transcript_enrichment_context(
        &self,
        pane_id: &str,
    ) -> Option<(&str, Option<&str>)> {
        self.panes
            .get(pane_id)?
            .values()
            // This marker is created only by a Codex needs-input title and is
            // cleared by every phase/ownership transition. It is therefore
            // the canonical eligibility invariant rather than a second set
            // of status predicates that can drift from marker lifecycle.
            .filter(|status| {
                self.codex_title_inferred
                    .contains(&(pane_id.to_owned(), status.session_id.clone()))
            })
            .max_by_key(|status| (status_priority(status), status.updated_at))
            .map(|status| {
                (
                    status.session_id.as_str(),
                    status.transcript_path.as_deref(),
                )
            })
    }

    pub(crate) fn apply_codex_transcript_enrichment(
        &mut self,
        pane_id: &str,
        session_id: &str,
        question: &crate::CodexTranscriptQuestion,
        now: u64,
    ) -> bool {
        let key = (pane_id.to_owned(), session_id.to_owned());
        if !self.codex_title_inferred.contains(&key) {
            return false;
        }
        let Some(status) = self
            .panes
            .get_mut(pane_id)
            .and_then(|sessions| sessions.get_mut(session_id))
        else {
            return false;
        };
        status.text = Some(question.text.clone());
        status.interaction = question.interaction;
        status.updated_at = now;
        self.codex_title_inferred.remove(&key);
        true
    }

    fn suppress_interrupted_codex_event(
        &mut self,
        pane_id: &str,
        session_id: &str,
        event: &crate::AgentEvent,
        now: u64,
    ) -> Option<bool> {
        let event_is_codex = event
            .agent_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("codex"))
            || self
                .codex_interrupt_suppression
                .get(pane_id)
                .is_some_and(|suppression| suppression.session_ids.contains(session_id));
        if !event_is_codex {
            return Some(false);
        }
        let active = self
            .codex_interrupt_suppression
            .get(pane_id)
            .is_some_and(|suppression| now < suppression.until);
        if event.kind() == "agent.idle" && active {
            return None;
        }
        let new_activity = matches!(
            event.kind(),
            "session.start" | "agent.running" | "agent.input-resolved" | "agent.needs-input"
        );
        if new_activity || !active {
            self.codex_interrupt_suppression.remove(pane_id);
        }
        Some(true)
    }

    fn resolve_session_id(&self, pane_id: &str, event: &crate::AgentEvent) -> String {
        event
            .session_id()
            .map(str::to_owned)
            .or_else(|| {
                if !event
                    .agent_name()
                    .is_some_and(|name| name.eq_ignore_ascii_case("codex"))
                {
                    return None;
                }
                self.panes.get(pane_id).and_then(|sessions| {
                    sessions
                        .values()
                        .filter(|status| status.agent_name.eq_ignore_ascii_case("codex"))
                        .max_by_key(|status| (status_priority(status), status.updated_at))
                        .map(|status| status.session_id.clone())
                })
            })
            .unwrap_or_else(|| "pane-default".to_owned())
    }

    pub fn apply(&mut self, authenticated: AuthenticatedAgentEvent, now: u64) {
        let target = authenticated.target;
        let event = authenticated.event;
        let pane_id = target.pane_id;
        let session_id = self.resolve_session_id(&pane_id, &event);
        let Some(event_is_codex) =
            self.suppress_interrupted_codex_event(&pane_id, &session_id, &event, now)
        else {
            return;
        };
        let sessions = self.panes.entry(pane_id.clone()).or_default();
        if event.kind() == "session.end" {
            sessions.remove(&session_id);
            self.codex_title_inferred
                .remove(&(pane_id.clone(), session_id.clone()));
            self.codex_idle_suppression_until
                .remove(&(pane_id.clone(), session_id.clone()));
            self.codex_observed_running
                .remove(&(pane_id.clone(), session_id.clone()));
            if sessions.is_empty() {
                self.panes.remove(&pane_id);
            }
            return;
        }

        let status = sessions
            .entry(session_id.clone())
            .or_insert_with(|| PaneAgentStatus {
                session_id: session_id.clone(),
                parent_session_id: event.parent_session_id().map(str::to_owned),
                agent_name: event
                    .agent_name()
                    .unwrap_or(if event_is_codex { "Codex" } else { "Agent" })
                    .to_owned(),
                phase: AgentPhase::Starting,
                text: None,
                interaction: AgentInteractionKind::None,
                progress: None,
                tracked_pid: None,
                transcript_path: None,
                updated_at: now,
            });
        update_status_identity(status, &event);
        if should_suppress_claude_post_stop_notification(status, &event, now) {
            return;
        }
        match event.kind() {
            "session.start" => {
                status.phase = AgentPhase::Starting;
                self.codex_title_inferred
                    .remove(&(pane_id.clone(), session_id.clone()));
                self.codex_idle_suppression_until
                    .remove(&(pane_id.clone(), session_id.clone()));
            }
            "agent.running" | "agent.input-resolved" => {
                status.phase = AgentPhase::Running;
                status.interaction = AgentInteractionKind::None;
                status.text = event.state_text().map(str::to_owned);
                self.codex_title_inferred
                    .remove(&(pane_id.clone(), session_id.clone()));
                self.codex_idle_suppression_until
                    .remove(&(pane_id.clone(), session_id.clone()));
                if status.agent_name.eq_ignore_ascii_case("codex") {
                    self.codex_observed_running
                        .insert((pane_id.clone(), session_id.clone()));
                }
            }
            "agent.idle" => {
                status.phase = AgentPhase::Idle;
                status.interaction = AgentInteractionKind::None;
                status.text = None;
                self.codex_title_inferred
                    .remove(&(pane_id.clone(), session_id.clone()));
                self.codex_idle_suppression_until.insert(
                    (pane_id.clone(), session_id.clone()),
                    now.saturating_add(CODEX_TITLE_IDLE_SUPPRESSION_MS),
                );
            }
            "agent.needs-input" => {
                status.phase = AgentPhase::NeedsInput;
                status.interaction = event.interaction();
                status.text = event
                    .interaction_text()
                    .or_else(|| event.state_text())
                    .map(str::to_owned);
                self.codex_title_inferred
                    .remove(&(pane_id.clone(), session_id.clone()));
                self.codex_idle_suppression_until
                    .remove(&(pane_id.clone(), session_id.clone()));
            }
            "task.progress" => {
                status.progress = event
                    .progress()
                    .map(|(done, total)| AgentProgress { done, total });
            }
            _ => unreachable!("AgentEvent exposes only known protocol events"),
        }
        status.updated_at = now;
    }

    /// Reconciles one real Ghostty title callback into the canonical Codex
    /// status store. Returns whether sidebar-visible status changed.
    pub fn apply_codex_title(&mut self, pane_id: &str, title: &str, now: u64) -> bool {
        let Some(signal) = classify_codex_terminal_title(title) else {
            return false;
        };
        let Some(session_id) = self.panes.get(pane_id).and_then(|sessions| {
            sessions
                .values()
                .filter(|status| status.agent_name.eq_ignore_ascii_case("codex"))
                .max_by_key(|status| (status_priority(status), status.updated_at))
                .map(|status| status.session_id.clone())
        }) else {
            return false;
        };
        let key = (pane_id.to_owned(), session_id.clone());
        let title_inferred = self.codex_title_inferred.contains(&key);
        let idle_suppressed = self
            .codex_idle_suppression_until
            .get(&key)
            .is_some_and(|deadline| now < *deadline);
        let Some(status) = self
            .panes
            .get_mut(pane_id)
            .and_then(|sessions| sessions.get_mut(&session_id))
        else {
            return false;
        };
        let mut changed = false;
        if status.progress != signal.progress && signal.progress.is_some() {
            status.progress = signal.progress;
            changed = true;
        }
        if signal.background_wait {
            if changed {
                status.updated_at = now;
            }
            return changed;
        }

        match signal.phase {
            CodexTitlePhase::NeedsInput => {
                if !status.requires_attention() {
                    status.phase = AgentPhase::NeedsInput;
                    status.interaction = signal.interaction;
                    status.text = Some(title.trim().to_owned());
                    self.codex_title_inferred.insert(key.clone());
                    changed = true;
                }
            }
            CodexTitlePhase::Running | CodexTitlePhase::Starting => {
                if status.requires_attention() && !title_inferred {
                    // A strong explicit question/decision owns the state until
                    // an explicit event or user input resolves it.
                } else if status.phase != AgentPhase::Idle || !idle_suppressed {
                    let phase = if signal.phase == CodexTitlePhase::Running {
                        AgentPhase::Running
                    } else {
                        AgentPhase::Starting
                    };
                    if status.phase != phase || status.text.is_some() {
                        status.phase = phase;
                        status.interaction = AgentInteractionKind::None;
                        status.text = None;
                        changed = true;
                    }
                    self.codex_title_inferred.remove(&key);
                    self.codex_idle_suppression_until.remove(&key);
                }
            }
            CodexTitlePhase::Idle => {
                if !status.requires_attention() || title_inferred {
                    if status.phase != AgentPhase::Idle {
                        status.phase = AgentPhase::Idle;
                        status.interaction = AgentInteractionKind::None;
                        status.text = None;
                        changed = true;
                    }
                    self.codex_title_inferred.remove(&key);
                    self.codex_idle_suppression_until
                        .insert(key, now.saturating_add(CODEX_TITLE_IDLE_SUPPRESSION_MS));
                }
            }
        }
        if changed {
            status.updated_at = now;
        }
        changed
    }

    /// Promotes an explicit Codex session after input is actually submitted to
    /// the terminal. A question remains visible for 350 ms so the Return that
    /// accepted the preceding terminal interaction cannot immediately erase a
    /// newly delivered question.
    pub fn apply_codex_user_submitted(&mut self, pane_id: &str, now: u64) -> bool {
        self.codex_interrupt_suppression.remove(pane_id);
        self.codex_idle_suppression_until
            .retain(|(tracked_pane, _), _| tracked_pane != pane_id);
        let observed_running = &self.codex_observed_running;
        let Some(status) = self.panes.get_mut(pane_id).and_then(|sessions| {
            sessions
                .values_mut()
                .filter(|status| status.agent_name.eq_ignore_ascii_case("codex"))
                .filter(|status| match status.phase {
                    AgentPhase::NeedsInput => {
                        now.saturating_sub(status.updated_at) >= CODEX_INPUT_SUBMIT_STABILIZATION_MS
                    }
                    AgentPhase::Starting => true,
                    AgentPhase::Idle => {
                        observed_running.contains(&(pane_id.to_owned(), status.session_id.clone()))
                    }
                    AgentPhase::Running | AgentPhase::UnresolvedStop => false,
                })
                .max_by_key(|status| (status_priority(status), status.updated_at))
        }) else {
            return false;
        };
        status.phase = AgentPhase::Running;
        status.interaction = AgentInteractionKind::None;
        status.text = None;
        status.updated_at = now;
        self.codex_title_inferred
            .remove(&(pane_id.to_owned(), status.session_id.clone()));
        true
    }

    /// Clears Codex state on an exact Ctrl-C terminal gesture and remembers
    /// the interrupted sessions long enough to reject their late idle event.
    pub fn apply_codex_user_interrupted(&mut self, pane_id: &str, now: u64) -> bool {
        let Some(sessions) = self.panes.get_mut(pane_id) else {
            return false;
        };
        let session_ids = sessions
            .values()
            .filter(|status| status.agent_name.eq_ignore_ascii_case("codex"))
            .map(|status| status.session_id.clone())
            .collect::<HashSet<_>>();
        if session_ids.is_empty() {
            return false;
        }
        sessions.retain(|_, status| !status.agent_name.eq_ignore_ascii_case("codex"));
        if sessions.is_empty() {
            self.panes.remove(pane_id);
        }
        self.codex_title_inferred
            .retain(|(tracked_pane, _)| tracked_pane != pane_id);
        self.codex_idle_suppression_until
            .retain(|(tracked_pane, _), _| tracked_pane != pane_id);
        self.codex_observed_running
            .retain(|(tracked_pane, _)| tracked_pane != pane_id);
        self.codex_interrupt_suppression.insert(
            pane_id.to_owned(),
            CodexInterruptSuppression {
                until: now.saturating_add(CODEX_INTERRUPT_SUPPRESSION_MS),
                session_ids,
            },
        );
        true
    }

    /// Clears active Codex state only when terminal metadata is the basename
    /// of a shell recognized by the source implementation.
    pub fn clear_codex_after_shell_return(&mut self, pane_id: &str, title: &str) -> bool {
        if !is_known_shell_name(title) {
            return false;
        }
        let has_active_codex = self.panes.get(pane_id).is_some_and(|sessions| {
            sessions.values().any(|status| {
                status.agent_name.eq_ignore_ascii_case("codex")
                    && (status.phase != AgentPhase::Idle || status.requires_attention())
            })
        }) || self.codex_interrupt_suppression.contains_key(pane_id);
        if !has_active_codex {
            return false;
        }
        if let Some(sessions) = self.panes.get_mut(pane_id) {
            sessions.retain(|_, status| !status.agent_name.eq_ignore_ascii_case("codex"));
            if sessions.is_empty() {
                self.panes.remove(pane_id);
            }
        }
        self.codex_title_inferred
            .retain(|(tracked_pane, _)| tracked_pane != pane_id);
        self.codex_idle_suppression_until
            .retain(|(tracked_pane, _), _| tracked_pane != pane_id);
        self.codex_observed_running
            .retain(|(tracked_pane, _)| tracked_pane != pane_id);
        self.codex_interrupt_suppression.remove(pane_id);
        true
    }

    #[must_use]
    pub fn status_for(&self, target: &AgentTarget) -> Option<&PaneAgentStatus> {
        self.panes
            .get(&target.pane_id)?
            .values()
            .max_by_key(|status| (status_priority(status), status.updated_at))
    }

    #[must_use]
    pub fn status_for_pane(&self, pane_id: &str) -> Option<&PaneAgentStatus> {
        self.panes
            .get(pane_id)?
            .values()
            .max_by_key(|status| (status_priority(status), status.updated_at))
    }

    pub fn remove_pane(&mut self, pane_id: &str) {
        self.panes.remove(pane_id);
        self.codex_title_inferred
            .retain(|(tracked_pane, _)| tracked_pane != pane_id);
        self.codex_idle_suppression_until
            .retain(|(tracked_pane, _), _| tracked_pane != pane_id);
        self.codex_observed_running
            .retain(|(tracked_pane, _)| tracked_pane != pane_id);
        self.codex_interrupt_suppression.remove(pane_id);
    }
}

fn should_suppress_claude_post_stop_notification(
    status: &PaneAgentStatus,
    event: &crate::AgentEvent,
    now: u64,
) -> bool {
    event.kind() == "agent.needs-input"
        && status.agent_name.eq_ignore_ascii_case("claude code")
        && status.phase == AgentPhase::Idle
        && event.interaction() == AgentInteractionKind::GenericInput
        && now.saturating_sub(status.updated_at) < CLAUDE_POST_STOP_NEEDS_INPUT_GRACE_MS
}

fn is_known_shell_name(value: &str) -> bool {
    let basename = value
        .trim()
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        basename.as_str(),
        "zsh" | "bash" | "fish" | "sh" | "pwsh" | "nu"
    )
}

fn update_status_identity(status: &mut PaneAgentStatus, event: &crate::AgentEvent) {
    if let Some(name) = event.agent_name() {
        name.clone_into(&mut status.agent_name);
    }
    if let Some(pid) = event.agent_pid() {
        status.tracked_pid = Some(pid);
    }
    if let Some(path) = event.transcript_path() {
        status.transcript_path = Some(path.to_owned());
    }
}

fn status_priority(status: &PaneAgentStatus) -> u8 {
    if status.requires_attention() {
        return 5;
    }
    match status.phase {
        AgentPhase::Running => 4,
        AgentPhase::Starting => 3,
        AgentPhase::Idle => 2,
        AgentPhase::UnresolvedStop => 1,
        AgentPhase::NeedsInput => 5,
    }
}
