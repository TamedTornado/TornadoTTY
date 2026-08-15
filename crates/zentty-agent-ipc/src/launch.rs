use crate::{generate_pane_token, install_integration};
use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use zentty_core::{AgentLaunchTool, build_agent_launch_plan, build_gemini_settings};

#[derive(Debug)]
pub enum LaunchError {
    UnsupportedTool(String),
    RealBinaryNotFound(String),
    InvalidRealBinary(String),
    Plan(String),
    Exec(std::io::Error),
}

impl fmt::Display for LaunchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedTool(tool) => write!(formatter, "unsupported agent tool: {tool}"),
            Self::RealBinaryNotFound(tool) => write!(formatter, "{tool} was not found in PATH"),
            Self::InvalidRealBinary(path) => {
                write!(formatter, "agent binary is not executable: {path}")
            }
            Self::Plan(error) => write!(formatter, "could not prepare agent launch: {error}"),
            Self::Exec(error) => write!(formatter, "could not execute agent: {error}"),
        }
    }
}

impl std::error::Error for LaunchError {}

/// Replaces the wrapper process with a real agent using ephemeral Zentty hooks.
///
/// # Errors
///
/// Returns an error when the tool or real binary is invalid, plan construction
/// fails, or the final `exec` system call fails.
pub fn launch_agent(tool: &str, arguments: &[String]) -> Result<(), LaunchError> {
    let tool =
        AgentLaunchTool::parse(tool).map_err(|_| LaunchError::UnsupportedTool(tool.to_owned()))?;
    let mut environment = std::env::vars().collect::<BTreeMap<_, _>>();
    let executable = resolve_real_binary(tool, &environment)?;
    let cli_path = environment
        .get("ZENTTY_CLI_BIN")
        .cloned()
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .map(|path| path.to_string_lossy().into_owned())
        })
        .ok_or_else(|| LaunchError::Plan("Zentty CLI path is unavailable".to_owned()))?;
    let session_id = random_session_id().map_err(|error| LaunchError::Plan(error.to_string()))?;
    if tool == AgentLaunchTool::Gemini
        && environment
            .get("ZENTTY_GEMINI_HOOKS_DISABLED")
            .map(String::as_str)
            != Some("1")
    {
        let overlay = prepare_gemini_overlay(&environment, &cli_path)?;
        environment.insert(
            "ZENTTY_GEMINI_SETTINGS_OVERLAY".to_owned(),
            overlay.to_string_lossy().into_owned(),
        );
    }
    let plan = build_agent_launch_plan(
        tool,
        executable.to_string_lossy().into_owned(),
        arguments,
        &cli_path,
        &session_id,
        &environment,
    )
    .map_err(|error| LaunchError::Plan(error.to_string()))?;
    let mut integrated = !plan.set_environment.is_empty();
    if integrated
        && let Some(target) = tool.persistent_integration_target()
        && let Err(error) = install_integration(target)
    {
        integrated = false;
        if environment.get("ZENTTY_CLI_DEBUG").map(String::as_str) == Some("1") {
            eprintln!("zentty: {tool:?} hook installation failed; launching directly: {error}");
        }
    }
    let mut command = Command::new(&plan.executable_path);
    command.args(&plan.arguments);
    for name in plan.unset_environment {
        command.env_remove(name);
    }
    if integrated {
        command.envs(plan.set_environment);
        command.env(agent_pid_environment(tool), std::process::id().to_string());
    }
    Err(LaunchError::Exec(command.exec()))
}

const fn agent_pid_environment(tool: AgentLaunchTool) -> &'static str {
    match tool {
        AgentLaunchTool::Amp => "ZENTTY_AMP_PID",
        AgentLaunchTool::Claude => "ZENTTY_CLAUDE_PID",
        AgentLaunchTool::Codex => "ZENTTY_CODEX_PID",
        AgentLaunchTool::Cursor => "ZENTTY_CURSOR_PID",
        AgentLaunchTool::Droid => "ZENTTY_DROID_PID",
        AgentLaunchTool::Gemini => "ZENTTY_GEMINI_PID",
        AgentLaunchTool::Kimi => "ZENTTY_KIMI_PID",
        AgentLaunchTool::Grok => "ZENTTY_GROK_PID",
        AgentLaunchTool::Agy => "ZENTTY_AGY_PID",
        AgentLaunchTool::Hermes => "ZENTTY_HERMES_PID",
        AgentLaunchTool::Vibe => "ZENTTY_VIBE_PID",
    }
}

fn prepare_gemini_overlay(
    environment: &BTreeMap<String, String>,
    cli_path: &str,
) -> Result<PathBuf, LaunchError> {
    let socket = environment
        .get("ZENTTY_INSTANCE_SOCKET")
        .ok_or_else(|| LaunchError::Plan("ZENTTY_INSTANCE_SOCKET is missing".to_owned()))?;
    let runtime = Path::new(socket)
        .parent()
        .ok_or_else(|| LaunchError::Plan("Zentty runtime directory is invalid".to_owned()))?;
    let root = runtime.join("agent-overlays");
    ensure_private_directory(&root)?;
    let identifier = generate_pane_token().map_err(|error| LaunchError::Plan(error.to_string()))?;
    let directory = root.join(format!("gemini-{}", &identifier[..32]));
    fs::create_dir(&directory)
        .map_err(|error| LaunchError::Plan(format!("could not create Gemini overlay: {error}")))?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
        .map_err(|error| LaunchError::Plan(format!("could not protect Gemini overlay: {error}")))?;

    let source = environment
        .get("GEMINI_CLI_SYSTEM_SETTINGS_PATH")
        .map(PathBuf::from);
    let existing = source
        .as_deref()
        .filter(|path| path.is_file())
        .map(fs::read)
        .transpose()
        .map_err(|error| {
            LaunchError::Plan(format!("could not read existing Gemini settings: {error}"))
        })?;
    let settings = build_gemini_settings(existing.as_deref(), cli_path)
        .map_err(|error| LaunchError::Plan(error.to_string()))?;
    let temporary = directory.join("settings.json.tmp");
    let destination = directory.join("settings.json");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|error| LaunchError::Plan(format!("could not create Gemini settings: {error}")))?;
    file.write_all(&settings)
        .and_then(|()| file.sync_all())
        .map_err(|error| LaunchError::Plan(format!("could not write Gemini settings: {error}")))?;
    fs::rename(&temporary, &destination).map_err(|error| {
        LaunchError::Plan(format!("could not publish Gemini settings: {error}"))
    })?;
    Ok(destination)
}

fn ensure_private_directory(path: &Path) -> Result<(), LaunchError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.file_type().is_dir() || metadata.permissions().mode() & 0o077 != 0 {
                return Err(LaunchError::Plan(format!(
                    "Gemini overlay root is not a private directory: {}",
                    path.display()
                )));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path).map_err(|error| {
                LaunchError::Plan(format!("could not create Gemini overlay root: {error}"))
            })?;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|error| {
                LaunchError::Plan(format!("could not protect Gemini overlay root: {error}"))
            })?;
        }
        Err(error) => {
            return Err(LaunchError::Plan(format!(
                "could not inspect Gemini overlay root: {error}"
            )));
        }
    }
    Ok(())
}

/// Finds the real agent executable while excluding Zentty's wrapper directories.
///
/// # Errors
///
/// Returns an error when an explicit binary is invalid or no real executable
/// exists on the inherited path.
pub fn resolve_real_binary(
    tool: AgentLaunchTool,
    environment: &BTreeMap<String, String>,
) -> Result<PathBuf, LaunchError> {
    if let Some(explicit) = environment.get("ZENTTY_REAL_BINARY") {
        let path = PathBuf::from(explicit);
        if !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| tool.binary_names().contains(&name))
            || !is_executable_file(&path)
        {
            return Err(LaunchError::InvalidRealBinary(explicit.clone()));
        }
        return Ok(path);
    }
    let excluded = wrapper_directories(environment);
    for directory in std::env::split_paths(environment.get("PATH").map_or("", String::as_str)) {
        if excluded.contains(&directory) {
            continue;
        }
        for binary in tool.binary_names() {
            let candidate = directory.join(binary);
            if is_executable_file(&candidate) {
                return Ok(candidate);
            }
        }
    }
    Err(LaunchError::RealBinaryNotFound(
        tool.binary_name().to_owned(),
    ))
}

fn wrapper_directories(environment: &BTreeMap<String, String>) -> HashSet<PathBuf> {
    [
        "ZENTTY_ALL_WRAPPER_BIN_DIRS",
        "ZENTTY_WRAPPER_BIN_DIRS",
        "ZENTTY_WRAPPER_BIN_DIR",
    ]
    .into_iter()
    .filter_map(|name| environment.get(name))
    .flat_map(|value| std::env::split_paths(value))
    .collect()
}

fn is_executable_file(path: &Path) -> bool {
    fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

fn random_session_id() -> Result<String, crate::AgentIpcError> {
    let token = generate_pane_token()?;
    Ok(format!(
        "{}-{}-4{}-8{}-{}",
        &token[..8],
        &token[8..12],
        &token[13..16],
        &token[17..20],
        &token[20..32]
    ))
}
