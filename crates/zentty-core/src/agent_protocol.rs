use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

use crate::AgentLaunchSnapshot;

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
    #[serde(rename = "agent.compacting")]
    Compacting,
    #[serde(rename = "agent.compacted")]
    Compacted,
    #[serde(rename = "agent.idle")]
    Idle,
    #[serde(rename = "agent.needs-input")]
    NeedsInput,
    #[serde(rename = "agent.input-resolved")]
    InputResolved,
    #[serde(rename = "agent.failed")]
    Failed,
    #[serde(rename = "task.progress")]
    TaskProgress,
    #[serde(rename = "task.snapshot")]
    TaskSnapshot,
    #[serde(rename = "task.delta")]
    TaskDelta,
    #[serde(rename = "task.started")]
    TaskStarted,
    #[serde(rename = "task.completed")]
    TaskCompleted,
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
struct TaskDescriptor {
    id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
struct TaskSnapshotDescriptor {
    id: Option<String>,
    #[serde(default)]
    completed: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
struct TaskDeltaDescriptor {
    done: u64,
    total: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub enum AgentArtifactKind {
    #[serde(rename = "pull-request")]
    PullRequest,
    #[serde(rename = "session")]
    Session,
    #[serde(rename = "share")]
    Share,
    #[serde(rename = "compare")]
    Compare,
    #[serde(rename = "generic")]
    Generic,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentArtifactLink {
    pub kind: AgentArtifactKind,
    pub label: String,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
struct ArtifactDescriptor {
    kind: Option<String>,
    label: Option<String>,
    url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
struct LaunchDescriptor {
    arguments: Option<Vec<String>>,
    environment: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ContextDescriptor {
    working_directory: Option<String>,
    launch: Option<LaunchDescriptor>,
}

pub(crate) fn canonical_working_directory(path: &str) -> Option<String> {
    let path = path.trim();
    if path.is_empty() || path.contains('\0') || !std::path::Path::new(path).is_absolute() {
        return None;
    }
    std::fs::canonicalize(path)
        .ok()
        .filter(|path| path.is_dir())
        .map(|path| path.to_string_lossy().into_owned())
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct AgentEvent {
    version: u32,
    event: AgentEventKind,
    agent: Option<AgentDescriptor>,
    session: Option<SessionDescriptor>,
    state: Option<StateDescriptor>,
    progress: Option<ProgressDescriptor>,
    task: Option<TaskDescriptor>,
    tasks: Option<Vec<TaskSnapshotDescriptor>>,
    delta: Option<TaskDeltaDescriptor>,
    #[serde(default)]
    merge: bool,
    artifact: Option<ArtifactDescriptor>,
    context: Option<ContextDescriptor>,
    #[serde(rename = "transcriptPath")]
    transcript_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentProtocolError {
    RequestTooLarge,
    InvalidJson(String),
    UnsupportedVersion(u32),
    InvalidProgress,
    InvalidTaskDelta,
    MissingTaskIdentity,
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
            Self::InvalidTaskDelta => formatter
                .write_str("task delta requires a non-empty session.id and a non-zero delta"),
            Self::MissingTaskIdentity => formatter
                .write_str("task lifecycle events require non-empty session.id and task.id"),
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
        if matches!(
            event.event,
            AgentEventKind::TaskStarted | AgentEventKind::TaskCompleted
        ) && (event.session_id().is_none_or(|id| id.trim().is_empty())
            || event.task_id().is_none_or(|id| id.trim().is_empty()))
        {
            return Err(AgentProtocolError::MissingTaskIdentity);
        }
        if event.event == AgentEventKind::TaskSnapshot
            && (event.session_id().is_none_or(|id| id.trim().is_empty())
                || event.tasks.as_ref().is_none_or(|tasks| {
                    tasks
                        .iter()
                        .any(|task| task.id.as_deref().is_none_or(|id| id.trim().is_empty()))
                }))
        {
            return Err(AgentProtocolError::MissingTaskIdentity);
        }
        if event.event == AgentEventKind::TaskDelta
            && (event.session_id().is_none_or(|id| id.trim().is_empty())
                || event
                    .delta
                    .is_none_or(|delta| delta.done == 0 && delta.total == 0))
        {
            return Err(AgentProtocolError::InvalidTaskDelta);
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
            task: None,
            tasks: None,
            delta: None,
            merge: false,
            artifact: None,
            context: None,
            transcript_path: None,
        }
    }

    pub(crate) fn kind(&self) -> &'static str {
        match self.event {
            AgentEventKind::SessionStart => "session.start",
            AgentEventKind::SessionEnd => "session.end",
            AgentEventKind::Running => "agent.running",
            AgentEventKind::Compacting => "agent.compacting",
            AgentEventKind::Compacted => "agent.compacted",
            AgentEventKind::Idle => "agent.idle",
            AgentEventKind::NeedsInput => "agent.needs-input",
            AgentEventKind::InputResolved => "agent.input-resolved",
            AgentEventKind::Failed => "agent.failed",
            AgentEventKind::TaskProgress => "task.progress",
            AgentEventKind::TaskSnapshot => "task.snapshot",
            AgentEventKind::TaskDelta => "task.delta",
            AgentEventKind::TaskStarted => "task.started",
            AgentEventKind::TaskCompleted => "task.completed",
        }
    }

    pub(crate) fn session_id(&self) -> Option<&str> {
        self.session.as_ref()?.id.as_deref()
    }

    pub(crate) fn parent_session_id(&self) -> Option<&str> {
        self.session.as_ref()?.parent_id.as_deref()
    }

    pub(crate) fn task_id(&self) -> Option<&str> {
        self.task.as_ref()?.id.as_deref()
    }

    pub(crate) fn task_snapshot(&self) -> Option<(bool, Vec<(&str, bool)>)> {
        let tasks = self.tasks.as_ref()?;
        Some((
            self.merge,
            tasks
                .iter()
                .filter_map(|task| Some((task.id.as_deref()?, task.completed)))
                .collect(),
        ))
    }

    pub(crate) fn task_delta(&self) -> Option<(u64, u64)> {
        self.delta.map(|delta| (delta.done, delta.total))
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

    pub(crate) fn stop_candidate(&self) -> bool {
        self.state
            .as_ref()
            .is_some_and(|state| state.stop_candidate)
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

    pub(crate) fn artifact_link(&self) -> Option<AgentArtifactLink> {
        let artifact = self.artifact.as_ref()?;
        let label = artifact.label.as_deref()?.trim();
        let url = artifact.url.as_deref()?.trim();
        if label.is_empty() || url.is_empty() {
            return None;
        }
        Some(AgentArtifactLink {
            kind: match artifact.kind.as_deref().unwrap_or("generic") {
                "pull-request" => AgentArtifactKind::PullRequest,
                "session" => AgentArtifactKind::Session,
                "share" => AgentArtifactKind::Share,
                "compare" => AgentArtifactKind::Compare,
                "generic" => AgentArtifactKind::Generic,
                _ => return None,
            },
            label: label.to_owned(),
            url: url.to_owned(),
        })
    }

    #[must_use]
    pub fn working_directory(&self) -> Option<String> {
        let path = self.context.as_ref()?.working_directory.as_deref()?;
        canonical_working_directory(path)
    }

    pub(crate) fn launch_snapshot(&self) -> Option<AgentLaunchSnapshot> {
        let launch = self.context.as_ref()?.launch.as_ref()?;
        let arguments = launch.arguments.clone()?;
        if arguments.is_empty() || arguments.iter().any(|argument| argument.contains('\0')) {
            return None;
        }
        Some(AgentLaunchSnapshot {
            arguments,
            environment: launch
                .environment
                .clone()
                .filter(|values| !values.is_empty()),
        })
    }

    pub(crate) fn with_transcript_path(mut self, path: Option<String>) -> Self {
        self.transcript_path = path;
        self
    }

    pub(crate) fn with_working_directory(mut self, path: Option<String>) -> Self {
        let Some(path) = path else {
            return self;
        };
        self.context
            .get_or_insert(ContextDescriptor {
                working_directory: None,
                launch: None,
            })
            .working_directory = Some(path);
        self
    }
}
