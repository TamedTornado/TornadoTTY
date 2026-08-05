use crate::generate_pane_token;
use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use zentty_core::{AgentLaunchTool, build_agent_launch_plan};

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
    let environment = std::env::vars().collect::<BTreeMap<_, _>>();
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
    let plan = build_agent_launch_plan(
        tool,
        executable.to_string_lossy().into_owned(),
        arguments,
        &cli_path,
        &session_id,
        &environment,
    )
    .map_err(|error| LaunchError::Plan(error.to_string()))?;
    let mut command = Command::new(&plan.executable_path);
    command.args(&plan.arguments);
    for name in plan.unset_environment {
        command.env_remove(name);
    }
    command.envs(plan.set_environment);
    command.env(
        match tool {
            AgentLaunchTool::Claude => "ZENTTY_CLAUDE_PID",
            AgentLaunchTool::Codex => "ZENTTY_CODEX_PID",
        },
        std::process::id().to_string(),
    );
    Err(LaunchError::Exec(command.exec()))
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
        if path.file_name().and_then(|name| name.to_str()) != Some(tool.binary_name())
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
        let candidate = directory.join(tool.binary_name());
        if is_executable_file(&candidate) {
            return Ok(candidate);
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
