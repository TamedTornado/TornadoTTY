use super::{
    AgentAdapterError, AgentEvent, Value, canonical, parse_payload, string_at, string_ref_at,
};

/// Converts GitHub Copilot CLI hook input into canonical status events.
///
/// The hook command supplies the event as a positional alias, while newer
/// payloads may carry `hook_event_name` directly.
///
/// # Errors
///
/// Returns an error for malformed input, a missing event name, an unknown
/// source event, or invalid canonical output.
pub fn adapt_copilot_hook(
    bytes: &[u8],
    default_event: Option<&str>,
    pid: Option<i32>,
) -> Result<Vec<AgentEvent>, AgentAdapterError> {
    let payload = parse_payload(bytes)?;
    let raw_event = string_ref_at(&payload, &["hook_event_name", "hookEventName"])
        .or(default_event)
        .ok_or(AgentAdapterError::MissingEventName)?;
    let event = normalized_event(raw_event)
        .ok_or_else(|| AgentAdapterError::UnsupportedEvent(raw_event.to_owned()))?;
    let session = string_at(&payload, &["session_id", "sessionId", "sessionID"]);
    match event {
        CopilotEvent::SessionStart => Ok(vec![
            canonical(
                "session.start",
                "Copilot",
                pid,
                session.as_deref(),
                None,
                None,
            )?,
            canonical("agent.idle", "Copilot", pid, session.as_deref(), None, None)?,
        ]),
        CopilotEvent::SessionEnd => Ok(vec![canonical(
            "session.end",
            "Copilot",
            pid,
            session.as_deref(),
            None,
            None,
        )?]),
        CopilotEvent::UserPromptSubmitted => Ok(vec![canonical(
            "agent.running",
            "Copilot",
            pid,
            session.as_deref(),
            None,
            None,
        )?]),
        CopilotEvent::PreToolUse | CopilotEvent::PostToolUse => {
            let tool = string_ref_at(&payload, &["toolName", "tool_name"]);
            if !tool.is_some_and(is_question_tool) {
                return Ok(Vec::new());
            }
            if event == CopilotEvent::PostToolUse {
                return Ok(vec![canonical(
                    "agent.idle",
                    "Copilot",
                    pid,
                    session.as_deref(),
                    None,
                    None,
                )?]);
            }
            let text = question_text(&payload)
                .unwrap_or_else(|| "Copilot is asking a question".to_owned());
            Ok(vec![canonical(
                "agent.needs-input",
                "Copilot",
                pid,
                session.as_deref(),
                Some(&text),
                Some("question"),
            )?])
        }
        CopilotEvent::ErrorOccurred => Ok(Vec::new()),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CopilotEvent {
    SessionStart,
    SessionEnd,
    UserPromptSubmitted,
    PreToolUse,
    PostToolUse,
    ErrorOccurred,
}

fn normalized_event(value: &str) -> Option<CopilotEvent> {
    let normalized = value
        .trim()
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    match normalized.as_str() {
        "sessionstart" => Some(CopilotEvent::SessionStart),
        "sessionend" => Some(CopilotEvent::SessionEnd),
        "userpromptsubmitted" => Some(CopilotEvent::UserPromptSubmitted),
        "pretooluse" => Some(CopilotEvent::PreToolUse),
        "posttooluse" => Some(CopilotEvent::PostToolUse),
        "erroroccurred" => Some(CopilotEvent::ErrorOccurred),
        _ => None,
    }
}

fn is_question_tool(value: &str) -> bool {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .contains("askuserquestion")
}

fn question_text(payload: &Value) -> Option<String> {
    let encoded = string_ref_at(payload, &["toolArgs", "tool_args"])?;
    let parsed = serde_json::from_str::<Value>(encoded).ok()?;
    string_at(&parsed, &["question", "prompt", "message", "title"])
}
