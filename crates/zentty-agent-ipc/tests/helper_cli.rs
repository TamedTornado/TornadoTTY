use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;
use zentty_agent_ipc::AgentIpcServer;
use zentty_core::{AgentPhase, AgentStatusStore, AgentTarget, PaneTokenRegistry};

struct Harness {
    directory: std::path::PathBuf,
    socket: std::path::PathBuf,
    server: Option<AgentIpcServer>,
    receiver: zentty_agent_ipc::IngressReceiver<zentty_core::AuthenticatedAgentEvent>,
}

impl Harness {
    fn start() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "zentty-agent-helper-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&directory);
        let socket = directory.join("instance.sock");
        let mut registry = PaneTokenRegistry::default();
        registry
            .register(
                "real-token",
                AgentTarget::new("window-real", "lane-real", "pane-real"),
            )
            .unwrap();
        let (sender, receiver) = zentty_agent_ipc::ingress_channel(128, 16);
        let server =
            AgentIpcServer::start(&socket, Arc::new(Mutex::new(registry)), sender).unwrap();
        Self {
            directory,
            socket,
            server: Some(server),
            receiver,
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_zentty"));
        command
            .env("ZENTTY_INSTANCE_SOCKET", &self.socket)
            .env("ZENTTY_PANE_TOKEN", "real-token")
            .env("ZENTTY_WINDOW_ID", "spoofed-window")
            .env("ZENTTY_WORKLANE_ID", "spoofed-lane")
            .env("ZENTTY_PANE_ID", "spoofed-pane")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        if let Some(server) = self.server.take() {
            server.shutdown().unwrap();
        }
        fs::remove_dir_all(&self.directory).unwrap();
    }
}

fn run_with_input(mut command: Command, input: &[u8]) -> std::process::Output {
    let mut child = command.spawn().unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn real_cli_process_does_not_route_pre_policy_codex_permission_requests() {
    let harness = Harness::start();
    let mut command = harness.command();
    command.arg("ipc").arg("agent-event").arg("--adapter=codex");
    let output = run_with_input(
        command,
        br#"{"session_id":"codex-real","turn_id":"turn-real","transcript_path":null,"cwd":"/tmp","hook_event_name":"PermissionRequest","model":"gpt-5.6","permission_mode":"default","tool_name":"shell","tool_input":{"command":"cargo test"}}"#,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(matches!(
        harness.receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
}

#[test]
fn real_cli_process_recovers_a_codex_question_from_recent_session_files() {
    let harness = Harness::start();
    let codex_home = harness.directory.join("codex-home");
    let project = harness.directory.join("project");
    let sessions = codex_home.join("sessions/2026/08/06");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&sessions).unwrap();
    fs::write(
        sessions.join("rollout.jsonl"),
        format!(
            "{{\"cwd\":{:?}}}\n{{\"type\":\"response_item\",\"payload\":{{\"type\":\"function_call\",\"name\":\"request_user_input\",\"arguments\":{{\"question\":\"Choose recovery?\",\"options\":[{{\"label\":\"Resume\"}},{{\"label\":\"Restart\"}}]}}}}}}\n",
            project.to_string_lossy()
        ),
    )
    .unwrap();
    let payload = serde_json::json!({
        "hook_event_name": "PreToolUse",
        "session_id": "codex-recent-real",
        "tool_name": "request_user_input",
        "cwd": project,
    });
    let mut command = harness.command();
    command
        .arg("ipc")
        .arg("agent-event")
        .arg("--adapter=codex")
        .env("CODEX_HOME", codex_home);
    let output = run_with_input(command, payload.to_string().as_bytes());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let received = harness
        .receiver
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    let target = received.target.clone();
    let mut statuses = AgentStatusStore::default();
    statuses.apply(received, 1);
    let status = statuses.status_for(&target).unwrap();
    assert_eq!(status.phase, AgentPhase::NeedsInput);
    assert_eq!(
        status.text.as_deref(),
        Some("Choose recovery?\n[Resume] [Restart]")
    );
}

#[test]
fn real_cli_process_supplies_default_claude_hook_event() {
    let harness = Harness::start();
    let mut command = harness.command();
    command
        .arg("ipc")
        .arg("agent-event")
        .arg("--adapter=claude")
        .arg("SessionStart");
    let output = run_with_input(command, br#"{"session_id":"claude-real"}"#);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let received = harness
        .receiver
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    let target = received.target.clone();
    let mut statuses = AgentStatusStore::default();
    statuses.apply(received, 1);
    let status = statuses.status_for(&target).unwrap();
    assert_eq!(status.agent_name, "Claude Code");
    assert_eq!(status.phase, AgentPhase::Starting);
}

#[test]
fn real_cli_process_adapts_gemini_permission_and_returns_empty_hook_json() {
    let harness = Harness::start();
    let mut command = harness.command();
    command
        .arg("ipc")
        .arg("agent-event")
        .arg("--adapter=gemini")
        .env("ZENTTY_GEMINI_PID", "7373");
    let output = run_with_input(
        command,
        br#"{"hook_event_name":"Notification","notification_type":"ToolPermission","session_id":"gemini-real","details":{"tool_name":"write_file","path":"README.md"}}"#,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"{}\n");
    let received = harness
        .receiver
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    assert_eq!(
        received.target,
        AgentTarget::new("window-real", "lane-real", "pane-real")
    );
    let target = received.target.clone();
    let mut statuses = AgentStatusStore::default();
    statuses.apply(received, 1);
    let status = statuses.status_for(&target).unwrap();
    assert_eq!(status.agent_name, "Gemini");
    assert_eq!(status.tracked_pid, Some(7373));
    assert_eq!(
        status.text.as_deref(),
        Some("Allow write_file on README.md?")
    );
    assert!(status.requires_attention());
}

#[test]
fn real_cli_process_runs_every_newly_installable_hook_adapter() {
    for (adapter, payload, expected_agent, expected_phase) in [
        (
            "cursor",
            br#"{"hook_event_name":"SessionStart","conversation_id":"cursor-real"}"#.as_slice(),
            "Cursor",
            AgentPhase::Starting,
        ),
        (
            "droid",
            br#"{"hook_event_name":"PreToolUse","session_id":"droid-real","tool_name":"AskUser","tool_input":{"question":"Pick one?"}}"#.as_slice(),
            "Droid",
            AgentPhase::NeedsInput,
        ),
        (
            "vibe",
            br#"{"hook_event_name":"post_agent_turn","session_id":"vibe-real"}"#.as_slice(),
            "Mistral Vibe",
            AgentPhase::Idle,
        ),
        (
            "kimi",
            br#"{"hook_event_name":"SessionStart","session_id":"kimi-real"}"#.as_slice(),
            "Kimi",
            AgentPhase::Starting,
        ),
        (
            "grok",
            br#"{"hook_event_name":"SessionStart","session_id":"grok-real"}"#.as_slice(),
            "Grok",
            AgentPhase::Starting,
        ),
        (
            "copilot",
            br#"{"hook_event_name":"preToolUse","session_id":"copilot-real","toolName":"AskUserQuestion","toolArgs":"{\"question\":\"Pick Copilot path?\"}"}"#.as_slice(),
            "Copilot",
            AgentPhase::NeedsInput,
        ),
        (
            "small-harness",
            br#"{"hook_event_name":"SessionStart","session_id":"small-real"}"#.as_slice(),
            "Small Harness",
            AgentPhase::Starting,
        ),
    ] {
        let harness = Harness::start();
        let mut command = harness.command();
        command.arg("ipc").arg("agent-event").arg(format!("--adapter={adapter}"));
        let output = run_with_input(command, payload);
        assert!(
            output.status.success(),
            "adapter={adapter} stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty(), "adapter={adapter} emitted an unexpected acknowledgement");
        let received = harness.receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        let target = received.target.clone();
        let mut statuses = AgentStatusStore::default();
        statuses.apply(received, 1);
        let status = statuses.status_for(&target).unwrap();
        assert_eq!(status.agent_name, expected_agent, "adapter={adapter}");
        assert_eq!(status.phase, expected_phase, "adapter={adapter}");
    }
}

#[test]
fn copilot_source_noop_events_succeed_without_a_routed_pane() {
    for (event, payload) in [
        (
            "error-occurred",
            br#"{"message":"ignored by source"}"#.as_slice(),
        ),
        ("pre-tool-use", br#"{"toolName":"ReadFile"}"#.as_slice()),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_zentty"));
        command
            .args(["ipc", "agent-event", "--adapter=copilot", event])
            .env_remove("ZENTTY_INSTANCE_SOCKET")
            .env_remove("ZENTTY_PANE_TOKEN")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let output = run_with_input(command, payload);
        assert!(
            output.status.success(),
            "event={event} stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn installed_hook_subcommands_acknowledge_calls_outside_a_zentty_pane() {
    for (command, event, expected) in [
        ("agy-hook", "pre-tool-use", "{\"decision\":\"allow\"}\n"),
        ("agy-hook", "stop", "{\"decision\":\"\"}\n"),
        ("hermes-hook", "pre-llm-call", "{}\n"),
    ] {
        let mut process = Command::new(env!("CARGO_BIN_EXE_zentty"));
        process
            .arg(command)
            .arg(event)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let output = run_with_input(process, br#"{"session_id":"outside"}"#);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
}

#[test]
fn real_cli_process_runs_installed_agy_and_hermes_subcommands() {
    for (command_name, event, payload, expected_agent) in [
        (
            "agy-hook",
            "prompt-submit",
            br#"{"session_id":"agy-real"}"#.as_slice(),
            "Antigravity",
        ),
        (
            "hermes-hook",
            "pre-llm-call",
            br#"{"session_id":"hermes-real"}"#.as_slice(),
            "Hermes Agent",
        ),
    ] {
        let harness = Harness::start();
        let mut command = harness.command();
        command.arg(command_name).arg(event);
        let output = run_with_input(command, payload);
        assert!(
            output.status.success(),
            "command={command_name} stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"{}\n", "command={command_name}");
        let received = harness
            .receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        let target = received.target.clone();
        let mut statuses = AgentStatusStore::default();
        statuses.apply(received, 1);
        let status = statuses.status_for(&target).unwrap();
        assert_eq!(status.agent_name, expected_agent);
        assert_eq!(status.phase, AgentPhase::Running);
    }
}

#[test]
fn real_codex_notify_command_forwards_positional_payload_to_the_canonical_socket() {
    let harness = Harness::start();
    let output = harness
        .command()
        .arg("codex-notify")
        .arg(
            r#"{"type":"question","session_id":"codex-notify-real","message":"Continue?\n[Yes] [No]"}"#,
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    let received = harness
        .receiver
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    let target = received.target.clone();
    let mut statuses = AgentStatusStore::default();
    statuses.apply(received, 1);
    let status = statuses.status_for(&target).unwrap();
    assert_eq!(status.phase, AgentPhase::NeedsInput);
    assert_eq!(
        status.interaction,
        zentty_core::AgentInteractionKind::Decision
    );
    assert_eq!(status.text.as_deref(), Some("Continue?\n[Yes] [No]"));
}

#[test]
fn codex_notify_without_pane_routing_is_a_quiet_best_effort_success() {
    for missing in ["ZENTTY_INSTANCE_SOCKET", "ZENTTY_PANE_TOKEN"] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_zentty"));
        command
            .arg("codex-notify")
            .arg(r#"{"type":"agent-turn-complete"}"#)
            .env("ZENTTY_INSTANCE_SOCKET", "/not/a/socket")
            .env("ZENTTY_PANE_TOKEN", "token")
            .env_remove(missing);
        let output = command.output().unwrap();
        assert!(output.status.success(), "{missing}");
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn codex_notify_accepts_stdin_and_only_surfaces_transport_failure_in_debug_mode() {
    let harness = Harness::start();
    let mut command = harness.command();
    command.arg("codex-notify");
    let output = run_with_input(
        command,
        br#"{"type":"agent-turn-complete","session_id":"codex-stdin"}"#,
    );
    assert!(output.status.success());
    let received = harness
        .receiver
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    let target = received.target.clone();
    let mut statuses = AgentStatusStore::default();
    statuses.apply(received, 1);
    assert_eq!(
        statuses.status_for(&target).unwrap().phase,
        AgentPhase::Idle
    );

    for debug in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_zentty"));
        command
            .arg("codex-notify")
            .arg(r#"{"type":"agent-turn-complete"}"#)
            .env("ZENTTY_INSTANCE_SOCKET", "/missing/zentty.sock")
            .env("ZENTTY_PANE_TOKEN", "token");
        if debug {
            command.env("ZENTTY_CLI_DEBUG", "1");
        } else {
            command.env_remove("ZENTTY_CLI_DEBUG");
        }
        let output = command.output().unwrap();
        assert_eq!(output.status.success(), !debug);
        if debug {
            assert!(String::from_utf8_lossy(&output.stderr).contains("codex-notify send failed"));
        } else {
            assert!(output.stderr.is_empty());
        }
    }
}

#[test]
fn codex_notify_stdin_enforces_the_exact_canonical_wire_ceiling() {
    let harness = Harness::start();
    let prefix = r#"{"type":"notice","message":""#;
    let suffix = r#""}"#;
    let target_length = zentty_core::AgentEvent::MAX_WIRE_BYTES + 1;
    let payload = format!(
        "{prefix}{}{suffix}",
        "x".repeat(target_length - prefix.len() - suffix.len())
    );
    assert_eq!(payload.len(), target_length);
    let mut command = harness.command();
    command.arg("codex-notify");
    let output = run_with_input(command, payload.as_bytes());
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("exceeds 64 KiB"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn real_cli_process_rejects_bad_tokens_without_delivering_events() {
    let harness = Harness::start();
    let mut command = harness.command();
    command
        .env("ZENTTY_PANE_TOKEN", "wrong-token")
        .arg("ipc")
        .arg("agent-event");
    let output = run_with_input(
        command,
        br#"{"version":1,"event":"agent.idle","agent":{"name":"Codex"}}"#,
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("pane token is invalid"), "{stderr}");
    assert!(
        harness
            .receiver
            .recv_timeout(Duration::from_millis(100))
            .is_err()
    );
}

#[test]
fn real_cli_process_reports_missing_socket_environment() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_zentty"));
    command
        .arg("ipc")
        .arg("agent-event")
        .env_remove("ZENTTY_INSTANCE_SOCKET")
        .env("ZENTTY_PANE_TOKEN", "unused")
        .stdin(Stdio::piped())
        .stderr(Stdio::piped());
    let output = run_with_input(
        command,
        br#"{"version":1,"event":"agent.idle","agent":{"name":"Codex"}}"#,
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("ZENTTY_INSTANCE_SOCKET is missing"));
}

#[test]
fn gemini_hook_without_pane_routing_returns_empty_json_without_delivery() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_zentty"));
    command
        .arg("ipc")
        .arg("agent-event")
        .arg("--adapter=gemini")
        .env_remove("ZENTTY_INSTANCE_SOCKET")
        .env_remove("ZENTTY_PANE_TOKEN")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = run_with_input(
        command,
        br#"{"hook_event_name":"SessionStart","session_id":"unrouted"}"#,
    );
    assert!(output.status.success());
    assert_eq!(output.stdout, b"{}\n");
    assert!(output.stderr.is_empty());
}
