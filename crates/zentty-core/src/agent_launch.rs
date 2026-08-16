use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

mod remaining;

pub use remaining::{build_copilot_config, build_small_harness_hooks};
use remaining::{copilot_plan, opencode_plan, pi_family_plan, small_harness_plan};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentLaunchTool {
    Amp,
    Claude,
    Codex,
    Copilot,
    Cursor,
    Droid,
    Gemini,
    OpenCode,
    Pi,
    Omp,
    Kimi,
    Grok,
    Agy,
    Hermes,
    Vibe,
    SmallHarness,
}

impl AgentLaunchTool {
    /// Parses a source-supported ephemeral agent name.
    ///
    /// # Errors
    ///
    /// Returns an error when the tool has no implemented launch plan.
    pub fn parse(value: &str) -> Result<Self, AgentLaunchError> {
        match value {
            "amp" => Ok(Self::Amp),
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            "copilot" => Ok(Self::Copilot),
            "cursor" | "cursor-agent" => Ok(Self::Cursor),
            "droid" => Ok(Self::Droid),
            "gemini" => Ok(Self::Gemini),
            "opencode" => Ok(Self::OpenCode),
            "pi" => Ok(Self::Pi),
            "omp" => Ok(Self::Omp),
            "kimi" | "kimi-cli" => Ok(Self::Kimi),
            "grok" => Ok(Self::Grok),
            "agy" => Ok(Self::Agy),
            "hermes" => Ok(Self::Hermes),
            "vibe" | "mistral-vibe" => Ok(Self::Vibe),
            "small-harness" => Ok(Self::SmallHarness),
            _ => Err(AgentLaunchError::UnsupportedTool(value.to_owned())),
        }
    }

    #[must_use]
    pub const fn binary_name(self) -> &'static str {
        match self {
            Self::Amp => "amp",
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Copilot => "copilot",
            Self::Cursor => "cursor-agent",
            Self::Droid => "droid",
            Self::Gemini => "gemini",
            Self::OpenCode => "opencode",
            Self::Pi => "pi",
            Self::Omp => "omp",
            Self::Kimi => "kimi",
            Self::Grok => "grok",
            Self::Agy => "agy",
            Self::Hermes => "hermes",
            Self::Vibe => "vibe",
            Self::SmallHarness => "small-harness",
        }
    }

    #[must_use]
    pub const fn binary_names(self) -> &'static [&'static str] {
        match self {
            Self::Amp => &["amp"],
            Self::Claude => &["claude"],
            Self::Codex => &["codex"],
            Self::Copilot => &["copilot"],
            Self::Cursor => &["cursor-agent"],
            Self::Droid => &["droid"],
            Self::Gemini => &["gemini"],
            Self::OpenCode => &["opencode"],
            Self::Pi => &["pi"],
            Self::Omp => &["omp"],
            Self::Kimi => &["kimi", "kimi-cli"],
            Self::Grok => &["grok"],
            Self::Agy => &["agy"],
            Self::Hermes => &["hermes"],
            Self::Vibe => &["vibe", "mistral-vibe"],
            Self::SmallHarness => &["small-harness"],
        }
    }

    #[must_use]
    pub const fn persistent_integration_target(self) -> Option<&'static str> {
        match self {
            Self::Amp => Some("amp-hooks"),
            Self::Cursor => Some("cursor-hooks"),
            Self::Droid => Some("droid-hooks"),
            Self::Kimi => Some("kimi-hooks"),
            Self::Grok => Some("grok-hooks"),
            Self::Agy => Some("agy-hooks"),
            Self::Hermes => Some("hermes-hooks"),
            Self::Vibe => Some("vibe-hooks"),
            Self::Claude
            | Self::Codex
            | Self::Copilot
            | Self::Gemini
            | Self::OpenCode
            | Self::Pi
            | Self::Omp
            | Self::SmallHarness => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentLaunchAction {
    pub standard_input: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentLaunchPlan {
    pub executable_path: String,
    pub arguments: Vec<String>,
    pub set_environment: BTreeMap<String, String>,
    pub unset_environment: Vec<String>,
    pub pre_launch_actions: Vec<AgentLaunchAction>,
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
            } else if tool == AgentLaunchTool::SmallHarness {
                vec![
                    "SMALL_HARNESS_MANAGED_HOOKS_FILE".to_owned(),
                    "SMALL_HARNESS_MANAGED_HOOKS_JSON".to_owned(),
                ]
            } else {
                Vec::new()
            },
            pre_launch_actions: Vec::new(),
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
        AgentLaunchTool::Codex => Ok(codex_plan(
            executable_path.into(),
            arguments,
            cli_path,
            environment,
        )),
        AgentLaunchTool::Gemini => Ok(gemini_plan(executable_path.into(), arguments, environment)),
        AgentLaunchTool::Copilot => {
            Ok(copilot_plan(executable_path.into(), arguments, environment))
        }
        AgentLaunchTool::OpenCode => Ok(opencode_plan(
            executable_path.into(),
            arguments,
            environment,
        )),
        AgentLaunchTool::Pi | AgentLaunchTool::Omp => Ok(pi_family_plan(
            tool,
            executable_path.into(),
            arguments,
            environment,
        )),
        AgentLaunchTool::SmallHarness => Ok(small_harness_plan(
            executable_path.into(),
            arguments,
            environment,
        )),
        tool => Ok(persistent_plan(tool, executable_path.into(), arguments)),
    }
}

fn integration_is_disabled(
    tool: AgentLaunchTool,
    arguments: &[String],
    environment: &BTreeMap<String, String>,
) -> bool {
    if matches!(
        tool,
        AgentLaunchTool::Copilot
            | AgentLaunchTool::OpenCode
            | AgentLaunchTool::Pi
            | AgentLaunchTool::Omp
            | AgentLaunchTool::SmallHarness
    ) && [
        "ZENTTY_INSTANCE_SOCKET",
        "ZENTTY_PANE_TOKEN",
        "ZENTTY_WORKLANE_ID",
        "ZENTTY_PANE_ID",
    ]
    .iter()
    .any(|key| environment.get(*key).is_none_or(String::is_empty))
    {
        return true;
    }
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
        AgentLaunchTool::Copilot => {
            environment
                .get("ZENTTY_COPILOT_HOOKS_DISABLED")
                .map(String::as_str)
                == Some("1")
        }
        AgentLaunchTool::OpenCode => false,
        AgentLaunchTool::Pi | AgentLaunchTool::Omp => {
            remaining::pi_family_integration_is_disabled(tool, arguments, environment)
        }
        AgentLaunchTool::SmallHarness => {
            remaining::small_harness_integration_is_disabled(arguments, environment)
        }
        tool => persistent_integration_is_disabled(tool, arguments, environment),
    }
}

/// Returns whether this invocation is eligible for pane-scoped integration.
///
/// Filesystem preparation must call this before creating ephemeral overlays so
/// management, early-exit, disabled, and outside-pane launches stay non-mutating.
#[must_use]
pub fn agent_launch_requires_bootstrap(
    tool: AgentLaunchTool,
    arguments: &[String],
    environment: &BTreeMap<String, String>,
) -> bool {
    !integration_is_disabled(tool, arguments, environment)
}

fn persistent_plan(
    tool: AgentLaunchTool,
    executable_path: String,
    arguments: &[String],
) -> AgentLaunchPlan {
    let mut set_environment = BTreeMap::from([(
        "ZENTTY_AGENT_TOOL".to_owned(),
        tool.binary_name().trim_end_matches("-agent").to_owned(),
    )]);
    if tool == AgentLaunchTool::Vibe {
        set_environment.insert(
            "VIBE_ENABLE_EXPERIMENTAL_HOOKS".to_owned(),
            "true".to_owned(),
        );
    }
    AgentLaunchPlan {
        executable_path,
        arguments: arguments.to_vec(),
        set_environment,
        unset_environment: Vec::new(),
        pre_launch_actions: Vec::new(),
    }
}

fn persistent_integration_is_disabled(
    tool: AgentLaunchTool,
    arguments: &[String],
    environment: &BTreeMap<String, String>,
) -> bool {
    if [
        "ZENTTY_INSTANCE_SOCKET",
        "ZENTTY_PANE_TOKEN",
        "ZENTTY_WORKLANE_ID",
        "ZENTTY_PANE_ID",
    ]
    .iter()
    .any(|key| environment.get(*key).is_none_or(String::is_empty))
    {
        return true;
    }
    let (flag, passthrough): (&str, &[&str]) = match tool {
        AgentLaunchTool::Amp => (
            "ZENTTY_AMP_HOOKS_DISABLED",
            &[
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
            ],
        ),
        AgentLaunchTool::Cursor => ("ZENTTY_CURSOR_HOOKS_DISABLED", &[]),
        AgentLaunchTool::Droid => ("ZENTTY_DROID_HOOKS_DISABLED", &[]),
        AgentLaunchTool::Kimi => (
            "ZENTTY_KIMI_HOOKS_DISABLED",
            &[
                "login", "logout", "term", "acp", "info", "export", "mcp", "plugin", "vis", "web",
            ],
        ),
        AgentLaunchTool::Grok => ("ZENTTY_GROK_HOOKS_DISABLED", &[]),
        AgentLaunchTool::Agy => (
            "ZENTTY_AGY_HOOKS_DISABLED",
            &[
                "changelog",
                "help",
                "install",
                "login",
                "logout",
                "plugin",
                "plugins",
                "update",
                "version",
            ],
        ),
        AgentLaunchTool::Hermes => ("ZENTTY_HERMES_HOOKS_DISABLED", &[]),
        AgentLaunchTool::Vibe => (
            "ZENTTY_VIBE_HOOKS_DISABLED",
            &["login", "logout", "setup", "install", "uninstall", "update"],
        ),
        AgentLaunchTool::Claude
        | AgentLaunchTool::Codex
        | AgentLaunchTool::Copilot
        | AgentLaunchTool::Gemini
        | AgentLaunchTool::OpenCode
        | AgentLaunchTool::Pi
        | AgentLaunchTool::Omp
        | AgentLaunchTool::SmallHarness => return false,
    };
    let hermes_passthrough = tool == AgentLaunchTool::Hermes
        && arguments
            .first()
            .is_some_and(|argument| !argument.starts_with('-') && argument != "chat");
    let early_exit = tool != AgentLaunchTool::Grok
        && arguments.iter().any(|argument| {
            matches!(
                argument.split('=').next().unwrap_or(argument),
                "--help" | "-h" | "--version" | "-V" | "-v" | "--list-tools" | "--list-toolsets"
            )
        });
    environment.get(flag).map(String::as_str) == Some("1")
        || arguments
            .first()
            .is_some_and(|argument| passthrough.contains(&argument.as_str()))
        || hermes_passthrough
        || early_exit
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
        pre_launch_actions: Vec::new(),
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
        pre_launch_actions: Vec::new(),
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

fn codex_plan(
    executable_path: String,
    arguments: &[String],
    cli_path: &str,
    environment: &BTreeMap<String, String>,
) -> AgentLaunchPlan {
    let mut session_config = codex_hook_config_arguments(cli_path);
    if environment
        .get("ZENTTY_CODEX_NOTIFY_DISABLED")
        .map(String::as_str)
        != Some("1")
        && !has_codex_notify_override(arguments)
    {
        session_config.extend([
            "-c".to_owned(),
            format!(
                "notify=[{},{}]",
                toml_string(cli_path),
                toml_string("codex-notify")
            ),
        ]);
    }
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
        unset_environment: environment
            .get("CODEX_HOME")
            .filter(|path| is_linux_zentty_launch_path(path))
            .map(|_| vec!["CODEX_HOME".to_owned()])
            .unwrap_or_default(),
        pre_launch_actions: Vec::new(),
    }
}

fn has_codex_notify_override(arguments: &[String]) -> bool {
    let mut arguments = arguments.iter();
    while let Some(argument) = arguments.next() {
        if matches!(argument.as_str(), "-c" | "--config") {
            if arguments
                .next()
                .is_some_and(|value| value.starts_with("notify="))
            {
                return true;
            }
            continue;
        }
        if argument.starts_with("-cnotify=") || argument.starts_with("--config=notify=") {
            return true;
        }
    }
    false
}

fn is_linux_zentty_launch_path(path: &str) -> bool {
    let components = Path::new(path)
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>();
    components
        .windows(2)
        .any(|pair| pair == ["zentty", "launch"])
        || components
            .iter()
            .any(|component| component.starts_with("zentty-runtime"))
            && components.contains(&"launch")
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
