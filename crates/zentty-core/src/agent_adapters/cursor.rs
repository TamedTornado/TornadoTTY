use super::{
    AgentAdapterError, AgentEvent, canonical, canonical_progress, canonical_stop_candidate,
    event_name, first_message, parse_payload, string_at, string_ref_at, task_lifecycle_event,
    todo_progress,
};

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
        "stop" => match string_ref_at(&payload, &["status"]).map(str::to_ascii_lowercase) {
            Some(status) if status == "error" => canonical(
                "agent.failed",
                "Cursor",
                pid,
                session.as_deref(),
                first_message(&payload).as_deref(),
                None,
            )?,
            Some(status) if status == "aborted" => canonical_stop_candidate(
                "Cursor",
                pid,
                session.as_deref(),
                first_message(&payload).as_deref(),
            )?,
            _ => canonical("agent.idle", "Cursor", pid, session.as_deref(), None, None)?,
        },
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
            let Some((done, total)) = todo_progress(&payload) else {
                return Ok(Vec::new());
            };
            canonical_progress("Cursor", session.as_deref(), done, total)?
        }
        _ => return Ok(Vec::new()),
    };
    Ok(vec![event.with_transcript_path(transcript)])
}
