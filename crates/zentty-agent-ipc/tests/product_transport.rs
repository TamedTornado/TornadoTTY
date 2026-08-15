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
    let root = std::env::temp_dir().join(format!(
        "zentty-product-ipc-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let socket = root.join("runtime/instance.sock");
    let mut registry = PaneTokenRegistry::default();
    registry
        .register(
            "caller-token",
            AgentTarget::new("window-1", "lane-1", "pane-1"),
        )
        .unwrap();
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
        Some(AgentTarget::new("window-1", "lane-1", "pane-1")),
    )
    .unwrap();
    assert_eq!(reply.stdout(), Some(r#"[{"id":"pane-1"}]"#));
    assert!(reply.error().is_none());
    worker.join().unwrap();
    server.shutdown().unwrap();
    std::fs::remove_dir_all(root).unwrap();
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
