#![forbid(unsafe_code)]

use serde_json::Value;
use std::io::{BufReader, IsTerminal, Read, Write};
use std::process::ExitCode;
use std::process::{Command as ProcessCommand, Stdio};
use std::time::Instant;
use zentty_agent_ipc::{
    AgentIpcClient, CliProductCommand, ServerCommand, discover_instances, install_integration,
    launch_agent, parse_product_cli, render_application_result, uninstall_integration,
};
use zentty_core::{
    AgentEvent, AgentTarget, adapt_agy_hook, adapt_claude_hook, adapt_codex_hook,
    adapt_codex_notify, adapt_copilot_hook, adapt_cursor_hook, adapt_droid_hook, adapt_gemini_hook,
    adapt_grok_hook, adapt_hermes_hook, adapt_kimi_hook, adapt_small_harness_hook, adapt_vibe_hook,
    detect_server_urls,
};
use zentty_tmux_compat::{
    Command, Invocation, TmuxCompatRequest, WAIT_POLL_INTERVAL, WaitForAction,
};

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
    let raw_arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let installed_hook = raw_arguments
        .first()
        .and_then(|command| match command.as_str() {
            "agy-hook" | "hermes-hook" => Some(command.clone()),
            _ => None,
        });
    let raw_arguments = normalize_hook_subcommand(raw_arguments);
    if let Some(command) = parse_product_cli(&raw_arguments).map_err(|error| error.to_string())? {
        return run_product_cli(command);
    }
    let mut arguments = raw_arguments.into_iter();
    let command = arguments.next();
    if command.as_deref() == Some("launch") {
        return run_launch(arguments);
    }
    if command.as_deref() == Some("__tmux-compat") {
        let arguments = arguments.collect::<Vec<_>>();
        return run_tmux_compat(&arguments);
    }
    if command.as_deref() == Some("codex-notify") {
        return run_codex_notify(arguments.next());
    }
    if command.as_deref() == Some("server") {
        return run_server(&arguments.collect::<Vec<_>>());
    }
    if command.as_deref() != Some("ipc") {
        return Err(
            "usage: zentty ipc <agent-event|agent-signal|agent-status> [arguments...]".to_owned(),
        );
    }
    let subcommand = arguments.next();
    if subcommand.as_deref() == Some("agent-signal") {
        return run_agent_signal(&arguments.collect::<Vec<_>>());
    }
    if subcommand.as_deref() == Some("agent-status") {
        let mut signal = vec!["lifecycle".to_owned()];
        signal.extend(arguments);
        return run_agent_signal(&signal);
    }
    if subcommand.as_deref() != Some("agent-event") {
        return Err(
            "usage: zentty ipc <agent-event|agent-signal|agent-status> [arguments...]".to_owned(),
        );
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
    let events = adapt_agent_events(adapter, &input)?;
    if adapter == Some("copilot") && events.is_empty() {
        return Ok(());
    }
    if adapter == Some("gemini")
        && (events.is_empty()
            || std::env::var_os("ZENTTY_INSTANCE_SOCKET").is_none()
            || std::env::var_os("ZENTTY_PANE_TOKEN").is_none())
    {
        println!("{{}}");
        return Ok(());
    }
    if let Some(command) = installed_hook.as_deref()
        && (events.is_empty()
            || std::env::var_os("ZENTTY_INSTANCE_SOCKET").is_none()
            || std::env::var_os("ZENTTY_PANE_TOKEN").is_none())
    {
        println!(
            "{}",
            installed_hook_response(command, default_event.map(String::as_str))
        );
        return Ok(());
    }
    let socket = std::env::var("ZENTTY_INSTANCE_SOCKET")
        .map_err(|_| "ZENTTY_INSTANCE_SOCKET is missing".to_owned())?;
    let token = require_pane_credential()?;
    for event in events {
        let bytes = serde_json::to_vec(&event).map_err(|error| error.to_string())?;
        AgentIpcClient::send_event(&socket, &token, &bytes, claimed_target_from_environment())
            .map_err(|error| error.to_string())?;
    }
    if adapter == Some("gemini") {
        println!("{{}}");
    } else if let Some(command) = installed_hook.as_deref() {
        println!(
            "{}",
            installed_hook_response(command, default_event.map(String::as_str))
        );
    }
    Ok(())
}

fn run_launch(mut arguments: impl Iterator<Item = String>) -> Result<(), String> {
    let tool = arguments.next().ok_or_else(|| {
        "usage: zentty launch <amp|claude|codex|copilot|cursor|droid|gemini|opencode|pi|omp|kimi|grok|agy|hermes|vibe|small-harness> [arguments...]".to_owned()
    })?;
    launch_agent(&tool, &arguments.collect::<Vec<_>>()).map_err(|error| error.to_string())
}

fn run_agent_signal(arguments: &[String]) -> Result<(), String> {
    // Shell hooks are deliberately non-invasive outside a live Zentty pane.
    // Validate and deliver only when the complete authenticated environment is
    // present; otherwise preserve normal shell startup and command behavior.
    let Some(target) = claimed_target_from_environment() else {
        return Ok(());
    };
    let Ok(socket) = std::env::var("ZENTTY_INSTANCE_SOCKET") else {
        return Ok(());
    };
    let Ok(Some(token)) = pane_credential_from_environment() else {
        return Ok(());
    };
    let request = zentty_agent_ipc::ApplicationRequest::new(
        zentty_agent_ipc::ApplicationScope::Pane,
        "shell-signal",
        arguments.to_vec(),
    )
    .map_err(|error| error.to_string())?;
    let reply = AgentIpcClient::send_application(socket, &token, &request, Some(target))
        .map_err(|error| error.to_string())?;
    if let Some(error) = reply.error() {
        return Err(format!("{}: {}", error.code(), error.message()));
    }
    Ok(())
}

fn adapt_agent_events(adapter: Option<&str>, input: &[u8]) -> Result<Vec<AgentEvent>, String> {
    let events = match adapter {
        Some("codex") => adapt_codex_hook(input, environment_pid("ZENTTY_CODEX_PID")),
        Some("small-harness") => {
            adapt_small_harness_hook(input, environment_pid("ZENTTY_SMALL_HARNESS_PID"))
        }
        Some("claude") => adapt_claude_hook(input, environment_pid("ZENTTY_CLAUDE_PID")),
        Some("copilot") => adapt_copilot_hook(input, None, environment_pid("ZENTTY_COPILOT_PID")),
        Some("gemini") => adapt_gemini_hook(input, environment_pid("ZENTTY_GEMINI_PID")),
        Some("cursor") => adapt_cursor_hook(input, environment_pid("ZENTTY_CURSOR_PID")),
        Some("droid") => adapt_droid_hook(input, environment_pid("ZENTTY_DROID_PID")),
        Some("kimi") => adapt_kimi_hook(input, environment_pid("ZENTTY_KIMI_PID")),
        Some("grok") => adapt_grok_hook(input, environment_pid("ZENTTY_GROK_PID")),
        Some("agy") => adapt_agy_hook(input, environment_pid("ZENTTY_AGY_PID")),
        Some("hermes") => adapt_hermes_hook(input, environment_pid("ZENTTY_HERMES_PID")),
        Some("codex-notify") => {
            return adapt_codex_notify(input).map_err(|error| error.to_string());
        }
        Some("vibe") => return adapt_vibe_hook(input).map_err(|error| error.to_string()),
        Some(value) => return Err(format!("unsupported agent adapter: {value}")),
        None => {
            return AgentEvent::parse(input)
                .map(|event| vec![event])
                .map_err(|error| error.to_string());
        }
    };
    events.map_err(|error| error.to_string())
}

fn installed_hook_response(command: &str, event: Option<&str>) -> &'static str {
    if command == "agy-hook" {
        match event
            .unwrap_or_default()
            .replace('_', "-")
            .to_ascii_lowercase()
            .as_str()
        {
            "stop" => r#"{"decision":""}"#,
            "pre-tool-use" | "pretooluse" => r#"{"decision":"allow"}"#,
            _ => "{}",
        }
    } else {
        "{}"
    }
}

fn normalize_hook_subcommand(arguments: Vec<String>) -> Vec<String> {
    let Some(command) = arguments.first().map(String::as_str) else {
        return arguments;
    };
    let adapter = match command {
        "agy-hook" => "agy",
        "hermes-hook" => "hermes",
        _ => return arguments,
    };
    let mut normalized = vec![
        "ipc".to_owned(),
        "agent-event".to_owned(),
        format!("--adapter={adapter}"),
    ];
    normalized.extend(arguments.into_iter().skip(1));
    normalized
}

fn run_product_cli(command: CliProductCommand) -> Result<(), String> {
    match command {
        CliProductCommand::Version => {
            println!(
                "zentty {} ({})",
                env!("CARGO_PKG_VERSION"),
                option_env!("ZENTTY_BUILD_COMMIT").unwrap_or("unknown")
            );
            Ok(())
        }
        CliProductCommand::ListColors => {
            for color in [
                "red", "orange", "amber", "yellow", "lime", "green", "teal", "cyan", "blue",
                "indigo", "purple", "pink",
            ] {
                println!("{color}");
            }
            Ok(())
        }
        CliProductCommand::InstallIntegration(target) => {
            println!("{}", install_integration(&target)?);
            Ok(())
        }
        CliProductCommand::UninstallIntegration(target) => {
            println!("{}", uninstall_integration(&target)?);
            Ok(())
        }
        CliProductCommand::Request(request) => {
            let (socket, caller_token, claimed_target) = application_endpoint()?;
            let reply =
                AgentIpcClient::send_application(socket, &caller_token, &request, claimed_target)
                    .map_err(|error| error.to_string())?;
            if let Some(error) = reply.error() {
                return Err(format!("{}: {}", error.code(), error.message()));
            }
            if let Some(result) = reply.result() {
                let stdout = render_application_result(&request, result)?;
                std::io::stdout()
                    .write_all(stdout.as_bytes())
                    .map_err(|error| format!("could not write product response: {error}"))?;
                if !stdout.is_empty() && !stdout.ends_with('\n') {
                    std::io::stdout().write_all(b"\n").map_err(|error| {
                        format!("could not terminate product response: {error}")
                    })?;
                }
            }
            Ok(())
        }
    }
}

fn application_endpoint() -> Result<(std::path::PathBuf, String, Option<AgentTarget>), String> {
    let pane_credential = pane_credential_from_environment()?;
    match (std::env::var_os("ZENTTY_INSTANCE_SOCKET"), pane_credential) {
        (Some(socket), Some(token)) => {
            return Ok((socket.into(), token, claimed_target_from_environment()));
        }
        (None, None) => {}
        _ => {
            return Err(
                "incomplete in-pane application endpoint environment; refusing discovery fallback"
                    .to_owned(),
            );
        }
    }
    let runtime_root = std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(|| "XDG_RUNTIME_DIR is unavailable for Zentty discovery".to_owned())?;
    let requested = std::env::var("ZENTTY_INSTANCE_ID").ok();
    let mut instances = discover_instances(&runtime_root)?;
    if let Some(requested) = requested.as_deref() {
        instances.retain(|instance| instance.instance_id == requested);
    }
    match instances.as_slice() {
        [] => Err(requested.map_or_else(
            || "no discoverable Zentty instance is running".to_owned(),
            |id| format!("Zentty instance {id:?} is not discoverable"),
        )),
        [instance] => Ok((
            instance.socket_path.clone(),
            instance.credential.expose().to_owned(),
            None,
        )),
        many => Err(format!(
            "multiple Zentty instances are running; set ZENTTY_INSTANCE_ID to one of: {}",
            many.iter()
                .map(|instance| instance.instance_id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn pane_credential_from_environment() -> Result<Option<String>, String> {
    if let Ok(token) = std::env::var("ZENTTY_PANE_TOKEN") {
        if let Some(path) = token.strip_prefix("@file:") {
            return read_pane_credential(std::path::Path::new(path)).map(Some);
        }
        return Ok(Some(token));
    }
    let Some(path) = std::env::var_os("ZENTTY_PANE_CREDENTIAL") else {
        return Ok(None);
    };
    read_pane_credential(std::path::Path::new(&path)).map(Some)
}

fn require_pane_credential() -> Result<String, String> {
    pane_credential_from_environment()?.ok_or_else(|| "ZENTTY_PANE_TOKEN is missing".to_owned())
}

fn read_pane_credential(path: &std::path::Path) -> Result<String, String> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("could not inspect pane credential: {error}"))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.permissions().mode() & 0o777 != 0o600
    {
        return Err("pane credential is not an owner-private regular file".to_owned());
    }
    let token = std::fs::read_to_string(path)
        .map_err(|error| format!("could not read pane credential: {error}"))?;
    let token = token.trim();
    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("pane credential is malformed".to_owned());
    }
    Ok(token.to_owned())
}

fn run_server(arguments: &[String]) -> Result<(), String> {
    let command = ServerCommand::parse(arguments).map_err(|error| error.to_string())?;
    if let ServerCommand::Watch { command } = command {
        return run_server_watch(&command);
    }
    send_server_command(&command)
}

fn send_server_command(command: &ServerCommand) -> Result<(), String> {
    let socket = std::env::var("ZENTTY_INSTANCE_SOCKET")
        .map_err(|_| "zentty server commands must run inside a Zentty pane".to_owned())?;
    let token = require_pane_credential()
        .map_err(|_| "zentty server commands must run inside a Zentty pane".to_owned())?;
    let route = command
        .route()
        .ok_or_else(|| "server watch must be handled by the watch runner".to_owned())?;
    let reply = AgentIpcClient::send_server(
        socket,
        &token,
        route,
        &command.ipc_arguments(),
        claimed_target_from_environment(),
    )
    .map_err(|error| error.to_string())?;
    if let Some(error) = reply.error() {
        return Err(format!("{}: {}", error.code(), error.message()));
    }
    if command.json()
        && let Some(stdout) = reply.stdout()
    {
        std::io::stdout()
            .write_all(stdout.as_bytes())
            .and_then(|()| std::io::stdout().write_all(b"\n"))
            .map_err(|error| format!("could not write server response: {error}"))?;
    }
    Ok(())
}

fn run_server_watch(command: &[String]) -> Result<(), String> {
    let executable = command
        .first()
        .ok_or_else(|| "missing command after zentty server watch --".to_owned())?;
    let _ = send_server_command(&ServerCommand::WatchClear { json: false });
    let mut child = ProcessCommand::new(executable)
        .args(&command[1..])
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not launch watched command {executable:?}: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "watched command stdout was unavailable".to_owned())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "watched command stderr was unavailable".to_owned())?;
    let forwarded = std::thread::scope(|scope| {
        let stdout_worker = scope.spawn(|| forward_watched_output(stdout, std::io::stdout()));
        let stderr_worker = scope.spawn(|| forward_watched_output(stderr, std::io::stderr()));
        let status = child
            .wait()
            .map_err(|error| format!("could not wait for watched command: {error}"));
        let stdout_result = stdout_worker
            .join()
            .map_err(|_| "watched stdout worker panicked".to_owned())?;
        let stderr_result = stderr_worker
            .join()
            .map_err(|_| "watched stderr worker panicked".to_owned())?;
        stdout_result?;
        stderr_result?;
        status
    });
    let _ = send_server_command(&ServerCommand::WatchClear { json: false });
    let status = forwarded?;
    if status.success() {
        Ok(())
    } else if let Some(code) = status.code() {
        std::process::exit(code);
    } else {
        Err(format!("watched command exited with {status}"))
    }
}

fn forward_watched_output(input: impl Read, mut output: impl Write) -> Result<(), String> {
    const CHUNK_BYTES: usize = 4096;
    const DETECTION_TAIL_BYTES: usize = 8192;
    let mut input = BufReader::new(input);
    let mut chunk = [0_u8; CHUNK_BYTES];
    let mut tail = String::new();
    let mut reported = std::collections::BTreeSet::new();
    loop {
        let count = input
            .read(&mut chunk)
            .map_err(|error| format!("could not read watched output: {error}"))?;
        if count == 0 {
            break;
        }
        output
            .write_all(&chunk[..count])
            .and_then(|()| output.flush())
            .map_err(|error| format!("could not forward watched output: {error}"))?;
        let text = String::from_utf8_lossy(&chunk[..count]);
        tail.push_str(&text);
        for candidate in detect_server_urls(&tail) {
            if reported.len() >= 128 || !reported.insert(candidate.url.clone()) {
                continue;
            }
            let _ = send_server_command(&ServerCommand::WatchSet {
                raw_url: candidate.url,
                pid: None,
                json: false,
            });
        }
        if tail.len() > DETECTION_TAIL_BYTES {
            let boundary = tail
                .char_indices()
                .find_map(|(index, _)| {
                    (tail.len() - index <= DETECTION_TAIL_BYTES).then_some(index)
                })
                .unwrap_or(0);
            tail.drain(..boundary);
        }
    }
    Ok(())
}

fn run_codex_notify(payload: Option<String>) -> Result<(), String> {
    if std::env::var_os("ZENTTY_INSTANCE_SOCKET").is_none()
        || std::env::var_os("ZENTTY_PANE_TOKEN").is_none()
    {
        return Ok(());
    }
    let input = if let Some(payload) = payload {
        payload.into_bytes()
    } else {
        let mut input = Vec::new();
        std::io::stdin()
            .take(u64::try_from(AgentEvent::MAX_WIRE_BYTES + 1).unwrap_or(u64::MAX))
            .read_to_end(&mut input)
            .map_err(|error| format!("could not read Codex notify payload: {error}"))?;
        if input.is_empty() {
            return Err("missing Codex notify payload".to_owned());
        }
        input
    };
    let events = adapt_codex_notify(&input).map_err(|error| error.to_string())?;
    let socket = std::env::var("ZENTTY_INSTANCE_SOCKET")
        .map_err(|_| "ZENTTY_INSTANCE_SOCKET is missing".to_owned())?;
    let token = require_pane_credential()?;
    for event in events {
        let bytes = serde_json::to_vec(&event).map_err(|error| error.to_string())?;
        if let Err(error) =
            AgentIpcClient::send_event(&socket, &token, &bytes, claimed_target_from_environment())
        {
            if std::env::var("ZENTTY_CLI_DEBUG").as_deref() == Ok("1") {
                return Err(format!("codex-notify send failed: {error}"));
            }
            return Ok(());
        }
    }
    Ok(())
}

fn run_tmux_compat(arguments: &[String]) -> Result<(), String> {
    let invocation = Invocation::parse(arguments).map_err(|error| error.to_string())?;
    let standard_input = if matches!(invocation.command, Command::SetBuffer | Command::LoadBuffer)
        && !std::io::stdin().is_terminal()
    {
        let mut input = Vec::new();
        std::io::stdin()
            .take(
                u64::try_from(TmuxCompatRequest::MAX_STANDARD_INPUT_BYTES + 1).unwrap_or(u64::MAX),
            )
            .read_to_end(&mut input)
            .map_err(|error| format!("could not read tmux standard input: {error}"))?;
        if input.len() > TmuxCompatRequest::MAX_STANDARD_INPUT_BYTES {
            return Err("tmux standard input exceeds 256 KiB".to_owned());
        }
        Some(
            String::from_utf8(input)
                .map_err(|error| format!("tmux standard input is not UTF-8: {error}"))?,
        )
    } else {
        None
    };
    let socket = std::env::var("ZENTTY_INSTANCE_SOCKET")
        .map_err(|_| "ZENTTY_INSTANCE_SOCKET is missing".to_owned())?;
    let token = require_pane_credential()?;
    let wait = if invocation.command == Command::WaitFor {
        match WaitForAction::parse(&invocation.arguments).map_err(str::to_owned)? {
            WaitForAction::Wait { name, timeout } => Some((name, timeout)),
            WaitForAction::Signal(_) => None,
        }
    } else {
        None
    };
    let deadline = wait
        .as_ref()
        .map(|(_, timeout)| {
            Instant::now()
                .checked_add(*timeout)
                .ok_or_else(|| "wait-for timeout is too large".to_owned())
        })
        .transpose()?;
    loop {
        let reply = AgentIpcClient::send_tmux(
            &socket,
            &token,
            invocation.command.as_str(),
            &invocation.arguments,
            standard_input.clone(),
            claimed_target_from_environment(),
        )
        .map_err(|error| error.to_string())?;
        if reply
            .error()
            .is_some_and(|error| error.code() == "wait_pending")
            && let (Some((name, _)), Some(deadline)) = (&wait, deadline)
        {
            let now = Instant::now();
            if now >= deadline {
                return Err(format!("tmux wait-for: timed out waiting for '{name}'"));
            }
            std::thread::sleep(WAIT_POLL_INTERVAL.min(deadline.duration_since(now)));
            continue;
        }
        if let Some(stdout) = reply.stdout() {
            std::io::stdout()
                .write_all(stdout.as_bytes())
                .map_err(|error| format!("could not write tmux output: {error}"))?;
        }
        if let Some(error) = reply.error() {
            return Err(format!(
                "tmux {}: {}",
                invocation.command.as_str(),
                error.message()
            ));
        }
        break;
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

#[cfg(test)]
mod tests {
    use super::read_pane_credential;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};

    #[test]
    fn pane_credential_reader_rejects_loose_symlinked_and_malformed_files() {
        let root = std::env::temp_dir().join(format!(
            "zentty-pane-credential-reader-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let credential = root.join("credential");
        let token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        fs::write(&credential, token).unwrap();
        fs::set_permissions(&credential, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(read_pane_credential(&credential).unwrap(), token);

        fs::set_permissions(&credential, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(read_pane_credential(&credential).is_err());
        fs::set_permissions(&credential, fs::Permissions::from_mode(0o600)).unwrap();
        let link = root.join("link");
        symlink(&credential, &link).unwrap();
        assert!(read_pane_credential(&link).is_err());
        fs::write(&credential, "not-a-token").unwrap();
        assert!(read_pane_credential(&credential).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
