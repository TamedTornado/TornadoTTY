use super::{
    AgentAdapterError, AgentEvent, Value, canonical, canonical_progress, event_name,
    is_question_tool_name, parse_payload, string_at, string_ref_at,
};

fn is_task_tool_name(tool: &str) -> bool {
    let normalized = tool.to_ascii_lowercase();
    normalized.contains("todo") || normalized.contains("task")
}

fn vibe_question_text(payload: &Value) -> Option<String> {
    let input = payload.get("tool_input")?;
    string_at(input, &["question", "text", "prompt"]).or_else(|| {
        input
            .get("questions")?
            .as_array()?
            .first()
            .and_then(|question| string_at(question, &["question"]))
    })
}

fn vibe_progress(payload: &Value) -> Option<(u64, u64)> {
    let output = payload.get("tool_output")?;
    let todos = output.get("todos")?.as_array()?;
    let total = output
        .get("total_count")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| u64::try_from(todos.len()).unwrap_or(0));
    if total == 0 {
        return None;
    }
    let done = u64::try_from(
        todos
            .iter()
            .filter(|todo| {
                string_ref_at(todo, &["status"])
                    .is_some_and(|status| status.eq_ignore_ascii_case("completed"))
            })
            .count(),
    )
    .unwrap_or(0);
    Some((done, total))
}

/// Converts a Mistral Vibe hook payload into canonical status events.
///
/// # Errors
///
/// Returns an error for malformed input, a missing hook name, or invalid
/// canonical output.
pub fn adapt_vibe_hook(bytes: &[u8]) -> Result<Vec<AgentEvent>, AgentAdapterError> {
    let payload = parse_payload(bytes)?;
    if payload.get("version").and_then(Value::as_u64) == Some(1)
        && payload.get("event").and_then(Value::as_str).is_some()
    {
        return AgentEvent::parse(bytes)
            .map(|event| vec![event])
            .map_err(AgentAdapterError::Protocol);
    }
    let hook = event_name(&payload)?;
    let session = string_at(&payload, &["session_id", "sessionId"]);
    let tool = string_at(&payload, &["tool_name", "toolName"]);
    if matches!(hook, "before_tool" | "after_tool") && tool.is_none() {
        return Ok(Vec::new());
    }
    let event = match hook {
        "post_agent_turn" => canonical(
            "agent.idle",
            "Mistral Vibe",
            None,
            session.as_deref(),
            None,
            None,
        )?,
        "before_tool" if tool.as_deref().is_some_and(is_question_tool_name) => {
            let text =
                vibe_question_text(&payload).unwrap_or_else(|| "Vibe needs your input".to_owned());
            canonical(
                "agent.needs-input",
                "Mistral Vibe",
                None,
                session.as_deref(),
                Some(&text),
                Some("question"),
            )?
        }
        "after_tool" if tool.as_deref().is_some_and(is_question_tool_name) => canonical(
            "agent.input-resolved",
            "Mistral Vibe",
            None,
            session.as_deref(),
            None,
            None,
        )?,
        "after_tool" if tool.as_deref().is_some_and(is_task_tool_name) => {
            if let Some((done, total)) = vibe_progress(&payload) {
                canonical_progress("Mistral Vibe", session.as_deref(), done, total)?
            } else {
                canonical(
                    "agent.running",
                    "Mistral Vibe",
                    None,
                    session.as_deref(),
                    None,
                    None,
                )?
            }
        }
        "before_tool" | "after_tool" => canonical(
            "agent.running",
            "Mistral Vibe",
            None,
            session.as_deref(),
            None,
            None,
        )?,
        _ => return Ok(Vec::new()),
    };
    Ok(vec![event])
}
