use super::{
    AgentAdapterError, AgentEvent, Value, canonical, canonical_progress, common_input_text,
    event_name, first_message, parse_payload, requires_human_input, source_task_id, string_at,
    task_lifecycle_event, todo_progress,
};

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
        return task_event(&payload, hook, pid, session.as_deref());
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

fn task_event(
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
