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
    mpsc::Receiver<zentty_agent_ipc::AuthenticatedTmuxRequest>,
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
    let (event_sender, _event_receiver) = mpsc::channel();
    let (tmux_sender, tmux_receiver) = mpsc::channel();
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
        "zentty: tmux set-buffer: set-buffer is not implemented\n"
    );
    handler.join().unwrap();
    server.shutdown().unwrap();
}
