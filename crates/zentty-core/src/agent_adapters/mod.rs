use crate::{AgentEvent, AgentProtocolError};
use serde_json::{Value, json};
use std::fmt;

mod agy;
mod claude;
mod codex;
mod cursor;
mod droid;
mod gemini;
mod grok;
mod hermes;
mod kimi;
mod vibe;

pub use agy::adapt_agy_hook;
pub use claude::adapt_claude_hook;
pub use codex::{adapt_codex_hook, adapt_codex_notify, adapt_small_harness_hook};
pub use cursor::adapt_cursor_hook;
pub use droid::adapt_droid_hook;
pub use gemini::adapt_gemini_hook;
pub use grok::adapt_grok_hook;
pub use hermes::adapt_hermes_hook;
pub use kimi::adapt_kimi_hook;
pub use vibe::adapt_vibe_hook;

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

fn normalize_hook(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn common_question_tool(tool: &str) -> bool {
    let normalized = normalize_hook(tool);
    normalized.contains("ask")
        || normalized.contains("permission")
        || normalized.contains("approval")
        || normalized == "strreplacefile"
        || normalized == "writefile"
        || normalized == "shell"
}

fn is_question_tool_name(tool: &str) -> bool {
    let normalized = tool.to_ascii_lowercase();
    normalized.contains("askuserquestion") || normalized.contains("ask_user_question")
}

fn common_input_text(payload: &Value, agent: &str, tool: &str) -> String {
    first_message(payload)
        .or_else(|| {
            payload
                .get("tool_input")
                .or_else(|| payload.get("toolInput"))
                .and_then(|input| {
                    string_at(
                        input,
                        &[
                            "question",
                            "prompt",
                            "plan",
                            "spec",
                            "file_path",
                            "filePath",
                            "path",
                        ],
                    )
                })
        })
        .map_or_else(
            || format!("{agent} needs your input for {tool}"),
            |detail| {
                if tool == "AskUser" || is_question_tool_name(tool) || tool == "ask_question" {
                    detail
                } else {
                    format!("Allow {tool} on {detail}?")
                }
            },
        )
}

fn todo_progress(payload: &Value) -> Option<(u64, u64)> {
    let input = payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"))?;
    let todos = input.get("todos")?.as_array()?;
    let total = u64::try_from(todos.len()).ok()?;
    if total == 0 {
        return None;
    }
    let done = u64::try_from(
        todos
            .iter()
            .filter(|todo| {
                string_ref_at(todo, &["status", "state"]).is_some_and(|status| {
                    matches!(
                        status.to_ascii_lowercase().as_str(),
                        "completed" | "done" | "cancelled"
                    )
                })
            })
            .count(),
    )
    .ok()?;
    Some((done, total))
}

fn canonical_progress(
    agent_name: &str,
    session_id: Option<&str>,
    done: u64,
    total: u64,
) -> Result<AgentEvent, AgentAdapterError> {
    AgentEvent::parse(
        json!({
            "version": 1,
            "event": "task.progress",
            "agent": {"name": agent_name},
            "session": {"id": session_id},
            "progress": {"done": done, "total": total},
        })
        .to_string()
        .as_bytes(),
    )
    .map_err(AgentAdapterError::Protocol)
}

fn task_lifecycle_event(
    event: &str,
    agent_name: &str,
    pid: Option<i32>,
    session_id: Option<&str>,
    task_id: Option<&str>,
    transcript_path: Option<&str>,
) -> Result<Vec<AgentEvent>, AgentAdapterError> {
    let (Some(session_id), Some(task_id)) = (
        session_id.map(str::trim).filter(|id| !id.is_empty()),
        task_id.map(str::trim).filter(|id| !id.is_empty()),
    ) else {
        return Ok(Vec::new());
    };
    let value = json!({
        "version": 1,
        "event": event,
        "agent": {"name": agent_name, "pid": pid},
        "session": {"id": session_id},
        "task": {"id": task_id},
        "transcriptPath": transcript_path,
    });
    let lifecycle = canonical(
        "agent.running",
        agent_name,
        pid,
        Some(session_id),
        None,
        None,
    )?
    .with_transcript_path(transcript_path.map(str::to_owned));
    AgentEvent::parse(value.to_string().as_bytes())
        .map(|event| vec![lifecycle, event])
        .map_err(AgentAdapterError::Protocol)
}

fn source_task_id(payload: &Value) -> Option<String> {
    string_at(
        payload,
        &[
            "task_id",
            "taskId",
            "subagent_id",
            "subagentId",
            "agent_id",
            "agentId",
            "tool_use_id",
            "toolUseId",
        ],
    )
}

fn explicit_progress(payload: &Value) -> Option<(u64, u64)> {
    let progress = payload.get("progress")?;
    let done = progress
        .get("done")
        .or_else(|| progress.get("doneCount"))?
        .as_u64()?;
    let total = progress
        .get("total")
        .or_else(|| progress.get("totalCount"))?
        .as_u64()?;
    (total > 0).then_some((done.min(total), total))
}

fn parse_payload(bytes: &[u8]) -> Result<Value, AgentAdapterError> {
    if bytes.len() > AgentEvent::MAX_WIRE_BYTES {
        return Err(AgentAdapterError::Protocol(
            AgentProtocolError::RequestTooLarge,
        ));
    }
    serde_json::from_slice(bytes)
        .map_err(|error| AgentAdapterError::InvalidPayload(error.to_string()))
}

fn event_name(payload: &Value) -> Result<&str, AgentAdapterError> {
    string_ref_at(payload, &["hook_event_name", "hookEventName"])
        .ok_or(AgentAdapterError::MissingEventName)
}

fn source_event_name(payload: &Value) -> Result<&str, AgentAdapterError> {
    string_ref_at(
        payload,
        &["hook_event_name", "hookEventName", "event", "type"],
    )
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

fn canonical_stop_candidate(
    agent_name: &str,
    pid: Option<i32>,
    session_id: Option<&str>,
    text: Option<&str>,
) -> Result<AgentEvent, AgentAdapterError> {
    AgentEvent::parse(
        json!({
            "version": 1,
            "event": "agent.idle",
            "agent": {"name": agent_name, "pid": pid},
            "session": {"id": session_id},
            "state": {"text": text, "stopCandidate": true},
        })
        .to_string()
        .as_bytes(),
    )
    .map_err(AgentAdapterError::Protocol)
}

fn is_question_tool(payload: &Value) -> bool {
    string_ref_at(payload, &["tool_name", "toolName", "tool"]).is_some_and(|tool| {
        let normalized = tool
            .chars()
            .filter(|character| character.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>();
        normalized.contains("askuserquestion")
            || normalized.contains("askuser")
            || normalized.contains("requestuserinput")
    })
}

fn question_text(payload: &Value) -> Option<String> {
    let question = payload
        .get("tool_input")
        .and_then(|input| input.get("questions"))
        .and_then(Value::as_array)
        .and_then(|questions| questions.first())
        .and_then(Value::as_object);
    if let Some(question) = question {
        let mut lines = Vec::new();
        if let Some(prompt) = ["question", "header"].into_iter().find_map(|key| {
            question
                .get(key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
        }) {
            lines.push(prompt.to_owned());
        }
        let labels = question
            .get("options")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|option| option.get("label").and_then(Value::as_str))
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .map(|label| format!("[{label}]"))
            .collect::<Vec<_>>();
        if !labels.is_empty() {
            lines.push(labels.join(" "));
        }
        if !lines.is_empty() {
            return Some(lines.join("\n"));
        }
    }
    first_message(payload)
}

fn requires_human_input(message: &str) -> bool {
    let normalized = message.trim().to_ascii_lowercase();
    [
        "waiting for your input",
        "waiting for input",
        "needs your input",
        "needs input",
        "needs your attention",
        "action required",
        "input-requested",
        "input requested",
        "approval-requested",
        "approval requested",
        "question requested",
        "questions requested",
        "plan-mode-prompt",
        "plan mode prompt",
        "permission",
        "approve",
        "approval",
        "allow ",
        "wants to edit",
        "confirm",
        "select ",
        "choose ",
        "grant access",
        "press enter",
        "log in",
        "login",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
        || normalized.contains('?')
}

fn first_message(payload: &Value) -> Option<String> {
    string_at(
        payload,
        &["message", "body", "text", "prompt", "error", "description"],
    )
    .or_else(|| question_text_without_fallback(payload))
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

fn bool_at(payload: &Value, keys: &[&str]) -> Option<bool> {
    keys.iter().find_map(|key| payload.get(*key)?.as_bool())
}

fn string_ref_at<'a>(payload: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| payload.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}
