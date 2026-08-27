use super::{
    AgentAdapterError, AgentEvent, Value, canonical, canonical_progress, event_name, first_message,
    parse_payload, source_task_id, string_at, string_ref_at, task_lifecycle_event,
};
use serde_json::json;

fn droid_manual_approval_tool(tool: &str) -> bool {
    matches!(
        tool,
        "Create" | "Edit" | "Execute" | "MultiEdit" | "NotebookEdit" | "Write"
    )
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
    if (hook == "PreToolUse" && tool.as_deref() == Some("Task")) || hook == "SubagentStop" {
        return task_event(&payload, hook, pid, session.as_deref(), working_directory);
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
        "Notification" => droid_notification_event(message, pid, session.as_deref())?,
        "PreToolUse" if tool.as_deref() == Some("AskUser") => {
            let (text, kind) = droid_ask_user_interaction(&payload);
            canonical(
                "agent.needs-input",
                "Droid",
                pid,
                session.as_deref(),
                Some(&text),
                Some(kind),
            )?
        }
        "PreToolUse" if tool.as_deref() == Some("ExitSpecMode") => {
            let text = droid_spec_proposal_text(&payload)
                .or(message)
                .unwrap_or_else(|| "Droid drafted a specification for your approval".to_owned());
            canonical(
                "agent.needs-input",
                "Droid",
                pid,
                session.as_deref(),
                Some(&text),
                Some("approval"),
            )?
        }
        "PreToolUse"
            if permission_mode
                .as_deref()
                .is_some_and(|mode| mode.eq_ignore_ascii_case("off"))
                && tool.as_deref().is_some_and(droid_manual_approval_tool) =>
        {
            let text = droid_approval_text(&payload, tool.as_deref().unwrap_or("tool"));
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
            return droid_todo_events(&payload, pid, session.as_deref(), working_directory);
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
    Ok(vec![event.with_working_directory(working_directory)])
}

fn droid_notification_event(
    message: Option<String>,
    pid: Option<i32>,
    session_id: Option<&str>,
) -> Result<AgentEvent, AgentAdapterError> {
    let text = message.unwrap_or_else(|| "Droid needs your input".to_owned());
    let normalized = text.to_ascii_lowercase();
    let kind = if ["permission", "approval", "approve"]
        .iter()
        .any(|marker| normalized.contains(marker))
    {
        "approval"
    } else if normalized.contains('?') {
        "question"
    } else {
        "generic-input"
    };
    canonical(
        "agent.needs-input",
        "Droid",
        pid,
        session_id,
        Some(&text),
        Some(kind),
    )
}

fn task_event(
    payload: &Value,
    hook: &str,
    pid: Option<i32>,
    session_id: Option<&str>,
    working_directory: Option<String>,
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
        )
        .map(|events| {
            events
                .into_iter()
                .map(|event| event.with_working_directory(working_directory.clone()))
                .collect()
        });
    }
    let Some(session_id) = session_id.map(str::trim).filter(|id| !id.is_empty()) else {
        return Ok(Vec::new());
    };
    let running = canonical("agent.running", "Droid", pid, Some(session_id), None, None)?
        .with_working_directory(working_directory);
    let delta = AgentEvent::parse(
        json!({
            "version": 1,
            "event": "task.delta",
            "agent": {"name": "Droid", "pid": pid},
            "session": {"id": session_id},
            "delta": if hook == "PreToolUse" {
                json!({"done": 0, "total": 1})
            } else {
                json!({"done": 1, "total": 0})
            },
        })
        .to_string()
        .as_bytes(),
    )
    .map_err(AgentAdapterError::Protocol)?;
    Ok(vec![running, delta])
}

fn droid_ask_user_interaction(payload: &Value) -> (String, &'static str) {
    let input = payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"));
    let question = input
        .and_then(|value| string_at(value, &["question", "prompt", "message", "text"]))
        .or_else(|| first_message(payload))
        .unwrap_or_else(|| "Droid needs your input".to_owned());
    let options = input
        .and_then(|value| value.get("options").or_else(|| value.get("choices")))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|option| {
            option
                .as_str()
                .map(str::to_owned)
                .or_else(|| string_at(option, &["label", "text", "value", "name"]))
        })
        .map(|option| option.trim().to_owned())
        .filter(|option| !option.is_empty())
        .collect::<Vec<_>>();
    if options.is_empty() {
        (question, "question")
    } else {
        (
            std::iter::once(question)
                .chain(options.into_iter().map(|option| format!("- {option}")))
                .collect::<Vec<_>>()
                .join("\n"),
            "decision",
        )
    }
}

fn droid_spec_proposal_text(payload: &Value) -> Option<String> {
    let input = payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"))?;
    let plan = string_at(input, &["plan", "spec", "proposal"])?;
    let first_line = plan.lines().map(str::trim).find(|line| !line.is_empty())?;
    Some(format!("Droid proposed a spec: {first_line}"))
}

fn droid_approval_text(payload: &Value, tool: &str) -> String {
    let input = payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"));
    if let Some(command) = input.and_then(|value| string_at(value, &["command"])) {
        return format!("Allow {tool}: {command}");
    }
    if let Some(path) = input.and_then(|value| string_at(value, &["file_path", "filePath", "path"]))
    {
        return format!("Allow {tool} on {path}?");
    }
    format!("Droid needs your permission to use {tool}")
}

fn droid_todo_events(
    payload: &Value,
    pid: Option<i32>,
    session_id: Option<&str>,
    working_directory: Option<String>,
) -> Result<Vec<AgentEvent>, AgentAdapterError> {
    let Some((done, total)) = droid_todo_progress(payload) else {
        return Ok(Vec::new());
    };
    let running = canonical("agent.running", "Droid", pid, session_id, None, None)?
        .with_working_directory(working_directory);
    let progress = if total == 0 {
        canonical_progress("Droid", session_id, 1, 1)?
    } else {
        canonical_progress("Droid", session_id, done, total)?
    };
    Ok(vec![running, progress])
}

fn droid_todo_progress(payload: &Value) -> Option<(u64, u64)> {
    let todos = payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"))?
        .get("todos")?;
    if let Some(items) = todos.as_array() {
        if items.is_empty() {
            return Some((0, 0));
        }
        if items.iter().all(Value::is_string) {
            return droid_todo_text_progress(
                &items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        let statuses = items
            .iter()
            .filter_map(|item| string_ref_at(item, &["status", "state"]))
            .collect::<Vec<_>>();
        if statuses.is_empty() {
            return None;
        }
        let total = u64::try_from(statuses.len()).ok()?;
        let done = u64::try_from(
            statuses
                .iter()
                .filter(|status| {
                    matches!(
                        status.trim().to_ascii_lowercase().as_str(),
                        "completed" | "complete" | "done"
                    )
                })
                .count(),
        )
        .ok()?;
        return Some((done, total));
    }
    droid_todo_text_progress(todos.as_str()?)
}

fn droid_todo_text_progress(text: &str) -> Option<(u64, u64)> {
    let mut saw_line = false;
    let mut done = 0_u64;
    let mut total = 0_u64;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        saw_line = true;
        let line = line.to_ascii_lowercase();
        if ["[completed]", "[done]", "[x]"]
            .iter()
            .any(|marker| line.contains(marker))
        {
            done += 1;
            total += 1;
        } else if ["[in_progress]", "[in-progress]", "[pending]", "[ ]"]
            .iter()
            .any(|marker| line.contains(marker))
        {
            total += 1;
        }
    }
    (total > 0 || !saw_line).then_some((done, total))
}
