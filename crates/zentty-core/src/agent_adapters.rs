use crate::codex_transcript::question_from_tool_input;
use crate::{
    AgentEvent, AgentProtocolError, codex_question_from_transcript_path,
    locate_recent_codex_transcript_path,
};
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
    let session = string_at(
        &payload,
        &[
            "session_id",
            "sessionId",
            "thread-id",
            "thread_id",
            "threadId",
        ],
    );
    let transcript_path = string_at(&payload, &["transcript_path", "transcriptPath"]);
    let event = match hook {
        "SessionStart" => canonical(
            "session.start",
            "Codex",
            pid,
            session.as_deref(),
            None,
            None,
        )?,
        "PreToolUse" if is_question_tool(&payload) => canonical(
            "agent.needs-input",
            "Codex",
            pid,
            session.as_deref(),
            codex_question_text(&payload).as_deref(),
            Some("decision"),
        )?,
        "UserPromptSubmit" | "PreToolUse" | "PostToolUse" | "PostCompact" => canonical(
            "agent.running",
            "Codex",
            pid,
            session.as_deref(),
            None,
            None,
        )?,
        "PermissionRequest" => {
            let is_question = is_question_tool(&payload);
            let text = if is_question {
                codex_question_text(&payload)
            } else {
                first_message(&payload)
            };
            canonical(
                "agent.needs-input",
                "Codex",
                pid,
                session.as_deref(),
                text.as_deref(),
                Some(if is_question { "decision" } else { "approval" }),
            )?
        }
        "PreCompact" => canonical(
            "agent.running",
            "Codex",
            pid,
            session.as_deref(),
            Some("Compacting"),
            None,
        )?,
        "Stop" => canonical("agent.idle", "Codex", pid, session.as_deref(), None, None)?,
        _ => return Ok(Vec::new()),
    };
    Ok(vec![event.with_transcript_path(transcript_path)])
}

/// Converts a Codex `notify` callback payload into canonical version-1 events.
///
/// Informational notifications and Codex's automatic approval-review chatter
/// are intentional no-ops, matching the source adapter.
///
/// # Errors
///
/// Returns an error for malformed JSON or a canonical protocol construction
/// failure.
pub fn adapt_codex_notify(bytes: &[u8]) -> Result<Vec<AgentEvent>, AgentAdapterError> {
    let payload = parse_payload(bytes)?;
    let payload_type =
        string_at(&payload, &["type", "event_type", "eventType"]).unwrap_or_default();
    let session = string_at(
        &payload,
        &[
            "session_id",
            "sessionId",
            "thread-id",
            "thread_id",
            "threadId",
        ],
    );
    let transcript_path = string_at(&payload, &["transcript_path", "transcriptPath"]);
    if payload_type == "agent-turn-complete" {
        return Ok(vec![
            canonical("agent.idle", "Codex", None, session.as_deref(), None, None)?
                .with_transcript_path(transcript_path),
        ]);
    }

    let message = string_at(
        &payload,
        &[
            "title",
            "message",
            "body",
            "text",
            "prompt",
            "description",
            "last_assistant_message",
            "lastAssistantMessage",
        ],
    )
    .or_else(|| {
        if payload_type.to_ascii_lowercase().contains("permission") {
            Some("Codex needs your approval".to_owned())
        } else if payload_type.to_ascii_lowercase().contains("question") {
            Some("Codex is waiting for your input".to_owned())
        } else {
            None
        }
    });
    let Some(message) = message else {
        return Ok(Vec::new());
    };
    if is_codex_auto_approval_message(&payload_type, &message) {
        return Ok(Vec::new());
    }
    let Some(kind) = codex_notify_interaction_kind(&payload_type, &message) else {
        return Ok(Vec::new());
    };
    Ok(vec![
        canonical(
            "agent.needs-input",
            "Codex",
            None,
            session.as_deref(),
            Some(&message),
            Some(kind),
        )?
        .with_transcript_path(transcript_path),
    ])
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
        "UserPromptSubmit" | "SubagentStart" | "PreToolUse" | "PostCompact" => canonical(
            "agent.running",
            "Claude Code",
            pid,
            session.as_deref(),
            None,
            None,
        )?,
        "PermissionRequest" => {
            let is_question = is_question_tool(&payload);
            let text = if is_question {
                question_text(&payload)
                    .or_else(|| Some("Claude is waiting for your decision".to_owned()))
            } else {
                first_message(&payload).or_else(|| Some("Claude needs your approval".to_owned()))
            };
            canonical(
                "agent.needs-input",
                "Claude Code",
                pid,
                session.as_deref(),
                text.as_deref(),
                Some(if is_question { "decision" } else { "approval" }),
            )?
        }
        "Notification"
            if string_at(&payload, &["notification_type", "notificationType"]).as_deref()
                == Some("idle_prompt") =>
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
        "Notification" => {
            let message = first_message(&payload);
            if !message.as_deref().is_some_and(requires_human_input) {
                return Ok(Vec::new());
            }
            canonical(
                "agent.needs-input",
                "Claude Code",
                pid,
                session.as_deref(),
                message.as_deref(),
                Some("generic-input"),
            )?
        }
        "PreCompact" => canonical(
            "agent.running",
            "Claude Code",
            pid,
            session.as_deref(),
            Some("Compacting"),
            None,
        )?,
        "Stop" | "SubagentStop" => canonical(
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
        _ => return Ok(Vec::new()),
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

/// Converts a Cursor hook payload into canonical version-1 status events.
///
/// Cursor's high-frequency tool hooks are intentionally ignored unless they
/// communicate a lifecycle transition. This keeps the adapter useful without
/// manufacturing status changes for every shell command.
///
/// # Errors
///
/// Returns an error for malformed input, a missing hook name, or invalid
/// canonical output.
pub fn adapt_cursor_hook(
    bytes: &[u8],
    pid: Option<i32>,
) -> Result<Vec<AgentEvent>, AgentAdapterError> {
    let payload = parse_payload(bytes)?;
    let hook = event_name(&payload)?.to_ascii_lowercase();
    let session = string_at(
        &payload,
        &[
            "conversation_id",
            "conversationId",
            "session_id",
            "sessionId",
        ],
    );
    let transcript = string_at(&payload, &["transcript_path", "transcriptPath"]);
    let event = match hook.as_str() {
        "sessionstart" => canonical(
            "session.start",
            "Cursor",
            pid,
            session.as_deref(),
            None,
            None,
        )?,
        "beforesubmitprompt" | "subagentstart" | "subagentstop" | "aftershellexecution" => {
            canonical(
                "agent.running",
                "Cursor",
                pid,
                session.as_deref(),
                None,
                None,
            )?
        }
        "stop" => {
            let event = if string_ref_at(&payload, &["status"])
                .is_some_and(|status| status.eq_ignore_ascii_case("error"))
            {
                "agent.needs-input"
            } else {
                "agent.idle"
            };
            canonical(event, "Cursor", pid, session.as_deref(), None, None)?
        }
        "sessionend" => canonical("session.end", "Cursor", pid, session.as_deref(), None, None)?,
        _ => return Ok(Vec::new()),
    };
    Ok(vec![event.with_transcript_path(transcript)])
}

/// Converts a Factory Droid hook payload into canonical status events.
///
/// # Errors
///
/// Returns an error for malformed input, a missing hook name, or invalid
/// canonical output.
pub fn adapt_droid_hook(
    bytes: &[u8],
    pid: Option<i32>,
) -> Result<Vec<AgentEvent>, AgentAdapterError> {
    let payload = parse_payload(bytes)?;
    let hook = event_name(&payload)?;
    let session = string_at(&payload, &["session_id", "sessionId"]);
    let tool = string_at(&payload, &["tool_name", "toolName"]);
    let message = first_message(&payload);
    let permission_mode = string_at(&payload, &["permission_mode", "permissionMode"]);
    let event = match hook {
        "SessionStart" => canonical(
            "session.start",
            "Droid",
            pid,
            session.as_deref(),
            None,
            None,
        )?,
        "SessionEnd" => canonical("session.end", "Droid", pid, session.as_deref(), None, None)?,
        "Stop"
            if permission_mode
                .as_deref()
                .is_some_and(|mode| mode.eq_ignore_ascii_case("spec")) =>
        {
            return Ok(Vec::new());
        }
        "Stop" => canonical("agent.idle", "Droid", pid, session.as_deref(), None, None)?,
        "Notification" => {
            let text = message.unwrap_or_else(|| "Droid needs your input".to_owned());
            let kind = if requires_human_input(&text) {
                "approval"
            } else {
                "generic-input"
            };
            canonical(
                "agent.needs-input",
                "Droid",
                pid,
                session.as_deref(),
                Some(&text),
                Some(kind),
            )?
        }
        "PreToolUse" if tool.as_deref().is_some_and(is_droid_input_tool) => {
            let text = droid_input_text(&payload, tool.as_deref().unwrap_or("tool"));
            let kind = if tool.as_deref() == Some("AskUser") {
                "question"
            } else {
                "approval"
            };
            canonical(
                "agent.needs-input",
                "Droid",
                pid,
                session.as_deref(),
                Some(&text),
                Some(kind),
            )?
        }
        "UserPromptSubmit" | "PreToolUse" | "PostToolUse" | "SubagentStop" => canonical(
            "agent.running",
            "Droid",
            pid,
            session.as_deref(),
            None,
            None,
        )?,
        _ => return Ok(Vec::new()),
    };
    Ok(vec![event])
}

/// Converts a Mistral Vibe hook payload into canonical status events.
///
/// # Errors
///
/// Returns an error for malformed input, a missing hook name, or invalid
/// canonical output.
pub fn adapt_vibe_hook(bytes: &[u8]) -> Result<Vec<AgentEvent>, AgentAdapterError> {
    let payload = parse_payload(bytes)?;
    if payload.get("version").and_then(Value::as_u64) == Some(1)
        && payload.get("event").and_then(Value::as_str).is_some()
    {
        return AgentEvent::parse(bytes)
            .map(|event| vec![event])
            .map_err(AgentAdapterError::Protocol);
    }
    let hook = event_name(&payload)?;
    let session = string_at(&payload, &["session_id", "sessionId"]);
    let tool = string_at(&payload, &["tool_name", "toolName"]);
    let event = match hook {
        "post_agent_turn" => canonical(
            "agent.idle",
            "Mistral Vibe",
            None,
            session.as_deref(),
            None,
            None,
        )?,
        "before_tool" if tool.as_deref().is_some_and(is_question_tool_name) => {
            let text =
                vibe_question_text(&payload).unwrap_or_else(|| "Vibe needs your input".to_owned());
            canonical(
                "agent.needs-input",
                "Mistral Vibe",
                None,
                session.as_deref(),
                Some(&text),
                Some("question"),
            )?
        }
        "after_tool" if tool.as_deref().is_some_and(is_question_tool_name) => canonical(
            "agent.input-resolved",
            "Mistral Vibe",
            None,
            session.as_deref(),
            None,
            None,
        )?,
        "after_tool" if tool.as_deref().is_some_and(is_task_tool_name) => {
            if let Some((done, total)) = vibe_progress(&payload) {
                canonical_progress("Mistral Vibe", session.as_deref(), done, total)?
            } else {
                canonical(
                    "agent.running",
                    "Mistral Vibe",
                    None,
                    session.as_deref(),
                    None,
                    None,
                )?
            }
        }
        "before_tool" | "after_tool" => canonical(
            "agent.running",
            "Mistral Vibe",
            None,
            session.as_deref(),
            None,
            None,
        )?,
        _ => return Ok(Vec::new()),
    };
    Ok(vec![event])
}

/// Converts Kimi hook input into canonical status events.
///
/// # Errors
///
/// Returns an error for malformed input, a missing hook name, or invalid
/// canonical output.
pub fn adapt_kimi_hook(
    bytes: &[u8],
    pid: Option<i32>,
) -> Result<Vec<AgentEvent>, AgentAdapterError> {
    adapt_common_hook(bytes, "Kimi", pid, CommonHookDialect::Kimi)
}

/// Converts Grok Build hook input into canonical status events.
///
/// # Errors
///
/// Returns an error for malformed input, a missing hook name, or invalid
/// canonical output.
pub fn adapt_grok_hook(
    bytes: &[u8],
    pid: Option<i32>,
) -> Result<Vec<AgentEvent>, AgentAdapterError> {
    adapt_common_hook(bytes, "Grok", pid, CommonHookDialect::Grok)
}

/// Converts Antigravity hook input into canonical status events.
///
/// # Errors
///
/// Returns an error for malformed input, a missing hook name, or invalid
/// canonical output.
pub fn adapt_agy_hook(
    bytes: &[u8],
    pid: Option<i32>,
) -> Result<Vec<AgentEvent>, AgentAdapterError> {
    adapt_common_hook(bytes, "Antigravity", pid, CommonHookDialect::Agy)
}

/// Converts Hermes hook input into canonical status events.
///
/// # Errors
///
/// Returns an error for malformed input, a missing hook name, or invalid
/// canonical output.
pub fn adapt_hermes_hook(
    bytes: &[u8],
    pid: Option<i32>,
) -> Result<Vec<AgentEvent>, AgentAdapterError> {
    adapt_common_hook(bytes, "Hermes", pid, CommonHookDialect::Hermes)
}

#[derive(Clone, Copy)]
enum CommonHookDialect {
    Kimi,
    Grok,
    Agy,
    Hermes,
}

fn adapt_common_hook(
    bytes: &[u8],
    agent: &str,
    pid: Option<i32>,
    dialect: CommonHookDialect,
) -> Result<Vec<AgentEvent>, AgentAdapterError> {
    let payload = parse_payload(bytes)?;
    if payload.get("version").and_then(Value::as_u64) == Some(1)
        && payload.get("event").and_then(Value::as_str).is_some()
    {
        return AgentEvent::parse(bytes)
            .map(|event| vec![event])
            .map_err(AgentAdapterError::Protocol);
    }
    let hook = normalize_hook(event_name(&payload)?);
    let session = string_at(
        &payload,
        &[
            "session_id",
            "sessionId",
            "conversation_id",
            "conversationId",
        ],
    );
    let transcript = string_at(&payload, &["transcript_path", "transcriptPath"]);
    let tool = string_at(&payload, &["tool_name", "toolName", "tool"]).or_else(|| {
        payload
            .get("tool_call")
            .and_then(|call| string_at(call, &["name", "tool_name", "toolName"]))
    });
    let (event_name, text, interaction) =
        match common_transition(&payload, &hook, tool.as_deref(), dialect) {
            Ok(transition) => transition,
            Err(AgentAdapterError::UnsupportedEvent(_)) => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
    let event = canonical(
        event_name,
        agent,
        pid,
        session.as_deref(),
        text.as_deref(),
        interaction,
    )?
    .with_transcript_path(transcript);
    Ok(vec![event])
}

fn common_transition(
    payload: &Value,
    hook: &str,
    tool: Option<&str>,
    dialect: CommonHookDialect,
) -> Result<(&'static str, Option<String>, Option<&'static str>), AgentAdapterError> {
    let start = matches!(
        hook,
        "sessionstart" | "start" | "onsessionstart" | "onsessionreset"
    );
    let end = matches!(
        hook,
        "sessionend" | "end" | "onsessionend" | "onsessionfinalize"
    );
    if start {
        return Ok(("session.start", None, None));
    }
    if end {
        return Ok(("session.end", None, None));
    }
    if matches!(
        hook,
        "stop" | "turncompletion" | "turncomplete" | "postllmcall"
    ) {
        return Ok(("agent.idle", None, None));
    }
    if matches!(
        hook,
        "notification" | "permission" | "approval" | "preapprovalrequest"
    ) {
        let text = first_message(payload)
            .unwrap_or_else(|| format!("{} needs your input", dialect_name(dialect)));
        return Ok(("agent.needs-input", Some(text), Some("approval")));
    }
    if matches!(hook, "pretooluse" | "pretool" | "pretoolcall")
        && tool.is_some_and(common_question_tool)
    {
        let text = droid_input_text(payload, tool.unwrap_or("tool"));
        let kind = if tool.is_some_and(is_question_tool_name) || tool == Some("ask_question") {
            "question"
        } else {
            "approval"
        };
        return Ok(("agent.needs-input", Some(text), Some(kind)));
    }
    if matches!(
        hook,
        "userpromptsubmit"
            | "promptsubmit"
            | "preinvocation"
            | "postinvocation"
            | "pretooluse"
            | "posttooluse"
            | "pretool"
            | "posttool"
            | "beforeagent"
            | "afteragent"
            | "prellmcall"
            | "pretoolcall"
            | "posttoolcall"
            | "postapprovalresponse"
    ) {
        return Ok(("agent.running", None, None));
    }
    Err(AgentAdapterError::UnsupportedEvent(hook.to_owned()))
}

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

fn dialect_name(dialect: CommonHookDialect) -> &'static str {
    match dialect {
        CommonHookDialect::Kimi => "Kimi",
        CommonHookDialect::Grok => "Grok",
        CommonHookDialect::Agy => "Antigravity",
        CommonHookDialect::Hermes => "Hermes",
    }
}

fn is_question_tool_name(tool: &str) -> bool {
    let normalized = tool.to_ascii_lowercase();
    normalized.contains("askuserquestion") || normalized.contains("ask_user_question")
}

fn is_task_tool_name(tool: &str) -> bool {
    let normalized = tool.to_ascii_lowercase();
    normalized.contains("todo") || normalized.contains("task")
}

fn is_droid_input_tool(tool: &str) -> bool {
    matches!(tool, "AskUser" | "ExitSpecMode")
}

fn droid_input_text(payload: &Value, tool: &str) -> String {
    first_message(payload)
        .or_else(|| {
            payload
                .get("tool_input")
                .and_then(|input| string_at(input, &["question", "prompt", "plan", "spec"]))
        })
        .unwrap_or_else(|| format!("Droid needs your input for {tool}"))
}

fn vibe_question_text(payload: &Value) -> Option<String> {
    let input = payload.get("tool_input")?;
    string_at(input, &["question", "text", "prompt"]).or_else(|| {
        input
            .get("questions")?
            .as_array()?
            .first()
            .and_then(|question| string_at(question, &["question"]))
    })
}

fn vibe_progress(payload: &Value) -> Option<(u64, u64)> {
    let output = payload.get("tool_output")?;
    let todos = output.get("todos")?.as_array()?;
    let total = output
        .get("total_count")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| u64::try_from(todos.len()).unwrap_or(0));
    if total == 0 {
        return None;
    }
    let done = u64::try_from(
        todos
            .iter()
            .filter(|todo| {
                string_ref_at(todo, &["status"])
                    .is_some_and(|status| status.eq_ignore_ascii_case("completed"))
            })
            .count(),
    )
    .unwrap_or(0);
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

fn codex_question_text(payload: &Value) -> Option<String> {
    let direct = payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"));
    let result = if let Some(value) = direct {
        question_from_tool_input(value)
    } else {
        string_ref_at(payload, &["tool_args", "toolArgs", "arguments"])
            .and_then(|encoded| serde_json::from_str::<Value>(encoded).ok())
            .and_then(|parsed| question_from_tool_input(&parsed))
    };
    result
        .or_else(|| {
            string_ref_at(payload, &["transcript_path", "transcriptPath"])
                .and_then(|path| codex_question_from_transcript_path(std::path::Path::new(path)))
                .map(|question| question.text)
        })
        .or_else(|| {
            let cwd = string_ref_at(
                payload,
                &[
                    "cwd",
                    "current_working_directory",
                    "currentWorkingDirectory",
                ],
            )?;
            let codex_home = std::env::var_os("CODEX_HOME")
                .map(std::path::PathBuf::from)
                .or_else(|| {
                    std::env::var_os("HOME")
                        .map(|home| std::path::PathBuf::from(home).join(".codex"))
                })?;
            let transcript = locate_recent_codex_transcript_path(&codex_home, cwd)?;
            codex_question_from_transcript_path(&transcript).map(|question| question.text)
        })
}

fn is_codex_auto_approval_message(payload_type: &str, message: &str) -> bool {
    let compact = format!("{payload_type} {message}")
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    compact.contains("autoapprovalreview")
        || compact.contains("automaticapprovalreview")
        || compact.contains("autoreviewer")
        || compact.contains("autoreviewreturned")
        || (compact.contains("guardian")
            && compact.contains("approval")
            && compact.contains("review"))
}

fn codex_notify_interaction_kind(payload_type: &str, message: &str) -> Option<&'static str> {
    let normalized = message.to_ascii_lowercase();
    let normalized_type = payload_type.to_ascii_lowercase();
    if ["log in", "login", "sign in", "sign-in"]
        .iter()
        .any(|needle| normalized.contains(needle))
    {
        return Some("auth");
    }
    if normalized_type.contains("permission")
        || [
            "plan-mode-prompt",
            "plan mode prompt",
            "approval requested",
            "approval-requested",
            "approval",
            "permission",
            "approve",
            "allow ",
            "grant access",
            "wants to edit",
        ]
        .iter()
        .any(|needle| normalized.contains(needle))
    {
        return Some("approval");
    }
    if normalized_type.contains("question") || normalized.contains('?') {
        return Some(if codex_notify_has_options(message) {
            "decision"
        } else {
            "generic-input"
        });
    }
    [
        "waiting for your input",
        "waiting for input",
        "needs your input",
        "needs input",
        "press enter",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
    .then_some("generic-input")
}

fn codex_notify_has_options(message: &str) -> bool {
    (message.contains('[') && message.contains(']'))
        || message.lines().any(|line| {
            let trimmed = line.trim();
            trimmed.split_once('.').is_some_and(|(prefix, rest)| {
                prefix.parse::<u64>().is_ok() && !rest.trim().is_empty()
            })
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
