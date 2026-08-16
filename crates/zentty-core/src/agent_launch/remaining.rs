use super::{
    AgentLaunchAction, AgentLaunchError, AgentLaunchPlan, AgentLaunchTool,
    shell_escape_double_quoted,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const SELF_PID_PLACEHOLDER: &str = "__ZENTTY_SELF_PID__";
const SMALL_HARNESS_MANAGED_KEYS: [&str; 2] = [
    "SMALL_HARNESS_MANAGED_HOOKS_FILE",
    "SMALL_HARNESS_MANAGED_HOOKS_JSON",
];

pub(super) fn copilot_plan(
    executable_path: String,
    arguments: &[String],
    environment: &BTreeMap<String, String>,
) -> AgentLaunchPlan {
    let Some(overlay) = environment
        .get("ZENTTY_COPILOT_HOME_OVERLAY")
        .filter(|value| !value.is_empty())
    else {
        return direct_plan(executable_path, arguments.to_vec(), Vec::new());
    };
    let (arguments, _) = extract_option(arguments, "--config-dir");
    AgentLaunchPlan {
        executable_path,
        arguments,
        set_environment: BTreeMap::from([
            ("ZENTTY_AGENT_TOOL".to_owned(), "copilot".to_owned()),
            ("COPILOT_HOME".to_owned(), overlay.clone()),
        ]),
        unset_environment: Vec::new(),
        pre_launch_actions: Vec::new(),
    }
}

pub(super) fn opencode_plan(
    executable_path: String,
    arguments: &[String],
    environment: &BTreeMap<String, String>,
) -> AgentLaunchPlan {
    let mut set_environment =
        BTreeMap::from([("ZENTTY_AGENT_TOOL".to_owned(), "opencode".to_owned())]);
    if let Some(source) = environment.get("ZENTTY_OPENCODE_BASE_CONFIG_DIR") {
        set_environment.insert("ZENTTY_OPENCODE_BASE_CONFIG_DIR".to_owned(), source.clone());
    }
    if let Some(overlay) = environment
        .get("ZENTTY_OPENCODE_CONFIG_OVERLAY")
        .filter(|value| !value.is_empty())
    {
        set_environment.insert("OPENCODE_CONFIG_DIR".to_owned(), overlay.clone());
    }
    AgentLaunchPlan {
        executable_path,
        arguments: arguments.to_vec(),
        set_environment,
        unset_environment: Vec::new(),
        pre_launch_actions: vec![session_start_action("OpenCode")],
    }
}

pub(super) fn pi_family_plan(
    tool: AgentLaunchTool,
    executable_path: String,
    arguments: &[String],
    environment: &BTreeMap<String, String>,
) -> AgentLaunchPlan {
    let (canonical_name, extension_key) = match tool {
        AgentLaunchTool::Pi => ("Pi", "ZENTTY_PI_EXTENSION"),
        AgentLaunchTool::Omp => ("OMP", "ZENTTY_OMP_EXTENSION"),
        _ => return direct_plan(executable_path, arguments.to_vec(), Vec::new()),
    };
    let Some(extension) = environment
        .get(extension_key)
        .filter(|value| !value.is_empty())
    else {
        return direct_plan(executable_path, arguments.to_vec(), Vec::new());
    };
    let mut planned = Vec::with_capacity(arguments.len() + 2);
    planned.extend(["-e".to_owned(), extension.clone()]);
    planned.extend_from_slice(arguments);
    AgentLaunchPlan {
        executable_path,
        arguments: planned,
        set_environment: BTreeMap::from([
            (
                "ZENTTY_AGENT_TOOL".to_owned(),
                tool.binary_name().to_owned(),
            ),
            (
                "ZENTTY_AGENT_CANONICAL_NAME".to_owned(),
                canonical_name.to_owned(),
            ),
        ]),
        unset_environment: Vec::new(),
        pre_launch_actions: vec![session_start_action(canonical_name)],
    }
}

pub(super) fn small_harness_plan(
    executable_path: String,
    arguments: &[String],
    environment: &BTreeMap<String, String>,
) -> AgentLaunchPlan {
    let Some(hooks_file) = environment
        .get("ZENTTY_SMALL_HARNESS_HOOKS_FILE")
        .filter(|value| !value.is_empty())
    else {
        return direct_plan(
            executable_path,
            arguments.to_vec(),
            SMALL_HARNESS_MANAGED_KEYS.map(str::to_owned).to_vec(),
        );
    };
    AgentLaunchPlan {
        executable_path,
        arguments: arguments.to_vec(),
        set_environment: BTreeMap::from([
            ("ZENTTY_AGENT_TOOL".to_owned(), "small-harness".to_owned()),
            (
                "SMALL_HARNESS_MANAGED_HOOKS_FILE".to_owned(),
                hooks_file.clone(),
            ),
        ]),
        unset_environment: vec!["SMALL_HARNESS_MANAGED_HOOKS_JSON".to_owned()],
        pre_launch_actions: Vec::new(),
    }
}

pub(super) fn pi_family_integration_is_disabled(
    tool: AgentLaunchTool,
    arguments: &[String],
    environment: &BTreeMap<String, String>,
) -> bool {
    if !has_pane_environment(environment) {
        return true;
    }
    let (disabled_key, passthrough, early_exit): (&str, &[&str], &[&str]) = match tool {
        AgentLaunchTool::Pi => (
            "ZENTTY_PI_HOOKS_DISABLED",
            &["install", "remove", "uninstall", "update", "list", "config"],
            &[
                "--help",
                "-h",
                "--version",
                "-v",
                "--list-models",
                "--export",
            ],
        ),
        AgentLaunchTool::Omp => (
            "ZENTTY_OMP_HOOKS_DISABLED",
            &[
                "acp",
                "agents",
                "auth-broker",
                "auth-gateway",
                "bench",
                "commit",
                "completions",
                "config",
                "dry-balance",
                "gallery",
                "gc",
                "grep",
                "grievances",
                "install",
                "join",
                "models",
                "plugin",
                "read",
                "say",
                "search",
                "setup",
                "shell",
                "ssh",
                "stats",
                "tiny-models",
                "token",
                "ttsr",
                "update",
                "usage",
                "worktree",
            ],
            &[
                "--help",
                "-h",
                "--version",
                "-v",
                "--list-models",
                "--export",
                "--alias",
            ],
        ),
        _ => return true,
    };
    environment.get(disabled_key).map(String::as_str) == Some("1")
        || first_scoped_subcommand(arguments).is_some_and(|value| passthrough.contains(&value))
        || arguments.iter().any(|argument| {
            let option = argument.split('=').next().unwrap_or(argument);
            early_exit.contains(&option)
        })
}

pub(super) fn small_harness_integration_is_disabled(
    arguments: &[String],
    environment: &BTreeMap<String, String>,
) -> bool {
    !has_pane_environment(environment)
        || environment
            .get("ZENTTY_SMALL_HARNESS_HOOKS_DISABLED")
            .map(String::as_str)
            == Some("1")
        || arguments
            .first()
            .is_some_and(|argument| argument == "completions")
        || arguments.iter().any(|argument| {
            matches!(
                argument.split('=').next().unwrap_or(argument),
                "--help" | "-h" | "--version" | "-V"
            )
        })
}

/// Merges Zentty's six Copilot hook groups into a source JSON/JSONC config.
///
/// # Errors
///
/// Returns an error for malformed input, a non-object root/hooks value, or
/// serialization failure.
pub fn build_copilot_config(
    existing: Option<&[u8]>,
    cli_path: &str,
) -> Result<Vec<u8>, AgentLaunchError> {
    let mut root = match existing {
        Some(bytes) => {
            let text = std::str::from_utf8(bytes)
                .map_err(|error| AgentLaunchError::Serialization(error.to_string()))?;
            serde_json::from_str::<Value>(&strip_jsonc(text))
                .map_err(|error| AgentLaunchError::Serialization(error.to_string()))?
        }
        None => json!({}),
    };
    let root = root.as_object_mut().ok_or_else(|| {
        AgentLaunchError::Serialization("Copilot config root must be an object".to_owned())
    })?;
    root.insert("version".to_owned(), Value::Number(1.into()));
    if !root.get("hooks").is_some_and(Value::is_object) {
        root.insert("hooks".to_owned(), json!({}));
    }
    let hooks = root
        .get_mut("hooks")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            AgentLaunchError::Serialization("could not initialize Copilot hooks".to_owned())
        })?;
    for (name, event, timeout) in [
        ("sessionStart", "session-start", 10_u64),
        ("sessionEnd", "session-end", 10),
        ("userPromptSubmitted", "user-prompt-submitted", 10),
        ("preToolUse", "pre-tool-use", 5),
        ("postToolUse", "post-tool-use", 5),
        ("errorOccurred", "error-occurred", 10),
    ] {
        let command = format!(
            "\"{}\" ipc agent-event --adapter=copilot {event} || true",
            shell_escape_double_quoted(cli_path)
        );
        if !hooks.get(name).is_some_and(Value::is_array) {
            hooks.insert(name.to_owned(), Value::Array(Vec::new()));
        }
        let entries = hooks
            .get_mut(name)
            .and_then(Value::as_array_mut)
            .ok_or_else(|| {
                AgentLaunchError::Serialization(format!(
                    "could not initialize Copilot {name} hooks"
                ))
            })?;
        if !entries.iter().any(|entry| {
            entry.get("type").and_then(Value::as_str) == Some("command")
                && entry.get("bash").and_then(Value::as_str) == Some(&command)
        }) {
            entries.push(json!({
                "type": "command",
                "bash": command,
                "timeoutSec": timeout,
            }));
        }
    }
    serde_json::to_vec_pretty(&root)
        .map_err(|error| AgentLaunchError::Serialization(error.to_string()))
}

/// Builds Small Harness's complete ephemeral managed-hook document.
///
/// # Errors
///
/// Returns an error if the bounded JSON document cannot be serialized.
pub fn build_small_harness_hooks(cli_path: &str) -> Result<Vec<u8>, AgentLaunchError> {
    let command = format!(
        "\"{}\" ipc agent-event --adapter=small-harness || printf '{{}}\\n'",
        shell_escape_double_quoted(cli_path)
    );
    let environment = [
        "ZENTTY_INSTANCE_SOCKET",
        "ZENTTY_WINDOW_ID",
        "ZENTTY_WORKLANE_ID",
        "ZENTTY_PANE_ID",
        "ZENTTY_PANE_TOKEN",
        "ZENTTY_INSTANCE_ID",
        "ZENTTY_SMALL_HARNESS_PID",
    ];
    let mut hooks = serde_json::Map::new();
    for (event, timeout) in [
        ("SessionStart", 10_u64),
        ("UserPromptSubmit", 10),
        ("PreToolUse", 10),
        ("PermissionRequest", 10),
        ("PostToolUse", 10),
        ("PreCompact", 10),
        ("PostCompact", 10),
        ("PlanUpdated", 10),
        ("SubagentStart", 10),
        ("SubagentStop", 10),
        ("Stop", 10),
        ("SessionEnd", 1),
    ] {
        hooks.insert(
            event.to_owned(),
            json!([{"hooks": [{
                "type": "command",
                "command": command,
                "envVars": environment,
                "timeoutSec": timeout,
            }]}]),
        );
    }
    serde_json::to_vec_pretty(&json!({"source": "zentty", "hooks": hooks}))
        .map_err(|error| AgentLaunchError::Serialization(error.to_string()))
}

fn direct_plan(
    executable_path: String,
    arguments: Vec<String>,
    unset_environment: Vec<String>,
) -> AgentLaunchPlan {
    AgentLaunchPlan {
        executable_path,
        arguments,
        set_environment: BTreeMap::new(),
        unset_environment,
        pre_launch_actions: Vec::new(),
    }
}

fn session_start_action(name: &str) -> AgentLaunchAction {
    AgentLaunchAction {
        standard_input: format!(
            r#"{{"version":1,"event":"session.start","agent":{{"name":"{name}","pid":{SELF_PID_PLACEHOLDER}}}}}"#
        ),
    }
}

fn has_pane_environment(environment: &BTreeMap<String, String>) -> bool {
    [
        "ZENTTY_INSTANCE_SOCKET",
        "ZENTTY_PANE_TOKEN",
        "ZENTTY_WORKLANE_ID",
        "ZENTTY_PANE_ID",
    ]
    .iter()
    .all(|key| environment.get(*key).is_some_and(|value| !value.is_empty()))
}

fn first_scoped_subcommand(arguments: &[String]) -> Option<&str> {
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index].as_str();
        if matches!(argument, "--profile" | "--cwd" | "--config") {
            index = index.saturating_add(2);
            continue;
        }
        if ["--profile=", "--cwd=", "--config="]
            .iter()
            .any(|prefix| argument.starts_with(prefix))
        {
            index += 1;
            continue;
        }
        return Some(argument);
    }
    None
}

fn extract_option(arguments: &[String], option: &str) -> (Vec<String>, Option<String>) {
    let mut forwarded = Vec::with_capacity(arguments.len());
    let mut selected = None;
    let mut index = 0;
    while index < arguments.len() {
        if arguments[index] == option && index + 1 < arguments.len() {
            selected = Some(arguments[index + 1].clone());
            index += 2;
        } else if let Some(value) = arguments[index].strip_prefix(&format!("{option}=")) {
            selected = Some(value.to_owned());
            index += 1;
        } else {
            forwarded.push(arguments[index].clone());
            index += 1;
        }
    }
    (forwarded, selected)
}

fn strip_jsonc(input: &str) -> String {
    let mut without_comments = String::with_capacity(input.len());
    let mut characters = input.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(character) = characters.next() {
        if in_string {
            without_comments.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        if character == '"' {
            in_string = true;
            without_comments.push(character);
        } else if character == '/' && characters.peek() == Some(&'/') {
            characters.next();
            for next in characters.by_ref() {
                if next == '\n' {
                    without_comments.push('\n');
                    break;
                }
            }
        } else if character == '/' && characters.peek() == Some(&'*') {
            characters.next();
            let mut previous = '\0';
            for next in characters.by_ref() {
                if previous == '*' && next == '/' {
                    break;
                }
                if next == '\n' {
                    without_comments.push('\n');
                }
                previous = next;
            }
        } else {
            without_comments.push(character);
        }
    }

    let characters = without_comments.chars().collect::<Vec<_>>();
    let mut output = String::with_capacity(characters.len());
    let mut in_string = false;
    let mut escaped = false;
    for (index, character) in characters.iter().copied().enumerate() {
        if in_string {
            output.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        if character == '"' {
            in_string = true;
            output.push(character);
            continue;
        }
        if character == ','
            && characters[index + 1..]
                .iter()
                .find(|next| !next.is_whitespace())
                .is_some_and(|next| matches!(next, '}' | ']'))
        {
            continue;
        }
        output.push(character);
    }
    output
}
