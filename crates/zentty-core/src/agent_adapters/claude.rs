use super::{
    AgentAdapterError, AgentEvent, Value, canonical, event_name, first_message, is_question_tool,
    parse_payload, question_text, requires_human_input, source_task_id, string_at,
    task_lifecycle_event,
};

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
        return Ok(notification_event(&payload, pid, session.as_deref())?
            .into_iter()
            .collect());
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
        "PermissionRequest" => permission_event(&payload, pid, session.as_deref())?,
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

fn permission_event(
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

fn notification_event(
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
