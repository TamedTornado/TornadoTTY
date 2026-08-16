use super::{
    AgentAdapterError, AgentEvent, canonical, common_input_text, common_question_tool,
    first_message, is_question_tool_name, normalize_hook, parse_payload, source_event_name,
    string_at,
};

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
