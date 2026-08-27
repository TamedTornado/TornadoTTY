use crate::integrations::{build_kimi_overlay, install_kimi_modern_at};
use crate::{AgentIpcClient, generate_pane_token, install_integration};
use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::fs;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use zentty_core::{
    AgentLaunchAction, AgentLaunchTool, AtomicFileAction, AtomicFileStore,
    agent_launch_requires_bootstrap, build_agent_launch_plan, build_copilot_config,
    build_cursor_hooks, build_gemini_settings, build_small_harness_hooks,
};

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
    let mut executable = resolve_real_binary(tool, &environment)?;
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
    let mut launch_arguments = arguments.to_vec();
    prepare_remaining_launch_environment(
        tool,
        &mut launch_arguments,
        &mut environment,
        &cli_path,
        &executable,
    )?;
    if tool == AgentLaunchTool::OpenCode {
        executable = resolve_opencode_sibling(executable);
    }
    let plan = build_agent_launch_plan(
        tool,
        executable.to_string_lossy().into_owned(),
        &launch_arguments,
        &cli_path,
        &session_id,
        &environment,
    )
    .map_err(|error| LaunchError::Plan(error.to_string()))?;
    let mut integrated = !plan.set_environment.is_empty();
    let install_result = if integrated && tool == AgentLaunchTool::Kimi {
        match environment.get("ZENTTY_KIMI_VARIANT").map(String::as_str) {
            Some("modern") => environment
                .get("ZENTTY_KIMI_HOME")
                .map(PathBuf::from)
                .ok_or_else(|| "modern Kimi home is unavailable".to_owned())
                .and_then(|home| {
                    install_kimi_modern_at(&home.join("config.toml"), Path::new(&cli_path))
                })
                .map(Some),
            Some("legacy") => Ok(None),
            _ => install_integration("kimi-hooks").map(Some),
        }
    } else if integrated
        && tool != AgentLaunchTool::Cursor
        && let Some(target) = tool.persistent_integration_target()
    {
        install_integration(target).map(Some)
    } else {
        Ok(None)
    };
    if let Err(error) = install_result {
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
        command.envs(&plan.set_environment);
        if let Some(name) = agent_pid_environment(tool) {
            command.env(name, std::process::id().to_string());
        }
        send_pre_launch_actions(&plan.pre_launch_actions, &environment);
    }
    Err(LaunchError::Exec(command.exec()))
}

const fn agent_pid_environment(tool: AgentLaunchTool) -> Option<&'static str> {
    match tool {
        AgentLaunchTool::Amp => Some("ZENTTY_AMP_PID"),
        AgentLaunchTool::Claude => Some("ZENTTY_CLAUDE_PID"),
        AgentLaunchTool::Codex => Some("ZENTTY_CODEX_PID"),
        AgentLaunchTool::Copilot => Some("ZENTTY_COPILOT_PID"),
        AgentLaunchTool::Cursor => Some("ZENTTY_CURSOR_PID"),
        AgentLaunchTool::Droid => Some("ZENTTY_DROID_PID"),
        AgentLaunchTool::Gemini => Some("ZENTTY_GEMINI_PID"),
        AgentLaunchTool::OpenCode | AgentLaunchTool::Pi | AgentLaunchTool::Omp => None,
        AgentLaunchTool::Kimi => Some("ZENTTY_KIMI_PID"),
        AgentLaunchTool::Grok => Some("ZENTTY_GROK_PID"),
        AgentLaunchTool::Agy => Some("ZENTTY_AGY_PID"),
        AgentLaunchTool::Hermes => Some("ZENTTY_HERMES_PID"),
        AgentLaunchTool::Vibe => Some("ZENTTY_VIBE_PID"),
        AgentLaunchTool::SmallHarness => Some("ZENTTY_SMALL_HARNESS_PID"),
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

fn prepare_remaining_launch_environment(
    tool: AgentLaunchTool,
    arguments: &mut Vec<String>,
    environment: &mut BTreeMap<String, String>,
    cli_path: &str,
    executable: &Path,
) -> Result<(), LaunchError> {
    if !agent_launch_requires_bootstrap(tool, arguments, environment) {
        return Ok(());
    }
    match tool {
        AgentLaunchTool::Copilot => prepare_copilot_overlay(arguments, environment, cli_path),
        AgentLaunchTool::Cursor => prepare_cursor_overlay(environment, cli_path),
        AgentLaunchTool::OpenCode => prepare_opencode_overlay(environment),
        AgentLaunchTool::Pi => select_extension(
            environment,
            "pi/extensions/zentty-pi-zentty.js",
            "ZENTTY_PI_EXTENSION",
        ),
        AgentLaunchTool::Omp => select_extension(
            environment,
            "omp/extensions/zentty-omp-zentty.js",
            "ZENTTY_OMP_EXTENSION",
        ),
        AgentLaunchTool::SmallHarness => prepare_small_harness_overlay(environment, cli_path),
        AgentLaunchTool::Kimi => prepare_kimi_launch(executable, arguments, environment, cli_path),
        AgentLaunchTool::Amp
        | AgentLaunchTool::Claude
        | AgentLaunchTool::Codex
        | AgentLaunchTool::Droid
        | AgentLaunchTool::Gemini
        | AgentLaunchTool::Grok
        | AgentLaunchTool::Agy
        | AgentLaunchTool::Hermes
        | AgentLaunchTool::Vibe => Ok(()),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KimiVariant {
    Modern,
    Legacy,
}

fn prepare_kimi_launch(
    executable: &Path,
    arguments: &mut Vec<String>,
    environment: &mut BTreeMap<String, String>,
    cli_path: &str,
) -> Result<(), LaunchError> {
    let variant = probe_kimi_variant(executable, environment)?;
    environment.insert(
        "ZENTTY_KIMI_VARIANT".to_owned(),
        match variant {
            KimiVariant::Modern => "modern",
            KimiVariant::Legacy => "legacy",
        }
        .to_owned(),
    );
    if variant == KimiVariant::Modern {
        let configured_home = environment
            .get("KIMI_CODE_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        let stale_overlay = configured_home
            .as_deref()
            .is_some_and(is_kimi_runtime_overlay);
        let home = match configured_home {
            Some(home) if !stale_overlay => home,
            _ => environment
                .get("HOME")
                .filter(|value| !value.is_empty())
                .map(|value| Path::new(value).join(".kimi-code"))
                .ok_or_else(|| LaunchError::Plan("modern Kimi home is unavailable".to_owned()))?,
        };
        if stale_overlay {
            environment.insert("ZENTTY_KIMI_UNSET_CODE_HOME".to_owned(), "1".to_owned());
        }
        if let Err(error) = canonicalize_kimi_session_index(&home)
            && environment.get("ZENTTY_CLI_DEBUG").map(String::as_str) == Some("1")
        {
            eprintln!("zentty: Kimi session index repair skipped: {error}");
        }
        environment.insert(
            "ZENTTY_KIMI_HOME".to_owned(),
            home.to_string_lossy().into_owned(),
        );
        return Ok(());
    }

    let source = take_kimi_config_argument(arguments).unwrap_or_else(|| {
        KimiConfigSource::File(
            environment
                .get("KIMI_SHARE_DIR")
                .filter(|value| !value.is_empty())
                .map(|path| Path::new(path).join("config.toml"))
                .or_else(|| {
                    environment
                        .get("HOME")
                        .filter(|value| !value.is_empty())
                        .map(|home| Path::new(home).join(".kimi/config.toml"))
                })
                .unwrap_or_default(),
        )
    });
    let existing = match source {
        KimiConfigSource::File(path) if path.is_file() => read_bounded_text(&path, "Kimi config")?,
        KimiConfigSource::File(_) => String::new(),
        KimiConfigSource::Inline(value) => value,
    };
    let merged = build_kimi_overlay(&existing, Path::new(cli_path)).map_err(LaunchError::Plan)?;
    let directory = create_private_tool_directory(environment, "kimi")?;
    let config = directory.join("config.toml");
    write_private_file(&config, merged.as_bytes(), "Kimi config overlay")?;
    arguments.splice(
        0..0,
        [
            "--config-file".to_owned(),
            config.to_string_lossy().into_owned(),
        ],
    );
    Ok(())
}

fn is_kimi_runtime_overlay(path: &Path) -> bool {
    let components = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>();
    components.windows(2).any(|pair| {
        pair[0] == "agent-overlays" && (pair[1].starts_with("kimi-") || pair[1] == "kimi")
    })
}

fn canonicalize_kimi_session_index(home: &Path) -> Result<(), LaunchError> {
    const MAX_BYTES: usize = 1024 * 1024;
    let index = home.join("session_index.jsonl");
    AtomicFileStore::new(&index, MAX_BYTES)
        .transaction(|bytes| {
            let Some(bytes) = bytes else {
                return Ok(AtomicFileAction::ReadOnly(()));
            };
            let source = std::str::from_utf8(bytes)
                .map_err(|_| "Kimi session index is not UTF-8".to_owned())?;
            let mut changed = false;
            let lines = source
                .split('\n')
                .map(|line| {
                    canonicalize_kimi_session_line(line, home).map(|(line, line_changed)| {
                        changed |= line_changed;
                        line
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            if changed {
                Ok(AtomicFileAction::Replace {
                    bytes: lines.join("\n").into_bytes(),
                    value: (),
                })
            } else {
                Ok(AtomicFileAction::ReadOnly(()))
            }
        })
        .map(|_| ())
        .map_err(|error| LaunchError::Plan(error.to_string()))
}

fn canonicalize_kimi_session_line(line: &str, home: &Path) -> Result<(String, bool), String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok((line.to_owned(), false));
    }
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return Ok((line.to_owned(), false));
    };
    let Some(object) = value.as_object_mut() else {
        return Ok((line.to_owned(), false));
    };
    if !object
        .get("sessionId")
        .is_some_and(serde_json::Value::is_string)
        && !object.get("id").is_some_and(serde_json::Value::is_string)
    {
        return Ok((line.to_owned(), false));
    }
    let Some(session_dir) = object.get("sessionDir").and_then(serde_json::Value::as_str) else {
        return Ok((line.to_owned(), false));
    };
    let session_path = Path::new(session_dir);
    if session_path == home || session_path.starts_with(home) {
        return Ok((line.to_owned(), false));
    }
    let components = session_path.components().collect::<Vec<_>>();
    let Some(sessions_index) = components
        .iter()
        .position(|component| component.as_os_str() == "sessions")
    else {
        return Ok((line.to_owned(), false));
    };
    let suffix = components[sessions_index..]
        .iter()
        .fold(PathBuf::new(), |path, component| {
            path.join(component.as_os_str())
        });
    let canonical = home.join(suffix);
    if !canonical.exists() {
        return Ok((line.to_owned(), false));
    }
    object.insert(
        "sessionDir".to_owned(),
        serde_json::Value::String(canonical.to_string_lossy().into_owned()),
    );
    serde_json::to_string(&value)
        .map(|line| (line, true))
        .map_err(|error| format!("could not serialize Kimi session index: {error}"))
}

enum KimiConfigSource {
    File(PathBuf),
    Inline(String),
}

fn take_kimi_config_argument(arguments: &mut Vec<String>) -> Option<KimiConfigSource> {
    let mut selected = None;
    let mut retained = Vec::with_capacity(arguments.len());
    let mut index = 0;
    while index < arguments.len() {
        if arguments[index] == "--config-file" && index + 1 < arguments.len() {
            selected = Some(KimiConfigSource::File(PathBuf::from(&arguments[index + 1])));
            index += 2;
        } else if let Some(value) = arguments[index].strip_prefix("--config-file=") {
            selected = Some(KimiConfigSource::File(PathBuf::from(value)));
            index += 1;
        } else if arguments[index] == "--config" && index + 1 < arguments.len() {
            selected = Some(KimiConfigSource::Inline(arguments[index + 1].clone()));
            index += 2;
        } else if let Some(value) = arguments[index].strip_prefix("--config=") {
            selected = Some(KimiConfigSource::Inline(value.to_owned()));
            index += 1;
        } else {
            retained.push(arguments[index].clone());
            index += 1;
        }
    }
    *arguments = retained;
    selected
}

fn read_bounded_text(path: &Path, label: &str) -> Result<String, LaunchError> {
    const MAX_BYTES: u64 = 1024 * 1024;
    let file = fs::File::open(path)
        .map_err(|error| LaunchError::Plan(format!("could not read {label}: {error}")))?;
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| LaunchError::Plan(format!("could not read {label}: {error}")))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_BYTES {
        return Err(LaunchError::Plan(format!("{label} exceeds 1 MiB")));
    }
    String::from_utf8(bytes).map_err(|_| LaunchError::Plan(format!("{label} is not UTF-8")))
}

fn probe_kimi_variant(
    executable: &Path,
    environment: &BTreeMap<String, String>,
) -> Result<KimiVariant, LaunchError> {
    const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
    let mut child = Command::new(executable)
        .arg("--help")
        .envs(environment)
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| LaunchError::Plan(format!("could not probe Kimi: {error}")))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| LaunchError::Plan("Kimi probe stdout was unavailable".to_owned()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| LaunchError::Plan("Kimi probe stderr was unavailable".to_owned()))?;
    let stdout_reader = std::thread::spawn(move || drain_bounded(stdout, MAX_OUTPUT_BYTES));
    let stderr_reader = std::thread::spawn(move || drain_bounded(stderr, MAX_OUTPUT_BYTES));
    let timeout = environment
        .get("ZENTTY_KIMI_PROBE_TIMEOUT_MS")
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|milliseconds| (1..=10_000).contains(milliseconds))
        .map_or_else(|| Duration::from_secs(10), Duration::from_millis);
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| LaunchError::Plan(format!("could not wait for Kimi probe: {error}")))?
        {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(LaunchError::Plan("Kimi variant probe timed out".to_owned()));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut output = join_kimi_probe_reader(stdout_reader, "stdout")?;
    output.extend(join_kimi_probe_reader(stderr_reader, "stderr")?);
    if !status.success() {
        return Err(LaunchError::Plan(format!(
            "Kimi variant probe exited with {status}"
        )));
    }
    let help = String::from_utf8(output)
        .map_err(|_| LaunchError::Plan("Kimi variant probe output was not UTF-8".to_owned()))?;
    Ok(if strip_ansi(&help).contains("--config-file") {
        KimiVariant::Legacy
    } else {
        KimiVariant::Modern
    })
}

fn join_kimi_probe_reader(
    reader: std::thread::JoinHandle<std::io::Result<Vec<u8>>>,
    stream: &str,
) -> Result<Vec<u8>, LaunchError> {
    reader
        .join()
        .map_err(|_| LaunchError::Plan(format!("Kimi {stream} reader failed")))?
        .map_err(|error| LaunchError::Plan(format!("could not read Kimi {stream}: {error}")))
}

fn drain_bounded(mut reader: impl Read, limit: usize) -> std::io::Result<Vec<u8>> {
    let mut retained = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            return Ok(retained);
        }
        let remaining = limit.saturating_sub(retained.len());
        retained.extend_from_slice(&buffer[..count.min(remaining)]);
    }
}

fn strip_ansi(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut characters = value.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '\u{1b}' {
            output.push(character);
            continue;
        }
        if characters.next_if_eq(&'[').is_some() {
            for character in characters.by_ref() {
                if ('@'..='~').contains(&character) {
                    break;
                }
            }
        } else {
            let _ = characters.next();
        }
    }
    output
}

fn prepare_cursor_overlay(
    environment: &mut BTreeMap<String, String>,
    cli_path: &str,
) -> Result<(), LaunchError> {
    let directory = create_private_tool_directory(environment, "cursor")?;
    let config = directory.join(".cursor");
    fs::create_dir(&config)
        .and_then(|()| fs::set_permissions(&config, fs::Permissions::from_mode(0o700)))
        .map_err(|error| LaunchError::Plan(format!("could not create Cursor overlay: {error}")))?;
    let source = environment
        .get("CURSOR_CONFIG_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            environment
                .get("HOME")
                .filter(|value| !value.is_empty())
                .map(|home| Path::new(home).join(".cursor"))
        });
    if let Some(source) = source.filter(|path| path.is_dir()) {
        for entry in fs::read_dir(&source)
            .map_err(|error| LaunchError::Plan(format!("could not read Cursor config: {error}")))?
        {
            let entry = entry.map_err(|error| {
                LaunchError::Plan(format!("could not inspect Cursor config: {error}"))
            })?;
            if entry.file_name() == "hooks.json" {
                continue;
            }
            std::os::unix::fs::symlink(entry.path(), config.join(entry.file_name())).map_err(
                |error| {
                    LaunchError::Plan(format!(
                        "could not project Cursor config entry {}: {error}",
                        entry.path().display()
                    ))
                },
            )?;
        }
    }
    let hooks =
        build_cursor_hooks(cli_path).map_err(|error| LaunchError::Plan(error.to_string()))?;
    write_private_file(&config.join("hooks.json"), &hooks, "Cursor hooks")?;
    environment.insert(
        "ZENTTY_CURSOR_CONFIG_OVERLAY".to_owned(),
        config.to_string_lossy().into_owned(),
    );
    Ok(())
}

fn prepare_copilot_overlay(
    arguments: &[String],
    environment: &mut BTreeMap<String, String>,
    cli_path: &str,
) -> Result<(), LaunchError> {
    let source = copilot_config_override(arguments)
        .or_else(|| environment.get("COPILOT_HOME").cloned())
        .or_else(|| {
            environment
                .get("HOME")
                .map(|home| format!("{home}/.copilot"))
        })
        .ok_or_else(|| LaunchError::Plan("Copilot source home is unavailable".to_owned()))?;
    let directory = create_private_tool_directory(environment, "copilot")?;
    let overlay_home = directory.join("home");
    fs::create_dir(&overlay_home)
        .and_then(|()| fs::set_permissions(&overlay_home, fs::Permissions::from_mode(0o700)))
        .map_err(|error| LaunchError::Plan(format!("could not create Copilot overlay: {error}")))?;
    let source_home = Path::new(&source);
    if source_home.is_dir() {
        for entry in fs::read_dir(source_home).map_err(|error| {
            LaunchError::Plan(format!("could not read Copilot source home: {error}"))
        })? {
            let entry = entry.map_err(|error| {
                LaunchError::Plan(format!("could not inspect Copilot source home: {error}"))
            })?;
            if entry.file_name() == "config.json" {
                continue;
            }
            std::os::unix::fs::symlink(entry.path(), overlay_home.join(entry.file_name()))
                .map_err(|error| {
                    LaunchError::Plan(format!("could not link Copilot source entry: {error}"))
                })?;
        }
    }
    let source_config = source_home.join("config.json");
    let existing = source_config
        .is_file()
        .then(|| fs::read(&source_config))
        .transpose()
        .map_err(|error| LaunchError::Plan(format!("could not read Copilot config: {error}")))?;
    let merged = build_copilot_config(existing.as_deref(), cli_path);
    let config = match (merged, existing) {
        (Ok(config), _) => config,
        (Err(_), Some(existing)) => existing,
        (Err(error), None) => return Err(LaunchError::Plan(error.to_string())),
    };
    write_private_file(&overlay_home.join("config.json"), &config, "Copilot config")?;
    environment.insert(
        "ZENTTY_COPILOT_HOME_OVERLAY".to_owned(),
        overlay_home.to_string_lossy().into_owned(),
    );
    Ok(())
}

fn prepare_opencode_overlay(environment: &mut BTreeMap<String, String>) -> Result<(), LaunchError> {
    let source = opencode_source_config(environment);
    environment.insert(
        "ZENTTY_OPENCODE_BASE_CONFIG_DIR".to_owned(),
        source
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default(),
    );
    let plugin = resource_path(environment, "opencode/plugins/zentty-opencode-zentty.js")?;
    if !plugin.is_file() {
        return Ok(());
    }
    let directory = create_private_opencode_directory(environment)?;
    let overlay = directory.join("config");
    fs::create_dir(&overlay)
        .and_then(|()| fs::set_permissions(&overlay, fs::Permissions::from_mode(0o700)))
        .map_err(|error| {
            LaunchError::Plan(format!("could not create OpenCode overlay: {error}"))
        })?;
    if let Some(source) = source.as_deref().filter(|path| path.is_dir()) {
        copy_directory_contents(source, &overlay)?;
    }
    let plugins = overlay.join("plugins");
    fs::create_dir_all(&plugins).map_err(|error| {
        LaunchError::Plan(format!(
            "could not create OpenCode plugin directory: {error}"
        ))
    })?;
    fs::set_permissions(&plugins, fs::Permissions::from_mode(0o700)).map_err(|error| {
        LaunchError::Plan(format!(
            "could not protect OpenCode plugin directory: {error}"
        ))
    })?;
    let plugin_destination = plugins.join("zentty-opencode-zentty.js");
    let bytes = fs::read(&plugin)
        .map_err(|error| LaunchError::Plan(format!("could not read OpenCode plugin: {error}")))?;
    write_private_file(&plugin_destination, &bytes, "OpenCode plugin")?;
    environment.insert(
        "ZENTTY_OPENCODE_CONFIG_OVERLAY".to_owned(),
        overlay.to_string_lossy().into_owned(),
    );
    if let Err(error) = apply_opencode_theme_sync(&overlay, environment)
        && environment.get("ZENTTY_CLI_DEBUG").map(String::as_str) == Some("1")
    {
        eprintln!("zentty: OpenCode theme synchronization skipped: {error}");
    }
    Ok(())
}

fn create_private_opencode_directory(
    environment: &BTreeMap<String, String>,
) -> Result<PathBuf, LaunchError> {
    let socket = environment
        .get("ZENTTY_INSTANCE_SOCKET")
        .ok_or_else(|| LaunchError::Plan("ZENTTY_INSTANCE_SOCKET is missing".to_owned()))?;
    let runtime = Path::new(socket)
        .parent()
        .ok_or_else(|| LaunchError::Plan("Zentty runtime directory is invalid".to_owned()))?;
    let token = environment
        .get("ZENTTY_PANE_TOKEN")
        .filter(|token| token.len() >= 32 && token.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| LaunchError::Plan("ZENTTY_PANE_TOKEN is invalid".to_owned()))?;
    let root = runtime.join("agent-overlays");
    ensure_private_directory(&root)?;
    let directory = root.join(format!("opencode-{}", &token[..32]));
    match fs::symlink_metadata(&directory) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(LaunchError::Plan(
                "OpenCode overlay path is not a private directory".to_owned(),
            ));
        }
        Ok(_) => fs::remove_dir_all(&directory).map_err(|error| {
            LaunchError::Plan(format!("could not reset OpenCode overlay: {error}"))
        })?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(LaunchError::Plan(format!(
                "could not inspect OpenCode overlay: {error}"
            )));
        }
    }
    fs::create_dir(&directory)
        .and_then(|()| fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)))
        .map_err(|error| {
            LaunchError::Plan(format!("could not create OpenCode overlay: {error}"))
        })?;
    Ok(directory)
}

fn apply_opencode_theme_sync(
    overlay: &Path,
    environment: &BTreeMap<String, String>,
) -> Result<(), LaunchError> {
    const MAX_THEME_BYTES: u64 = 64 * 1024;
    let Some(source) = environment
        .get("ZENTTY_OPENCODE_SYNC_THEME_FILE")
        .filter(|value| !value.is_empty())
        .map(Path::new)
    else {
        return Ok(());
    };
    let metadata = fs::symlink_metadata(source).map_err(|error| {
        LaunchError::Plan(format!(
            "could not inspect synchronized OpenCode theme: {error}"
        ))
    })?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > MAX_THEME_BYTES
    {
        return Err(LaunchError::Plan(
            "synchronized OpenCode theme is not a bounded regular file".to_owned(),
        ));
    }
    let theme = fs::read(source).map_err(|error| {
        LaunchError::Plan(format!(
            "could not read synchronized OpenCode theme: {error}"
        ))
    })?;
    serde_json::from_slice::<serde_json::Value>(&theme)
        .ok()
        .filter(serde_json::Value::is_object)
        .ok_or_else(|| {
            LaunchError::Plan("synchronized OpenCode theme is invalid JSON".to_owned())
        })?;

    let tui_path = overlay.join("tui.json");
    let mut tui = match fs::read(&tui_path) {
        Ok(bytes) => serde_json::from_slice::<serde_json::Value>(&bytes)
            .ok()
            .and_then(|value| value.as_object().cloned())
            .ok_or_else(|| {
                LaunchError::Plan(
                    "OpenCode tui.json is malformed; refusing to overwrite it".to_owned(),
                )
            })?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => serde_json::Map::new(),
        Err(error) => {
            return Err(LaunchError::Plan(format!(
                "could not read OpenCode tui.json: {error}"
            )));
        }
    };
    let themes = overlay.join("themes");
    if !themes.exists() {
        fs::create_dir(&themes)
            .and_then(|()| fs::set_permissions(&themes, fs::Permissions::from_mode(0o700)))
            .map_err(|error| {
                LaunchError::Plan(format!(
                    "could not create OpenCode theme directory: {error}"
                ))
            })?;
    }
    write_private_replacing_file(
        &themes.join("zentty-synced.json"),
        &theme,
        "synchronized OpenCode theme",
    )?;
    tui.insert(
        "$schema".to_owned(),
        serde_json::Value::String("https://opencode.ai/tui.json".to_owned()),
    );
    tui.insert(
        "theme".to_owned(),
        serde_json::Value::String("zentty-synced".to_owned()),
    );
    let bytes = serde_json::to_vec_pretty(&tui).map_err(|error| {
        LaunchError::Plan(format!("could not encode OpenCode TUI config: {error}"))
    })?;
    write_private_replacing_file(&tui_path, &bytes, "OpenCode TUI config")?;
    Ok(())
}

fn prepare_small_harness_overlay(
    environment: &mut BTreeMap<String, String>,
    cli_path: &str,
) -> Result<(), LaunchError> {
    let directory = create_private_tool_directory(environment, "small-harness")?;
    let path = directory.join("managed-hooks.json");
    let bytes = build_small_harness_hooks(cli_path)
        .map_err(|error| LaunchError::Plan(error.to_string()))?;
    write_private_file(&path, &bytes, "Small Harness hooks")?;
    environment.insert(
        "ZENTTY_SMALL_HARNESS_HOOKS_FILE".to_owned(),
        path.to_string_lossy().into_owned(),
    );
    Ok(())
}

fn select_extension(
    environment: &mut BTreeMap<String, String>,
    relative: &str,
    key: &str,
) -> Result<(), LaunchError> {
    let path = resource_path(environment, relative)?;
    if path.is_file() {
        environment.insert(key.to_owned(), path.to_string_lossy().into_owned());
    }
    Ok(())
}

fn create_private_tool_directory(
    environment: &BTreeMap<String, String>,
    tool: &str,
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
    let directory = root.join(format!("{tool}-{}", &identifier[..32]));
    fs::create_dir(&directory)
        .and_then(|()| fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)))
        .map_err(|error| LaunchError::Plan(format!("could not create {tool} overlay: {error}")))?;
    Ok(directory)
}

fn write_private_file(path: &Path, bytes: &[u8], label: &str) -> Result<(), LaunchError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| LaunchError::Plan(format!("could not create {label}: {error}")))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| LaunchError::Plan(format!("could not write {label}: {error}")))
}

fn write_private_replacing_file(path: &Path, bytes: &[u8], label: &str) -> Result<(), LaunchError> {
    if let Ok(metadata) = fs::symlink_metadata(path)
        && (metadata.file_type().is_symlink() || !metadata.is_file())
    {
        return Err(LaunchError::Plan(format!(
            "refusing to replace non-regular {label}"
        )));
    }
    let parent = path
        .parent()
        .ok_or_else(|| LaunchError::Plan(format!("{label} path has no parent")))?;
    let temporary = parent.join(format!(".zentty-{}.tmp", std::process::id()));
    let _ = fs::remove_file(&temporary);
    write_private_file(&temporary, bytes, label)?;
    fs::rename(&temporary, path)
        .map_err(|error| LaunchError::Plan(format!("could not publish {label}: {error}")))
}

fn resource_path(
    environment: &BTreeMap<String, String>,
    relative: &str,
) -> Result<PathBuf, LaunchError> {
    if let Some(root) = environment.get("ZENTTY_RESOURCE_ROOT") {
        return Ok(Path::new(root).join(relative));
    }
    let executable = std::env::current_exe().map_err(|error| {
        LaunchError::Plan(format!("could not locate Zentty resources: {error}"))
    })?;
    for ancestor in executable.ancestors().skip(1) {
        let candidate = ancestor.join("share/zentty").join(relative);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    let prefix = executable
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| LaunchError::Plan("Zentty installation prefix is invalid".to_owned()))?;
    Ok(prefix.join("share/zentty").join(relative))
}

fn copilot_config_override(arguments: &[String]) -> Option<String> {
    let mut selected = None;
    let mut index = 0;
    while index < arguments.len() {
        if arguments[index] == "--config-dir" && index + 1 < arguments.len() {
            selected = Some(arguments[index + 1].clone());
            index += 2;
        } else if let Some(value) = arguments[index].strip_prefix("--config-dir=") {
            selected = Some(value.to_owned());
            index += 1;
        } else {
            index += 1;
        }
    }
    selected.filter(|value| !value.is_empty())
}

fn opencode_source_config(environment: &BTreeMap<String, String>) -> Option<PathBuf> {
    for key in ["ZENTTY_OPENCODE_BASE_CONFIG_DIR", "OPENCODE_CONFIG_DIR"] {
        if let Some(value) = environment.get(key).filter(|value| !value.is_empty()) {
            return Some(PathBuf::from(value));
        }
    }
    if let Some(value) = environment
        .get("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
    {
        let path = Path::new(value).join("opencode");
        if path.exists() {
            return Some(path);
        }
    }
    environment
        .get("HOME")
        .filter(|value| !value.is_empty())
        .map(|home| Path::new(home).join(".config/opencode"))
}

fn copy_directory_contents(source: &Path, destination: &Path) -> Result<(), LaunchError> {
    for entry in fs::read_dir(source)
        .map_err(|error| LaunchError::Plan(format!("could not read OpenCode config: {error}")))?
    {
        let entry = entry.map_err(|error| {
            LaunchError::Plan(format!("could not inspect OpenCode config: {error}"))
        })?;
        let metadata = entry.file_type().map_err(|error| {
            LaunchError::Plan(format!("could not inspect OpenCode config entry: {error}"))
        })?;
        if metadata.is_symlink() {
            return Err(LaunchError::Plan(format!(
                "OpenCode source config contains a symlink: {}",
                entry.path().display()
            )));
        }
        let target = destination.join(entry.file_name());
        if metadata.is_dir() {
            fs::create_dir(&target).map_err(|error| {
                LaunchError::Plan(format!(
                    "could not create OpenCode overlay directory: {error}"
                ))
            })?;
            fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).map_err(|error| {
                LaunchError::Plan(format!(
                    "could not protect OpenCode overlay directory: {error}"
                ))
            })?;
            copy_directory_contents(&entry.path(), &target)?;
        } else if metadata.is_file() {
            let bytes = fs::read(entry.path()).map_err(|error| {
                LaunchError::Plan(format!("could not read OpenCode config entry: {error}"))
            })?;
            write_private_file(&target, &bytes, "OpenCode config entry")?;
        }
    }
    Ok(())
}

fn resolve_opencode_sibling(executable: PathBuf) -> PathBuf {
    if executable.file_name().and_then(|name| name.to_str()) != Some("opencode") {
        return executable;
    }
    for candidate in [Some(executable.clone()), executable.canonicalize().ok()]
        .into_iter()
        .flatten()
    {
        let sibling = candidate.with_file_name(".opencode");
        if is_executable_file(&sibling) {
            return sibling;
        }
    }
    executable
}

fn send_pre_launch_actions(actions: &[AgentLaunchAction], environment: &BTreeMap<String, String>) {
    let Some(socket) = environment.get("ZENTTY_INSTANCE_SOCKET") else {
        return;
    };
    let Some(token) = environment.get("ZENTTY_PANE_TOKEN") else {
        return;
    };
    for action in actions {
        let event = action
            .standard_input
            .replace("__ZENTTY_SELF_PID__", &std::process::id().to_string());
        if let Err(error) = AgentIpcClient::send_event(socket, token, event.as_bytes(), None)
            && environment.get("ZENTTY_CLI_DEBUG").map(String::as_str) == Some("1")
        {
            eprintln!("zentty: pre-launch agent event was not delivered: {error}");
        }
    }
}

fn ensure_private_directory(path: &Path) -> Result<(), LaunchError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.file_type().is_dir() || metadata.permissions().mode() & 0o077 != 0 {
                return Err(LaunchError::Plan(format!(
                    "agent overlay root is not a private directory: {}",
                    path.display()
                )));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path).map_err(|error| {
                LaunchError::Plan(format!("could not create agent overlay root: {error}"))
            })?;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|error| {
                LaunchError::Plan(format!("could not protect agent overlay root: {error}"))
            })?;
        }
        Err(error) => {
            return Err(LaunchError::Plan(format!(
                "could not inspect agent overlay root: {error}"
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
