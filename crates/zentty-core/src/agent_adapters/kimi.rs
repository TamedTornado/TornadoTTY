use super::{
    AgentAdapterError, AgentEvent, Value, canonical, event_name, first_message, parse_payload,
    string_at,
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
    let working_directory = string_at(
        &payload,
        &[
            "cwd",
            "working_directory",
            "workingDirectory",
            "project_dir",
            "projectDir",
        ],
    );
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
            let text = kimi_tool_input(&payload)
                .and_then(|input| string_at(input, &["question", "prompt", "message", "title"]))
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
            let text = kimi_approval_text(&payload, tool.as_deref().unwrap_or("tool"));
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
    Ok(vec![event.with_working_directory(working_directory)])
}

fn kimi_tool_input(payload: &Value) -> Option<&Value> {
    payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"))
}

fn kimi_approval_text(payload: &Value, tool: &str) -> String {
    let input = kimi_tool_input(payload);
    match tool.trim().to_ascii_lowercase().as_str() {
        "shell" => input
            .and_then(|value| string_at(value, &["command", "cmd"]))
            .map_or_else(
                || "Shell is requesting approval to run a command".to_owned(),
                |command| format!("Shell is requesting approval to run command: {command}"),
            ),
        "writefile" => input
            .and_then(|value| string_at(value, &["path", "file_path", "filePath"]))
            .map_or_else(
                || "WriteFile is requesting approval to write a file".to_owned(),
                |path| format!("WriteFile is requesting approval to write file: {path}"),
            ),
        "strreplacefile" => input
            .and_then(|value| string_at(value, &["path", "file_path", "filePath"]))
            .map_or_else(
                || "StrReplaceFile is requesting approval to edit a file".to_owned(),
                |path| format!("StrReplaceFile is requesting approval to edit file: {path}"),
            ),
        _ => "Kimi needs your approval".to_owned(),
    }
}
