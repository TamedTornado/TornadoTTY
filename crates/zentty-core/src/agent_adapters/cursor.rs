use super::{
    AgentAdapterError, AgentEvent, canonical, canonical_progress, canonical_stop_candidate,
    event_name, first_message, parse_payload, string_at, string_ref_at, task_lifecycle_event,
    todo_progress,
};
use serde_json::{Value, json};

/// Converts a Cursor hook payload into canonical version-1 status events.
///
/// Cursor's high-frequency tool hooks are intentionally ignored unless they
/// communicate a lifecycle transition. This keeps the adapter useful without
/// manufacturing status changes for every shell command.
///
/// # Errors
///
/// Returns an error for malformed input, a missing hook name, or invalid
/// canonical output.
pub fn adapt_cursor_hook(
    bytes: &[u8],
    pid: Option<i32>,
) -> Result<Vec<AgentEvent>, AgentAdapterError> {
    let payload = parse_payload(bytes)?;
    let hook = event_name(&payload)?.to_ascii_lowercase();
    let session = string_at(
        &payload,
        &[
            "conversation_id",
            "conversationId",
            "session_id",
            "sessionId",
        ],
    );
    let transcript = string_at(&payload, &["transcript_path", "transcriptPath"]);
    let working_directory = cursor_working_directory(&payload);
    let tool = string_at(&payload, &["tool_name", "toolName", "tool"]);
    let event = match hook.as_str() {
        "sessionstart" => canonical(
            "session.start",
            "Cursor",
            pid,
            session.as_deref(),
            None,
            None,
        )?,
        "beforesubmitprompt" | "aftershellexecution" => canonical(
            "agent.running",
            "Cursor",
            pid,
            session.as_deref(),
            None,
            None,
        )?,
        "stop" => cursor_stop_event(&payload, pid, session.as_deref())?,
        "subagentstart" | "subagentstop" => {
            let parent_session = string_at(
                &payload,
                &[
                    "parent_conversation_id",
                    "parentConversationId",
                    "conversation_id",
                    "conversationId",
                    "session_id",
                    "sessionId",
                ],
            );
            return task_lifecycle_event(
                if hook == "subagentstart" {
                    "task.started"
                } else {
                    "task.completed"
                },
                "Cursor",
                pid,
                parent_session.as_deref(),
                string_at(&payload, &["subagent_id", "subagentId"]).as_deref(),
                transcript.as_deref(),
            );
        }
        "sessionend" => canonical("session.end", "Cursor", pid, session.as_deref(), None, None)?,
        "pretooluse" | "posttooluse"
            if tool
                .as_deref()
                .is_some_and(|name| name.eq_ignore_ascii_case("TodoWrite")) =>
        {
            return cursor_todo_events(
                &payload,
                pid,
                session.as_deref(),
                working_directory,
                transcript,
            );
        }
        _ => return Ok(Vec::new()),
    };
    Ok(vec![
        event
            .with_working_directory(working_directory)
            .with_transcript_path(transcript),
    ])
}

fn cursor_stop_event(
    payload: &Value,
    pid: Option<i32>,
    session_id: Option<&str>,
) -> Result<AgentEvent, AgentAdapterError> {
    match string_ref_at(payload, &["status"]).map(str::to_ascii_lowercase) {
        Some(status) if status == "error" => canonical(
            "agent.failed",
            "Cursor",
            pid,
            session_id,
            first_message(payload).as_deref(),
            None,
        ),
        Some(status) if status == "aborted" => {
            canonical_stop_candidate("Cursor", pid, session_id, first_message(payload).as_deref())
        }
        _ => canonical("agent.idle", "Cursor", pid, session_id, None, None),
    }
}

fn cursor_todo_events(
    payload: &Value,
    pid: Option<i32>,
    session_id: Option<&str>,
    working_directory: Option<String>,
    transcript: Option<String>,
) -> Result<Vec<AgentEvent>, AgentAdapterError> {
    let running = canonical("agent.running", "Cursor", pid, session_id, None, None)?
        .with_working_directory(working_directory)
        .with_transcript_path(transcript);
    if let Some(snapshot) = cursor_task_snapshot(payload, session_id, pid)? {
        return Ok(vec![running, snapshot]);
    }
    let Some((done, total)) = todo_progress(payload).or_else(|| cursor_checklist_progress(payload))
    else {
        return Ok(Vec::new());
    };
    Ok(vec![
        running,
        canonical_progress("Cursor", session_id, done, total)?,
    ])
}

fn cursor_checklist_progress(payload: &Value) -> Option<(u64, u64)> {
    let input = payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"))
        .or_else(|| payload.get("input"))?;
    let checklist = input.get("todos")?.as_str()?;
    let entries = checklist
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let marker = line.get(..5)?.to_ascii_lowercase();
            match marker.as_str() {
                "- [x]" => Some(true),
                "- [ ]" => Some(false),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    let total = u64::try_from(entries.len()).ok()?;
    (total > 0).then(|| {
        let done = u64::try_from(entries.iter().filter(|done| **done).count()).unwrap_or(total);
        (done.min(total), total)
    })
}

fn cursor_working_directory(payload: &Value) -> Option<String> {
    payload
        .get("workspace_roots")
        .or_else(|| payload.get("workspaceRoots"))?
        .as_array()?
        .iter()
        .find_map(Value::as_str)
        .map(str::to_owned)
}

fn cursor_task_snapshot(
    payload: &Value,
    session_id: Option<&str>,
    pid: Option<i32>,
) -> Result<Option<AgentEvent>, AgentAdapterError> {
    let Some(session_id) = session_id.map(str::trim).filter(|id| !id.is_empty()) else {
        return Ok(None);
    };
    let Some(input) = payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"))
        .or_else(|| payload.get("input"))
    else {
        return Ok(None);
    };
    let Some(todos) = input.get("todos").and_then(Value::as_array) else {
        return Ok(None);
    };
    let tasks = todos
        .iter()
        .filter_map(|todo| {
            let id = string_at(todo, &["id", "content", "text", "title"])?;
            let completed = string_ref_at(todo, &["status", "state"]).is_some_and(|status| {
                matches!(
                    status.to_ascii_lowercase().as_str(),
                    "completed" | "done" | "cancelled"
                )
            });
            Some(json!({"id": id, "completed": completed}))
        })
        .collect::<Vec<_>>();
    if !todos.is_empty() && tasks.is_empty() {
        return Ok(None);
    }
    AgentEvent::parse(
        json!({
            "version": 1,
            "event": "task.snapshot",
            "agent": {"name": "Cursor", "pid": pid},
            "session": {"id": session_id},
            "merge": input.get("merge").and_then(Value::as_bool).unwrap_or(false),
            "tasks": tasks,
        })
        .to_string()
        .as_bytes(),
    )
    .map(Some)
    .map_err(AgentAdapterError::Protocol)
}
