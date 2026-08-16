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
    adapt_codex_family_hook(bytes, pid, "Codex", false)
}

/// Converts a source Small Harness hook payload into canonical events. The
/// managed launcher remains owned by GH-47; this adapter is independently
/// usable through the authenticated CLI protocol.
///
/// # Errors
///
/// Returns the same bounded parse and canonical-protocol errors as the Codex
/// family adapter.
pub fn adapt_small_harness_hook(
    bytes: &[u8],
    pid: Option<i32>,
) -> Result<Vec<AgentEvent>, AgentAdapterError> {
    adapt_codex_family_hook(bytes, pid, "Small Harness", true)
}

fn adapt_codex_family_hook(
    bytes: &[u8],
    pid: Option<i32>,
    agent_name: &str,
    extended_lifecycle: bool,
) -> Result<Vec<AgentEvent>, AgentAdapterError> {
    let payload = parse_payload(bytes)?;
    let raw_hook = event_name(&payload)?;
    let hook = if extended_lifecycle {
        raw_hook
    } else {
        codex_source_event_alias(raw_hook)
    };
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
    if extended_lifecycle
        && matches!(
            hook,
            "PlanUpdated" | "SubagentStart" | "SubagentStop" | "SessionEnd"
        )
    {
        return adapt_small_harness_lifecycle(
            &payload,
            hook,
            pid,
            session.as_deref(),
            transcript_path.as_deref(),
        );
    }
    let event = match hook {
        "SessionStart" => canonical(
            "session.start",
            agent_name,
            pid,
            session.as_deref(),
            None,
            None,
        )?,
        "PreToolUse" if is_question_tool(&payload) => canonical(
            "agent.needs-input",
            agent_name,
            pid,
            session.as_deref(),
            codex_question_text(&payload).as_deref(),
            Some("decision"),
        )?,
        "UserPromptSubmit" | "PreToolUse" | "PostToolUse" => canonical(
            "agent.running",
            agent_name,
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
                agent_name,
                pid,
                session.as_deref(),
                text.as_deref(),
                Some(if is_question { "decision" } else { "approval" }),
            )?
        }
        "PreCompact" => canonical(
            "agent.compacting",
            agent_name,
            pid,
            session.as_deref(),
            Some("Compacting"),
            None,
        )?,
        "PostCompact" => canonical(
            "agent.compacted",
            agent_name,
            pid,
            session.as_deref(),
            None,
            None,
        )?,
        "Stop" => canonical(
            "agent.idle",
            agent_name,
            pid,
            session.as_deref(),
            None,
            None,
        )?,
        _ => return Ok(Vec::new()),
    };
    Ok(vec![event.with_transcript_path(transcript_path)])
}

fn adapt_small_harness_lifecycle(
    payload: &Value,
    hook: &str,
    pid: Option<i32>,
    session_id: Option<&str>,
    transcript_path: Option<&str>,
) -> Result<Vec<AgentEvent>, AgentAdapterError> {
    match hook {
        "PlanUpdated" => {
            let running = canonical(
                "agent.running",
                "Small Harness",
                pid,
                session_id,
                None,
                None,
            )?
            .with_transcript_path(transcript_path.map(str::to_owned));
            let Some((done, total)) = explicit_progress(payload) else {
                return Ok(vec![running]);
            };
            Ok(vec![
                running,
                canonical_progress("Small Harness", session_id, done, total)?
                    .with_transcript_path(transcript_path.map(str::to_owned)),
            ])
        }
        "SubagentStart" | "SubagentStop" => task_lifecycle_event(
            if hook == "SubagentStart" {
                "task.started"
            } else {
                "task.completed"
            },
            "Small Harness",
            pid,
            session_id,
            source_task_id(payload).as_deref(),
            transcript_path,
        ),
        "SessionEnd" => Ok(vec![
            canonical("session.end", "Small Harness", pid, session_id, None, None)?
                .with_transcript_path(transcript_path.map(str::to_owned)),
        ]),
        _ => Ok(Vec::new()),
    }
}

fn codex_source_event_alias(event: &str) -> &str {
    match event.to_ascii_lowercase().as_str() {
        "session-start" => "SessionStart",
        "pre-tool-use" => "PreToolUse",
        "permission-request" => "PermissionRequest",
        "post-tool-use" => "PostToolUse",
        "prompt-submit" => "UserPromptSubmit",
        "pre-compact" => "PreCompact",
        "post-compact" => "PostCompact",
        "stop" => "Stop",
        _ => event,
    }
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
    let session = string_at(&payload, &["session_id", "sessionId", "sessionID"]);
    if let Some(task_event) = match hook {
        "TaskCreated" => Some("task.started"),
        "TaskCompleted" => Some("task.completed"),
        _ => None,
    } {
        return task_lifecycle_event(
            task_event,
            "Claude Code",
            pid,
            session.as_deref(),
            source_task_id(&payload).as_deref(),
            None,
        );
    }
    if hook == "Notification" {
        return Ok(
            claude_notification_event(&payload, pid, session.as_deref())?
                .into_iter()
                .collect(),
        );
    }
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
        "UserPromptSubmit" | "SubagentStart" | "PreToolUse" => canonical(
            "agent.running",
            "Claude Code",
            pid,
            session.as_deref(),
            None,
            None,
        )?,
        "PermissionRequest" => claude_permission_event(&payload, pid, session.as_deref())?,
        "PreCompact" => canonical(
            "agent.compacting",
            "Claude Code",
            pid,
            session.as_deref(),
            Some("Compacting"),
            None,
        )?,
        "PostCompact" => canonical(
            "agent.compacted",
            "Claude Code",
            pid,
            session.as_deref(),
            None,
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

fn claude_permission_event(
    payload: &Value,
    pid: Option<i32>,
    session_id: Option<&str>,
) -> Result<AgentEvent, AgentAdapterError> {
    let is_question = is_question_tool(payload);
    let text = if is_question {
        question_text(payload).or_else(|| Some("Claude is waiting for your decision".to_owned()))
    } else {
        first_message(payload).or_else(|| Some("Claude needs your approval".to_owned()))
    };
    canonical(
        "agent.needs-input",
        "Claude Code",
        pid,
        session_id,
        text.as_deref(),
        Some(if is_question { "decision" } else { "approval" }),
    )
}

fn claude_notification_event(
    payload: &Value,
    pid: Option<i32>,
    session_id: Option<&str>,
) -> Result<Option<AgentEvent>, AgentAdapterError> {
    if string_at(payload, &["notification_type", "notificationType"]).as_deref()
        == Some("idle_prompt")
    {
        return canonical("agent.idle", "Claude Code", pid, session_id, None, None).map(Some);
    }
    let message = first_message(payload);
    if !message.as_deref().is_some_and(requires_human_input) {
        return Ok(None);
    }
    canonical(
        "agent.needs-input",
        "Claude Code",
        pid,
        session_id,
        message.as_deref(),
        Some("generic-input"),
    )
    .map(Some)
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
    let session = string_at(&payload, &["session_id", "sessionId", "sessionID"]);
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
    let tool = string_at(&payload, &["tool_name", "toolName", "tool"]);
    let event = match hook.as_str() {
        "sessionstart" => canonical(
            "session.start",
            "Cursor",
            pid,
            session.as_deref(),
            None,
            None,
        )?,
        "beforesubmitprompt" | "aftershellexecution" => canonical(
            "agent.running",
            "Cursor",
            pid,
            session.as_deref(),
            None,
            None,
        )?,
        "stop" => match string_ref_at(&payload, &["status"]).map(str::to_ascii_lowercase) {
            Some(status) if status == "error" => canonical(
                "agent.failed",
                "Cursor",
                pid,
                session.as_deref(),
                first_message(&payload).as_deref(),
                None,
            )?,
            Some(status) if status == "aborted" => canonical_stop_candidate(
                "Cursor",
                pid,
                session.as_deref(),
                first_message(&payload).as_deref(),
            )?,
            _ => canonical("agent.idle", "Cursor", pid, session.as_deref(), None, None)?,
        },
        "subagentstart" | "subagentstop" => {
            let parent_session = string_at(
                &payload,
                &[
                    "parent_conversation_id",
                    "parentConversationId",
                    "conversation_id",
                    "conversationId",
                    "session_id",
                    "sessionId",
                ],
            );
            return task_lifecycle_event(
                if hook == "subagentstart" {
                    "task.started"
                } else {
                    "task.completed"
                },
                "Cursor",
                pid,
                parent_session.as_deref(),
                string_at(&payload, &["subagent_id", "subagentId"]).as_deref(),
                transcript.as_deref(),
            );
        }
        "sessionend" => canonical("session.end", "Cursor", pid, session.as_deref(), None, None)?,
        "pretooluse" | "posttooluse"
            if tool
                .as_deref()
                .is_some_and(|name| name.eq_ignore_ascii_case("TodoWrite")) =>
        {
            let Some((done, total)) = todo_progress(&payload) else {
                return Ok(Vec::new());
            };
            canonical_progress("Cursor", session.as_deref(), done, total)?
        }
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
    if (hook == "PreToolUse" && tool.as_deref() == Some("Task")) || hook == "SubagentStop" {
        return droid_task_event(&payload, hook, pid, session.as_deref());
    }
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
        "PreToolUse"
            if permission_mode
                .as_deref()
                .is_some_and(|mode| mode.eq_ignore_ascii_case("off"))
                && tool.as_deref().is_some_and(droid_manual_approval_tool) =>
        {
            let text = droid_input_text(&payload, tool.as_deref().unwrap_or("tool"));
            canonical(
                "agent.needs-input",
                "Droid",
                pid,
                session.as_deref(),
                Some(&text),
                Some("approval"),
            )?
        }
        "PreToolUse" | "PostToolUse" if tool.as_deref().is_some_and(|name| name == "TodoWrite") => {
            let Some((done, total)) = todo_progress(&payload) else {
                return Ok(Vec::new());
            };
            canonical_progress("Droid", session.as_deref(), done, total)?
        }
        "PostToolUse" if tool.as_deref() == Some("ExitSpecMode") => return Ok(Vec::new()),
        "UserPromptSubmit" | "PreToolUse" | "PostToolUse" => canonical(
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

fn droid_task_event(
    payload: &Value,
    hook: &str,
    pid: Option<i32>,
    session_id: Option<&str>,
) -> Result<Vec<AgentEvent>, AgentAdapterError> {
    let task_id = source_task_id(payload);
    if task_id.is_some() {
        return task_lifecycle_event(
            if hook == "PreToolUse" {
                "task.started"
            } else {
                "task.completed"
            },
            "Droid",
            pid,
            session_id,
            task_id.as_deref(),
            None,
        );
    }
    Ok(vec![canonical(
        "agent.running",
        "Droid",
        pid,
        session_id,
        None,
        None,
    )?])
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
    if matches!(hook, "before_tool" | "after_tool") && tool.is_none() {
        return Ok(Vec::new());
    }
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
    let payload = parse_payload(bytes)?;
    let hook = event_name(&payload)?;
    let session = string_at(&payload, &["session_id", "sessionId"]);
    let tool = string_at(&payload, &["tool_name", "toolName"]);
    let event = match hook {
        "SessionStart" => canonical("session.start", "Kimi", pid, session.as_deref(), None, None)?,
        "UserPromptSubmit" => {
            canonical("agent.running", "Kimi", pid, session.as_deref(), None, None)?
        }
        "Stop" => canonical("agent.idle", "Kimi", pid, session.as_deref(), None, None)?,
        "SessionEnd" => canonical("session.end", "Kimi", pid, session.as_deref(), None, None)?,
        "Notification"
            if string_at(&payload, &["notification_type", "notificationType"])
                .is_some_and(|kind| kind.eq_ignore_ascii_case("permission_prompt")) =>
        {
            let text = first_message(&payload).unwrap_or_else(|| "Kimi needs your approval".into());
            canonical(
                "agent.needs-input",
                "Kimi",
                pid,
                session.as_deref(),
                Some(&text),
                Some("approval"),
            )?
        }
        "PreToolUse" if tool.as_deref() == Some("AskUserQuestion") => {
            let text = question_text(&payload)
                .unwrap_or_else(|| "Kimi is waiting for your input".to_owned());
            canonical(
                "agent.needs-input",
                "Kimi",
                pid,
                session.as_deref(),
                Some(&text),
                Some("question"),
            )?
        }
        "PreToolUse" if tool.as_deref().is_some_and(kimi_tool_requires_approval) => {
            let text = common_input_text(&payload, "Kimi", tool.as_deref().unwrap_or("tool"));
            canonical(
                "agent.needs-input",
                "Kimi",
                pid,
                session.as_deref(),
                Some(&text),
                Some("approval"),
            )?
        }
        "PostToolUse"
            if tool.as_deref() == Some("AskUserQuestion")
                || tool.as_deref().is_some_and(kimi_tool_requires_approval) =>
        {
            canonical(
                "agent.input-resolved",
                "Kimi",
                pid,
                session.as_deref(),
                None,
                None,
            )?
        }
        _ => return Ok(Vec::new()),
    };
    Ok(vec![event])
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
    let payload = parse_payload(bytes)?;
    if payload.get("version").and_then(Value::as_u64) == Some(1)
        && payload.get("event").and_then(Value::as_str).is_some()
    {
        return AgentEvent::parse(bytes)
            .map(|event| vec![event])
            .map_err(AgentAdapterError::Protocol);
    }
    let hook = normalize_hook(source_event_name(&payload)?);
    let session = string_at(&payload, &["session_id", "sessionId", "sessionID"]);
    let tool = string_at(&payload, &["tool_name", "toolName", "tool"]);
    let event = match hook.as_str() {
        "sessionstart" | "start" => {
            canonical("session.start", "Grok", pid, session.as_deref(), None, None)?
        }
        "userpromptsubmit" | "promptsubmit" => {
            canonical("agent.running", "Grok", pid, session.as_deref(), None, None)?
        }
        "stop" | "turncomplete" => {
            canonical("agent.idle", "Grok", pid, session.as_deref(), None, None)?
        }
        "sessionend" | "end" => {
            canonical("session.end", "Grok", pid, session.as_deref(), None, None)?
        }
        "pretooluse" | "pretool" | "posttooluse"
            if !tool.as_deref().is_some_and(is_question_tool_name) =>
        {
            canonical("agent.running", "Grok", pid, session.as_deref(), None, None)?
        }
        _ => return Ok(Vec::new()),
    };
    Ok(vec![event])
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
    let payload = parse_payload(bytes)?;
    let hook = normalize_hook(source_event_name(&payload)?);
    if matches!(hook.as_str(), "stop" | "turncompletion")
        && bool_at(&payload, &["fullyIdle", "fully_idle"]) == Some(false)
    {
        let session = string_at(&payload, &["session_id", "sessionId"]);
        return Ok(vec![canonical(
            "agent.failed",
            "Antigravity",
            pid,
            session.as_deref(),
            first_message(&payload).as_deref(),
            None,
        )?]);
    }
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
    let payload = parse_payload(bytes)?;
    let hook = normalize_hook(source_event_name(&payload)?);
    let session = string_at(&payload, &["session_id", "sessionId", "sessionID", "id"])
        .or_else(|| {
            payload
                .get("session")
                .and_then(|value| string_at(value, &["id", "session_id"]))
        })
        .or_else(|| {
            payload
                .get("context")
                .and_then(|value| string_at(value, &["session_id", "sessionId"]))
        });
    let tool = string_at(&payload, &["tool_name", "toolName", "tool"]).or_else(|| {
        payload
            .get("tool_call")
            .and_then(|call| string_at(call, &["name", "tool_name"]))
    });
    let event = match hook.as_str() {
        "onsessionstart" | "onsessionreset" | "sessionstart" | "start" => canonical(
            "session.start",
            "Hermes",
            pid,
            session.as_deref(),
            None,
            None,
        )?,
        "pretoolcall" if tool.as_deref().is_some_and(common_question_tool) => {
            let text = common_input_text(&payload, "Hermes", tool.as_deref().unwrap_or("tool"));
            canonical(
                "agent.needs-input",
                "Hermes",
                pid,
                session.as_deref(),
                Some(&text),
                Some(if tool.as_deref().is_some_and(is_question_tool_name) {
                    "question"
                } else {
                    "approval"
                }),
            )?
        }
        "prellmcall" | "pretoolcall" | "posttoolcall" | "postapprovalresponse" => canonical(
            "agent.running",
            "Hermes",
            pid,
            session.as_deref(),
            None,
            None,
        )?,
        "postllmcall" | "onsessionend" | "onsessionfinalize" | "sessionend" | "end" => {
            canonical("agent.idle", "Hermes", pid, session.as_deref(), None, None)?
        }
        "preapprovalrequest" => {
            let text =
                first_message(&payload).unwrap_or_else(|| "Hermes needs your approval".into());
            canonical(
                "agent.needs-input",
                "Hermes",
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

#[derive(Clone, Copy)]
enum CommonHookDialect {
    Agy,
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
    let hook = normalize_hook(source_event_name(&payload)?);
    let session = string_at(
        &payload,
        &[
            "session_id",
            "sessionId",
            "sessionID",
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
        let text = common_input_text(payload, dialect_name(dialect), tool.unwrap_or("tool"));
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
        CommonHookDialect::Agy => "Antigravity",
    }
}

fn kimi_tool_requires_approval(tool: &str) -> bool {
    matches!(
        tool.trim().to_ascii_lowercase().as_str(),
        "shell" | "writefile" | "strreplacefile"
    )
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

fn droid_manual_approval_tool(tool: &str) -> bool {
    matches!(
        tool,
        "Create" | "Edit" | "Execute" | "MultiEdit" | "NotebookEdit" | "Write"
    )
}

fn droid_input_text(payload: &Value, tool: &str) -> String {
    common_input_text(payload, "Droid", tool)
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

fn bool_at(payload: &Value, keys: &[&str]) -> Option<bool> {
    keys.iter().find_map(|key| payload.get(*key)?.as_bool())
}

fn string_ref_at<'a>(payload: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| payload.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}
