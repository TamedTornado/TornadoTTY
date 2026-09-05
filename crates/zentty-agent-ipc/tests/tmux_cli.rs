use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;
use zentty_agent_ipc::AgentIpcServer;
use zentty_core::{AgentTarget, PaneTokenRegistry};
use zentty_tmux_compat::{Command as TmuxCommand, TmuxCompatReply};

fn server() -> (
    std::path::PathBuf,
    AgentIpcServer,
    zentty_agent_ipc::IngressReceiver<zentty_agent_ipc::AuthenticatedTmuxRequest>,
) {
    let directory = std::env::temp_dir().join(format!(
        "zentty-tmux-cli-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir_all(&directory).unwrap();
    let socket = directory.join("instance.sock");
    let mut registry = PaneTokenRegistry::default();
    registry
        .register(
            "real-token",
            AgentTarget::new("real-window", "real-lane", "real-pane"),
        )
        .unwrap();
    let (event_sender, _event_receiver) = zentty_agent_ipc::ingress_channel(128, 16);
    let (tmux_sender, tmux_receiver) = zentty_agent_ipc::ingress_channel(32, 4);
    let server = AgentIpcServer::start_with_tmux(
        &socket,
        Arc::new(Mutex::new(registry)),
        event_sender,
        tmux_sender,
    )
    .unwrap();
    (socket, server, tmux_receiver)
}

fn cli(socket: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_zentty"));
    command
        .env("ZENTTY_INSTANCE_SOCKET", socket)
        .env("ZENTTY_PANE_TOKEN", "real-token")
        .env("ZENTTY_WINDOW_ID", "spoof-window")
        .env("ZENTTY_WORKLANE_ID", "spoof-lane")
        .env("ZENTTY_PANE_ID", "spoof-pane");
    command
}

#[test]
fn real_cli_process_routes_canonical_command_and_prints_exact_product_output() {
    let (socket, server, receiver) = server();
    let handler = std::thread::spawn(move || {
        let request = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(
            request.target,
            AgentTarget::new("real-window", "real-lane", "real-pane")
        );
        assert_eq!(request.request.command(), TmuxCommand::ListPanes);
        assert_eq!(request.request.arguments(), ["-F", "#{pane_id}"]);
        request
            .respond(TmuxCompatReply::success("%pane-1\n".to_owned()).unwrap())
            .unwrap();
    });

    let output = cli(&socket)
        .args([
            "__tmux-compat",
            "-S",
            "ignored-source-socket",
            "list-panes",
            "-F",
            "#{pane_id}",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(output.stdout, b"%pane-1\n");
    assert!(output.stderr.is_empty());
    handler.join().unwrap();
    server.shutdown().unwrap();
}

#[test]
fn real_cli_process_forwards_piped_stdin_and_surfaces_product_failure() {
    let (socket, server, receiver) = server();
    let handler = std::thread::spawn(move || {
        let request = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(request.request.command(), TmuxCommand::SetBuffer);
        assert_eq!(request.request.standard_input(), Some("clipboard text"));
        request
            .respond(
                TmuxCompatReply::failure("not_implemented", "set-buffer is not implemented")
                    .unwrap(),
            )
            .unwrap();
    });

    let mut child = cli(&socket)
        .args(["__tmux-compat", "set-buffer"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"clipboard text")
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "tornadotty-cli: tmux set-buffer: set-buffer is not implemented\n"
    );
    handler.join().unwrap();
    server.shutdown().unwrap();
}

#[test]
fn separate_cli_processes_wait_and_signal_without_holding_a_socket_worker() {
    let (socket, server, receiver) = server();
    let (first_probe_sender, first_probe_receiver) = mpsc::channel();
    let handler = std::thread::spawn(move || {
        let mut pending = false;
        let mut first_probe = true;
        loop {
            let request = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
            assert_eq!(request.request.command(), TmuxCommand::WaitFor);
            if request
                .request
                .arguments()
                .iter()
                .any(|value| value == "-S")
            {
                pending = true;
                request
                    .respond(TmuxCompatReply::success(String::new()).unwrap())
                    .unwrap();
            } else if pending {
                request
                    .respond(TmuxCompatReply::success(String::new()).unwrap())
                    .unwrap();
                break;
            } else {
                request
                    .respond(
                        TmuxCompatReply::failure("wait_pending", "signal is not pending").unwrap(),
                    )
                    .unwrap();
                if first_probe {
                    first_probe = false;
                    first_probe_sender.send(()).unwrap();
                }
            }
        }
    });

    let mut waiter = cli(&socket)
        .args(["__tmux-compat", "wait-for", "--timeout", "1", "agent-ready"])
        .spawn()
        .unwrap();
    first_probe_receiver
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    let independent = cli(&socket)
        .args(["__tmux-compat", "wait-for", "-S", "agent-ready"])
        .output()
        .unwrap();
    assert!(independent.status.success());
    assert!(waiter.wait().unwrap().success());
    handler.join().unwrap();
    server.shutdown().unwrap();
}

#[test]
fn real_cli_reports_deterministic_wait_timeout() {
    let (socket, server, receiver) = server();
    let handler = std::thread::spawn(move || {
        while let Ok(request) = receiver.recv_timeout(Duration::from_millis(250)) {
            assert_eq!(request.request.command(), TmuxCommand::WaitFor);
            request
                .respond(TmuxCompatReply::failure("wait_pending", "signal is not pending").unwrap())
                .unwrap();
        }
    });

    let output = cli(&socket)
        .args([
            "__tmux-compat",
            "wait-for",
            "--timeout",
            "0.06",
            "never-ready",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "tornadotty-cli: tmux wait-for: timed out waiting for 'never-ready'\n"
    );
    handler.join().unwrap();
    server.shutdown().unwrap();
}

#[test]
fn waiting_cli_fails_promptly_when_the_instance_shuts_down() {
    let (socket, server, receiver) = server();
    let (probe_sender, probe_receiver) = mpsc::channel();
    let handler = std::thread::spawn(move || {
        let request = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(request.request.command(), TmuxCommand::WaitFor);
        request
            .respond(TmuxCompatReply::failure("wait_pending", "signal is not pending").unwrap())
            .unwrap();
        probe_sender.send(()).unwrap();
    });

    let waiter = cli(&socket)
        .args(["__tmux-compat", "wait-for", "--timeout", "30", "shutdown"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    probe_receiver.recv_timeout(Duration::from_secs(1)).unwrap();
    handler.join().unwrap();
    server.shutdown().unwrap();
    let output = waiter.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .starts_with("tornadotty-cli: ")
    );
}
