use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub enum AgentInteractionKind {
    #[serde(rename = "approval")]
    Approval,
    #[serde(rename = "decision")]
    Decision,
    #[serde(rename = "question")]
    Question,
    #[serde(rename = "generic-input")]
    GenericInput,
    #[serde(rename = "auth")]
    Auth,
    #[serde(skip)]
    None,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
enum AgentEventKind {
    #[serde(rename = "session.start")]
    SessionStart,
    #[serde(rename = "session.end")]
    SessionEnd,
    #[serde(rename = "agent.running")]
    Running,
    #[serde(rename = "agent.idle")]
    Idle,
    #[serde(rename = "agent.needs-input")]
    NeedsInput,
    #[serde(rename = "agent.input-resolved")]
    InputResolved,
    #[serde(rename = "task.progress")]
    TaskProgress,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
struct AgentDescriptor {
    name: Option<String>,
    pid: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
struct SessionDescriptor {
    id: Option<String>,
    #[serde(rename = "parentId")]
    parent_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
struct InteractionDescriptor {
    kind: Option<AgentInteractionKind>,
    text: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
struct StateDescriptor {
    text: Option<String>,
    #[serde(rename = "stopCandidate", default)]
    stop_candidate: bool,
    interaction: Option<InteractionDescriptor>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
struct ProgressDescriptor {
    done: u64,
    total: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct AgentEvent {
    version: u32,
    event: AgentEventKind,
    agent: Option<AgentDescriptor>,
    session: Option<SessionDescriptor>,
    state: Option<StateDescriptor>,
    progress: Option<ProgressDescriptor>,
    #[serde(rename = "transcriptPath")]
    transcript_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentProtocolError {
    RequestTooLarge,
    InvalidJson(String),
    UnsupportedVersion(u32),
    InvalidProgress,
}

impl fmt::Display for AgentProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestTooLarge => formatter.write_str("agent event exceeds 64 KiB"),
            Self::InvalidJson(error) => write!(formatter, "invalid agent event: {error}"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported agent protocol version {version}")
            }
            Self::InvalidProgress => {
                formatter.write_str("task progress total must be greater than zero")
            }
        }
    }
}

impl std::error::Error for AgentProtocolError {}

impl AgentEvent {
    pub const MAX_WIRE_BYTES: usize = 64 * 1024;

    /// Parses one versioned canonical agent event.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized input, malformed JSON, unsupported
    /// protocol versions, unknown events, or invalid progress.
    pub fn parse(bytes: &[u8]) -> Result<Self, AgentProtocolError> {
        if bytes.len() > Self::MAX_WIRE_BYTES {
            return Err(AgentProtocolError::RequestTooLarge);
        }
        let event: Self = serde_json::from_slice(bytes)
            .map_err(|error| AgentProtocolError::InvalidJson(error.to_string()))?;
        if event.version != 1 {
            return Err(AgentProtocolError::UnsupportedVersion(event.version));
        }
        if event.event == AgentEventKind::TaskProgress
            && event.progress.is_none_or(|progress| progress.total == 0)
        {
            return Err(AgentProtocolError::InvalidProgress);
        }
        Ok(event)
    }

    #[must_use]
    pub fn idle(session_id: impl Into<String>) -> Self {
        Self {
            version: 1,
            event: AgentEventKind::Idle,
            agent: None,
            session: Some(SessionDescriptor {
                id: Some(session_id.into()),
                parent_id: None,
            }),
            state: None,
            progress: None,
            transcript_path: None,
        }
    }

    pub(crate) fn kind(&self) -> &'static str {
        match self.event {
            AgentEventKind::SessionStart => "session.start",
            AgentEventKind::SessionEnd => "session.end",
            AgentEventKind::Running => "agent.running",
            AgentEventKind::Idle => "agent.idle",
            AgentEventKind::NeedsInput => "agent.needs-input",
            AgentEventKind::InputResolved => "agent.input-resolved",
            AgentEventKind::TaskProgress => "task.progress",
        }
    }

    pub(crate) fn session_id(&self) -> Option<&str> {
        self.session.as_ref()?.id.as_deref()
    }

    pub(crate) fn parent_session_id(&self) -> Option<&str> {
        self.session.as_ref()?.parent_id.as_deref()
    }

    pub(crate) fn agent_name(&self) -> Option<&str> {
        self.agent.as_ref()?.name.as_deref()
    }

    pub(crate) fn agent_pid(&self) -> Option<i32> {
        self.agent.as_ref()?.pid
    }

    pub(crate) fn state_text(&self) -> Option<&str> {
        self.state.as_ref()?.text.as_deref()
    }

    pub(crate) fn interaction(&self) -> AgentInteractionKind {
        self.state
            .as_ref()
            .and_then(|state| state.interaction.as_ref())
            .and_then(|interaction| interaction.kind)
            .unwrap_or(AgentInteractionKind::GenericInput)
    }

    pub(crate) fn interaction_text(&self) -> Option<&str> {
        self.state.as_ref()?.interaction.as_ref()?.text.as_deref()
    }

    pub(crate) fn progress(&self) -> Option<(u64, u64)> {
        self.progress
            .map(|progress| (progress.done.min(progress.total), progress.total))
    }

    pub(crate) fn transcript_path(&self) -> Option<&str> {
        self.transcript_path.as_deref()
    }

    pub(crate) fn with_transcript_path(mut self, path: Option<String>) -> Self {
        self.transcript_path = path;
        self
    }
}
