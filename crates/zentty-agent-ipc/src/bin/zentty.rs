#![forbid(unsafe_code)]

use serde_json::Value;
use std::io::Read;
use std::process::ExitCode;
use zentty_agent_ipc::{AgentIpcClient, launch_agent};
use zentty_core::{AgentEvent, adapt_claude_hook, adapt_codex_hook};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("zentty: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    let command = arguments.next();
    if command.as_deref() == Some("launch") {
        let tool = arguments
            .next()
            .ok_or_else(|| "usage: zentty launch <claude|codex> [arguments...]".to_owned())?;
        return launch_agent(&tool, &arguments.collect::<Vec<_>>())
            .map_err(|error| error.to_string());
    }
    if command.as_deref() != Some("ipc") || arguments.next().as_deref() != Some("agent-event") {
        return Err("usage: zentty ipc agent-event [--adapter=codex|claude] [event]".to_owned());
    }
    let remaining = arguments.collect::<Vec<_>>();
    let adapter = remaining
        .iter()
        .find_map(|argument| argument.strip_prefix("--adapter="));
    let default_event = remaining
        .iter()
        .find(|argument| !argument.starts_with("--"));
    let mut input = Vec::new();
    std::io::stdin()
        .take(u64::try_from(AgentEvent::MAX_WIRE_BYTES + 1).unwrap_or(u64::MAX))
        .read_to_end(&mut input)
        .map_err(|error| format!("could not read event: {error}"))?;
    let input = add_default_hook_event(input, default_event.map(String::as_str))?;
    let events = match adapter {
        Some("codex") => adapt_codex_hook(&input, environment_pid("ZENTTY_CODEX_PID"))
            .map_err(|error| error.to_string())?,
        Some("claude") => adapt_claude_hook(&input, environment_pid("ZENTTY_CLAUDE_PID"))
            .map_err(|error| error.to_string())?,
        Some(value) => return Err(format!("unsupported agent adapter: {value}")),
        None => vec![AgentEvent::parse(&input).map_err(|error| error.to_string())?],
    };
    let socket = std::env::var("ZENTTY_INSTANCE_SOCKET")
        .map_err(|_| "ZENTTY_INSTANCE_SOCKET is missing".to_owned())?;
    let token = std::env::var("ZENTTY_PANE_TOKEN")
        .map_err(|_| "ZENTTY_PANE_TOKEN is missing".to_owned())?;
    for event in events {
        let bytes = serde_json::to_vec(&event).map_err(|error| error.to_string())?;
        AgentIpcClient::send_event(&socket, &token, &bytes, claimed_target_from_environment())
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn add_default_hook_event(mut input: Vec<u8>, event: Option<&str>) -> Result<Vec<u8>, String> {
    let Some(event) = event else {
        return Ok(input);
    };
    let mut value: Value =
        serde_json::from_slice(&input).map_err(|error| format!("invalid hook JSON: {error}"))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| "hook payload must be a JSON object".to_owned())?;
    object
        .entry("hook_event_name")
        .or_insert_with(|| Value::String(event.to_owned()));
    input = serde_json::to_vec(&value).map_err(|error| error.to_string())?;
    Ok(input)
}

fn environment_pid(name: &str) -> Option<i32> {
    std::env::var(name).ok()?.parse().ok()
}

fn claimed_target_from_environment() -> Option<zentty_core::AgentTarget> {
    Some(zentty_core::AgentTarget::new(
        std::env::var("ZENTTY_WINDOW_ID").ok()?,
        std::env::var("ZENTTY_WORKLANE_ID").ok()?,
        std::env::var("ZENTTY_PANE_ID").ok()?,
    ))
}
