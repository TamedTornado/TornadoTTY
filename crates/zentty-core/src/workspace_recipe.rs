use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::agent_launch::sanitize_amp_resume_arguments;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRecipe {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<i64>,
    pub windows: Vec<WindowRecipe>,
    #[serde(
        default,
        rename = "activeWindowID",
        skip_serializing_if = "Option::is_none"
    )]
    pub active_window_id: Option<String>,
}

impl WorkspaceRecipe {
    pub const CURRENT_SCHEMA_VERSION: i64 = 3;

    /// Decodes the recipe using the same permissive unknown-field behavior as
    /// Swift's synthesized `Decodable` implementation.
    ///
    /// # Errors
    ///
    /// Returns the JSON decoding error for malformed or type-incompatible
    /// input.
    pub fn from_json(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    /// Encodes the recipe with Swift-compatible camel-case field names.
    ///
    /// # Errors
    ///
    /// Returns a JSON encoding error if serialization fails.
    pub fn to_json(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Applies `WorkspaceRecipeMigration`: only an unversioned recipe has
    /// legacy generated titles sanitized, after which every recipe is marked
    /// as the current version.
    #[must_use]
    pub fn migrated(mut self) -> Self {
        if self.schema_version.is_none() {
            for window in &mut self.windows {
                for worklane in &mut window.worklanes {
                    worklane.title = meaningful_legacy_title(worklane.title.take());
                }
            }
        }
        self.schema_version = Some(Self::CURRENT_SCHEMA_VERSION);
        self
    }

    /// Returns whether the recipe contains user-meaningful state worth
    /// retaining as a session snapshot.
    #[must_use]
    pub fn is_meaningful(&self, default_working_directory: &str) -> bool {
        let recipe = self.clone().migrated();
        let [window] = recipe.windows.as_slice() else {
            return !recipe.windows.is_empty();
        };
        let [worklane] = window.worklanes.as_slice() else {
            return true;
        };
        let [column] = worklane.columns.as_slice() else {
            return true;
        };
        let [pane] = column.panes.as_slice() else {
            return true;
        };

        if trimmed(worklane.title.as_deref()).is_some() || worklane.next_pane_number > 2 {
            return true;
        }
        if window.active_worklane_id.as_deref() != Some(worklane.id.as_str())
            || worklane.focused_column_id.as_deref() != Some(column.id.as_str())
            || column.focused_pane_id.as_deref() != Some(pane.id.as_str())
            || column.last_focused_pane_id.as_deref() != Some(pane.id.as_str())
        {
            return true;
        }
        if normalized_path(pane.working_directory.as_deref())
            != normalized_path(Some(default_working_directory))
        {
            return true;
        }
        if trimmed(pane.custom_title.as_deref()).is_some() {
            return true;
        }
        if pane
            .title_seed
            .as_deref()
            .is_some_and(|value| value != "shell")
            || pane
                .last_activity_title
                .as_deref()
                .is_some_and(|value| value != "shell")
            || trimmed(pane.last_run_command.as_deref()).is_some_and(|value| value != "shell")
        {
            return true;
        }
        false
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowRecipe {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<WindowFrame>,
    pub worklanes: Vec<WorklaneRecipe>,
    #[serde(
        default,
        rename = "activeWorklaneID",
        skip_serializing_if = "Option::is_none"
    )]
    pub active_worklane_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen_x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen_y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen_width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen_height: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorklaneRecipe {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub next_pane_number: i64,
    #[serde(
        default,
        rename = "focusedColumnID",
        skip_serializing_if = "Option::is_none"
    )]
    pub focused_column_id: Option<String>,
    pub columns: Vec<ColumnRecipe>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(
        default,
        rename = "bookmarkOriginID",
        skip_serializing_if = "Option::is_none"
    )]
    pub bookmark_origin_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnRecipe {
    pub id: String,
    pub width: f64,
    #[serde(
        default,
        rename = "focusedPaneID",
        skip_serializing_if = "Option::is_none"
    )]
    pub focused_pane_id: Option<String>,
    #[serde(
        default,
        rename = "lastFocusedPaneID",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_focused_pane_id: Option<String>,
    pub pane_heights: Vec<f64>,
    pub panes: Vec<PaneRecipe>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaneRecipe {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_seed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_activity_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run_command: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRestoreEnvelope {
    pub schema_version: i64,
    pub saved_at: f64,
    pub reason: SaveReason,
    pub workspace: WorkspaceRecipe,
    pub restore_draft_windows: Vec<SessionRestoreDraftWindow>,
}

impl SessionRestoreEnvelope {
    /// Decodes a compact or pretty-printed session envelope.
    ///
    /// # Errors
    ///
    /// Returns the JSON decoding error for malformed or type-incompatible
    /// input.
    pub fn from_json(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    /// Encodes the session envelope using Swift-compatible field names and
    /// enum values.
    ///
    /// # Errors
    ///
    /// Returns a JSON encoding error if serialization fails.
    pub fn to_json(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SaveReason {
    LiveSnapshot,
    CleanExit,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRestoreDraftWindow {
    #[serde(rename = "windowID")]
    pub window_id: String,
    pub pane_drafts: Vec<PaneRestoreDraft>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaneRestoreDraft {
    #[serde(rename = "paneID")]
    pub pane_id: String,
    pub kind: RestoreDraftKind,
    pub tool_name: String,
    #[serde(rename = "sessionID")]
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
    #[serde(rename = "trackedPID")]
    pub tracked_pid: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_launch_snapshot: Option<AgentLaunchSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_progress: Option<crate::AgentProgress>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tasks: BTreeMap<String, bool>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub task_progress_authoritative: bool,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(value: &bool) -> bool {
    !*value
}

impl PaneRestoreDraft {
    /// Builds the source-compatible resume command for an implemented agent.
    ///
    /// Session identifiers are validated before they enter a command string;
    /// unsupported tools and malformed identifiers are intentionally ignored.
    #[must_use]
    pub fn resume_command(&self) -> Option<String> {
        if self.kind != RestoreDraftKind::AgentResume {
            return None;
        }
        if self.tool_name.eq_ignore_ascii_case("codex") {
            let session_id = validated_codex_session_id(&self.session_id)?;
            return Some(format!("codex resume {session_id}"));
        }
        if self.tool_name.eq_ignore_ascii_case("claude")
            || self.tool_name.eq_ignore_ascii_case("claude code")
        {
            let session_id = validated_uuid(&self.session_id)?;
            return Some(format!("claude --resume {session_id}"));
        }
        if self.tool_name.eq_ignore_ascii_case("gemini")
            || self.tool_name.eq_ignore_ascii_case("gemini cli")
        {
            self.require_working_directory()?;
            return Some("gemini --resume".to_owned());
        }
        if self.tool_name.eq_ignore_ascii_case("copilot")
            || self.tool_name.eq_ignore_ascii_case("github copilot")
            || self.tool_name.eq_ignore_ascii_case("github copilot cli")
        {
            let session_id = validated_uuid(&self.session_id)?;
            return Some(format!("copilot --resume={session_id}"));
        }
        if self.tool_name.eq_ignore_ascii_case("cursor") {
            let session_id = validated_uuid(&self.session_id)?;
            return Some(format!("cursor-agent --resume={session_id}"));
        }
        if self.tool_name.eq_ignore_ascii_case("droid") {
            let session_id = validated_droid_session_id(&self.session_id)?;
            return Some(format!("droid exec -s {session_id}"));
        }
        if self.tool_name.eq_ignore_ascii_case("kimi") {
            if let Some(uuid) = self.session_id.strip_prefix("session_") {
                let session_id = validated_uuid(uuid)?;
                return Some(format!("kimi -S session_{session_id}"));
            }
            let session_id = validated_uuid(&self.session_id)?;
            return Some(format!("kimi -r {session_id}"));
        }
        if self.tool_name.eq_ignore_ascii_case("opencode") {
            let session_id = validated_opencode_session_id(&self.session_id)?;
            return Some(format!("opencode --session {session_id}"));
        }
        if self.tool_name.eq_ignore_ascii_case("amp") {
            let session_id = validated_amp_thread_id(&self.session_id)?;
            let resume_arguments = sanitize_amp_resume_arguments(
                self.agent_launch_snapshot
                    .as_ref()
                    .map_or(&[], |snapshot| snapshot.arguments.as_slice()),
            )?;
            return Some(
                ["amp", "threads", "continue"]
                    .into_iter()
                    .map(str::to_owned)
                    .chain(resume_arguments)
                    .chain([session_id])
                    .map(|argument| shell_quoted_argument(&argument))
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        }
        if self.tool_name.eq_ignore_ascii_case("pi") {
            self.require_working_directory()?;
            return Some("pi -c".to_owned());
        }
        if self.tool_name.eq_ignore_ascii_case("omp")
            || self.tool_name.eq_ignore_ascii_case("oh my pi")
        {
            self.require_working_directory()?;
            return Some("omp -c".to_owned());
        }
        if self.tool_name.eq_ignore_ascii_case("small harness")
            || self.tool_name.eq_ignore_ascii_case("small-harness")
        {
            self.require_working_directory()?;
            return Some("small-harness --continue".to_owned());
        }
        None
    }

    fn require_working_directory(&self) -> Option<()> {
        self.working_directory
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(|_| ())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RestoreDraftKind {
    AgentResume,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentLaunchSnapshot {
    pub arguments: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<BTreeMap<String, String>>,
}

fn validated_codex_session_id(value: &str) -> Option<String> {
    let mut characters = value.chars();
    if !characters.next()?.is_ascii_alphanumeric()
        || !characters.all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
    {
        return None;
    }
    Some(value.to_owned())
}

fn validated_opencode_session_id(value: &str) -> Option<String> {
    let suffix = value.strip_prefix("ses_")?;
    (!suffix.is_empty()
        && suffix
            .chars()
            .all(|character| character.is_ascii_alphanumeric()))
    .then(|| value.to_owned())
}

fn validated_amp_thread_id(value: &str) -> Option<String> {
    let suffix = value.strip_prefix("T-")?;
    (!suffix.is_empty()
        && suffix.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        }))
    .then(|| value.to_owned())
}

fn validated_droid_session_id(value: &str) -> Option<String> {
    let mut characters = value.chars();
    if value.starts_with("zentty-placeholder-")
        || !characters.next()?.is_ascii_alphanumeric()
        || !characters.all(|character| {
            character.is_ascii_alphanumeric()
                || character == '_'
                || character == '.'
                || character == ':'
                || character == '-'
        })
    {
        return None;
    }
    Some(value.to_owned())
}

fn shell_quoted_argument(argument: &str) -> String {
    if !argument.is_empty()
        && argument
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_./:=+-".contains(character))
    {
        return argument.to_owned();
    }
    format!("'{}'", argument.replace('\'', "'\\''"))
}

fn validated_uuid(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    if bytes.len() != 36
        || [8, 13, 18, 23]
            .into_iter()
            .any(|index| bytes[index] != b'-')
        || bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| ![8, 13, 18, 23].contains(&index) && !byte.is_ascii_hexdigit())
    {
        return None;
    }
    Some(value.to_ascii_lowercase())
}

fn meaningful_legacy_title(raw_title: Option<String>) -> Option<String> {
    let title = raw_title?.trim().to_owned();
    if title.is_empty() || title.eq_ignore_ascii_case("MAIN") {
        return None;
    }
    if let Some(number) = title.strip_prefix("WS ")
        && number.parse::<i64>().is_ok_and(|value| value >= 1)
    {
        return None;
    }
    Some(title)
}

fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn normalized_path(value: Option<&str>) -> Option<PathBuf> {
    let value = trimmed(value)?;
    let mut normalized = PathBuf::new();
    for component in Path::new(value).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    Some(normalized)
}
