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
    receiver: mpsc::Receiver<zentty_core::AuthenticatedAgentEvent>,
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
        let (sender, receiver) = mpsc::channel();
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
fn real_cli_process_adapts_codex_and_server_uses_canonical_target() {
    let harness = Harness::start();
    let mut command = harness.command();
    command.arg("ipc").arg("agent-event").arg("--adapter=codex");
    let output = run_with_input(
        command,
        br#"{"hook_event_name":"PermissionRequest","session_id":"codex-real","message":"Run tests?"}"#,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
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
    assert_eq!(status.agent_name, "Codex");
    assert_eq!(status.text.as_deref(), Some("Run tests?"));
    assert!(status.requires_attention());
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
