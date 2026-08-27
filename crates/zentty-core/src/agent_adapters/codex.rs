use super::{
    AgentAdapterError, AgentEvent, Value, canonical, canonical_progress, event_name,
    explicit_progress, first_message, is_question_tool, parse_payload, source_task_id, string_at,
    string_ref_at, task_lifecycle_event,
};
use crate::codex_transcript::question_from_tool_input;
use crate::{codex_question_from_transcript_path, locate_recent_codex_transcript_path};

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
    Ok(vec![codex_event_with_context(event, &payload)])
}

fn codex_event_with_context(event: AgentEvent, payload: &Value) -> AgentEvent {
    let transcript_path = string_at(payload, &["transcript_path", "transcriptPath"]);
    let working_directory = string_at(
        payload,
        &[
            "cwd",
            "current_working_directory",
            "currentWorkingDirectory",
        ],
    );
    event
        .with_transcript_path(transcript_path)
        .with_working_directory(working_directory)
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
