use crate::{AgentInteractionKind, AgentTarget, AuthenticatedAgentEvent};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentPhase {
    Starting,
    Running,
    NeedsInput,
    Idle,
    UnresolvedStop,
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
}

impl AgentStatusStore {
    pub fn apply(&mut self, authenticated: AuthenticatedAgentEvent, now: u64) {
        let target = authenticated.target;
        let event = authenticated.event;
        let session_id = event.session_id().unwrap_or("pane-default").to_owned();
        let pane_id = target.pane_id;
        let sessions = self.panes.entry(pane_id.clone()).or_default();
        if event.kind() == "session.end" {
            sessions.remove(&session_id);
            if sessions.is_empty() {
                self.panes.remove(&pane_id);
            }
            return;
        }

        let status = sessions
            .entry(session_id.clone())
            .or_insert_with(|| PaneAgentStatus {
                session_id,
                parent_session_id: event.parent_session_id().map(str::to_owned),
                agent_name: event.agent_name().unwrap_or("Agent").to_owned(),
                phase: AgentPhase::Starting,
                text: None,
                interaction: AgentInteractionKind::None,
                progress: None,
                tracked_pid: None,
                updated_at: now,
            });
        if let Some(name) = event.agent_name() {
            name.clone_into(&mut status.agent_name);
        }
        if let Some(pid) = event.agent_pid() {
            status.tracked_pid = Some(pid);
        }
        match event.kind() {
            "session.start" => status.phase = AgentPhase::Starting,
            "agent.running" | "agent.input-resolved" => {
                status.phase = AgentPhase::Running;
                status.interaction = AgentInteractionKind::None;
                status.text = event.state_text().map(str::to_owned);
            }
            "agent.idle" => {
                status.phase = AgentPhase::Idle;
                status.interaction = AgentInteractionKind::None;
                status.text = None;
            }
            "agent.needs-input" => {
                status.phase = AgentPhase::NeedsInput;
                status.interaction = event.interaction();
                status.text = event
                    .interaction_text()
                    .or_else(|| event.state_text())
                    .map(str::to_owned);
            }
            "task.progress" => {
                status.progress = event
                    .progress()
                    .map(|(done, total)| AgentProgress { done, total });
            }
            "session.end" => unreachable!("handled before session creation"),
            _ => unreachable!("AgentEvent exposes only known protocol events"),
        }
        status.updated_at = now;
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
