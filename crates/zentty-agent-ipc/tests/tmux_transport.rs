use std::fs;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;
use zentty_agent_ipc::{AgentIpcClient, AgentIpcServer};
use zentty_core::{AgentTarget, PaneTokenRegistry};
use zentty_tmux_compat::{Command, TmuxCompatReply};

fn temporary_socket() -> (std::path::PathBuf, std::path::PathBuf) {
    let directory = std::env::temp_dir().join(format!(
        "zentty-tmux-ipc-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir_all(&directory).unwrap();
    let socket = directory.join("instance.sock");
    (directory, socket)
}

#[test]
fn real_socket_routes_tmux_to_the_token_target_and_returns_bounded_stdout() {
    let (_directory, socket) = temporary_socket();
    let canonical = AgentTarget::new("window-real", "lane-real", "pane-real");
    let mut registry = PaneTokenRegistry::default();
    registry.register("token-real", canonical.clone()).unwrap();
    let (event_sender, _event_receiver) = mpsc::channel();
    let (tmux_sender, tmux_receiver) = mpsc::channel();
    let server = AgentIpcServer::start_with_tmux(
        &socket,
        Arc::new(Mutex::new(registry)),
        event_sender,
        tmux_sender,
    )
    .unwrap();

    let handler = std::thread::spawn(move || {
        let command = tmux_receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(command.target, canonical);
        assert_eq!(command.request.command(), Command::SplitWindow);
        assert_eq!(command.request.arguments(), ["-h", "-P"]);
        assert_eq!(command.request.standard_input(), Some("stdin"));
        command
            .respond(TmuxCompatReply::success("%pane-2\n".to_owned()).unwrap())
            .unwrap();
    });

    let reply = AgentIpcClient::send_tmux(
        &socket,
        "token-real",
        "splitw",
        &["-h".to_owned(), "-P".to_owned()],
        Some("stdin".to_owned()),
        Some(AgentTarget::new("spoof", "spoof", "spoof")),
    )
    .unwrap();
    assert!(reply.is_ok());
    assert_eq!(reply.stdout(), Some("%pane-2\n"));
    assert_eq!(reply.exit_code(), 0);

    handler.join().unwrap();
    server.shutdown().unwrap();
}

#[test]
fn tmux_route_rejects_bad_auth_protocol_and_absent_product_handler() {
    let (_directory, socket) = temporary_socket();
    let mut registry = PaneTokenRegistry::default();
    registry
        .register("token", AgentTarget::new("window", "lane", "pane"))
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

    assert!(AgentIpcClient::send_tmux(&socket, "wrong", "split-window", &[], None, None).is_err());
    assert!(
        tmux_receiver
            .recv_timeout(Duration::from_millis(100))
            .is_err()
    );
    assert!(
        AgentIpcClient::send_raw_frame(
            &socket,
            br#"{"version":2,"id":"bad-version","kind":"tmux_compat","arguments":[],"standardInput":null,"environment":{"ZENTTY_PANE_TOKEN":"token"},"expectsResponse":true,"subcommand":"split-window"}"#,
        )
        .is_err()
    );
    assert!(
        AgentIpcClient::send_raw_frame(
            &socket,
            br#"{"version":1,"id":"no-response","kind":"tmux_compat","arguments":[],"standardInput":null,"environment":{"ZENTTY_PANE_TOKEN":"token"},"expectsResponse":false,"subcommand":"split-window"}"#,
        )
        .is_err()
    );
    assert!(
        tmux_receiver
            .recv_timeout(Duration::from_millis(100))
            .is_err()
    );
    server.shutdown().unwrap();

    let (_directory, socket) = temporary_socket();
    let mut registry = PaneTokenRegistry::default();
    registry
        .register("token", AgentTarget::new("window", "lane", "pane"))
        .unwrap();
    let (event_sender, _event_receiver) = mpsc::channel();
    let server =
        AgentIpcServer::start(&socket, Arc::new(Mutex::new(registry)), event_sender).unwrap();
    assert!(AgentIpcClient::send_tmux(&socket, "token", "split-window", &[], None, None).is_err());
    server.shutdown().unwrap();
}

#[test]
fn waiting_tmux_handler_does_not_block_independent_authenticated_events() {
    let (_directory, socket) = temporary_socket();
    let canonical = AgentTarget::new("window", "lane", "pane");
    let mut registry = PaneTokenRegistry::default();
    registry.register("token", canonical.clone()).unwrap();
    let (event_sender, event_receiver) = mpsc::channel();
    let (tmux_sender, tmux_receiver) = mpsc::channel();
    let server = AgentIpcServer::start_with_tmux(
        &socket,
        Arc::new(Mutex::new(registry)),
        event_sender,
        tmux_sender,
    )
    .unwrap();

    let tmux_socket = socket.clone();
    let waiting_client = std::thread::spawn(move || {
        AgentIpcClient::send_tmux(&tmux_socket, "token", "split-window", &[], None, None)
    });
    let pending = tmux_receiver.recv_timeout(Duration::from_secs(1)).unwrap();

    let event_socket = socket.clone();
    let event_client = std::thread::spawn(move || {
        AgentIpcClient::send_event(
            event_socket,
            "token",
            br#"{"version":1,"event":"agent.idle"}"#,
            None,
        )
    });
    assert_eq!(
        event_receiver
            .recv_timeout(Duration::from_millis(500))
            .unwrap()
            .target,
        canonical
    );
    event_client.join().unwrap().unwrap();

    pending
        .respond(TmuxCompatReply::success(String::new()).unwrap())
        .unwrap();
    waiting_client.join().unwrap().unwrap();
    server.shutdown().unwrap();
}

#[test]
fn maximum_bounded_tmux_stdin_and_stdout_cross_the_real_socket() {
    assert_eq!(AgentIpcServer::MAX_FRAME_BYTES, 393_216);
    assert_eq!(AgentIpcServer::MAX_FRAME_READ_BYTES, 393_217);
    let (_directory, socket) = temporary_socket();
    let mut registry = PaneTokenRegistry::default();
    registry
        .register("token", AgentTarget::new("window", "lane", "pane"))
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

    let handler = std::thread::spawn(move || {
        let command = tmux_receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(
            command.request.standard_input().unwrap().len(),
            zentty_tmux_compat::TmuxCompatRequest::MAX_STANDARD_INPUT_BYTES
        );
        command
            .respond(
                TmuxCompatReply::success("x".repeat(TmuxCompatReply::MAX_STDOUT_BYTES)).unwrap(),
            )
            .unwrap();
    });
    let reply = AgentIpcClient::send_tmux(
        &socket,
        "token",
        "set-buffer",
        &[],
        Some("x".repeat(zentty_tmux_compat::TmuxCompatRequest::MAX_STANDARD_INPUT_BYTES)),
        None,
    )
    .unwrap();
    assert_eq!(
        reply.stdout().unwrap().len(),
        TmuxCompatReply::MAX_STDOUT_BYTES
    );

    handler.join().unwrap();
    server.shutdown().unwrap();
}

#[test]
fn client_and_server_enforce_the_exact_wire_frame_boundary() {
    let (_directory, socket) = temporary_socket();
    let mut registry = PaneTokenRegistry::default();
    registry
        .register("token", AgentTarget::new("window", "lane", "pane"))
        .unwrap();
    let (event_sender, _event_receiver) = mpsc::channel();
    let server =
        AgentIpcServer::start(&socket, Arc::new(Mutex::new(registry)), event_sender).unwrap();

    let exact = vec![b'x'; AgentIpcServer::MAX_FRAME_BYTES];
    assert!(matches!(
        AgentIpcClient::send_raw_frame(&socket, &exact),
        Err(zentty_agent_ipc::AgentIpcError::Rejected(message))
            if !message.contains("request exceeds transport limit")
    ));
    assert!(matches!(
        AgentIpcClient::send_raw_frame(
            &socket,
            &vec![b'x'; AgentIpcServer::MAX_FRAME_BYTES + 1]
        ),
        Err(zentty_agent_ipc::AgentIpcError::InvalidRequest(message))
            if message == "request exceeds transport limit"
    ));

    let mut raw = UnixStream::connect(&socket).unwrap();
    raw.write_all(&vec![b'x'; AgentIpcServer::MAX_FRAME_BYTES + 1])
        .unwrap();
    raw.shutdown(std::net::Shutdown::Write).unwrap();
    let mut response = String::new();
    raw.read_to_string(&mut response).unwrap();
    assert!(response.contains("request exceeds transport limit"));

    server.shutdown().unwrap();
}

#[test]
fn product_command_failure_remains_an_exit_one_reply_not_a_transport_error() {
    let (_directory, socket) = temporary_socket();
    let mut registry = PaneTokenRegistry::default();
    registry
        .register("token", AgentTarget::new("window", "lane", "pane"))
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
    let handler = std::thread::spawn(move || {
        tmux_receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .respond(TmuxCompatReply::failure("unsupported", "popup is unsupported").unwrap())
            .unwrap();
    });

    let reply = AgentIpcClient::send_tmux(&socket, "token", "popup", &[], None, None).unwrap();
    assert!(!reply.is_ok());
    assert_eq!(reply.exit_code(), 1);
    assert_eq!(
        reply.error().map(|error| (error.code(), error.message())),
        Some(("unsupported", "popup is unsupported"))
    );

    handler.join().unwrap();
    server.shutdown().unwrap();
}
