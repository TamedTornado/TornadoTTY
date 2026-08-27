use crate::{
    AgentInteractionKind, AgentTarget, AuthenticatedAgentEvent, CodexTitlePhase,
    classify_codex_terminal_title,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

const CODEX_TITLE_IDLE_SUPPRESSION_MS: u64 = 1_000;
const CODEX_RUNNING_TITLE_ATTENTION_GRACE_MS: u64 = 1_000;
const CODEX_INPUT_SUBMIT_STABILIZATION_MS: u64 = 350;
const CODEX_INTERRUPT_SUPPRESSION_MS: u64 = 3_000;
const CLAUDE_POST_STOP_NEEDS_INPUT_GRACE_MS: u64 = 5_000;
const STOP_GRACE_MS: u64 = 2_000;
const EPHEMERAL_START_EXIT_MS: u64 = 1_000;
const IDLE_VISIBILITY_MS: u64 = 120_000;
const UNRESOLVED_STOP_VISIBILITY_MS: u64 = 600_000;
const STALE_SESSION_VISIBILITY_MS: u64 = 1_800_000;

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
pub enum AgentSignalOrigin {
    Compatibility,
    ExplicitHook,
    ExplicitApi,
    Heuristic,
    Shell,
    Inferred,
}

impl AgentSignalOrigin {
    const fn priority(self) -> u8 {
        match self {
            Self::ExplicitHook | Self::ExplicitApi => 4,
            Self::Heuristic => 3,
            Self::Compatibility => 2,
            Self::Shell => 1,
            Self::Inferred => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentSignalConfidence {
    Weak,
    Strong,
    Explicit,
}

impl AgentSignalConfidence {
    const fn priority(self) -> u8 {
        match self {
            Self::Explicit => 2,
            Self::Strong => 1,
            Self::Weak => 0,
        }
    }
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

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct AgentProgress {
    pub done: u64,
    pub total: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct RestoredTaskState<'a> {
    pub(crate) progress: Option<AgentProgress>,
    pub(crate) tasks: &'a BTreeMap<String, bool>,
    pub(crate) authoritative: bool,
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
    pub artifact_link: Option<crate::AgentArtifactLink>,
    pub working_directory: Option<String>,
    pub agent_launch_snapshot: Option<crate::AgentLaunchSnapshot>,
    pub signal_origin: AgentSignalOrigin,
    pub signal_confidence: AgentSignalConfidence,
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
    session_bookkeeping: HashMap<SessionKey, SessionBookkeeping>,
    codex_interrupt_suppression: HashMap<String, CodexInterruptSuppression>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct SessionKey {
    pane_id: String,
    session_id: String,
}

impl SessionKey {
    fn new(pane_id: &str, session_id: &str) -> Self {
        Self {
            pane_id: pane_id.to_owned(),
            session_id: session_id.to_owned(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SessionBookkeeping {
    tasks: HashMap<String, bool>,
    task_progress_authority: TaskProgressAuthority,
    lifecycle: SessionLifecycle,
    codex_title_ownership: CodexTitleOwnership,
    codex_idle_suppression_until: Option<u64>,
    observed_running: bool,
    completion_candidate_deadline: Option<u64>,
    idle_visible_until: Option<u64>,
    unresolved_stop_visible_until: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum TaskProgressAuthority {
    #[default]
    IdentityEvents,
    CounterEvents,
    ExplicitSnapshot,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum SessionLifecycle {
    #[default]
    Active,
    Ended,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum CodexTitleOwnership {
    #[default]
    Explicit,
    Inferred,
}

impl AgentStatusStore {
    pub(crate) fn seed_restored_starting(
        &mut self,
        pane_id: &str,
        session_id: &str,
        agent_name: &str,
        working_directory: Option<&str>,
        task_state: RestoredTaskState<'_>,
        now: u64,
    ) {
        self.panes.entry(pane_id.to_owned()).or_default().insert(
            session_id.to_owned(),
            PaneAgentStatus {
                session_id: session_id.to_owned(),
                parent_session_id: None,
                agent_name: agent_name.to_owned(),
                phase: AgentPhase::Starting,
                text: None,
                interaction: AgentInteractionKind::None,
                progress: task_state.progress,
                tracked_pid: None,
                transcript_path: None,
                artifact_link: None,
                working_directory: working_directory
                    .and_then(crate::agent_protocol::canonical_working_directory),
                agent_launch_snapshot: None,
                signal_origin: AgentSignalOrigin::Compatibility,
                signal_confidence: AgentSignalConfidence::Strong,
                updated_at: now,
            },
        );
        if !task_state.tasks.is_empty() || task_state.authoritative || task_state.progress.is_some()
        {
            self.session_bookkeeping.insert(
                SessionKey::new(pane_id, session_id),
                SessionBookkeeping {
                    tasks: task_state.tasks.clone().into_iter().collect(),
                    task_progress_authority: if task_state.authoritative {
                        TaskProgressAuthority::ExplicitSnapshot
                    } else if task_state.tasks.is_empty() && task_state.progress.is_some() {
                        TaskProgressAuthority::CounterEvents
                    } else {
                        TaskProgressAuthority::IdentityEvents
                    },
                    ..SessionBookkeeping::default()
                },
            );
        }
    }

    pub(crate) fn task_restore_state(
        &self,
        pane_id: &str,
        session_id: &str,
    ) -> (BTreeMap<String, bool>, bool) {
        let bookkeeping = self
            .session_bookkeeping
            .get(&SessionKey::new(pane_id, session_id));
        let tasks = bookkeeping
            .into_iter()
            .flat_map(|state| state.tasks.iter())
            .map(|(id, completed)| (id.clone(), *completed))
            .collect();
        (
            tasks,
            bookkeeping.is_some_and(|state| {
                state.task_progress_authority == TaskProgressAuthority::ExplicitSnapshot
            }),
        )
    }

    /// Reconciles Ghostty's OSC 9;4 activity report without treating its
    /// optional percentage as task completion. Explicit attention remains
    /// authoritative. A pane without an existing Codex or Copilot status has
    /// no session to promote, so an unauthenticated progress report cannot
    /// create one.
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
                .filter(|status| {
                    status.agent_name.eq_ignore_ascii_case("codex")
                        || is_copilot_agent_name(&status.agent_name)
                })
                .max_by_key(|status| (status_priority(status), status.updated_at))
        }) else {
            return false;
        };
        if status.requires_attention() {
            return false;
        }
        let visible_changed = status.phase != AgentPhase::Running || status.text.is_some();
        if visible_changed {
            status.phase = AgentPhase::Running;
            status.interaction = AgentInteractionKind::None;
            status.text = None;
            status.updated_at = now;
        }
        let is_codex = status.agent_name.eq_ignore_ascii_case("codex");
        let lifecycle = self
            .session_bookkeeping
            .entry(SessionKey::new(pane_id, &status.session_id))
            .or_default();
        if is_codex {
            lifecycle.codex_title_ownership = CodexTitleOwnership::Explicit;
            lifecycle.codex_idle_suppression_until = None;
        }
        lifecycle.observed_running = true;
        let candidate_cancelled = lifecycle.completion_candidate_deadline.take().is_some();
        lifecycle.idle_visible_until = None;
        lifecycle.unresolved_stop_visible_until = None;
        visible_changed || candidate_cancelled
    }

    /// Applies terminal-title presentation rules through the canonical agent
    /// store. Copilot question titles can update only an existing recognized
    /// Copilot session; title text alone never creates agent state.
    pub fn apply_terminal_title(&mut self, pane_id: &str, title: &str, now: u64) -> bool {
        if let Some(changed) = self.apply_hermes_status_title(pane_id, title, now) {
            return changed;
        }
        if let Some(changed) = self.apply_copilot_question_title(pane_id, title, now) {
            return changed;
        }
        self.clear_codex_after_shell_return(pane_id, title)
            || self.apply_codex_title(pane_id, title, now)
    }

    fn apply_hermes_status_title(&mut self, pane_id: &str, title: &str, now: u64) -> Option<bool> {
        let normalized = title.trim();
        let mut characters = normalized.chars();
        let phase = match characters.next()? {
            '⏳' => AgentPhase::Running,
            '✓' => AgentPhase::Idle,
            '⚠' => AgentPhase::NeedsInput,
            _ => return None,
        };
        let subject = characters.as_str().trim_start_matches('\u{fe0f}').trim();
        if subject.is_empty() {
            return None;
        }
        let status = self.panes.get_mut(pane_id).and_then(|sessions| {
            sessions
                .values_mut()
                .filter(|status| {
                    status.agent_name.eq_ignore_ascii_case("hermes")
                        || status.agent_name.eq_ignore_ascii_case("hermes agent")
                })
                .max_by_key(|status| (status_priority(status), status.updated_at))
        })?;
        let interaction = if phase == AgentPhase::NeedsInput {
            AgentInteractionKind::Question
        } else {
            AgentInteractionKind::None
        };
        let changed =
            status.phase != phase || status.interaction != interaction || status.text.is_some();
        if changed {
            status.phase = phase;
            status.interaction = interaction;
            status.text = None;
            status.updated_at = now;
        }
        Some(changed)
    }

    fn apply_copilot_question_title(
        &mut self,
        pane_id: &str,
        title: &str,
        now: u64,
    ) -> Option<bool> {
        if !copilot_title_indicates_needs_input(title) {
            return None;
        }
        let status = self.panes.get_mut(pane_id).and_then(|sessions| {
            sessions
                .values_mut()
                .filter(|status| is_copilot_agent_name(&status.agent_name))
                .max_by_key(|status| (status_priority(status), status.updated_at))
        })?;
        let changed = status.phase != AgentPhase::NeedsInput
            || status.interaction != AgentInteractionKind::Question
            || status.text.is_some();
        if !changed {
            return Some(false);
        }
        status.phase = AgentPhase::NeedsInput;
        status.interaction = AgentInteractionKind::Question;
        status.text = None;
        status.updated_at = now;
        let lifecycle = self
            .session_bookkeeping
            .entry(SessionKey::new(pane_id, &status.session_id))
            .or_default();
        lifecycle.completion_candidate_deadline = None;
        lifecycle.idle_visible_until = None;
        lifecycle.unresolved_stop_visible_until = None;
        Some(true)
    }

    /// Reconciles the two Gemini desktop-notification phrases owned by the
    /// source application. The terminal path is heuristic and deliberately
    /// narrow: unrelated notifications, including the same completion copy
    /// from a non-Gemini process, cannot create agent state.
    pub fn apply_terminal_notification(
        &mut self,
        pane_id: &str,
        title: Option<&str>,
        body: Option<&str>,
        now: u64,
    ) -> bool {
        let existing = self.status_for_pane(pane_id);
        let recognized_gemini = existing.map_or_else(
            || title.is_some_and(|value| value.trim().eq_ignore_ascii_case("gemini")),
            |status| status.agent_name.eq_ignore_ascii_case("gemini"),
        );
        if !recognized_gemini {
            return false;
        }

        let combined = [title, body]
            .into_iter()
            .flatten()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join(": ")
            .to_ascii_lowercase();
        let phase = if combined.contains("session complete") {
            AgentPhase::Idle
        } else if combined.contains("action required") {
            AgentPhase::NeedsInput
        } else {
            return false;
        };
        let session_id = existing.map_or_else(
            || "terminal-gemini".to_owned(),
            |status| status.session_id.clone(),
        );
        let status = self
            .panes
            .entry(pane_id.to_owned())
            .or_default()
            .entry(session_id.clone())
            .or_insert_with(|| PaneAgentStatus {
                session_id,
                parent_session_id: None,
                agent_name: "Gemini".to_owned(),
                phase: AgentPhase::Starting,
                text: None,
                interaction: AgentInteractionKind::None,
                progress: None,
                tracked_pid: None,
                transcript_path: None,
                artifact_link: None,
                working_directory: None,
                agent_launch_snapshot: None,
                signal_origin: AgentSignalOrigin::Heuristic,
                signal_confidence: AgentSignalConfidence::Strong,
                updated_at: now,
            });

        let text = (phase == AgentPhase::NeedsInput)
            .then(|| body.map(str::trim).filter(|value| !value.is_empty()))
            .flatten()
            .map(str::to_owned);
        let interaction = if phase == AgentPhase::NeedsInput {
            AgentInteractionKind::Approval
        } else {
            AgentInteractionKind::None
        };
        let changed =
            status.phase != phase || status.interaction != interaction || status.text != text;
        status.phase = phase;
        status.interaction = interaction;
        status.text = text;
        status.updated_at = now;
        let lifecycle = self
            .session_bookkeeping
            .entry(SessionKey::new(pane_id, &status.session_id))
            .or_default();
        lifecycle.completion_candidate_deadline = None;
        lifecycle.unresolved_stop_visible_until = None;
        if phase == AgentPhase::Idle {
            lifecycle.idle_visible_until = Some(now.saturating_add(IDLE_VISIBILITY_MS));
        } else {
            lifecycle.idle_visible_until = None;
        }
        changed
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
                self.session_bookkeeping
                    .get(&SessionKey::new(pane_id, &status.session_id))
                    .is_some_and(|state| {
                        state.codex_title_ownership == CodexTitleOwnership::Inferred
                    })
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
        let key = SessionKey::new(pane_id, session_id);
        if !self
            .session_bookkeeping
            .get(&key)
            .is_some_and(|state| state.codex_title_ownership == CodexTitleOwnership::Inferred)
        {
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
        if let Some(state) = self.session_bookkeeping.get_mut(&key) {
            state.codex_title_ownership = CodexTitleOwnership::Explicit;
        }
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
            "session.start"
                | "agent.running"
                | "agent.compacting"
                | "agent.compacted"
                | "agent.input-resolved"
                | "agent.needs-input"
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

    fn rekey_matching_provisional_session(
        &mut self,
        pane_id: &str,
        session_id: &str,
        event: &crate::AgentEvent,
    ) {
        if session_id == "pane-default" || event.kind() != "session.start" {
            return;
        }
        let (Some(agent_name), Some(agent_pid)) = (event.agent_name(), event.agent_pid()) else {
            return;
        };
        let matches = self
            .panes
            .get(pane_id)
            .and_then(|sessions| sessions.get("pane-default"))
            .is_some_and(|status| {
                status.agent_name.eq_ignore_ascii_case(agent_name)
                    && status.tracked_pid == Some(agent_pid)
            });
        if !matches {
            return;
        }
        if let Some(sessions) = self.panes.get_mut(pane_id)
            && let Some(mut provisional) = sessions.remove("pane-default")
        {
            session_id.clone_into(&mut provisional.session_id);
            sessions.entry(session_id.to_owned()).or_insert(provisional);
        }
        let provisional_key = SessionKey::new(pane_id, "pane-default");
        if let Some(bookkeeping) = self.session_bookkeeping.remove(&provisional_key) {
            self.session_bookkeeping
                .entry(SessionKey::new(pane_id, session_id))
                .or_insert(bookkeeping);
        }
    }

    pub fn apply(&mut self, authenticated: AuthenticatedAgentEvent, now: u64) {
        let target = authenticated.target;
        let event = authenticated.event;
        self.apply_for_target(target, &event, now);
    }

    pub(crate) fn apply_for_target(
        &mut self,
        target: AgentTarget,
        event: &crate::AgentEvent,
        now: u64,
    ) {
        self.apply_for_target_with_signal(
            target,
            event,
            AgentSignalOrigin::ExplicitHook,
            AgentSignalConfidence::Explicit,
            now,
        );
    }

    // Keeping the canonical lifecycle phases in one exhaustive transition
    // table makes precedence reviewable; task identity bookkeeping is split
    // into `apply_task_projection` below.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn apply_for_target_with_signal(
        &mut self,
        target: AgentTarget,
        event: &crate::AgentEvent,
        origin: AgentSignalOrigin,
        confidence: AgentSignalConfidence,
        now: u64,
    ) {
        let pane_id = target.pane_id;
        let session_id = self.resolve_session_id(&pane_id, event);
        self.rekey_matching_provisional_session(&pane_id, &session_id, event);
        let session_key = SessionKey::new(&pane_id, &session_id);
        if self
            .session_bookkeeping
            .get(&session_key)
            .is_some_and(|state| state.lifecycle == SessionLifecycle::Ended)
            && event.kind() != "session.start"
        {
            return;
        }
        if event.kind() == "session.start"
            && let Some(state) = self.session_bookkeeping.get_mut(&session_key)
        {
            state.lifecycle = SessionLifecycle::Active;
        }
        let Some(event_is_codex) =
            self.suppress_interrupted_codex_event(&pane_id, &session_id, event, now)
        else {
            return;
        };
        if event.kind() == "session.end" {
            self.remove_session(&pane_id, &session_id);
            self.session_bookkeeping
                .entry(session_key)
                .or_default()
                .lifecycle = SessionLifecycle::Ended;
            return;
        }

        let sessions = self.panes.entry(pane_id.clone()).or_default();
        let status = sessions.entry(session_id.clone()).or_insert_with(|| {
            new_status(&session_id, event, event_is_codex, origin, confidence, now)
        });
        if !should_apply_signal(status, event, origin, confidence) {
            return;
        }
        update_status_identity(status, event);
        if should_suppress_claude_post_stop_notification(status, event, now) {
            return;
        }
        let lifecycle = self.session_bookkeeping.entry(session_key).or_default();
        match event.kind() {
            "session.start" => {
                status.phase = AgentPhase::Starting;
                lifecycle.codex_title_ownership = CodexTitleOwnership::Explicit;
                lifecycle.codex_idle_suppression_until = None;
                lifecycle.completion_candidate_deadline = None;
                lifecycle.idle_visible_until = None;
                lifecycle.unresolved_stop_visible_until = None;
            }
            "agent.running" | "agent.input-resolved" | "agent.compacting" | "agent.compacted" => {
                status.phase = AgentPhase::Running;
                status.interaction = AgentInteractionKind::None;
                status.text = if event.kind() == "agent.compacting" {
                    Some(event.state_text().unwrap_or("Compacting").to_owned())
                } else {
                    event.state_text().map(str::to_owned)
                };
                lifecycle.codex_title_ownership = CodexTitleOwnership::Explicit;
                lifecycle.codex_idle_suppression_until = None;
                lifecycle.observed_running = true;
                lifecycle.completion_candidate_deadline = None;
                lifecycle.idle_visible_until = None;
                lifecycle.unresolved_stop_visible_until = None;
            }
            "agent.idle" => {
                if event.stop_candidate() {
                    status.phase = AgentPhase::Running;
                    status.interaction = AgentInteractionKind::None;
                    status.text = None;
                    lifecycle.completion_candidate_deadline =
                        Some(now.saturating_add(STOP_GRACE_MS));
                    lifecycle.idle_visible_until = None;
                    lifecycle.unresolved_stop_visible_until = None;
                    status.updated_at = now;
                    return;
                }
                status.phase = AgentPhase::Idle;
                status.interaction = AgentInteractionKind::None;
                status.text = None;
                lifecycle.codex_title_ownership = CodexTitleOwnership::Explicit;
                lifecycle.codex_idle_suppression_until =
                    Some(now.saturating_add(CODEX_TITLE_IDLE_SUPPRESSION_MS));
                lifecycle.completion_candidate_deadline = None;
                lifecycle.idle_visible_until = Some(now.saturating_add(IDLE_VISIBILITY_MS));
                lifecycle.unresolved_stop_visible_until = None;
            }
            "agent.needs-input" => {
                status.phase = AgentPhase::NeedsInput;
                status.interaction = event.interaction();
                status.text = event
                    .interaction_text()
                    .or_else(|| event.state_text())
                    .map(str::to_owned);
                lifecycle.codex_title_ownership = CodexTitleOwnership::Explicit;
                lifecycle.codex_idle_suppression_until = None;
                lifecycle.completion_candidate_deadline = None;
                lifecycle.idle_visible_until = None;
                lifecycle.unresolved_stop_visible_until = None;
            }
            "agent.failed" => {
                status.phase = AgentPhase::UnresolvedStop;
                status.interaction = AgentInteractionKind::None;
                status.text = event.state_text().map(str::to_owned);
                lifecycle.codex_title_ownership = CodexTitleOwnership::Explicit;
                lifecycle.codex_idle_suppression_until = None;
                lifecycle.completion_candidate_deadline = None;
                lifecycle.idle_visible_until = None;
                lifecycle.unresolved_stop_visible_until =
                    Some(now.saturating_add(UNRESOLVED_STOP_VISIBILITY_MS));
            }
            "task.progress" | "task.snapshot" | "task.delta" | "task.started"
            | "task.completed" => {
                if !apply_task_projection(status, event, lifecycle) {
                    return;
                }
            }
            _ => unreachable!("AgentEvent exposes only known protocol events"),
        }
        status.signal_origin = origin;
        status.signal_confidence = confidence;
        status.updated_at = now;
    }

    pub(crate) fn apply_pid_signal(
        &mut self,
        pane_id: &str,
        session_id: Option<&str>,
        parent_session_id: Option<&str>,
        tool: Option<&str>,
        pid: Option<i32>,
        now: u64,
    ) -> bool {
        if pid.is_none() {
            let Some(sessions) = self.panes.get_mut(pane_id) else {
                return false;
            };
            let mut changed = false;
            if let Some(session_id) = session_id {
                if let Some(status) = sessions.get_mut(session_id)
                    && status.tracked_pid.take().is_some()
                {
                    status.updated_at = now;
                    changed = true;
                }
            } else {
                for status in sessions.values_mut() {
                    if status.tracked_pid.take().is_some() {
                        status.updated_at = now;
                        changed = true;
                    }
                }
            }
            return changed;
        }
        let session_id = session_id.unwrap_or("pane-default");
        let status = self
            .panes
            .entry(pane_id.to_owned())
            .or_default()
            .entry(session_id.to_owned())
            .or_insert_with(|| PaneAgentStatus {
                session_id: session_id.to_owned(),
                parent_session_id: parent_session_id.map(str::to_owned),
                agent_name: tool.unwrap_or("Agent").to_owned(),
                phase: AgentPhase::Starting,
                text: None,
                interaction: AgentInteractionKind::None,
                progress: None,
                tracked_pid: None,
                transcript_path: None,
                artifact_link: None,
                working_directory: None,
                agent_launch_snapshot: None,
                signal_origin: AgentSignalOrigin::ExplicitApi,
                signal_confidence: AgentSignalConfidence::Explicit,
                updated_at: now,
            });
        let changed = status.tracked_pid != pid;
        status.tracked_pid = pid;
        if let Some(parent_session_id) = parent_session_id {
            status.parent_session_id = Some(parent_session_id.to_owned());
        }
        if let Some(tool) = tool {
            tool.clone_into(&mut status.agent_name);
        }
        if changed {
            status.updated_at = now;
        }
        changed
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
        let key = SessionKey::new(pane_id, &session_id);
        let lifecycle = self.session_bookkeeping.entry(key).or_default();
        let title_inferred = lifecycle.codex_title_ownership == CodexTitleOwnership::Inferred;
        let idle_suppressed = lifecycle
            .codex_idle_suppression_until
            .is_some_and(|deadline| now < deadline);
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
                    lifecycle.codex_title_ownership = CodexTitleOwnership::Inferred;
                    lifecycle.completion_candidate_deadline = None;
                    lifecycle.idle_visible_until = None;
                    lifecycle.unresolved_stop_visible_until = None;
                    changed = true;
                }
            }
            CodexTitlePhase::Running | CodexTitlePhase::Starting => {
                if status.requires_attention()
                    && !title_inferred
                    && now.saturating_sub(status.updated_at)
                        < CODEX_RUNNING_TITLE_ATTENTION_GRACE_MS
                {
                    // Preserve a fresh explicit question against a stale title
                    // frame already in flight. A persistently animated Working
                    // title is authoritative activity and clears the stale
                    // attention state after this bounded grace period.
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
                    lifecycle.codex_title_ownership = CodexTitleOwnership::Explicit;
                    lifecycle.codex_idle_suppression_until = None;
                    if lifecycle.completion_candidate_deadline.take().is_some() {
                        changed = true;
                    }
                    lifecycle.idle_visible_until = None;
                    lifecycle.unresolved_stop_visible_until = None;
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
                    lifecycle.codex_title_ownership = CodexTitleOwnership::Explicit;
                    lifecycle.codex_idle_suppression_until =
                        Some(now.saturating_add(CODEX_TITLE_IDLE_SUPPRESSION_MS));
                    lifecycle.completion_candidate_deadline = None;
                    lifecycle.idle_visible_until = Some(now.saturating_add(IDLE_VISIBILITY_MS));
                    lifecycle.unresolved_stop_visible_until = None;
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
        for (key, state) in &mut self.session_bookkeeping {
            if key.pane_id == pane_id {
                state.codex_idle_suppression_until = None;
            }
        }
        let session_bookkeeping = &self.session_bookkeeping;
        let Some(status) = self.panes.get_mut(pane_id).and_then(|sessions| {
            sessions
                .values_mut()
                .filter(|status| status.agent_name.eq_ignore_ascii_case("codex"))
                .filter(|status| match status.phase {
                    AgentPhase::NeedsInput => {
                        now.saturating_sub(status.updated_at) >= CODEX_INPUT_SUBMIT_STABILIZATION_MS
                    }
                    AgentPhase::Starting => true,
                    AgentPhase::Idle => session_bookkeeping
                        .get(&SessionKey::new(pane_id, &status.session_id))
                        .is_some_and(|state| state.observed_running),
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
        let lifecycle = self
            .session_bookkeeping
            .entry(SessionKey::new(pane_id, &status.session_id))
            .or_default();
        lifecycle.codex_title_ownership = CodexTitleOwnership::Explicit;
        lifecycle.observed_running = true;
        lifecycle.completion_candidate_deadline = None;
        lifecycle.idle_visible_until = None;
        lifecycle.unresolved_stop_visible_until = None;
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
        self.session_bookkeeping
            .retain(|key, _| key.pane_id != pane_id);
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
        self.session_bookkeeping
            .retain(|key, _| key.pane_id != pane_id);
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

    /// Advances source-defined lifecycle grace and visibility windows.
    ///
    /// Process liveness is supplied by the platform so this reducer remains
    /// deterministic and does not acquire a second runtime or pane registry.
    /// Returns whether any status visible to callers changed.
    pub fn sweep(&mut self, now: u64, mut is_process_alive: impl FnMut(i32) -> bool) -> bool {
        let keys = self
            .panes
            .iter()
            .flat_map(|(pane_id, sessions)| {
                sessions
                    .keys()
                    .map(|session_id| (pane_id.clone(), session_id.clone()))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut changed = false;

        for (pane_id, session_id) in keys {
            let lifecycle = self
                .session_bookkeeping
                .entry(SessionKey::new(&pane_id, &session_id))
                .or_default();
            let Some(status) = self
                .panes
                .get_mut(&pane_id)
                .and_then(|sessions| sessions.get_mut(&session_id))
            else {
                continue;
            };
            let mut remove = false;

            if status.tracked_pid.is_some_and(|pid| !is_process_alive(pid)) {
                status.tracked_pid = None;
                if status.phase == AgentPhase::Idle {
                    remove = true;
                } else {
                    let silence_ephemeral_start = status.phase == AgentPhase::Starting
                        && now.saturating_sub(status.updated_at) <= EPHEMERAL_START_EXIT_MS;
                    if silence_ephemeral_start {
                        remove = true;
                    } else if matches!(status.phase, AgentPhase::Starting | AgentPhase::Running)
                        || status.requires_attention()
                        || lifecycle.completion_candidate_deadline.is_some()
                    {
                        status.phase = AgentPhase::UnresolvedStop;
                        status.interaction = AgentInteractionKind::None;
                        status.text = None;
                        status.updated_at = now;
                        lifecycle.completion_candidate_deadline = None;
                        lifecycle.idle_visible_until = None;
                        lifecycle.unresolved_stop_visible_until =
                            Some(now.saturating_add(UNRESOLVED_STOP_VISIBILITY_MS));
                        changed = true;
                    }
                }
            }

            if !remove
                && lifecycle
                    .completion_candidate_deadline
                    .is_some_and(|deadline| now >= deadline)
            {
                if lifecycle.observed_running {
                    status.phase = AgentPhase::Idle;
                    status.interaction = AgentInteractionKind::None;
                    status.text = None;
                    status.tracked_pid = None;
                    status.updated_at = now;
                    lifecycle.completion_candidate_deadline = None;
                    lifecycle.idle_visible_until = Some(now.saturating_add(IDLE_VISIBILITY_MS));
                    lifecycle.unresolved_stop_visible_until = None;
                    changed = true;
                } else {
                    remove = true;
                }
            }

            if !remove {
                let idle_expired = status.phase == AgentPhase::Idle
                    && status.tracked_pid.is_none()
                    && lifecycle
                        .idle_visible_until
                        .is_some_and(|deadline| now >= deadline);
                let unresolved_expired = status.phase == AgentPhase::UnresolvedStop
                    && status.tracked_pid.is_none()
                    && lifecycle
                        .unresolved_stop_visible_until
                        .is_some_and(|deadline| now >= deadline);
                let stale = status.tracked_pid.is_none()
                    && !status.requires_attention()
                    && now.saturating_sub(status.updated_at) >= STALE_SESSION_VISIBILITY_MS;
                remove = idle_expired || unresolved_expired || stale;
            }

            if remove {
                self.remove_session(&pane_id, &session_id);
                changed = true;
            }
        }
        changed
    }

    pub fn remove_pane(&mut self, pane_id: &str) {
        self.panes.remove(pane_id);
        self.session_bookkeeping
            .retain(|key, _| key.pane_id != pane_id);
        self.codex_interrupt_suppression.remove(pane_id);
    }

    pub(crate) fn take_pane(&mut self, pane_id: &str) -> Self {
        let mut taken = Self::default();
        if let Some(statuses) = self.panes.remove(pane_id) {
            taken.panes.insert(pane_id.to_owned(), statuses);
        }
        taken.session_bookkeeping = self
            .session_bookkeeping
            .iter()
            .filter(|(key, _)| key.pane_id == pane_id)
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        self.session_bookkeeping
            .retain(|key, _| key.pane_id != pane_id);
        if let Some(suppression) = self.codex_interrupt_suppression.remove(pane_id) {
            taken
                .codex_interrupt_suppression
                .insert(pane_id.to_owned(), suppression);
        }
        taken
    }

    pub(crate) fn has_pane_data(&self, pane_id: &str) -> bool {
        self.panes.contains_key(pane_id)
            || self
                .session_bookkeeping
                .keys()
                .any(|key| key.pane_id == pane_id)
            || self.codex_interrupt_suppression.contains_key(pane_id)
    }

    pub(crate) fn adopt_pane_data(&mut self, pane_id: &str, source: Self) -> bool {
        if self.has_pane_data(pane_id)
            || source
                .panes
                .keys()
                .any(|source_pane_id| source_pane_id != pane_id)
            || source
                .session_bookkeeping
                .keys()
                .any(|key| key.pane_id != pane_id)
            || source
                .codex_interrupt_suppression
                .keys()
                .any(|source_pane_id| source_pane_id != pane_id)
        {
            return false;
        }
        self.panes.extend(source.panes);
        self.session_bookkeeping.extend(source.session_bookkeeping);
        self.codex_interrupt_suppression
            .extend(source.codex_interrupt_suppression);
        true
    }

    fn remove_session(&mut self, pane_id: &str, session_id: &str) {
        if let Some(sessions) = self.panes.get_mut(pane_id) {
            sessions.remove(session_id);
            if sessions.is_empty() {
                self.panes.remove(pane_id);
            }
        }
        self.clear_session_lifecycle(pane_id, session_id);
    }

    fn clear_session_lifecycle(&mut self, pane_id: &str, session_id: &str) {
        self.session_bookkeeping
            .remove(&SessionKey::new(pane_id, session_id));
    }
}

fn apply_task_projection(
    status: &mut PaneAgentStatus,
    event: &crate::AgentEvent,
    bookkeeping: &mut SessionBookkeeping,
) -> bool {
    if event.kind() == "task.delta" {
        if bookkeeping.task_progress_authority == TaskProgressAuthority::ExplicitSnapshot {
            return false;
        }
        let Some((done_delta, total_delta)) = event.task_delta() else {
            return false;
        };
        let current = status
            .progress
            .unwrap_or(AgentProgress { done: 0, total: 0 });
        let total = current.total.saturating_add(total_delta);
        let done = current.done.saturating_add(done_delta).min(total);
        status.progress = (total > 0).then_some(AgentProgress { done, total });
        bookkeeping.tasks.clear();
        bookkeeping.task_progress_authority = TaskProgressAuthority::CounterEvents;
        return true;
    }
    if event.kind() == "task.snapshot" {
        let Some((merge, tasks)) = event.task_snapshot() else {
            return false;
        };
        if !merge {
            bookkeeping.tasks.clear();
        }
        for (task_id, completed) in tasks {
            bookkeeping.tasks.insert(task_id.to_owned(), completed);
        }
        status.progress = (!bookkeeping.tasks.is_empty()).then(|| AgentProgress {
            done: u64::try_from(bookkeeping.tasks.values().filter(|done| **done).count())
                .unwrap_or(u64::MAX),
            total: u64::try_from(bookkeeping.tasks.len()).unwrap_or(u64::MAX),
        });
        bookkeeping.task_progress_authority = TaskProgressAuthority::ExplicitSnapshot;
        return true;
    }
    if event.kind() == "task.progress" {
        status.progress = event
            .progress()
            .map(|(done, total)| AgentProgress { done, total });
        bookkeeping.tasks.clear();
        bookkeeping.task_progress_authority = TaskProgressAuthority::ExplicitSnapshot;
        return true;
    }
    let Some(task_id) = event.task_id().map(str::trim).filter(|id| !id.is_empty()) else {
        return false;
    };
    if bookkeeping.task_progress_authority != TaskProgressAuthority::IdentityEvents {
        return false;
    }
    let completed = event.kind() == "task.completed";
    bookkeeping
        .tasks
        .entry(task_id.to_owned())
        .and_modify(|was_completed| *was_completed |= completed)
        .or_insert(completed);
    status.progress = Some(AgentProgress {
        done: u64::try_from(bookkeeping.tasks.values().filter(|done| **done).count())
            .unwrap_or(u64::MAX),
        total: u64::try_from(bookkeeping.tasks.len()).unwrap_or(u64::MAX),
    });
    true
}

fn new_status(
    session_id: &str,
    event: &crate::AgentEvent,
    event_is_codex: bool,
    signal_origin: AgentSignalOrigin,
    signal_confidence: AgentSignalConfidence,
    now: u64,
) -> PaneAgentStatus {
    PaneAgentStatus {
        session_id: session_id.to_owned(),
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
        artifact_link: None,
        working_directory: None,
        agent_launch_snapshot: None,
        signal_origin,
        signal_confidence,
        updated_at: now,
    }
}

fn should_apply_signal(
    status: &PaneAgentStatus,
    event: &crate::AgentEvent,
    origin: AgentSignalOrigin,
    confidence: AgentSignalConfidence,
) -> bool {
    if origin.priority() > status.signal_origin.priority() {
        return true;
    }
    if origin.priority() < status.signal_origin.priority() {
        return event.kind() == "agent.needs-input"
            && matches!(status.phase, AgentPhase::Starting | AgentPhase::Running);
    }
    if confidence.priority() < status.signal_confidence.priority()
        && event.kind() != "agent.needs-input"
    {
        return false;
    }
    if event.kind() == "agent.needs-input" && status.phase == AgentPhase::NeedsInput {
        return interaction_priority(event.interaction())
            >= interaction_priority(status.interaction);
    }
    true
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

fn copilot_title_indicates_needs_input(title: &str) -> bool {
    let normalized = title.trim().to_lowercase();
    let first_word = normalized
        .chars()
        .take_while(|character| character.is_alphabetic())
        .collect::<String>();
    if matches!(
        first_word.as_str(),
        "asking" | "awaiting" | "waiting" | "requesting" | "prompting" | "confirming" | "needing"
    ) {
        return true;
    }
    normalized
        .split(|character: char| !character.is_alphabetic())
        .any(|word| word == "question")
}

fn is_copilot_agent_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("copilot")
        || name.eq_ignore_ascii_case("github copilot")
        || name.eq_ignore_ascii_case("github copilot cli")
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
    if let Some(artifact) = event.artifact_link() {
        status.artifact_link = Some(artifact);
    }
    if let Some(working_directory) = event.working_directory() {
        status.working_directory = Some(working_directory);
    }
    if let Some(snapshot) = event.launch_snapshot() {
        status.agent_launch_snapshot = Some(snapshot);
    }
}

fn status_priority(status: &PaneAgentStatus) -> (u16, u8, u8, u8) {
    let state = if status.requires_attention() || status.phase == AgentPhase::NeedsInput {
        500 + u16::from(interaction_priority(status.interaction))
    } else {
        match status.phase {
            AgentPhase::UnresolvedStop => 400,
            AgentPhase::Running => 300,
            AgentPhase::Starting => 250,
            AgentPhase::Idle => 200,
            AgentPhase::NeedsInput => 500,
        }
    };
    (
        state,
        status.signal_confidence.priority(),
        status.signal_origin.priority(),
        u8::from(status.parent_session_id.is_none()),
    )
}

const fn interaction_priority(interaction: AgentInteractionKind) -> u8 {
    match interaction {
        AgentInteractionKind::Approval => 5,
        AgentInteractionKind::Question => 4,
        AgentInteractionKind::Decision => 3,
        AgentInteractionKind::Auth => 2,
        AgentInteractionKind::GenericInput => 1,
        AgentInteractionKind::None => 0,
    }
}
