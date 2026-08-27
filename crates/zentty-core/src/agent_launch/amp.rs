use std::collections::BTreeMap;

use super::{AgentLaunchAction, AgentLaunchError, AgentLaunchPlan};

const SAFE_VALUE_OPTIONS: &[&str] = &[
    "--mode",
    "-m",
    "--effort",
    "--settings-file",
    "--log-level",
    "--log-file",
    "--mcp-config",
    "--visibility",
];
const DROPPED_VALUE_OPTIONS: &[&str] = &["--label", "-l"];
const DROPPED_FLAGS: &[&str] = &[
    "--archive",
    "--stream-json",
    "--stream-json-input",
    "--stream-json-thinking",
    "--json",
    "--output-format",
];
const REJECTED_FLAGS: &[&str] = &[
    "--execute",
    "--print",
    "-x",
    "--help",
    "-h",
    "--version",
    "-V",
    "--jetbrains",
];
const REJECTED_SUBCOMMANDS: &[&str] = &[
    "login",
    "logout",
    "mcp",
    "permission",
    "permissions",
    "review",
    "skill",
    "skills",
    "tool",
    "tools",
    "update",
    "up",
    "usage",
    "version",
];

pub(super) fn plan(
    executable_path: String,
    arguments: &[String],
) -> Result<AgentLaunchPlan, AgentLaunchError> {
    let resume_arguments = sanitize_resume_arguments(arguments);
    let snapshot = resume_arguments.as_deref().unwrap_or_default();
    let arguments_json = serde_json::to_string(snapshot)
        .map_err(|error| AgentLaunchError::Serialization(error.to_string()))?;
    let event = |kind: &str| AgentLaunchAction {
        standard_input: format!(
            r#"{{"version":1,"event":"{kind}","agent":{{"name":"Amp","pid":__ZENTTY_SELF_PID__}},"context":{{"launch":{{"arguments":{arguments_json}}}}}}}"#
        ),
    };
    let mut set_environment = BTreeMap::from([
        ("ZENTTY_AGENT_TOOL".to_owned(), "amp".to_owned()),
        ("PLUGINS".to_owned(), "all".to_owned()),
    ]);
    if let Some(arguments) = resume_arguments.filter(|arguments| !arguments.is_empty()) {
        set_environment.insert(
            "ZENTTY_AMP_RESUME_ARGUMENTS_JSON".to_owned(),
            serde_json::to_string(&arguments)
                .map_err(|error| AgentLaunchError::Serialization(error.to_string()))?,
        );
    }
    Ok(AgentLaunchPlan {
        executable_path,
        arguments: arguments.to_vec(),
        set_environment,
        unset_environment: Vec::new(),
        pre_launch_actions: vec![event("session.start"), event("agent.running")],
    })
}

/// Preserves only source-approved Amp options that are safe and meaningful
/// when constructing a future `threads continue` command.
#[must_use]
pub fn sanitize_resume_arguments(arguments: &[String]) -> Option<Vec<String>> {
    let mut remaining = arguments;
    if remaining.first().is_some_and(|argument| argument == "amp") {
        remaining = &remaining[1..];
    }
    if remaining
        .first()
        .is_some_and(|argument| REJECTED_SUBCOMMANDS.contains(&argument.as_str()))
        || remaining.iter().any(|argument| {
            let option = if argument.starts_with("--") {
                argument.split('=').next().unwrap_or(argument)
            } else {
                argument
            };
            REJECTED_FLAGS.contains(&option)
        })
    {
        return None;
    }
    if remaining.len() >= 2
        && matches!(remaining[0].as_str(), "threads" | "thread" | "t")
        && matches!(remaining[1].as_str(), "continue" | "c")
    {
        remaining = &remaining[2..];
        if remaining.first().is_some_and(|argument| {
            argument.strip_prefix("T-").is_some_and(|suffix| {
                !suffix.is_empty()
                    && suffix.chars().all(|character| {
                        character.is_ascii_alphanumeric() || character == '_' || character == '-'
                    })
            })
        }) {
            remaining = &remaining[1..];
        }
    }

    let mut sanitized = Vec::new();
    let mut index = 0;
    while index < remaining.len() {
        let argument = &remaining[index];
        if argument.starts_with("--") {
            let option = argument.split('=').next().unwrap_or(argument);
            if SAFE_VALUE_OPTIONS.contains(&option) {
                if argument.contains('=') {
                    sanitized.push(argument.clone());
                } else if remaining
                    .get(index + 1)
                    .is_some_and(|value| !value.starts_with('-'))
                {
                    sanitized.push(argument.clone());
                    sanitized.push(remaining[index + 1].clone());
                    index += 1;
                }
            } else if DROPPED_VALUE_OPTIONS.contains(&option) {
                if !argument.contains('=')
                    && remaining
                        .get(index + 1)
                        .is_some_and(|value| !value.starts_with('-'))
                {
                    index += 1;
                }
            } else if DROPPED_FLAGS.contains(&option)
                && option == "--output-format"
                && !argument.contains('=')
                && remaining
                    .get(index + 1)
                    .is_some_and(|value| !value.starts_with('-'))
            {
                index += 1;
            }
        } else if argument == "-m" {
            if remaining
                .get(index + 1)
                .is_some_and(|value| !value.starts_with('-'))
            {
                sanitized.push(argument.clone());
                sanitized.push(remaining[index + 1].clone());
                index += 1;
            }
        } else if argument == "-l" {
            if remaining
                .get(index + 1)
                .is_some_and(|value| !value.starts_with('-'))
            {
                index += 1;
            }
        } else if !argument.starts_with('-') {
            break;
        }
        index += 1;
    }
    Some(sanitized)
}
