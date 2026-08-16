use super::{
    AgentAdapterError, AgentEvent, Value, bool_at, canonical, common_input_text,
    common_question_tool, first_message, is_question_tool_name, normalize_hook, parse_payload,
    source_event_name, string_at,
};

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
    adapt_source_hook(bytes, pid)
}

fn adapt_source_hook(bytes: &[u8], pid: Option<i32>) -> Result<Vec<AgentEvent>, AgentAdapterError> {
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
    let (event_name, text, interaction) = match transition(&payload, &hook, tool.as_deref()) {
        Ok(transition) => transition,
        Err(AgentAdapterError::UnsupportedEvent(_)) => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let event = canonical(
        event_name,
        "Antigravity",
        pid,
        session.as_deref(),
        text.as_deref(),
        interaction,
    )?
    .with_transcript_path(transcript);
    Ok(vec![event])
}

fn transition(
    payload: &Value,
    hook: &str,
    tool: Option<&str>,
) -> Result<(&'static str, Option<String>, Option<&'static str>), AgentAdapterError> {
    if matches!(
        hook,
        "sessionstart" | "start" | "onsessionstart" | "onsessionreset"
    ) {
        return Ok(("session.start", None, None));
    }
    if matches!(
        hook,
        "sessionend" | "end" | "onsessionend" | "onsessionfinalize"
    ) {
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
        let text = first_message(payload).unwrap_or_else(|| "Antigravity needs your input".into());
        return Ok(("agent.needs-input", Some(text), Some("approval")));
    }
    if matches!(hook, "pretooluse" | "pretool" | "pretoolcall")
        && tool.is_some_and(common_question_tool)
    {
        let text = common_input_text(payload, "Antigravity", tool.unwrap_or("tool"));
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
