use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;
use zentty_agent_ipc::{AgentIpcClient, AgentIpcServer, generate_pane_token};
use zentty_core::{AgentPhase, AgentStatusStore, AgentTarget, PaneTokenRegistry};

fn temporary_socket() -> (std::path::PathBuf, std::path::PathBuf) {
    let directory = std::env::temp_dir().join(format!(
        "zentty-agent-ipc-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir_all(&directory).unwrap();
    let socket = directory.join("instance.sock");
    (directory, socket)
}

#[test]
fn pane_tokens_are_distinct_256_bit_os_random_capabilities() {
    let first = generate_pane_token().unwrap();
    let second = generate_pane_token().unwrap();
    assert_eq!(first.len(), 64);
    assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_ne!(first, second);
}

#[test]
fn real_unix_socket_authenticates_and_canonicalizes_the_target() {
    let (_directory, socket) = temporary_socket();
    let canonical = AgentTarget::new("window-a", "lane-a", "pane-a");
    let mut registry = PaneTokenRegistry::default();
    registry.register("token-a", canonical.clone()).unwrap();
    let registry = Arc::new(Mutex::new(registry));
    let (sender, receiver) = mpsc::channel();
    let server = AgentIpcServer::start(&socket, registry, sender).unwrap();

    let event = br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"session-a"}}"#;
    AgentIpcClient::send_event(
        &socket,
        "token-a",
        event,
        Some(AgentTarget::new("spoofed", "spoofed", "spoofed")),
    )
    .unwrap();
    let authenticated = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(authenticated.target, canonical);
    let mut statuses = AgentStatusStore::default();
    statuses.apply(authenticated, 1);
    assert_eq!(
        statuses.status_for(&canonical).unwrap().phase,
        AgentPhase::Running
    );
    assert_eq!(
        fs::metadata(&socket).unwrap().permissions().mode() & 0o777,
        0o600
    );

    server.shutdown().unwrap();
    assert!(!socket.exists());
}

#[test]
fn real_server_rejects_wrong_tokens_malformed_events_and_oversized_frames() {
    let (_directory, socket) = temporary_socket();
    let mut registry = PaneTokenRegistry::default();
    registry
        .register("token-a", AgentTarget::new("window-a", "lane-a", "pane-a"))
        .unwrap();
    let (sender, receiver) = mpsc::channel();
    let server = AgentIpcServer::start(&socket, Arc::new(Mutex::new(registry)), sender).unwrap();

    assert!(
        AgentIpcClient::send_event(
            &socket,
            "wrong",
            br#"{"version":1,"event":"agent.idle"}"#,
            None
        )
        .is_err()
    );
    assert!(AgentIpcClient::send_event(&socket, "token-a", b"not-json", None).is_err());
    assert!(
        AgentIpcClient::send_raw_frame(&socket, &vec![b'x'; AgentIpcServer::MAX_FRAME_BYTES + 1])
            .is_err()
    );
    assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());

    server.shutdown().unwrap();
}

#[test]
fn stalled_client_is_bounded_and_cannot_permanently_block_later_events() {
    let (_directory, socket) = temporary_socket();
    let canonical = AgentTarget::new("window-a", "lane-a", "pane-a");
    let mut registry = PaneTokenRegistry::default();
    registry.register("token-a", canonical.clone()).unwrap();
    let (sender, receiver) = mpsc::channel();
    let server = AgentIpcServer::start(&socket, Arc::new(Mutex::new(registry)), sender).unwrap();

    let stalled = UnixStream::connect(&socket).unwrap();
    std::thread::sleep(AgentIpcServer::CONNECTION_TIMEOUT + Duration::from_millis(100));
    AgentIpcClient::send_event(
        &socket,
        "token-a",
        br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"}}"#,
        None,
    )
    .unwrap();
    assert_eq!(
        receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .target,
        canonical
    );

    drop(stalled);
    server.shutdown().unwrap();
}
