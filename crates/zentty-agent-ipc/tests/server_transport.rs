use std::fs;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use zentty_agent_ipc::{AgentIpcClient, AgentIpcServer, ServerIpcReply};
use zentty_core::{AgentTarget, PaneTokenRegistry};

#[test]
fn real_socket_authenticates_and_routes_server_commands_without_trusting_claimed_target() {
    let directory = std::env::temp_dir().join(format!(
        "zentty-server-ipc-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir_all(&directory).unwrap();
    let socket = directory.join("instance.sock");
    let canonical = AgentTarget::new("window-real", "lane-real", "pane-real");
    let mut registry = PaneTokenRegistry::default();
    registry.register("token-real", canonical.clone()).unwrap();
    let (event_sender, _event_receiver) = zentty_agent_ipc::ingress_channel(128, 16);
    let (tmux_sender, _tmux_receiver) = zentty_agent_ipc::ingress_channel(32, 4);
    let (server_sender, server_receiver) = zentty_agent_ipc::ingress_channel(32, 4);
    let server = AgentIpcServer::start_with_product_routes(
        &socket,
        Arc::new(Mutex::new(registry)),
        event_sender,
        tmux_sender,
        server_sender,
    )
    .unwrap();

    let handler = std::thread::spawn(move || {
        let request = server_receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        assert_eq!(request.target, canonical);
        assert_eq!(request.request.subcommand(), "server-set");
        assert_eq!(request.request.arguments(), ["localhost:5173", "--json"]);
        request
            .respond(ServerIpcReply::success("{\"version\":2}").unwrap())
            .unwrap();
    });

    let reply = AgentIpcClient::send_server(
        &socket,
        "token-real",
        "server-set",
        &["localhost:5173".to_owned(), "--json".to_owned()],
        Some(AgentTarget::new("spoof", "spoof", "spoof")),
    )
    .unwrap();
    assert_eq!(reply.stdout(), Some("{\"version\":2}"));
    handler.join().unwrap();
    server.shutdown().unwrap();
}

#[test]
fn waiting_server_handler_does_not_hold_authentication_or_block_agent_events() {
    let directory = std::env::temp_dir().join(format!(
        "zentty-server-concurrency-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir_all(&directory).unwrap();
    let socket = directory.join("instance.sock");
    let canonical = AgentTarget::new("window", "lane", "pane");
    let mut registry = PaneTokenRegistry::default();
    registry.register("token", canonical.clone()).unwrap();
    let (event_sender, event_receiver) = zentty_agent_ipc::ingress_channel(128, 16);
    let (tmux_sender, _tmux_receiver) = zentty_agent_ipc::ingress_channel(32, 4);
    let (server_sender, server_receiver) = zentty_agent_ipc::ingress_channel(32, 4);
    let server = AgentIpcServer::start_with_product_routes(
        &socket,
        Arc::new(Mutex::new(registry)),
        event_sender,
        tmux_sender,
        server_sender,
    )
    .unwrap();

    let command_socket = socket.clone();
    let command_client = std::thread::spawn(move || {
        AgentIpcClient::send_server(&command_socket, "token", "server-list", &[], None)
    });
    let pending = server_receiver
        .recv_timeout(Duration::from_secs(1))
        .unwrap();

    AgentIpcClient::send_event(
        &socket,
        "token",
        br#"{"version":1,"event":"agent.idle"}"#,
        None,
    )
    .unwrap();
    assert_eq!(
        event_receiver
            .recv_timeout(Duration::from_millis(500))
            .unwrap()
            .target,
        canonical
    );
    pending
        .respond(ServerIpcReply::success(String::new()).unwrap())
        .unwrap();
    command_client.join().unwrap().unwrap();
    server.shutdown().unwrap();
}
