use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentLaunchTool {
    Claude,
    Codex,
    Gemini,
}

impl AgentLaunchTool {
    /// Parses a source-supported ephemeral agent name.
    ///
    /// # Errors
    ///
    /// Returns an error when the tool has no implemented launch plan.
    pub fn parse(value: &str) -> Result<Self, AgentLaunchError> {
        match value {
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            "gemini" => Ok(Self::Gemini),
            _ => Err(AgentLaunchError::UnsupportedTool(value.to_owned())),
        }
    }

    #[must_use]
    pub const fn binary_name(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentLaunchPlan {
    pub executable_path: String,
    pub arguments: Vec<String>,
    pub set_environment: BTreeMap<String, String>,
    pub unset_environment: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentLaunchError {
    UnsupportedTool(String),
    InvalidSessionId,
    Serialization(String),
}

impl fmt::Display for AgentLaunchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedTool(tool) => write!(formatter, "unsupported agent tool: {tool}"),
            Self::InvalidSessionId => formatter.write_str("agent launch session ID is invalid"),
            Self::Serialization(error) => {
                write!(formatter, "agent launch settings are invalid: {error}")
            }
        }
    }
}

impl std::error::Error for AgentLaunchError {}

/// Builds a source-compatible, per-launch hook plan without modifying user configuration.
///
/// # Errors
///
/// Returns an error for an invalid generated Claude session ID or settings serialization.
pub fn build_agent_launch_plan(
    tool: AgentLaunchTool,
    executable_path: impl Into<String>,
    arguments: &[String],
    cli_path: &str,
    session_id: &str,
    environment: &BTreeMap<String, String>,
) -> Result<AgentLaunchPlan, AgentLaunchError> {
    if integration_is_disabled(tool, arguments, environment) {
        return Ok(AgentLaunchPlan {
            executable_path: executable_path.into(),
            arguments: arguments.to_vec(),
            set_environment: BTreeMap::new(),
            unset_environment: if tool == AgentLaunchTool::Claude {
                vec!["CLAUDECODE".to_owned()]
            } else {
                Vec::new()
            },
        });
    }
    match tool {
        AgentLaunchTool::Claude => claude_plan(
            executable_path.into(),
            arguments,
            cli_path,
            session_id,
            environment,
        ),
        AgentLaunchTool::Codex => Ok(codex_plan(executable_path.into(), arguments, cli_path)),
        AgentLaunchTool::Gemini => Ok(gemini_plan(executable_path.into(), arguments, environment)),
    }
}

fn integration_is_disabled(
    tool: AgentLaunchTool,
    arguments: &[String],
    environment: &BTreeMap<String, String>,
) -> bool {
    match tool {
        AgentLaunchTool::Claude => {
            environment
                .get("ZENTTY_CLAUDE_HOOKS_DISABLED")
                .map(String::as_str)
                == Some("1")
                || arguments.first().is_some_and(|argument| {
                    matches!(argument.as_str(), "mcp" | "config" | "api-key")
                })
        }
        AgentLaunchTool::Codex => {
            environment
                .get("ZENTTY_CODEX_HOOKS_DISABLED")
                .map(String::as_str)
                == Some("1")
        }
        AgentLaunchTool::Gemini => {
            environment
                .get("ZENTTY_GEMINI_HOOKS_DISABLED")
                .map(String::as_str)
                == Some("1")
        }
    }
}

fn gemini_plan(
    executable_path: String,
    arguments: &[String],
    environment: &BTreeMap<String, String>,
) -> AgentLaunchPlan {
    let mut set_environment =
        BTreeMap::from([("ZENTTY_AGENT_TOOL".to_owned(), "gemini".to_owned())]);
    if let Some(path) = environment.get("ZENTTY_GEMINI_SETTINGS_OVERLAY") {
        set_environment.insert("GEMINI_CLI_SYSTEM_SETTINGS_PATH".to_owned(), path.clone());
    }
    AgentLaunchPlan {
        executable_path,
        arguments: arguments.to_vec(),
        set_environment,
        unset_environment: Vec::new(),
    }
}

/// Merges Zentty's per-launch Gemini hooks into existing system settings.
///
/// Existing object fields and hook groups are retained, notifications are
/// enabled for wrapped terminals, and the exact Zentty group is de-duplicated.
///
/// # Errors
///
/// Returns an error when existing settings are malformed, are not a JSON
/// object, or when the merged value cannot be serialized.
pub fn build_gemini_settings(
    existing: Option<&[u8]>,
    cli_path: &str,
) -> Result<Vec<u8>, AgentLaunchError> {
    let mut root = match existing {
        Some(bytes) => serde_json::from_slice::<Value>(bytes)
            .map_err(|error| AgentLaunchError::Serialization(error.to_string()))?,
        None => json!({}),
    };
    let root = root.as_object_mut().ok_or_else(|| {
        AgentLaunchError::Serialization("Gemini settings root must be an object".to_owned())
    })?;
    let general = root
        .entry("general")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| {
            AgentLaunchError::Serialization("Gemini general settings must be an object".to_owned())
        })?;
    general.insert("enableNotifications".to_owned(), Value::Bool(true));
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| {
            AgentLaunchError::Serialization("Gemini hooks settings must be an object".to_owned())
        })?;
    let command = format!(
        "\"{}\" ipc agent-event --adapter=gemini || echo '{{}}'",
        shell_escape_double_quoted(cli_path)
    );
    for (event, timeout) in [
        ("SessionStart", 10_000_u64),
        ("SessionEnd", 1_000),
        ("BeforeAgent", 10_000),
        ("AfterAgent", 10_000),
        ("Notification", 10_000),
        ("BeforeTool", 5_000),
    ] {
        let groups = hooks
            .entry(event)
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .ok_or_else(|| {
                AgentLaunchError::Serialization(format!(
                    "Gemini {event} hook settings must be an array"
                ))
            })?;
        groups.retain(|group| {
            group
                .get("hooks")
                .and_then(Value::as_array)
                .is_none_or(|entries| {
                    !entries.iter().any(|entry| {
                        entry.get("command").and_then(Value::as_str) == Some(command.as_str())
                    })
                })
        });
        groups.push(json!({
            "matcher": "*",
            "hooks": [{"type": "command", "command": command, "timeout": timeout}],
        }));
    }
    serde_json::to_vec_pretty(&root)
        .map_err(|error| AgentLaunchError::Serialization(error.to_string()))
}

fn claude_plan(
    executable_path: String,
    arguments: &[String],
    cli_path: &str,
    session_id: &str,
    environment: &BTreeMap<String, String>,
) -> Result<AgentLaunchPlan, AgentLaunchError> {
    if session_id.is_empty()
        || session_id
            .bytes()
            .any(|byte| !byte.is_ascii_hexdigit() && byte != b'-')
    {
        return Err(AgentLaunchError::InvalidSessionId);
    }
    let command = format!(
        "\"{}\" ipc agent-event --adapter=claude",
        shell_escape_double_quoted(cli_path)
    );
    let mut planned = Vec::with_capacity(arguments.len() + 4);
    if !claude_reuses_session(arguments) {
        planned.extend(["--session-id".to_owned(), session_id.to_owned()]);
    }
    planned.extend(["--settings".to_owned(), claude_settings(&command)?]);
    planned.extend_from_slice(arguments);
    let mut set_environment =
        BTreeMap::from([("ZENTTY_AGENT_TOOL".to_owned(), "claude".to_owned())]);
    if environment
        .get("NO_COLOR")
        .is_none_or(|value| value.trim().is_empty())
    {
        if environment
            .get("FORCE_COLOR")
            .is_none_or(|value| value.trim().is_empty())
        {
            set_environment.insert("FORCE_COLOR".to_owned(), "3".to_owned());
        }
        if environment
            .get("COLORTERM")
            .is_none_or(|value| value.trim().is_empty())
        {
            set_environment.insert("COLORTERM".to_owned(), "truecolor".to_owned());
        }
    }
    Ok(AgentLaunchPlan {
        executable_path,
        arguments: planned,
        set_environment,
        unset_environment: vec!["CLAUDECODE".to_owned()],
    })
}

fn claude_settings(command: &str) -> Result<String, AgentLaunchError> {
    let ordinary = |timeout| hook_groups(&[""], command, timeout);
    let hooks = json!({
        "SessionStart": hook_groups(&["startup", "resume", "clear", "compact"], command, 10),
        "Stop": ordinary(10),
        "SessionEnd": ordinary(1),
        "Notification": ordinary(10),
        "PermissionRequest": ordinary(10),
        "UserPromptSubmit": ordinary(10),
        "PreToolUse": hook_groups(&["AskUserQuestion", "Bash|Write|Edit|MultiEdit|NotebookEdit"], command, 5),
        "PreCompact": ordinary(10),
        "PostCompact": ordinary(10),
        "TaskCreated": ordinary(5),
        "TaskCompleted": ordinary(5),
    });
    serde_json::to_string(&json!({"hooks": hooks}))
        .map_err(|error| AgentLaunchError::Serialization(error.to_string()))
}

fn hook_groups(matchers: &[&str], command: &str, timeout: u64) -> Vec<Value> {
    matchers
        .iter()
        .map(|matcher| {
            json!({
                "matcher": matcher,
                "hooks": [{"type": "command", "command": command, "timeout": timeout}],
            })
        })
        .collect()
}

fn claude_reuses_session(arguments: &[String]) -> bool {
    arguments.iter().any(|argument| {
        matches!(
            argument.as_str(),
            "--resume" | "--continue" | "-c" | "--session-id"
        ) || argument.starts_with("--resume=")
            || argument.starts_with("--session-id=")
    })
}

fn codex_plan(executable_path: String, arguments: &[String], cli_path: &str) -> AgentLaunchPlan {
    let mut session_config = codex_hook_config_arguments(cli_path);
    session_config.extend([
        "-c".to_owned(),
        "tui.notification_method=osc9".to_owned(),
        "-c".to_owned(),
        r#"tui.terminal_title=["status","spinner","project","task-progress"]"#.to_owned(),
    ]);
    let mut planned = arguments.to_vec();
    if let Some(index) = planned
        .iter()
        .position(|argument| matches!(argument.as_str(), "exec" | "fork" | "resume" | "review"))
    {
        for (offset, argument) in session_config.into_iter().enumerate() {
            planned.insert(index + 1 + offset, argument);
        }
    } else {
        planned.splice(0..0, session_config);
    }
    AgentLaunchPlan {
        executable_path,
        arguments: planned,
        set_environment: BTreeMap::from([("ZENTTY_AGENT_TOOL".to_owned(), "codex".to_owned())]),
        unset_environment: Vec::new(),
    }
}

struct CodexHookSpec {
    name: &'static str,
    key: &'static str,
    argument: &'static str,
}

const CODEX_HOOKS: &[CodexHookSpec] = &[
    CodexHookSpec {
        name: "SessionStart",
        key: "session_start",
        argument: "session-start",
    },
    CodexHookSpec {
        name: "PreToolUse",
        key: "pre_tool_use",
        argument: "pre-tool-use",
    },
    CodexHookSpec {
        name: "PermissionRequest",
        key: "permission_request",
        argument: "permission-request",
    },
    CodexHookSpec {
        name: "PostToolUse",
        key: "post_tool_use",
        argument: "post-tool-use",
    },
    CodexHookSpec {
        name: "UserPromptSubmit",
        key: "user_prompt_submit",
        argument: "prompt-submit",
    },
    CodexHookSpec {
        name: "PreCompact",
        key: "pre_compact",
        argument: "pre-compact",
    },
    CodexHookSpec {
        name: "PostCompact",
        key: "post_compact",
        argument: "post-compact",
    },
    CodexHookSpec {
        name: "Stop",
        key: "stop",
        argument: "stop",
    },
];

fn codex_hook_config_arguments(cli_path: &str) -> Vec<String> {
    let mut values = vec!["features.hooks=true".to_owned()];
    let mut states = Vec::new();
    for spec in CODEX_HOOKS {
        let command = format!(
            "\"{}\" ipc agent-event --adapter=codex {} || true; echo '{{}}'",
            shell_escape_double_quoted(cli_path),
            spec.argument
        );
        values.push(format!(
            "hooks.{}=[{{hooks=[{{type=\"command\",command={},timeout=10}}]}}]",
            spec.name,
            toml_string(&command)
        ));
        states.push(format!(
            "{}={{trusted_hash={}}}",
            toml_string(&format!("/<session-flags>/config.toml:{}:0:0", spec.key)),
            toml_string(&codex_trusted_hash(spec.key, &command))
        ));
    }
    values.push(format!("hooks.state={{{}}}", states.join(",")));
    values
        .into_iter()
        .flat_map(|value| ["-c".to_owned(), value])
        .collect()
}

fn codex_trusted_hash(event_key: &str, command: &str) -> String {
    let mut hook = Map::new();
    hook.insert("async".to_owned(), Value::Bool(false));
    hook.insert("command".to_owned(), Value::String(command.to_owned()));
    hook.insert("timeout".to_owned(), Value::Number(10.into()));
    hook.insert("type".to_owned(), Value::String("command".to_owned()));
    let mut identity = Map::new();
    identity.insert("event_name".to_owned(), Value::String(event_key.to_owned()));
    identity.insert("hooks".to_owned(), Value::Array(vec![Value::Object(hook)]));
    let canonical = serde_json::to_string(&Value::Object(identity)).expect("JSON values serialize");
    format!("sha256:{:x}", Sha256::digest(canonical.as_bytes()))
}

fn shell_escape_double_quoted(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('`', "\\`")
}

fn toml_string(value: &str) -> String {
    let mut output = String::from("\"");
    for character in value.chars() {
        match character {
            '\u{8}' => output.push_str("\\b"),
            '\t' => output.push_str("\\t"),
            '\n' => output.push_str("\\n"),
            '\u{c}' => output.push_str("\\f"),
            '\r' => output.push_str("\\r"),
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            value if value < '\u{20}' => {
                use std::fmt::Write as _;
                let _ = write!(output, "\\u{:04x}", u32::from(value));
            }
            value => output.push(value),
        }
    }
    output.push('"');
    output
}
