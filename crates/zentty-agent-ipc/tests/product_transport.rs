use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::Command;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;
use zentty_agent_ipc::{
    AgentIpcClient, AgentIpcServer, AuthenticatedProductRequest, ProductIpcKind, ProductIpcReply,
};
use zentty_core::{AgentTarget, PaneTokenRegistry};

fn running_server() -> (
    std::path::PathBuf,
    std::path::PathBuf,
    AgentIpcServer,
    mpsc::Receiver<AuthenticatedProductRequest>,
) {
    running_server_named(
        "default",
        "caller-token",
        AgentTarget::new("window-1", "lane-1", "pane-1"),
    )
}

fn running_server_named(
    name: &str,
    token: &str,
    target: AgentTarget,
) -> (
    std::path::PathBuf,
    std::path::PathBuf,
    AgentIpcServer,
    mpsc::Receiver<AuthenticatedProductRequest>,
) {
    let root = std::env::temp_dir().join(format!(
        "zentty-product-ipc-{}-{:?}-{name}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let socket = root.join("runtime/instance.sock");
    let mut registry = PaneTokenRegistry::default();
    registry.register(token, target).unwrap();
    let (event_sender, _event_receiver) = mpsc::channel();
    let (tmux_sender, _tmux_receiver) = mpsc::channel();
    let (server_sender, _server_receiver) = mpsc::channel();
    let (product_sender, product_receiver) = mpsc::channel();
    let server = AgentIpcServer::start_with_cli_routes(
        &socket,
        Arc::new(Mutex::new(registry)),
        event_sender,
        tmux_sender,
        server_sender,
        product_sender,
    )
    .unwrap();
    (root, socket, server, product_receiver)
}

#[test]
fn real_socket_authenticates_and_returns_bounded_product_reply() {
    let (root, socket, server, receiver) = running_server();
    let worker = std::thread::spawn(move || {
        let request = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(request.target.window_id, "window-1");
        assert_eq!(request.target.worklane_id, "lane-1");
        assert_eq!(request.target.pane_id, "pane-1");
        assert_eq!(request.request.kind(), ProductIpcKind::Discover);
        assert_eq!(request.request.subcommand(), "panes");
        assert_eq!(request.request.arguments(), ["--json"]);
        request
            .respond(ProductIpcReply::success(r#"[{"id":"pane-1"}]"#).unwrap())
            .unwrap();
    });

    let reply = AgentIpcClient::send_product(
        &socket,
        "caller-token",
        ProductIpcKind::Discover,
        "panes",
        &["--json".to_owned()],
        Some(AgentTarget::new(
            "forged-window",
            "forged-lane",
            "forged-pane",
        )),
    )
    .unwrap();
    assert_eq!(reply.stdout(), Some(r#"[{"id":"pane-1"}]"#));
    assert!(reply.error().is_none());
    worker.join().unwrap();
    server.shutdown().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn partial_frame_writes_are_reassembled_before_authentication_and_dispatch() {
    let (root, socket, server, receiver) = running_server();
    let worker = std::thread::spawn(move || {
        let request = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(
            request.target,
            AgentTarget::new("window-1", "lane-1", "pane-1")
        );
        assert_eq!(request.request.subcommand(), "panes");
        request
            .respond(ProductIpcReply::success("partial-ok").unwrap())
            .unwrap();
    });
    let frame = br#"{"version":1,"id":"partial","kind":"discover","arguments":["--json"],"standardInput":null,"environment":{"ZENTTY_PANE_TOKEN":"caller-token","ZENTTY_WINDOW_ID":"forged-window","ZENTTY_WORKLANE_ID":"forged-lane","ZENTTY_PANE_ID":"forged-pane"},"expectsResponse":true,"subcommand":"panes"}"#;
    let mut stream = UnixStream::connect(&socket).unwrap();
    for chunk in frame.chunks(7) {
        stream.write_all(chunk).unwrap();
    }
    stream.shutdown(std::net::Shutdown::Write).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let response: serde_json::Value = serde_json::from_str(&response).unwrap();
    assert_eq!(response["id"], "partial");
    assert_eq!(response["ok"], true);
    assert_eq!(response["result"]["stdout"], "partial-ok");
    worker.join().unwrap();
    server.shutdown().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn concurrent_mixed_auth_clients_dispatch_only_canonical_authorized_targets() {
    const AUTHORIZED: usize = 8;
    const UNAUTHORIZED: usize = 8;
    let (root, socket, server, receiver) = running_server();
    let responder = std::thread::spawn(move || {
        for sequence in 0..AUTHORIZED {
            let request = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
            assert_eq!(
                request.target,
                AgentTarget::new("window-1", "lane-1", "pane-1")
            );
            request
                .respond(ProductIpcReply::success(format!("authorized-{sequence}")).unwrap())
                .unwrap();
        }
        assert!(receiver.recv_timeout(Duration::from_millis(150)).is_err());
    });
    let mut clients = Vec::new();
    for sequence in 0..AUTHORIZED + UNAUTHORIZED {
        let socket = socket.clone();
        clients.push(std::thread::spawn(move || {
            let authorized = sequence < AUTHORIZED;
            let result = AgentIpcClient::send_product(
                socket,
                if authorized {
                    "caller-token"
                } else {
                    "unauthorized-token"
                },
                ProductIpcKind::Discover,
                "panes",
                &["--json".to_owned()],
                Some(AgentTarget::new(
                    format!("claim-window-{sequence}"),
                    format!("claim-lane-{sequence}"),
                    format!("claim-pane-{sequence}"),
                )),
            );
            assert_eq!(result.is_ok(), authorized);
        }));
    }
    for client in clients {
        client.join().unwrap();
    }
    responder.join().unwrap();
    server.shutdown().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn concurrent_instances_reject_each_others_capabilities() {
    let (root_a, socket_a, server_a, receiver_a) = running_server_named(
        "instance-a",
        "token-a",
        AgentTarget::new("window-a", "lane-a", "pane-a"),
    );
    let (root_b, socket_b, server_b, receiver_b) = running_server_named(
        "instance-b",
        "token-b",
        AgentTarget::new("window-b", "lane-b", "pane-b"),
    );
    assert!(
        AgentIpcClient::send_product(
            &socket_a,
            "token-b",
            ProductIpcKind::Discover,
            "panes",
            &[],
            None,
        )
        .is_err()
    );
    assert!(
        AgentIpcClient::send_product(
            &socket_b,
            "token-a",
            ProductIpcKind::Discover,
            "panes",
            &[],
            None,
        )
        .is_err()
    );
    assert!(receiver_a.recv_timeout(Duration::from_millis(100)).is_err());
    assert!(receiver_b.recv_timeout(Duration::from_millis(100)).is_err());
    server_a.shutdown().unwrap();
    server_b.shutdown().unwrap();
    std::fs::remove_dir_all(root_a).unwrap();
    std::fs::remove_dir_all(root_b).unwrap();
}

#[test]
fn forged_token_is_rejected_before_product_dispatch() {
    let (root, socket, server, receiver) = running_server();
    let error = AgentIpcClient::send_product(
        &socket,
        "forged-token",
        ProductIpcKind::Pane,
        "split",
        &["right".to_owned()],
        Some(AgentTarget::new("window-1", "lane-1", "pane-1")),
    )
    .unwrap_err();
    assert!(error.to_string().contains("rejected"));
    assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());
    server.shutdown().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn product_failure_preserves_machine_code_and_message() {
    let (root, socket, server, receiver) = running_server();
    let worker = std::thread::spawn(move || {
        receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .respond(
                ProductIpcReply::failure("ambiguous_target", "select one pane explicitly").unwrap(),
            )
            .unwrap();
    });
    let reply = AgentIpcClient::send_product(
        &socket,
        "caller-token",
        ProductIpcKind::Pane,
        "focus",
        &[],
        None,
    )
    .unwrap();
    let error = reply.error().unwrap();
    assert_eq!(error.code(), "ambiguous_target");
    assert_eq!(error.message(), "select one pane explicitly");
    worker.join().unwrap();
    server.shutdown().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn real_shell_signal_cli_uses_the_authenticated_product_route() {
    let (root, socket, server, receiver) = running_server();
    let worker = std::thread::spawn(move || {
        let request = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(
            request.target,
            AgentTarget::new("window-1", "lane-1", "pane-1")
        );
        assert_eq!(request.request.kind(), ProductIpcKind::Pane);
        assert_eq!(request.request.subcommand(), "shell-signal");
        assert_eq!(
            request.request.arguments(),
            ["pane-context", "local", "--path", "/tmp/space and λ"]
        );
        request
            .respond(ProductIpcReply::success("").unwrap())
            .unwrap();
    });
    let output = Command::new(env!("CARGO_BIN_EXE_zentty"))
        .args([
            "ipc",
            "agent-signal",
            "pane-context",
            "local",
            "--path",
            "/tmp/space and λ",
        ])
        .env("ZENTTY_INSTANCE_SOCKET", &socket)
        .env("ZENTTY_PANE_TOKEN", "caller-token")
        .env("ZENTTY_WINDOW_ID", "forged-window")
        .env("ZENTTY_WORKLANE_ID", "forged-lane")
        .env("ZENTTY_PANE_ID", "forged-pane")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    worker.join().unwrap();
    server.shutdown().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn shell_signal_is_silent_and_non_invasive_outside_a_live_pane() {
    let output = Command::new(env!("CARGO_BIN_EXE_zentty"))
        .args(["ipc", "agent-signal", "shell-state", "prompt"])
        .env_clear()
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn real_agent_status_cli_maps_to_the_authenticated_lifecycle_route() {
    let (root, socket, server, receiver) = running_server();
    let worker = std::thread::spawn(move || {
        let request = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(
            request.target,
            AgentTarget::new("window-1", "lane-1", "pane-1")
        );
        assert_eq!(request.request.subcommand(), "shell-signal");
        assert_eq!(
            request.request.arguments(),
            [
                "lifecycle",
                "needs-input",
                "--tool",
                "Custom Agent",
                "--text",
                "Approve λ?",
                "--session-id",
                "child-a",
                "--parent-session-id",
                "parent-a",
                "--interaction-kind",
                "approval",
            ]
        );
        request
            .respond(ProductIpcReply::success("").unwrap())
            .unwrap();
    });
    let output = Command::new(env!("CARGO_BIN_EXE_zentty"))
        .args([
            "ipc",
            "agent-status",
            "needs-input",
            "--tool",
            "Custom Agent",
            "--text",
            "Approve λ?",
            "--session-id",
            "child-a",
            "--parent-session-id",
            "parent-a",
            "--interaction-kind",
            "approval",
        ])
        .env("ZENTTY_INSTANCE_SOCKET", &socket)
        .env("ZENTTY_PANE_TOKEN", "caller-token")
        .env("ZENTTY_WINDOW_ID", "forged-window")
        .env("ZENTTY_WORKLANE_ID", "forged-lane")
        .env("ZENTTY_PANE_ID", "forged-pane")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    worker.join().unwrap();
    server.shutdown().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
