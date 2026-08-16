#![forbid(unsafe_code)]

mod cli;
mod integrations;
mod launch;
mod server;

pub use cli::{CliProductCommand, parse_product_cli};
pub use integrations::{install_integration, uninstall_integration};
pub use launch::{LaunchError, launch_agent, resolve_real_binary};
pub use server::{
    ServerCommand, ServerIpcError, ServerIpcReply, ServerIpcReplyError, ServerIpcRequest,
};
pub use zentty_api::{
    ApplicationApiError, ApplicationOperation, ApplicationReply, ApplicationReplyError,
    ApplicationRequest, ApplicationScope, ProductIpcError, ProductIpcKind, ProductIpcReply,
    ProductIpcReplyError, ProductIpcRequest,
};

use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::fs::FileTypeExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use zentty_core::{
    AgentEvent, AgentTarget, AuthenticatedAgentEvent, PaneTokenError, PaneTokenRegistry,
};
use zentty_tmux_compat::{TmuxCompatReply, TmuxCompatRequest};

/// Creates a 256-bit pane capability token from the operating system CSPRNG.
///
/// # Errors
///
/// Returns an error if `/dev/urandom` cannot supply all 32 bytes.
pub fn generate_pane_token() -> Result<String, AgentIpcError> {
    let mut random = [0_u8; 32];
    File::open("/dev/urandom")?.read_exact(&mut random)?;
    let mut token = String::with_capacity(64);
    for byte in random {
        use std::fmt::Write as _;
        write!(&mut token, "{byte:02x}")
            .map_err(|error| AgentIpcError::InvalidRequest(error.to_string()))?;
    }
    Ok(token)
}

#[derive(Debug)]
pub enum AgentIpcError {
    Io(std::io::Error),
    InvalidRequest(String),
    Rejected(String),
    WorkerPanicked,
}

impl fmt::Display for AgentIpcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "agent IPC I/O failed: {error}"),
            Self::InvalidRequest(error) => write!(formatter, "invalid agent IPC request: {error}"),
            Self::Rejected(error) => write!(formatter, "agent IPC request rejected: {error}"),
            Self::WorkerPanicked => formatter.write_str("agent IPC worker panicked"),
        }
    }
}

impl std::error::Error for AgentIpcError {}

impl From<std::io::Error> for AgentIpcError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct WireRequest {
    version: u32,
    id: String,
    kind: String,
    arguments: Vec<String>,
    #[serde(rename = "standardInput")]
    standard_input: Option<String>,
    environment: std::collections::BTreeMap<String, String>,
    #[serde(rename = "expectsResponse")]
    expects_response: bool,
    subcommand: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct WireResponse {
    version: u32,
    id: String,
    ok: bool,
    result: Option<WireResponseResult>,
    error: Option<WireResponseError>,
}

#[derive(Debug, Deserialize, Serialize)]
struct WireResponseResult {
    stdout: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct WireResponseError {
    code: String,
    message: String,
}

pub struct AuthenticatedTmuxRequest {
    pub target: AgentTarget,
    pub request: TmuxCompatRequest,
    responder: mpsc::SyncSender<TmuxCompatReply>,
}

pub struct AuthenticatedServerRequest {
    pub target: AgentTarget,
    pub request: ServerIpcRequest,
    responder: mpsc::SyncSender<ServerIpcReply>,
}

pub struct AuthenticatedProductRequest {
    pub target: AgentTarget,
    pub request: ProductIpcRequest,
    responder: mpsc::SyncSender<ProductIpcReply>,
}

impl AuthenticatedProductRequest {
    /// Returns one bounded product-command result to the waiting CLI process.
    ///
    /// # Errors
    ///
    /// Returns an error if the client disconnected or timed out.
    pub fn respond(self, reply: ProductIpcReply) -> Result<(), AgentIpcError> {
        self.responder
            .send(reply)
            .map_err(|_| AgentIpcError::Rejected("product command client disconnected".to_owned()))
    }
}

impl AuthenticatedServerRequest {
    /// Returns the product result to the blocked server-command client.
    ///
    /// # Errors
    ///
    /// Returns an error if the client has disconnected or timed out.
    pub fn respond(self, reply: ServerIpcReply) -> Result<(), AgentIpcError> {
        self.responder.send(reply).map_err(|_| {
            AgentIpcError::Rejected("development-server client disconnected".to_owned())
        })
    }
}

impl AuthenticatedTmuxRequest {
    /// Returns a product result to the blocked compatibility client.
    ///
    /// # Errors
    ///
    /// Returns an error if the client has disconnected or timed out.
    pub fn respond(self, reply: TmuxCompatReply) -> Result<(), AgentIpcError> {
        self.responder.send(reply).map_err(|_| {
            AgentIpcError::Rejected("tmux compatibility client disconnected".to_owned())
        })
    }
}

pub struct AgentIpcServer {
    socket_path: PathBuf,
    running: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl AgentIpcServer {
    pub const MAX_FRAME_BYTES: usize = 384 * 1024;
    pub const MAX_FRAME_READ_BYTES: usize = Self::MAX_FRAME_BYTES + 1;
    pub const CONNECTION_TIMEOUT: Duration = Duration::from_millis(250);
    pub const TMUX_REPLY_TIMEOUT: Duration = Duration::from_secs(2);
    pub const CONNECTION_WORKERS: usize = 4;
    pub const MAX_PENDING_CONNECTIONS: usize = 32;

    /// Starts a private, pane-authenticated Unix-domain event server.
    ///
    /// # Errors
    ///
    /// Returns an error if the path already exists, its parent cannot be
    /// secured, or the socket cannot be bound.
    pub fn start(
        socket_path: impl AsRef<Path>,
        registry: Arc<Mutex<PaneTokenRegistry>>,
        sender: mpsc::Sender<AuthenticatedAgentEvent>,
    ) -> Result<Self, AgentIpcError> {
        Self::start_inner(socket_path, registry, sender, None, None, None)
    }

    /// Starts the existing event server with a tmux-compat product-command
    /// route on the same authenticated socket.
    ///
    /// # Errors
    ///
    /// Returns an error if the path already exists, its parent cannot be
    /// secured, or the socket cannot be bound.
    pub fn start_with_tmux(
        socket_path: impl AsRef<Path>,
        registry: Arc<Mutex<PaneTokenRegistry>>,
        sender: mpsc::Sender<AuthenticatedAgentEvent>,
        tmux_sender: mpsc::Sender<AuthenticatedTmuxRequest>,
    ) -> Result<Self, AgentIpcError> {
        Self::start_inner(socket_path, registry, sender, Some(tmux_sender), None, None)
    }

    /// Starts the one authenticated socket with both product command routes.
    ///
    /// # Errors
    ///
    /// Returns an error if the private endpoint cannot be created.
    pub fn start_with_product_routes(
        socket_path: impl AsRef<Path>,
        registry: Arc<Mutex<PaneTokenRegistry>>,
        sender: mpsc::Sender<AuthenticatedAgentEvent>,
        tmux_sender: mpsc::Sender<AuthenticatedTmuxRequest>,
        server_sender: mpsc::Sender<AuthenticatedServerRequest>,
    ) -> Result<Self, AgentIpcError> {
        Self::start_inner(
            socket_path,
            registry,
            sender,
            Some(tmux_sender),
            Some(server_sender),
            None,
        )
    }

    /// Starts the one private endpoint with every delivered CLI product route.
    ///
    /// # Errors
    ///
    /// Returns an error if the private endpoint cannot be created.
    pub fn start_with_cli_routes(
        socket_path: impl AsRef<Path>,
        registry: Arc<Mutex<PaneTokenRegistry>>,
        sender: mpsc::Sender<AuthenticatedAgentEvent>,
        tmux_sender: mpsc::Sender<AuthenticatedTmuxRequest>,
        server_sender: mpsc::Sender<AuthenticatedServerRequest>,
        product_sender: mpsc::Sender<AuthenticatedProductRequest>,
    ) -> Result<Self, AgentIpcError> {
        Self::start_inner(
            socket_path,
            registry,
            sender,
            Some(tmux_sender),
            Some(server_sender),
            Some(product_sender),
        )
    }

    fn start_inner(
        socket_path: impl AsRef<Path>,
        registry: Arc<Mutex<PaneTokenRegistry>>,
        sender: mpsc::Sender<AuthenticatedAgentEvent>,
        tmux_sender: Option<mpsc::Sender<AuthenticatedTmuxRequest>>,
        server_sender: Option<mpsc::Sender<AuthenticatedServerRequest>>,
        product_sender: Option<mpsc::Sender<AuthenticatedProductRequest>>,
    ) -> Result<Self, AgentIpcError> {
        let socket_path = socket_path.as_ref().to_owned();
        match fs::symlink_metadata(&socket_path) {
            Ok(_) => {
                return Err(AgentIpcError::Io(std::io::Error::new(
                    std::io::ErrorKind::AddrInUse,
                    "agent IPC socket path already exists",
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(AgentIpcError::Io(error)),
        }
        let parent = socket_path.parent().ok_or_else(|| {
            AgentIpcError::InvalidRequest("socket path has no parent directory".to_owned())
        })?;
        match fs::symlink_metadata(parent) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(AgentIpcError::InvalidRequest(
                    "socket parent must not be a symlink".to_owned(),
                ));
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(AgentIpcError::InvalidRequest(
                    "socket parent is not a directory".to_owned(),
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir_all(parent)?;
            }
            Err(error) => return Err(AgentIpcError::Io(error)),
        }
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        if fs::symlink_metadata(parent)?.permissions().mode() & 0o777 != 0o700 {
            return Err(AgentIpcError::InvalidRequest(
                "socket parent permissions are not private".to_owned(),
            ));
        }
        let listener = UnixListener::bind(&socket_path)?;
        fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600))?;
        let socket_metadata = fs::symlink_metadata(&socket_path)?;
        if !socket_metadata.file_type().is_socket()
            || socket_metadata.permissions().mode() & 0o777 != 0o600
        {
            let _ = fs::remove_file(&socket_path);
            return Err(AgentIpcError::InvalidRequest(
                "bound IPC endpoint is not a private Unix socket".to_owned(),
            ));
        }
        listener.set_nonblocking(true)?;

        let running = Arc::new(AtomicBool::new(true));
        let worker_running = Arc::clone(&running);
        let worker = thread::Builder::new()
            .name("zentty-agent-ipc".to_owned())
            .spawn(move || {
                serve(
                    &listener,
                    &worker_running,
                    &registry,
                    &sender,
                    tmux_sender.as_ref(),
                    server_sender.as_ref(),
                    product_sender.as_ref(),
                );
            })?;
        Ok(Self {
            socket_path,
            running,
            worker: Some(worker),
        })
    }

    /// Stops the worker and removes its socket.
    ///
    /// # Errors
    ///
    /// Returns an error if the worker panics or the owned socket cannot be
    /// removed.
    pub fn shutdown(mut self) -> Result<(), AgentIpcError> {
        self.stop_worker()?;
        if self.socket_path.exists() {
            fs::remove_file(&self.socket_path)?;
        }
        Ok(())
    }

    fn stop_worker(&mut self) -> Result<(), AgentIpcError> {
        self.running.store(false, Ordering::Release);
        let _ = UnixStream::connect(&self.socket_path);
        if self
            .worker
            .take()
            .is_some_and(|worker| worker.join().is_err())
        {
            return Err(AgentIpcError::WorkerPanicked);
        }
        Ok(())
    }
}

impl Drop for AgentIpcServer {
    fn drop(&mut self) {
        let _ = self.stop_worker();
        let _ = fs::remove_file(&self.socket_path);
    }
}

pub struct AgentIpcClient;

impl AgentIpcClient {
    /// Sends one canonical event through a real Unix-domain socket.
    ///
    /// The optional claimed target exists only to prove that server routing is
    /// derived from the pane token rather than client-controlled identifiers.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid UTF-8, serialization/I/O failure, or a
    /// server-side protocol/authentication rejection.
    pub fn send_event(
        socket_path: impl AsRef<Path>,
        pane_token: &str,
        event: &[u8],
        claimed_target: Option<AgentTarget>,
    ) -> Result<(), AgentIpcError> {
        let standard_input = std::str::from_utf8(event)
            .map_err(|error| AgentIpcError::InvalidRequest(error.to_string()))?
            .to_owned();
        let mut environment = std::collections::BTreeMap::from([(
            "ZENTTY_PANE_TOKEN".to_owned(),
            pane_token.to_owned(),
        )]);
        if let Some(target) = claimed_target {
            environment.insert("ZENTTY_WINDOW_ID".to_owned(), target.window_id);
            environment.insert("ZENTTY_WORKLANE_ID".to_owned(), target.worklane_id);
            environment.insert("ZENTTY_PANE_ID".to_owned(), target.pane_id);
        }
        let request = WireRequest {
            version: 1,
            id: request_id(),
            kind: "ipc".to_owned(),
            arguments: Vec::new(),
            standard_input: Some(standard_input),
            environment,
            expects_response: true,
            subcommand: Some("agent-event".to_owned()),
        };
        let frame = serde_json::to_vec(&request)
            .map_err(|error| AgentIpcError::InvalidRequest(error.to_string()))?;
        Self::send_raw_frame(socket_path, &frame)
    }

    /// Sends one bounded tmux-compat command through the existing authenticated
    /// agent IPC socket and waits for the product result.
    ///
    /// # Errors
    ///
    /// Rejects invalid compatibility payloads, transport failures, malformed
    /// responses, and authentication or routing failures.
    pub fn send_tmux(
        socket_path: impl AsRef<Path>,
        pane_token: &str,
        subcommand: &str,
        arguments: &[String],
        standard_input: Option<String>,
        claimed_target: Option<AgentTarget>,
    ) -> Result<TmuxCompatReply, AgentIpcError> {
        TmuxCompatRequest::new(1, subcommand, arguments.to_vec(), standard_input.clone())
            .map_err(|error| AgentIpcError::InvalidRequest(error.to_string()))?;
        let mut environment = std::collections::BTreeMap::from([(
            "ZENTTY_PANE_TOKEN".to_owned(),
            pane_token.to_owned(),
        )]);
        if let Some(target) = claimed_target {
            environment.insert("ZENTTY_WINDOW_ID".to_owned(), target.window_id);
            environment.insert("ZENTTY_WORKLANE_ID".to_owned(), target.worklane_id);
            environment.insert("ZENTTY_PANE_ID".to_owned(), target.pane_id);
        }
        let request = WireRequest {
            version: 1,
            id: request_id(),
            kind: "tmux_compat".to_owned(),
            arguments: arguments.to_vec(),
            standard_input,
            environment,
            expects_response: true,
            subcommand: Some(subcommand.to_owned()),
        };
        let frame = serde_json::to_vec(&request)
            .map_err(|error| AgentIpcError::InvalidRequest(error.to_string()))?;
        let response = Self::exchange_raw_frame(socket_path, &frame)?;
        if response.ok {
            let stdout = response
                .result
                .and_then(|result| result.stdout)
                .unwrap_or_default();
            TmuxCompatReply::success(stdout)
                .map_err(|error| AgentIpcError::InvalidRequest(error.to_string()))
        } else {
            let error = response.error.ok_or_else(|| {
                AgentIpcError::InvalidRequest("failed response omitted its error".to_owned())
            })?;
            if error.code == "request_rejected" {
                return Err(AgentIpcError::Rejected(error.message));
            }
            TmuxCompatReply::failure(error.code, error.message)
                .map_err(|error| AgentIpcError::InvalidRequest(error.to_string()))
        }
    }

    /// Sends one bounded development-server command through the pane's
    /// authenticated product socket.
    ///
    /// # Errors
    ///
    /// Rejects invalid payloads, transport/authentication failures, malformed
    /// responses, and replies outside the server protocol bounds.
    pub fn send_server(
        socket_path: impl AsRef<Path>,
        pane_token: &str,
        subcommand: &str,
        arguments: &[String],
        claimed_target: Option<AgentTarget>,
    ) -> Result<ServerIpcReply, AgentIpcError> {
        ServerIpcRequest::new(subcommand, arguments.to_vec())
            .map_err(|error| AgentIpcError::InvalidRequest(error.to_string()))?;
        let mut environment = std::collections::BTreeMap::from([(
            "ZENTTY_PANE_TOKEN".to_owned(),
            pane_token.to_owned(),
        )]);
        if let Some(target) = claimed_target {
            environment.insert("ZENTTY_WINDOW_ID".to_owned(), target.window_id);
            environment.insert("ZENTTY_WORKLANE_ID".to_owned(), target.worklane_id);
            environment.insert("ZENTTY_PANE_ID".to_owned(), target.pane_id);
        }
        let request = WireRequest {
            version: 1,
            id: request_id(),
            kind: "server".to_owned(),
            arguments: arguments.to_vec(),
            standard_input: None,
            environment,
            expects_response: true,
            subcommand: Some(subcommand.to_owned()),
        };
        let frame = serde_json::to_vec(&request)
            .map_err(|error| AgentIpcError::InvalidRequest(error.to_string()))?;
        let response = Self::exchange_raw_frame(socket_path, &frame)?;
        if response.ok {
            ServerIpcReply::success(
                response
                    .result
                    .and_then(|result| result.stdout)
                    .unwrap_or_default(),
            )
            .map_err(|error| AgentIpcError::InvalidRequest(error.to_string()))
        } else {
            let error = response.error.ok_or_else(|| {
                AgentIpcError::InvalidRequest("failed response omitted its error".to_owned())
            })?;
            if error.code == "request_rejected" {
                return Err(AgentIpcError::Rejected(error.message));
            }
            ServerIpcReply::failure(error.code, error.message)
                .map_err(|error| AgentIpcError::InvalidRequest(error.to_string()))
        }
    }

    /// Sends one bounded source-compatible discovery or topology command.
    ///
    /// # Errors
    ///
    /// Rejects invalid command payloads, authentication failures, malformed
    /// responses, and replies outside the product protocol bounds.
    pub fn send_product(
        socket_path: impl AsRef<Path>,
        pane_token: &str,
        kind: ProductIpcKind,
        subcommand: &str,
        arguments: &[String],
        claimed_target: Option<AgentTarget>,
    ) -> Result<ProductIpcReply, AgentIpcError> {
        ProductIpcRequest::new(kind, subcommand, arguments.to_vec())
            .map_err(|error| AgentIpcError::InvalidRequest(error.to_string()))?;
        let mut environment = std::collections::BTreeMap::from([(
            "ZENTTY_PANE_TOKEN".to_owned(),
            pane_token.to_owned(),
        )]);
        if let Some(target) = claimed_target {
            environment.insert("ZENTTY_WINDOW_ID".to_owned(), target.window_id);
            environment.insert("ZENTTY_WORKLANE_ID".to_owned(), target.worklane_id);
            environment.insert("ZENTTY_PANE_ID".to_owned(), target.pane_id);
        }
        let request = WireRequest {
            version: 1,
            id: request_id(),
            kind: kind.wire_name().to_owned(),
            arguments: arguments.to_vec(),
            standard_input: None,
            environment,
            expects_response: true,
            subcommand: Some(subcommand.to_owned()),
        };
        let frame = serde_json::to_vec(&request)
            .map_err(|error| AgentIpcError::InvalidRequest(error.to_string()))?;
        let response = Self::exchange_raw_frame(socket_path, &frame)?;
        if response.ok {
            ProductIpcReply::success(
                response
                    .result
                    .and_then(|result| result.stdout)
                    .unwrap_or_default(),
            )
            .map_err(|error| AgentIpcError::InvalidRequest(error.to_string()))
        } else {
            let error = response.error.ok_or_else(|| {
                AgentIpcError::InvalidRequest("failed response omitted its error".to_owned())
            })?;
            if error.code == "request_rejected" {
                return Err(AgentIpcError::Rejected(error.message));
            }
            ProductIpcReply::failure(error.code, error.message)
                .map_err(|error| AgentIpcError::InvalidRequest(error.to_string()))
        }
    }

    /// Sends an already encoded frame, primarily for negative transport tests.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized frames, I/O failures, malformed
    /// responses, or explicit server rejection.
    pub fn send_raw_frame(
        socket_path: impl AsRef<Path>,
        frame: &[u8],
    ) -> Result<(), AgentIpcError> {
        let response = Self::exchange_raw_frame(socket_path, frame)?;
        if response.ok {
            Ok(())
        } else {
            Err(AgentIpcError::Rejected(response.error.map_or_else(
                || "unknown error".to_owned(),
                |error| error.message,
            )))
        }
    }

    fn exchange_raw_frame(
        socket_path: impl AsRef<Path>,
        frame: &[u8],
    ) -> Result<WireResponse, AgentIpcError> {
        if frame.len() > AgentIpcServer::MAX_FRAME_BYTES {
            return Err(AgentIpcError::InvalidRequest(
                "request exceeds transport limit".to_owned(),
            ));
        }
        let mut stream = UnixStream::connect(socket_path)?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        stream.write_all(frame)?;
        stream.shutdown(std::net::Shutdown::Write)?;
        let mut response = Vec::new();
        stream
            .take(u64::try_from(AgentIpcServer::MAX_FRAME_READ_BYTES).unwrap_or(u64::MAX))
            .read_to_end(&mut response)?;
        let response: WireResponse = serde_json::from_slice(&response)
            .map_err(|error| AgentIpcError::InvalidRequest(error.to_string()))?;
        if response.version != 1 {
            return Err(AgentIpcError::InvalidRequest(format!(
                "unsupported response version {}",
                response.version
            )));
        }
        Ok(response)
    }
}

fn serve(
    listener: &UnixListener,
    running: &AtomicBool,
    registry: &Mutex<PaneTokenRegistry>,
    sender: &mpsc::Sender<AuthenticatedAgentEvent>,
    tmux_sender: Option<&mpsc::Sender<AuthenticatedTmuxRequest>>,
    server_sender: Option<&mpsc::Sender<AuthenticatedServerRequest>>,
    product_sender: Option<&mpsc::Sender<AuthenticatedProductRequest>>,
) {
    let (connections, receiver) = mpsc::sync_channel(AgentIpcServer::MAX_PENDING_CONNECTIONS);
    let receiver = Arc::new(Mutex::new(receiver));
    thread::scope(|scope| {
        for _ in 0..AgentIpcServer::CONNECTION_WORKERS {
            let receiver = Arc::clone(&receiver);
            scope.spawn(move || {
                while running.load(Ordering::Acquire) {
                    let stream = receiver
                        .lock()
                        .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
                        .and_then(|receiver| receiver.recv_timeout(Duration::from_millis(5)));
                    match stream {
                        Ok(stream) => {
                            handle_connection(
                                stream,
                                registry,
                                sender,
                                tmux_sender,
                                server_sender,
                                product_sender,
                            );
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            });
        }

        while running.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((stream, _)) => {
                    let _ = connections.try_send(stream);
                }
                Err(_) => thread::sleep(Duration::from_millis(5)),
            }
        }
    });
}

fn handle_connection(
    mut stream: UnixStream,
    registry: &Mutex<PaneTokenRegistry>,
    sender: &mpsc::Sender<AuthenticatedAgentEvent>,
    tmux_sender: Option<&mpsc::Sender<AuthenticatedTmuxRequest>>,
    server_sender: Option<&mpsc::Sender<AuthenticatedServerRequest>>,
    product_sender: Option<&mpsc::Sender<AuthenticatedProductRequest>>,
) {
    let _ = stream.set_read_timeout(Some(AgentIpcServer::CONNECTION_TIMEOUT));
    let _ = stream.set_write_timeout(Some(AgentIpcServer::CONNECTION_TIMEOUT));
    let result = receive_request(
        &mut stream,
        registry,
        sender,
        tmux_sender,
        server_sender,
        product_sender,
    );
    let response = match result {
        Ok(ReceivedResponse { id, reply: None }) => WireResponse {
            version: 1,
            id,
            ok: true,
            result: None,
            error: None,
        },
        Ok(ReceivedResponse {
            id,
            reply: Some(reply),
        }) if reply.error.is_none() => WireResponse {
            version: 1,
            id,
            ok: true,
            result: Some(WireResponseResult {
                stdout: reply.stdout,
            }),
            error: None,
        },
        Ok(ReceivedResponse {
            id,
            reply: Some(reply),
        }) => match reply.error {
            Some(error) => WireResponse {
                version: 1,
                id,
                ok: false,
                result: None,
                error: Some(WireResponseError {
                    code: error.code,
                    message: error.message,
                }),
            },
            None => WireResponse {
                version: 1,
                id,
                ok: false,
                result: None,
                error: Some(WireResponseError {
                    code: "invalid_product_reply".to_owned(),
                    message: "failed compatibility reply omitted its error".to_owned(),
                }),
            },
        },
        Err(error) => WireResponse {
            version: 1,
            id: String::new(),
            ok: false,
            result: None,
            error: Some(WireResponseError {
                code: "request_rejected".to_owned(),
                message: error.to_string(),
            }),
        },
    };
    if let Ok(bytes) = serde_json::to_vec(&response) {
        let _ = stream.write_all(&bytes);
    }
}

struct ReceivedResponse {
    id: String,
    reply: Option<ProductReply>,
}

struct ProductReply {
    stdout: Option<String>,
    error: Option<ProductReplyError>,
}

struct ProductReplyError {
    code: String,
    message: String,
}

impl From<TmuxCompatReply> for ProductReply {
    fn from(reply: TmuxCompatReply) -> Self {
        Self {
            stdout: reply.stdout().map(str::to_owned),
            error: reply.error().map(|error| ProductReplyError {
                code: error.code().to_owned(),
                message: error.message().to_owned(),
            }),
        }
    }
}

impl From<ServerIpcReply> for ProductReply {
    fn from(reply: ServerIpcReply) -> Self {
        Self {
            stdout: reply.stdout().map(str::to_owned),
            error: reply.error().map(|error| ProductReplyError {
                code: error.code().to_owned(),
                message: error.message().to_owned(),
            }),
        }
    }
}

impl From<ProductIpcReply> for ProductReply {
    fn from(reply: ProductIpcReply) -> Self {
        Self {
            stdout: reply.stdout().map(str::to_owned),
            error: reply.error().map(|error| ProductReplyError {
                code: error.code().to_owned(),
                message: error.message().to_owned(),
            }),
        }
    }
}

fn receive_request(
    stream: &mut UnixStream,
    registry: &Mutex<PaneTokenRegistry>,
    sender: &mpsc::Sender<AuthenticatedAgentEvent>,
    tmux_sender: Option<&mpsc::Sender<AuthenticatedTmuxRequest>>,
    server_sender: Option<&mpsc::Sender<AuthenticatedServerRequest>>,
    product_sender: Option<&mpsc::Sender<AuthenticatedProductRequest>>,
) -> Result<ReceivedResponse, AgentIpcError> {
    let mut frame = Vec::new();
    stream
        .take(u64::try_from(AgentIpcServer::MAX_FRAME_READ_BYTES).unwrap_or(u64::MAX))
        .read_to_end(&mut frame)?;
    if frame.len() > AgentIpcServer::MAX_FRAME_BYTES {
        return Err(AgentIpcError::InvalidRequest(
            "request exceeds transport limit".to_owned(),
        ));
    }
    let request: WireRequest = serde_json::from_slice(&frame)
        .map_err(|error| AgentIpcError::InvalidRequest(error.to_string()))?;
    validate_envelope(&request)?;
    let token = request
        .environment
        .get("ZENTTY_PANE_TOKEN")
        .ok_or_else(|| AgentIpcError::Rejected("missing pane token".to_owned()))?
        .clone();
    let registry = registry
        .lock()
        .map_err(|_| AgentIpcError::Rejected("pane registry unavailable".to_owned()))?;
    match (request.kind.as_str(), request.subcommand.clone()) {
        ("ipc", Some(subcommand)) if subcommand == "agent-event" => {
            let standard_input = request.standard_input.ok_or_else(|| {
                AgentIpcError::Rejected("agent event omitted standard input".to_owned())
            })?;
            let event = AgentEvent::parse(standard_input.as_bytes())
                .map_err(|error| AgentIpcError::Rejected(error.to_string()))?;
            let authenticated = registry
                .authenticate(&token, event)
                .map_err(pane_token_rejection)?;
            drop(registry);
            sender.send(authenticated).map_err(|_| {
                AgentIpcError::Rejected("application event receiver unavailable".to_owned())
            })?;
            Ok(ReceivedResponse {
                id: request.id,
                reply: None,
            })
        }
        ("tmux_compat", Some(subcommand)) => {
            let target = registry
                .authenticate_target(&token)
                .map_err(pane_token_rejection)?;
            drop(registry);
            let payload = TmuxCompatRequest::new(
                request.version,
                &subcommand,
                request.arguments,
                request.standard_input,
            )
            .map_err(|error| AgentIpcError::Rejected(error.to_string()))?;
            let tmux_sender = tmux_sender.ok_or_else(|| {
                AgentIpcError::Rejected("tmux compatibility handler unavailable".to_owned())
            })?;
            let (responder, response) = mpsc::sync_channel(1);
            tmux_sender
                .send(AuthenticatedTmuxRequest {
                    target,
                    request: payload,
                    responder,
                })
                .map_err(|_| {
                    AgentIpcError::Rejected(
                        "tmux compatibility product receiver unavailable".to_owned(),
                    )
                })?;
            let reply = response
                .recv_timeout(AgentIpcServer::TMUX_REPLY_TIMEOUT)
                .map_err(|_| {
                    AgentIpcError::Rejected("tmux compatibility response timed out".to_owned())
                })?;
            Ok(ReceivedResponse {
                id: request.id,
                reply: Some(reply.into()),
            })
        }
        ("server", Some(subcommand)) => {
            let target = registry
                .authenticate_target(&token)
                .map_err(pane_token_rejection)?;
            drop(registry);
            receive_server_request(request, target, &subcommand, server_sender)
        }
        ("discover" | "pane", Some(subcommand)) => {
            let target = registry
                .authenticate_target(&token)
                .map_err(pane_token_rejection)?;
            drop(registry);
            receive_product_request(request, target, &subcommand, product_sender)
        }
        _ => Err(AgentIpcError::Rejected("unsupported IPC route".to_owned())),
    }
}

fn receive_product_request(
    request: WireRequest,
    target: AgentTarget,
    subcommand: &str,
    product_sender: Option<&mpsc::Sender<AuthenticatedProductRequest>>,
) -> Result<ReceivedResponse, AgentIpcError> {
    let kind = if request.kind == "discover" {
        ProductIpcKind::Discover
    } else {
        ProductIpcKind::Pane
    };
    let payload = ProductIpcRequest::new(kind, subcommand, request.arguments)
        .map_err(|error| AgentIpcError::Rejected(error.to_string()))?;
    let product_sender = product_sender
        .ok_or_else(|| AgentIpcError::Rejected("product command handler unavailable".to_owned()))?;
    let (responder, response) = mpsc::sync_channel(1);
    product_sender
        .send(AuthenticatedProductRequest {
            target,
            request: payload,
            responder,
        })
        .map_err(|_| AgentIpcError::Rejected("product command receiver unavailable".to_owned()))?;
    let reply = response
        .recv_timeout(AgentIpcServer::TMUX_REPLY_TIMEOUT)
        .map_err(|_| AgentIpcError::Rejected("product command response timed out".to_owned()))?;
    Ok(ReceivedResponse {
        id: request.id,
        reply: Some(reply.into()),
    })
}

fn receive_server_request(
    request: WireRequest,
    target: AgentTarget,
    subcommand: &str,
    server_sender: Option<&mpsc::Sender<AuthenticatedServerRequest>>,
) -> Result<ReceivedResponse, AgentIpcError> {
    let payload = ServerIpcRequest::new(subcommand, request.arguments)
        .map_err(|error| AgentIpcError::Rejected(error.to_string()))?;
    let server_sender = server_sender.ok_or_else(|| {
        AgentIpcError::Rejected("development-server handler unavailable".to_owned())
    })?;
    let (responder, response) = mpsc::sync_channel(1);
    server_sender
        .send(AuthenticatedServerRequest {
            target,
            request: payload,
            responder,
        })
        .map_err(|_| {
            AgentIpcError::Rejected("development-server product receiver unavailable".to_owned())
        })?;
    let reply = response
        .recv_timeout(AgentIpcServer::TMUX_REPLY_TIMEOUT)
        .map_err(|_| AgentIpcError::Rejected("development-server response timed out".to_owned()))?;
    Ok(ReceivedResponse {
        id: request.id,
        reply: Some(reply.into()),
    })
}

fn validate_envelope(request: &WireRequest) -> Result<(), AgentIpcError> {
    if request.version != 1 {
        return Err(AgentIpcError::Rejected(format!(
            "unsupported IPC version {}",
            request.version
        )));
    }
    if !request.expects_response {
        return Err(AgentIpcError::Rejected(
            "IPC request must request a response".to_owned(),
        ));
    }
    Ok(())
}

fn pane_token_rejection(error: PaneTokenError) -> AgentIpcError {
    AgentIpcError::Rejected(error.to_string())
}

fn request_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("{}-{nanos}", std::process::id())
}
