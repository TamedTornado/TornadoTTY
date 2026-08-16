use super::{
    AgentAdapterError, AgentEvent, Value, canonical, is_question_tool_name, normalize_hook,
    parse_payload, source_event_name, string_at,
};

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
