use super::{AgentLaunchError, AgentLaunchPlan, shell_escape_double_quoted};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

const UNFILTERED_EVENTS: [&str; 8] = [
    "sessionStart",
    "sessionEnd",
    "beforeSubmitPrompt",
    "stop",
    "beforeShellExecution",
    "afterShellExecution",
    "subagentStart",
    "subagentStop",
];

const TODO_EVENTS: [&str; 2] = ["preToolUse", "postToolUse"];

pub(super) fn plan(
    executable_path: String,
    arguments: &[String],
    environment: &BTreeMap<String, String>,
) -> AgentLaunchPlan {
    let mut set_environment =
        BTreeMap::from([("ZENTTY_AGENT_TOOL".to_owned(), "cursor".to_owned())]);
    if let Some(path) = environment.get("ZENTTY_CURSOR_CONFIG_OVERLAY") {
        set_environment.insert("CURSOR_CONFIG_DIR".to_owned(), path.clone());
    }
    AgentLaunchPlan {
        executable_path,
        arguments: arguments.to_vec(),
        set_environment,
        unset_environment: Vec::new(),
        pre_launch_actions: Vec::new(),
    }
}

/// Builds the complete source-compatible Cursor hook file for one launch.
///
/// # Errors
///
/// Returns an error if the generated JSON cannot be serialized.
pub fn build_hooks(cli_path: &str) -> Result<Vec<u8>, AgentLaunchError> {
    let command = format!(
        "\"{}\" ipc agent-event --adapter=cursor",
        shell_escape_double_quoted(cli_path)
    );
    let mut hooks = Map::new();
    for event in UNFILTERED_EVENTS {
        hooks.insert(event.to_owned(), json!([{"command": command}]));
    }
    for event in TODO_EVENTS {
        hooks.insert(
            event.to_owned(),
            json!([{"matcher": "TodoWrite", "command": command}]),
        );
    }
    serde_json::to_vec_pretty(&json!({"version": 1, "hooks": Value::Object(hooks)}))
        .map_err(|error| AgentLaunchError::Serialization(error.to_string()))
}
