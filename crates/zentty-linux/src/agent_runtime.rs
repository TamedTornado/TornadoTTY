use std::collections::BTreeMap;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use zentty_agent_ipc::{AgentIpcServer, AuthenticatedTmuxRequest, generate_pane_token};
use zentty_core::{AgentTarget, AuthenticatedAgentEvent, PaneTokenRegistry};

pub(crate) struct AgentRuntime {
    server: Option<AgentIpcServer>,
    registry: Arc<Mutex<PaneTokenRegistry>>,
    receiver: mpsc::Receiver<AuthenticatedAgentEvent>,
    tmux_receiver: mpsc::Receiver<AuthenticatedTmuxRequest>,
    tokens_by_pane: BTreeMap<String, String>,
    worklane_by_pane: BTreeMap<String, String>,
    runtime_directory: PathBuf,
    socket_path: PathBuf,
    cli_path: PathBuf,
    wrapper_directories: Vec<PathBuf>,
    window_id: String,
}

impl AgentRuntime {
    pub(crate) fn start(window_id: impl Into<String>) -> Result<Self, String> {
        let window_id = window_id.into();
        let instance = generate_pane_token().map_err(|error| error.to_string())?;
        let runtime_directory = instance_runtime_directory(
            std::env::var_os("XDG_RUNTIME_DIR").as_deref(),
            &std::env::temp_dir(),
            std::process::id(),
            &instance[..32],
        );
        let socket_path = runtime_directory.join("instance.sock");
        let registry = Arc::new(Mutex::new(PaneTokenRegistry::default()));
        let (sender, receiver) = mpsc::channel();
        let (tmux_sender, tmux_receiver) = mpsc::channel();
        let server = AgentIpcServer::start_with_tmux(
            &socket_path,
            Arc::clone(&registry),
            sender,
            tmux_sender,
        )
        .map_err(|error| error.to_string())?;
        eprintln!(
            "zentty-linux: agent-runtime socket={}",
            socket_path.display()
        );
        let cli_path = std::env::current_exe()
            .map_err(|error| format!("could not resolve Zentty executable: {error}"))?
            .with_file_name("zentty");
        let wrapper_root = cli_path
            .parent()
            .and_then(std::path::Path::parent)
            .map(|root| root.join("libexec/zentty/agent-wrappers"));
        let wrapper_directories = wrapper_root
            .map(|root| enabled_wrapper_directories(&root, &current_path()))
            .unwrap_or_default();
        Ok(Self {
            server: Some(server),
            registry,
            receiver,
            tmux_receiver,
            tokens_by_pane: BTreeMap::new(),
            worklane_by_pane: BTreeMap::new(),
            runtime_directory,
            socket_path,
            cli_path,
            wrapper_directories,
            window_id,
        })
    }

    pub(crate) fn environment_for_pane(
        &mut self,
        worklane_id: &str,
        pane_id: &str,
    ) -> Result<Vec<(String, String)>, String> {
        let target = AgentTarget::new(&self.window_id, worklane_id, pane_id);
        let token = if let Some(token) = self.tokens_by_pane.get(pane_id) {
            self.registry
                .lock()
                .map_err(|_| "agent pane registry is unavailable".to_owned())?
                .retarget(token, target)
                .map_err(|error| error.to_string())?;
            token.clone()
        } else {
            let token = generate_pane_token().map_err(|error| error.to_string())?;
            self.registry
                .lock()
                .map_err(|_| "agent pane registry is unavailable".to_owned())?
                .register(&token, target)
                .map_err(|error| error.to_string())?;
            self.tokens_by_pane
                .insert(pane_id.to_owned(), token.clone());
            token
        };
        self.worklane_by_pane
            .insert(pane_id.to_owned(), worklane_id.to_owned());
        let cli = self.cli_path.to_string_lossy().into_owned();
        let mut environment = vec![
            ("ZENTTY_CLI_BIN".to_owned(), cli.clone()),
            (
                "ZENTTY_AGENT_EVENT_COMMAND".to_owned(),
                format!("{cli} ipc agent-event"),
            ),
            (
                "ZENTTY_INSTANCE_SOCKET".to_owned(),
                self.socket_path.to_string_lossy().into_owned(),
            ),
            ("ZENTTY_PANE_TOKEN".to_owned(), token),
            ("ZENTTY_WINDOW_ID".to_owned(), self.window_id.clone()),
            ("ZENTTY_WORKLANE_ID".to_owned(), worklane_id.to_owned()),
            ("ZENTTY_PANE_ID".to_owned(), pane_id.to_owned()),
        ];
        if !self.wrapper_directories.is_empty() {
            let wrappers = std::env::join_paths(&self.wrapper_directories)
                .map_err(|error| format!("agent wrapper path is invalid: {error}"))?
                .to_string_lossy()
                .into_owned();
            let path = std::env::join_paths(
                self.wrapper_directories
                    .iter()
                    .cloned()
                    .chain(std::env::split_paths(&current_path())),
            )
            .map_err(|error| format!("agent PATH is invalid: {error}"))?
            .to_string_lossy()
            .into_owned();
            environment.push(("ZENTTY_ALL_WRAPPER_BIN_DIRS".to_owned(), wrappers));
            environment.push(("PATH".to_owned(), path));
        }
        Ok(environment)
    }

    pub(crate) fn unregister_pane(&mut self, pane_id: &str) {
        self.worklane_by_pane.remove(pane_id);
        let Some(token) = self.tokens_by_pane.remove(pane_id) else {
            return;
        };
        if let Ok(mut registry) = self.registry.lock() {
            let _ = registry.unregister(&token);
        }
    }

    pub(crate) fn retarget_registered_panes<'a>(
        &mut self,
        panes: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Result<(), String> {
        for (worklane_id, pane_id) in panes {
            if self.worklane_by_pane.get(pane_id).map(String::as_str) == Some(worklane_id) {
                continue;
            }
            let Some(token) = self.tokens_by_pane.get(pane_id) else {
                continue;
            };
            self.registry
                .lock()
                .map_err(|_| "agent pane registry is unavailable".to_owned())?
                .retarget(
                    token,
                    AgentTarget::new(&self.window_id, worklane_id, pane_id),
                )
                .map_err(|error| error.to_string())?;
            self.worklane_by_pane
                .insert(pane_id.to_owned(), worklane_id.to_owned());
        }
        Ok(())
    }

    pub(crate) fn drain(&self) -> Vec<AuthenticatedAgentEvent> {
        self.receiver.try_iter().collect()
    }

    pub(crate) fn drain_tmux(&self) -> Vec<AuthenticatedTmuxRequest> {
        self.tmux_receiver.try_iter().collect()
    }
}

fn current_path() -> std::ffi::OsString {
    std::env::var_os("PATH").unwrap_or_else(|| "/usr/bin:/bin".into())
}

fn instance_runtime_directory(
    xdg_runtime_directory: Option<&std::ffi::OsStr>,
    temporary_directory: &std::path::Path,
    process_id: u32,
    nonce: &str,
) -> PathBuf {
    let xdg_candidate = xdg_runtime_directory
        .map(std::path::Path::new)
        .filter(|path| path.is_absolute())
        .map(|runtime| {
            runtime
                .join("zentty")
                .join(format!("instance-{process_id}-{nonce}"))
        });
    if let Some(candidate) = xdg_candidate.filter(|path| socket_path_fits(path)) {
        return candidate;
    }
    let temporary_candidate =
        temporary_directory.join(format!("zentty-agent-{process_id}-{nonce}"));
    if socket_path_fits(&temporary_candidate) {
        temporary_candidate
    } else {
        std::path::Path::new("/tmp").join(format!("zentty-agent-{process_id}-{nonce}"))
    }
}

fn socket_path_fits(directory: &std::path::Path) -> bool {
    directory.join("instance.sock").as_os_str().as_bytes().len() <= 107
}

fn enabled_wrapper_directories(
    wrapper_root: &std::path::Path,
    path: &std::ffi::OsStr,
) -> Vec<PathBuf> {
    ["claude", "codex"]
        .into_iter()
        .filter_map(|tool| {
            let wrapper_directory = wrapper_root.join(tool);
            if !is_executable(&wrapper_directory.join(tool)) {
                return None;
            }
            std::env::split_paths(path)
                .any(|directory| {
                    !directory.starts_with(wrapper_root) && is_executable(&directory.join(tool))
                })
                .then_some(wrapper_directory)
        })
        .collect()
}

fn is_executable(path: &std::path::Path) -> bool {
    std::fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

impl Drop for AgentRuntime {
    fn drop(&mut self) {
        if let Some(server) = self.server.take() {
            let _ = server.shutdown();
        }
        let _ = std::fs::remove_dir(&self.runtime_directory);
    }
}

#[cfg(test)]
mod tests {
    use super::{enabled_wrapper_directories, instance_runtime_directory};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn wrappers_are_enabled_only_for_installed_real_tools() {
        let root =
            std::env::temp_dir().join(format!("zentty-wrapper-selection-{}", std::process::id()));
        let wrappers = root.join("wrappers");
        let real = root.join("real");
        let _ = fs::remove_dir_all(&root);
        for tool in ["claude", "codex"] {
            fs::create_dir_all(wrappers.join(tool)).unwrap();
            let wrapper = wrappers.join(tool).join(tool);
            fs::write(&wrapper, "wrapper").unwrap();
            fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
        }
        fs::create_dir_all(&real).unwrap();
        let codex = real.join("codex");
        fs::write(&codex, "real").unwrap();
        fs::set_permissions(&codex, fs::Permissions::from_mode(0o700)).unwrap();

        assert_eq!(
            enabled_wrapper_directories(&wrappers, real.as_os_str()),
            [wrappers.join("codex")]
        );
        assert!(
            enabled_wrapper_directories(&wrappers, wrappers.join("codex").as_os_str()).is_empty()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn instance_socket_directory_prefers_absolute_xdg_runtime_paths() {
        assert_eq!(
            instance_runtime_directory(Some("/run/user/1000".as_ref()), "/tmp".as_ref(), 42, "abc"),
            std::path::Path::new("/run/user/1000/zentty/instance-42-abc")
        );
        assert_eq!(
            instance_runtime_directory(Some("relative".as_ref()), "/tmp".as_ref(), 42, "abc"),
            std::path::Path::new("/tmp/zentty-agent-42-abc")
        );
        assert_eq!(
            instance_runtime_directory(None, "/tmp".as_ref(), 42, "abc"),
            std::path::Path::new("/tmp/zentty-agent-42-abc")
        );
        let long_runtime = format!("/tmp/{}", "nested-runtime/".repeat(10));
        assert_eq!(
            instance_runtime_directory(
                Some(long_runtime.as_ref()),
                long_runtime.as_ref(),
                42,
                "abc"
            ),
            std::path::Path::new("/tmp/zentty-agent-42-abc")
        );
    }
}
