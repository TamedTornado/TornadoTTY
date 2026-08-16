use super::{
    AgentAdapterError, AgentEvent, Value, canonical, event_name, first_message, parse_payload,
    string_at,
};

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
