use super::{
    AgentAdapterError, AgentEvent, canonical, common_input_text, event_name, first_message,
    parse_payload, question_text, string_at,
};

fn kimi_tool_requires_approval(tool: &str) -> bool {
    matches!(
        tool.trim().to_ascii_lowercase().as_str(),
        "shell" | "writefile" | "strreplacefile"
    )
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
