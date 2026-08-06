use crate::{AgentEvent, AgentProtocolError};
use serde_json::{Value, json};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentAdapterError {
    InvalidPayload(String),
    MissingEventName,
    UnsupportedEvent(String),
    Protocol(AgentProtocolError),
}

impl fmt::Display for AgentAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPayload(error) => write!(formatter, "invalid hook payload: {error}"),
            Self::MissingEventName => formatter.write_str("hook payload has no event name"),
            Self::UnsupportedEvent(event) => write!(formatter, "unsupported hook event {event}"),
            Self::Protocol(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for AgentAdapterError {}

/// Converts a Codex hook payload into canonical version-1 status events.
///
/// # Errors
///
/// Returns an error for malformed payloads, missing/unsupported hook names, or
/// a canonical protocol construction failure.
pub fn adapt_codex_hook(
    bytes: &[u8],
    pid: Option<i32>,
) -> Result<Vec<AgentEvent>, AgentAdapterError> {
    let payload = parse_payload(bytes)?;
    let hook = event_name(&payload)?;
    let session = string_at(&payload, &["session_id", "sessionId"]);
    let event = match hook {
        "SessionStart" => canonical(
            "session.start",
            "Codex",
            pid,
            session.as_deref(),
            None,
            None,
        )?,
        "UserPromptSubmit" | "PreToolUse" | "PostToolUse" => canonical(
            "agent.running",
            "Codex",
            pid,
            session.as_deref(),
            None,
            None,
        )?,
        "PermissionRequest" => canonical(
            "agent.needs-input",
            "Codex",
            pid,
            session.as_deref(),
            first_message(&payload).as_deref(),
            Some(if is_question_tool(&payload) {
                "decision"
            } else {
                "approval"
            }),
        )?,
        "Stop" => canonical("agent.idle", "Codex", pid, session.as_deref(), None, None)?,
        other => return Err(AgentAdapterError::UnsupportedEvent(other.to_owned())),
    };
    Ok(vec![event])
}

/// Converts a Claude Code hook payload into canonical version-1 status events.
///
/// # Errors
///
/// Returns an error for malformed payloads, missing/unsupported hook names, or
/// a canonical protocol construction failure.
pub fn adapt_claude_hook(
    bytes: &[u8],
    pid: Option<i32>,
) -> Result<Vec<AgentEvent>, AgentAdapterError> {
    let payload = parse_payload(bytes)?;
    let hook = event_name(&payload)?;
    let session = string_at(&payload, &["session_id", "sessionId"]);
    let event = match hook {
        "SessionStart" => canonical(
            "session.start",
            "Claude Code",
            pid,
            session.as_deref(),
            None,
            None,
        )?,
        "PreToolUse" if is_question_tool(&payload) => canonical(
            "agent.needs-input",
            "Claude Code",
            pid,
            session.as_deref(),
            question_text(&payload).as_deref(),
            Some("decision"),
        )?,
        "UserPromptSubmit" | "PreToolUse" => canonical(
            "agent.running",
            "Claude Code",
            pid,
            session.as_deref(),
            None,
            None,
        )?,
        "PermissionRequest" => canonical(
            "agent.needs-input",
            "Claude Code",
            pid,
            session.as_deref(),
            first_message(&payload).as_deref(),
            Some(if is_question_tool(&payload) {
                "decision"
            } else {
                "approval"
            }),
        )?,
        "Notification"
            if string_at(&payload, &["notification_type"]).as_deref() == Some("idle_prompt") =>
        {
            canonical(
                "agent.idle",
                "Claude Code",
                pid,
                session.as_deref(),
                None,
                None,
            )?
        }
        "Notification" => canonical(
            "agent.needs-input",
            "Claude Code",
            pid,
            session.as_deref(),
            first_message(&payload).as_deref(),
            Some("generic-input"),
        )?,
        "Stop" => canonical(
            "agent.idle",
            "Claude Code",
            pid,
            session.as_deref(),
            None,
            None,
        )?,
        "SessionEnd" => canonical(
            "session.end",
            "Claude Code",
            pid,
            session.as_deref(),
            None,
            None,
        )?,
        other => return Err(AgentAdapterError::UnsupportedEvent(other.to_owned())),
    };
    Ok(vec![event])
}

/// Converts a Gemini CLI hook payload into canonical version-1 status events.
///
/// Unknown hooks and non-permission notifications are intentional no-ops,
/// matching the source adapter.
///
/// # Errors
///
/// Returns an error for malformed payloads, a missing hook name, or a
/// canonical protocol construction failure.
pub fn adapt_gemini_hook(
    bytes: &[u8],
    pid: Option<i32>,
) -> Result<Vec<AgentEvent>, AgentAdapterError> {
    let payload = parse_payload(bytes)?;
    let hook = event_name(&payload)?;
    let session = string_at(&payload, &["session_id", "sessionId"]);
    let event = match hook {
        "SessionStart" => canonical(
            "session.start",
            "Gemini",
            pid,
            session.as_deref(),
            None,
            None,
        )?,
        "BeforeAgent" | "BeforeTool" => canonical(
            "agent.running",
            "Gemini",
            pid,
            session.as_deref(),
            None,
            None,
        )?,
        "AfterAgent" => canonical("agent.idle", "Gemini", pid, session.as_deref(), None, None)?,
        "SessionEnd" => canonical("session.end", "Gemini", pid, session.as_deref(), None, None)?,
        "Notification"
            if string_at(&payload, &["notification_type", "notificationType"])
                .is_some_and(|kind| kind.eq_ignore_ascii_case("ToolPermission")) =>
        {
            let text = gemini_permission_text(&payload)
                .unwrap_or_else(|| "Gemini needs your approval".to_owned());
            canonical(
                "agent.needs-input",
                "Gemini",
                pid,
                session.as_deref(),
                Some(&text),
                Some("approval"),
            )?
        }
        _ => return Ok(Vec::new()),
    };
    Ok(vec![event])
}

fn parse_payload(bytes: &[u8]) -> Result<Value, AgentAdapterError> {
    serde_json::from_slice(bytes)
        .map_err(|error| AgentAdapterError::InvalidPayload(error.to_string()))
}

fn event_name(payload: &Value) -> Result<&str, AgentAdapterError> {
    string_ref_at(payload, &["hook_event_name", "hookEventName"])
        .ok_or(AgentAdapterError::MissingEventName)
}

fn canonical(
    event: &str,
    agent_name: &str,
    pid: Option<i32>,
    session_id: Option<&str>,
    text: Option<&str>,
    interaction: Option<&str>,
) -> Result<AgentEvent, AgentAdapterError> {
    let state = interaction.map_or_else(
        || text.map(|text| json!({"text": text})),
        |kind| Some(json!({"text": text, "interaction": {"kind": kind, "text": text}})),
    );
    let value = json!({
        "version": 1,
        "event": event,
        "agent": {"name": agent_name, "pid": pid},
        "session": {"id": session_id},
        "state": state,
    });
    AgentEvent::parse(value.to_string().as_bytes()).map_err(AgentAdapterError::Protocol)
}

fn is_question_tool(payload: &Value) -> bool {
    string_ref_at(payload, &["tool_name", "toolName", "tool"])
        .is_some_and(|tool| tool.eq_ignore_ascii_case("AskUserQuestion"))
}

fn question_text(payload: &Value) -> Option<String> {
    payload
        .get("tool_input")
        .and_then(|input| input.get("questions"))
        .and_then(Value::as_array)
        .and_then(|questions| questions.first())
        .and_then(|question| question.get("question"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| first_message(payload))
}

fn first_message(payload: &Value) -> Option<String> {
    string_at(
        payload,
        &["message", "body", "text", "prompt", "description"],
    )
    .or_else(|| question_text_without_fallback(payload))
}

fn gemini_permission_text(payload: &Value) -> Option<String> {
    let summary = first_message(payload);
    if summary
        .as_deref()
        .is_some_and(|value| !is_generic_approval(value))
    {
        return summary;
    }
    let details = payload.get("details")?;
    if let Some(value) = details
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(value.to_owned());
    }
    let tool = string_at(details, &["tool_name", "toolName", "tool", "name"]);
    let path = string_at(details, &["file_path", "filePath", "path"]);
    match (tool, path) {
        (Some(tool), Some(path)) => Some(format!("Allow {tool} on {path}?")),
        (Some(tool), None) => Some(format!("Allow {tool}?")),
        (None, Some(path)) => Some(format!("Allow access to {path}?")),
        (None, None) => summary,
    }
}

fn is_generic_approval(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    normalized == "action required"
        || [
            "claude needs your approval",
            "claude needs your permission",
            "gemini needs your approval",
            "gemini needs your permission",
            "approval needed",
            "permission required",
        ]
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
}

fn question_text_without_fallback(payload: &Value) -> Option<String> {
    payload
        .get("tool_input")
        .and_then(|input| input.get("question"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn string_at(payload: &Value, keys: &[&str]) -> Option<String> {
    string_ref_at(payload, keys).map(str::to_owned)
}

fn string_ref_at<'a>(payload: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| payload.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}
