#![forbid(unsafe_code)]

mod cli;
mod discovery;
mod ingress;
mod integrations;
mod launch;
mod presentation;
mod server;

pub use cli::{CliProductCommand, parse_product_cli};
pub use discovery::{
    DiscoveredInstance, InstanceCredential, discover_instances, publish_instance,
    publish_pane_credential, remove_pane_credential,
};
pub use ingress::{
    IngressMessage, IngressPressure, IngressReceiver, IngressSendError, IngressSender,
    ingress_channel,
};
pub use integrations::{install_integration, uninstall_integration};
pub use launch::{LaunchError, launch_agent, resolve_real_binary};
pub use presentation::render_application_result;
pub use server::{
    ServerCommand, ServerIpcError, ServerIpcReply, ServerIpcReplyError, ServerIpcRequest,
};
pub use zentty_api::{
    APPLICATION_API_VERSION, ApplicationApiError, ApplicationAuthority, ApplicationErrorCategory,
    ApplicationOperation, ApplicationReply, ApplicationReplyError, ApplicationRequest,
    ApplicationResult, ApplicationResultKind, ApplicationScope, ApplicationTarget, ProductIpcError,
    ProductIpcKind, ProductIpcReply, ProductIpcReplyError, ProductIpcRequest,
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
    AgentEvent, AgentTarget, AuthenticatedAgentEvent, CapabilityAuthority, PaneTokenError,
    PaneTokenRegistry,
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
    Authorization(String),
    UnsupportedVersion(u32),
    Remote {
        category: ApplicationErrorCategory,
        code: String,
        message: String,
    },
    Rejected(String),
    WorkerPanicked,
}

impl fmt::Display for AgentIpcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "agent IPC I/O failed: {error}"),
            Self::InvalidRequest(error) => write!(formatter, "invalid agent IPC request: {error}"),
            Self::Authorization(error) => {
                write!(formatter, "application capability rejected: {error}")
            }
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported IPC version {version}")
            }
            Self::Remote {
                category,
                code,
                message,
            } => write!(
                formatter,
                "application transport rejected request ({category:?}/{code}): {message}"
            ),
            Self::Rejected(error) => write!(formatter, "agent IPC request rejected: {error}"),
            Self::WorkerPanicked => formatter.write_str("agent IPC worker panicked"),
        }
    }
}

impl AgentIpcError {
    #[must_use]
    pub fn category(&self) -> ApplicationErrorCategory {
        match self {
            Self::InvalidRequest(_) => ApplicationErrorCategory::InvalidArguments,
            Self::Authorization(_) => ApplicationErrorCategory::AuthorizationFailure,
            Self::UnsupportedVersion(_) => ApplicationErrorCategory::UnsupportedVersion,
            Self::Remote { category, .. } => *category,
            Self::Rejected(_) => ApplicationErrorCategory::ProductRejection,
            Self::WorkerPanicked => ApplicationErrorCategory::PermanentTransportFailure,
            Self::Io(error) => match error.kind() {
                std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused => {
                    ApplicationErrorCategory::StaleInstance
                }
                std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::ConnectionAborted
                | std::io::ErrorKind::BrokenPipe => {
                    ApplicationErrorCategory::RetryableInstanceReplacement
                }
                _ => ApplicationErrorCategory::PermanentTransportFailure,
            },
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
    #[serde(
        rename = "applicationApiVersion",
        skip_serializing_if = "Option::is_none"
    )]
    application_api_version: Option<u32>,
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
    #[serde(
        rename = "applicationApiVersion",
        skip_serializing_if = "Option::is_none"
    )]
    application_api_version: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    capabilities: Vec<String>,
    id: String,
    ok: bool,
    result: Option<WireResponseResult>,
    error: Option<WireResponseError>,
}

#[derive(Debug, Deserialize, Serialize)]
struct WireResponseResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    application: Option<ApplicationResult>,
}

#[derive(Debug, Deserialize, Serialize)]
struct WireResponseError {
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<ApplicationErrorCategory>,
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
    pub target: ApplicationTarget,
    pub authority: ApplicationAuthority,
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
    pub const APPLICATION_REPLY_TIMEOUT: Duration = Duration::from_secs(5);
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
        sender: IngressSender<AuthenticatedAgentEvent>,
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
        sender: IngressSender<AuthenticatedAgentEvent>,
        tmux_sender: IngressSender<AuthenticatedTmuxRequest>,
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
        sender: IngressSender<AuthenticatedAgentEvent>,
        tmux_sender: IngressSender<AuthenticatedTmuxRequest>,
        server_sender: IngressSender<AuthenticatedServerRequest>,
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
        sender: IngressSender<AuthenticatedAgentEvent>,
        tmux_sender: IngressSender<AuthenticatedTmuxRequest>,
        server_sender: IngressSender<AuthenticatedServerRequest>,
        product_sender: IngressSender<AuthenticatedProductRequest>,
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
        sender: IngressSender<AuthenticatedAgentEvent>,
        tmux_sender: Option<IngressSender<AuthenticatedTmuxRequest>>,
        server_sender: Option<IngressSender<AuthenticatedServerRequest>>,
        product_sender: Option<IngressSender<AuthenticatedProductRequest>>,
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
            application_api_version: None,
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
            application_api_version: None,
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
            if matches!(error.code.as_str(), "request_rejected" | "ingress_full") {
                return Err(AgentIpcError::Remote {
                    category: error
                        .category
                        .unwrap_or(ApplicationErrorCategory::ProductRejection),
                    code: error.code,
                    message: error.message,
                });
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
            application_api_version: None,
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
            if matches!(error.code.as_str(), "request_rejected" | "ingress_full") {
                return Err(AgentIpcError::Rejected(error.message));
            }
            ServerIpcReply::failure(error.code, error.message)
                .map_err(|error| AgentIpcError::InvalidRequest(error.to_string()))
        }
    }

    /// Sends one bounded application API request through the authenticated
    /// Unix transport.
    ///
    /// # Errors
    ///
    /// Rejects invalid command payloads, authentication failures, malformed
    /// responses, and replies outside the product protocol bounds.
    pub fn send_application(
        socket_path: impl AsRef<Path>,
        pane_token: &str,
        request: &ApplicationRequest,
        claimed_target: Option<AgentTarget>,
    ) -> Result<ProductIpcReply, AgentIpcError> {
        let requested_operation = request.operation();
        let requested_name = request.subcommand().to_owned();
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
            application_api_version: Some(APPLICATION_API_VERSION),
            id: request_id(),
            kind: request.kind().wire_name().to_owned(),
            arguments: request.arguments().to_vec(),
            standard_input: None,
            environment,
            expects_response: true,
            subcommand: Some(request.subcommand().to_owned()),
        };
        let frame = serde_json::to_vec(&request)
            .map_err(|error| AgentIpcError::InvalidRequest(error.to_string()))?;
        let response = Self::exchange_raw_frame_with_timeout(
            socket_path,
            &frame,
            AgentIpcServer::APPLICATION_REPLY_TIMEOUT,
        )?;
        if response
            .application_api_version
            .is_some_and(|version| version != APPLICATION_API_VERSION)
        {
            return Err(AgentIpcError::Remote {
                category: ApplicationErrorCategory::UnsupportedVersion,
                code: "unsupported_version".to_owned(),
                message: format!(
                    "server application API version {:?} is incompatible with client version {}",
                    response.application_api_version, APPLICATION_API_VERSION
                ),
            });
        }
        if !response.capabilities.is_empty()
            && requested_operation != ApplicationOperation::ShellSignal
            && !response
                .capabilities
                .iter()
                .any(|capability| capability == &requested_name)
        {
            return Err(AgentIpcError::Remote {
                category: ApplicationErrorCategory::UnsupportedOperation,
                code: "unsupported_operation".to_owned(),
                message: format!(
                    "server did not advertise application operation {requested_name:?}"
                ),
            });
        }
        if response.ok {
            let result = response
                .result
                .and_then(|result| result.application)
                .ok_or_else(|| {
                    AgentIpcError::InvalidRequest(
                        "successful application response omitted its structured result".to_owned(),
                    )
                })?;
            ProductIpcReply::success(result)
                .map_err(|error| AgentIpcError::InvalidRequest(error.to_string()))
        } else {
            let error = response.error.ok_or_else(|| {
                AgentIpcError::InvalidRequest("failed response omitted its error".to_owned())
            })?;
            if matches!(error.code.as_str(), "request_rejected" | "ingress_full") {
                return Err(AgentIpcError::Remote {
                    category: error
                        .category
                        .unwrap_or(ApplicationErrorCategory::ProductRejection),
                    code: error.code,
                    message: error.message,
                });
            }
            let category = error.category;
            let reply = ProductIpcReply::failure(error.code, error.message)
                .map_err(|error| AgentIpcError::InvalidRequest(error.to_string()))?;
            if category
                .zip(reply.error().map(ApplicationReplyError::category))
                .is_some_and(|(documented, derived)| documented != derived)
            {
                return Err(AgentIpcError::InvalidRequest(
                    "response error category contradicts its stable code".to_owned(),
                ));
            }
            Ok(reply)
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
            match response.error {
                Some(error) if error.code == "ingress_full" => Err(AgentIpcError::Remote {
                    category: error
                        .category
                        .unwrap_or(ApplicationErrorCategory::ProductUnavailable),
                    code: error.code,
                    message: error.message,
                }),
                error => Err(AgentIpcError::Rejected(
                    error.map_or_else(|| "unknown error".to_owned(), |error| error.message),
                )),
            }
        }
    }

    fn exchange_raw_frame(
        socket_path: impl AsRef<Path>,
        frame: &[u8],
    ) -> Result<WireResponse, AgentIpcError> {
        Self::exchange_raw_frame_with_timeout(
            socket_path,
            frame,
            AgentIpcServer::TMUX_REPLY_TIMEOUT,
        )
    }

    fn exchange_raw_frame_with_timeout(
        socket_path: impl AsRef<Path>,
        frame: &[u8],
        response_timeout: Duration,
    ) -> Result<WireResponse, AgentIpcError> {
        if frame.len() > AgentIpcServer::MAX_FRAME_BYTES {
            return Err(AgentIpcError::InvalidRequest(
                "request exceeds transport limit".to_owned(),
            ));
        }
        let mut stream = UnixStream::connect(socket_path)?;
        stream.set_read_timeout(Some(response_timeout))?;
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
    sender: &IngressSender<AuthenticatedAgentEvent>,
    tmux_sender: Option<&IngressSender<AuthenticatedTmuxRequest>>,
    server_sender: Option<&IngressSender<AuthenticatedServerRequest>>,
    product_sender: Option<&IngressSender<AuthenticatedProductRequest>>,
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
    sender: &IngressSender<AuthenticatedAgentEvent>,
    tmux_sender: Option<&IngressSender<AuthenticatedTmuxRequest>>,
    server_sender: Option<&IngressSender<AuthenticatedServerRequest>>,
    product_sender: Option<&IngressSender<AuthenticatedProductRequest>>,
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
            application_api_version: Some(APPLICATION_API_VERSION),
            capabilities: application_capabilities(),
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
            application_api_version: Some(APPLICATION_API_VERSION),
            capabilities: application_capabilities(),
            id,
            ok: true,
            result: Some(WireResponseResult {
                stdout: reply.stdout,
                application: reply.application,
            }),
            error: None,
        },
        Ok(ReceivedResponse {
            id,
            reply: Some(reply),
        }) => match reply.error {
            Some(error) => WireResponse {
                version: 1,
                application_api_version: Some(APPLICATION_API_VERSION),
                capabilities: application_capabilities(),
                id,
                ok: false,
                result: None,
                error: Some(WireResponseError {
                    category: error.category,
                    code: error.code,
                    message: error.message,
                }),
            },
            None => WireResponse {
                version: 1,
                application_api_version: Some(APPLICATION_API_VERSION),
                capabilities: application_capabilities(),
                id,
                ok: false,
                result: None,
                error: Some(WireResponseError {
                    category: Some(ApplicationErrorCategory::ProductRejection),
                    code: "invalid_product_reply".to_owned(),
                    message: "failed compatibility reply omitted its error".to_owned(),
                }),
            },
        },
        Err(error) => WireResponse {
            version: 1,
            application_api_version: Some(APPLICATION_API_VERSION),
            capabilities: application_capabilities(),
            id: String::new(),
            ok: false,
            result: None,
            error: Some(WireResponseError {
                category: Some(match error {
                    AgentIpcError::InvalidRequest(_) => ApplicationErrorCategory::InvalidArguments,
                    AgentIpcError::Authorization(_) => {
                        ApplicationErrorCategory::AuthorizationFailure
                    }
                    AgentIpcError::UnsupportedVersion(_) => {
                        ApplicationErrorCategory::UnsupportedVersion
                    }
                    AgentIpcError::Io(_) | AgentIpcError::WorkerPanicked => {
                        ApplicationErrorCategory::PermanentTransportFailure
                    }
                    AgentIpcError::Remote { category, .. } => category,
                    AgentIpcError::Rejected(_) => ApplicationErrorCategory::ProductRejection,
                }),
                code: match &error {
                    AgentIpcError::Remote { code, .. } => code.clone(),
                    _ => "request_rejected".to_owned(),
                },
                message: error.to_string(),
            }),
        },
    };
    if let Ok(bytes) = serde_json::to_vec(&response) {
        let _ = stream.write_all(&bytes);
    }
}

fn application_capabilities() -> Vec<String> {
    ApplicationOperation::ALL
        .into_iter()
        .filter(|operation| *operation != ApplicationOperation::ShellSignal)
        .map(|operation| operation.wire_name().to_owned())
        .collect()
}

struct ReceivedResponse {
    id: String,
    reply: Option<ProductReply>,
}

struct ProductReply {
    stdout: Option<String>,
    application: Option<ApplicationResult>,
    error: Option<ProductReplyError>,
}

struct ProductReplyError {
    category: Option<ApplicationErrorCategory>,
    code: String,
    message: String,
}

impl From<TmuxCompatReply> for ProductReply {
    fn from(reply: TmuxCompatReply) -> Self {
        Self {
            stdout: reply.stdout().map(str::to_owned),
            application: None,
            error: reply.error().map(|error| ProductReplyError {
                category: Some(ApplicationErrorCategory::ProductRejection),
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
            application: None,
            error: reply.error().map(|error| ProductReplyError {
                category: Some(ApplicationErrorCategory::ProductRejection),
                code: error.code().to_owned(),
                message: error.message().to_owned(),
            }),
        }
    }
}

impl From<ProductIpcReply> for ProductReply {
    fn from(reply: ProductIpcReply) -> Self {
        Self {
            stdout: None,
            application: reply.result().cloned(),
            error: reply.error().map(|error| ProductReplyError {
                category: Some(error.category()),
                code: error.code().to_owned(),
                message: error.message().to_owned(),
            }),
        }
    }
}

fn receive_request(
    stream: &mut UnixStream,
    registry: &Mutex<PaneTokenRegistry>,
    sender: &IngressSender<AuthenticatedAgentEvent>,
    tmux_sender: Option<&IngressSender<AuthenticatedTmuxRequest>>,
    server_sender: Option<&IngressSender<AuthenticatedServerRequest>>,
    product_sender: Option<&IngressSender<AuthenticatedProductRequest>>,
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
        .ok_or_else(|| AgentIpcError::Authorization("missing pane token".to_owned()))?
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
            sender.send(authenticated).map_err(ingress_rejection)?;
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
                .map_err(ingress_rejection)?;
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
            let authenticated = registry
                .authenticate_application_target(&token)
                .map_err(pane_token_rejection)?;
            drop(registry);
            receive_product_request(
                request,
                authenticated.target,
                authenticated.authority,
                &subcommand,
                product_sender,
            )
        }
        _ => Err(AgentIpcError::Rejected("unsupported IPC route".to_owned())),
    }
}

fn receive_product_request(
    request: WireRequest,
    target: AgentTarget,
    authority: CapabilityAuthority,
    subcommand: &str,
    product_sender: Option<&IngressSender<AuthenticatedProductRequest>>,
) -> Result<ReceivedResponse, AgentIpcError> {
    if let Some(version) = request.application_api_version
        && version != APPLICATION_API_VERSION
    {
        return Err(AgentIpcError::UnsupportedVersion(version));
    }
    let kind = if request.kind == "discover" {
        ProductIpcKind::Discover
    } else {
        ProductIpcKind::Pane
    };
    if authority == CapabilityAuthority::Instance && kind == ProductIpcKind::Pane {
        return Err(AgentIpcError::Authorization(
            "instance discovery capability requires explicit pane selection".to_owned(),
        ));
    }
    let payload = ProductIpcRequest::new(kind, subcommand, request.arguments)
        .map_err(|error| AgentIpcError::Rejected(error.to_string()))?;
    let product_sender = product_sender
        .ok_or_else(|| AgentIpcError::Rejected("product command handler unavailable".to_owned()))?;
    let (responder, response) = mpsc::sync_channel(1);
    product_sender
        .send(AuthenticatedProductRequest {
            target: ApplicationTarget::new(target.window_id, target.worklane_id, target.pane_id),
            authority: match authority {
                CapabilityAuthority::Pane => ApplicationAuthority::Pane,
                CapabilityAuthority::Instance => ApplicationAuthority::Instance,
            },
            request: payload,
            responder,
        })
        .map_err(ingress_rejection)?;
    let reply = response
        .recv_timeout(AgentIpcServer::APPLICATION_REPLY_TIMEOUT)
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
    server_sender: Option<&IngressSender<AuthenticatedServerRequest>>,
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
        .map_err(ingress_rejection)?;
    let reply = response
        .recv_timeout(AgentIpcServer::TMUX_REPLY_TIMEOUT)
        .map_err(|_| AgentIpcError::Rejected("development-server response timed out".to_owned()))?;
    Ok(ReceivedResponse {
        id: request.id,
        reply: Some(reply.into()),
    })
}

fn ingress_rejection<T: IngressMessage>(error: IngressSendError<T>) -> AgentIpcError {
    match error {
        IngressSendError::Full(message) => AgentIpcError::Remote {
            category: ApplicationErrorCategory::ProductUnavailable,
            code: "ingress_full".to_owned(),
            message: format!(
                "application ingress full for pane {}; request was not accepted",
                message.pane_id()
            ),
        },
        IngressSendError::Disconnected(_) => {
            AgentIpcError::Rejected("application ingress receiver unavailable".to_owned())
        }
    }
}

fn validate_envelope(request: &WireRequest) -> Result<(), AgentIpcError> {
    if request.version != 1 {
        return Err(AgentIpcError::UnsupportedVersion(request.version));
    }
    if !request.expects_response {
        return Err(AgentIpcError::Rejected(
            "IPC request must request a response".to_owned(),
        ));
    }
    Ok(())
}

fn pane_token_rejection(error: PaneTokenError) -> AgentIpcError {
    AgentIpcError::Authorization(error.to_string())
}

fn request_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("{}-{nanos}", std::process::id())
}
