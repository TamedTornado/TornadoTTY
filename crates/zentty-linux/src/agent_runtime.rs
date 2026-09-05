use std::cell::Cell;
use std::collections::BTreeMap;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use zentty_agent_ipc::{
    AgentIpcServer, AuthenticatedProductRequest, AuthenticatedServerRequest,
    AuthenticatedTmuxRequest, IngressReceiver, generate_pane_token, ingress_channel,
    publish_instance, publish_pane_credential, remove_pane_credential,
};
use zentty_core::{AgentTarget, AuthenticatedAgentEvent, PaneTokenRegistry};

pub(crate) struct AgentRuntime {
    server: Option<AgentIpcServer>,
    registry: Arc<Mutex<PaneTokenRegistry>>,
    receiver: IngressReceiver<AuthenticatedAgentEvent>,
    tmux_receiver: IngressReceiver<AuthenticatedTmuxRequest>,
    server_receiver: IngressReceiver<AuthenticatedServerRequest>,
    product_receiver: IngressReceiver<AuthenticatedProductRequest>,
    last_pressure_log: Cell<Option<Instant>>,
    tokens_by_pane: BTreeMap<String, String>,
    credentials_by_pane: BTreeMap<String, PathBuf>,
    target_by_pane: BTreeMap<String, (String, String)>,
    runtime_directory: PathBuf,
    socket_path: PathBuf,
    cli_path: PathBuf,
    wrapper_directories: Vec<PathBuf>,
    tmux_shim_directory: PathBuf,
    shell_integration_directory: PathBuf,
    instance_id: String,
    automation_token: String,
    automation_target_pane: Option<String>,
    agent_teams_enabled: bool,
    integration_states: std::collections::BTreeMap<String, zentty_core::AgentIntegrationState>,
}

impl AgentRuntime {
    pub(crate) fn start() -> Result<Self, String> {
        let instance = generate_pane_token().map_err(|error| error.to_string())?;
        let automation_token = generate_pane_token().map_err(|error| error.to_string())?;
        let runtime_directory = instance_runtime_directory(
            std::env::var_os("XDG_RUNTIME_DIR").as_deref(),
            &std::env::temp_dir(),
            std::process::id(),
            &instance[..32],
        );
        let socket_path = runtime_directory.join("instance.sock");
        let registry = Arc::new(Mutex::new(PaneTokenRegistry::default()));
        // Frame validation is capped at 384 KiB. These are message counts,
        // not PTY-byte limits; a single pane cannot fill a route by itself.
        let (sender, receiver) = ingress_channel(128, 16);
        let (tmux_sender, tmux_receiver) = ingress_channel(32, 4);
        let (server_sender, server_receiver) = ingress_channel(32, 4);
        let (product_sender, product_receiver) = ingress_channel(32, 4);
        let server = AgentIpcServer::start_with_cli_routes(
            &socket_path,
            Arc::clone(&registry),
            sender,
            tmux_sender,
            server_sender,
            product_sender,
        )
        .map_err(|error| error.to_string())?;
        eprintln!(
            "zentty-linux: agent-runtime socket={}",
            socket_path.display()
        );
        let cli_path = installed_cli_path(
            &std::env::current_exe()
                .map_err(|error| format!("could not resolve Tornado TTY executable: {error}"))?,
        );
        let wrapper_root = cli_path
            .parent()
            .and_then(std::path::Path::parent)
            .map(|root| root.join("libexec/zentty/agent-wrappers"));
        let wrapper_directories = wrapper_root
            .map(|root| installed_wrapper_directories(&root))
            .unwrap_or_default();
        let tmux_shim_directory = cli_path
            .parent()
            .and_then(Path::parent)
            .map_or_else(PathBuf::new, |root| root.join("libexec/zentty/tmux-shim"));
        let shell_integration_directory = cli_path
            .parent()
            .and_then(Path::parent)
            .map_or_else(PathBuf::new, |root| {
                root.join("share/zentty/shell-integration")
            });
        Ok(Self {
            server: Some(server),
            registry,
            receiver,
            tmux_receiver,
            server_receiver,
            product_receiver,
            last_pressure_log: Cell::new(None),
            tokens_by_pane: BTreeMap::new(),
            credentials_by_pane: BTreeMap::new(),
            target_by_pane: BTreeMap::new(),
            runtime_directory,
            socket_path,
            cli_path,
            wrapper_directories,
            tmux_shim_directory,
            shell_integration_directory,
            instance_id: instance,
            automation_token,
            automation_target_pane: None,
            agent_teams_enabled: std::env::var_os("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS")
                .as_deref()
                == Some(std::ffi::OsStr::new("1")),
            integration_states: std::collections::BTreeMap::new(),
        })
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn environment_for_pane(
        &mut self,
        window_id: &str,
        worklane_id: &str,
        pane_id: &str,
    ) -> Result<Vec<(String, String)>, String> {
        let target = AgentTarget::new(window_id, worklane_id, pane_id);
        let token = if let Some(token) = self.tokens_by_pane.get(pane_id) {
            self.registry
                .lock()
                .map_err(|_| "agent pane registry is unavailable".to_owned())?
                .retarget(token, target.clone())
                .map_err(|error| error.to_string())?;
            token.clone()
        } else {
            let token = generate_pane_token().map_err(|error| error.to_string())?;
            let credential_id = generate_pane_token().map_err(|error| error.to_string())?;
            self.registry
                .lock()
                .map_err(|_| "agent pane registry is unavailable".to_owned())?
                .register(&token, target.clone())
                .map_err(|error| error.to_string())?;
            let credential_path =
                publish_pane_credential(&self.runtime_directory, &credential_id, &token)
                    .inspect_err(|_| {
                        if let Ok(mut registry) = self.registry.lock() {
                            let _ = registry.unregister(&token);
                        }
                    })?;
            self.tokens_by_pane
                .insert(pane_id.to_owned(), token.clone());
            self.credentials_by_pane
                .insert(pane_id.to_owned(), credential_path);
            token
        };
        self.target_by_pane.insert(
            pane_id.to_owned(),
            (window_id.to_owned(), worklane_id.to_owned()),
        );
        if self.automation_target_pane.is_none() {
            let mut registry = self
                .registry
                .lock()
                .map_err(|_| "agent pane registry is unavailable".to_owned())?;
            registry
                .register_instance(&self.automation_token, target)
                .map_err(|error| error.to_string())?;
            drop(registry);
            if let Err(error) = publish_instance(
                &self.runtime_directory,
                &self.instance_id,
                &self.automation_token,
            ) {
                if let Ok(mut registry) = self.registry.lock() {
                    let _ = registry.unregister(&self.automation_token);
                }
                return Err(format!("could not publish instance discovery: {error}"));
            }
            self.automation_target_pane = Some(pane_id.to_owned());
        }
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
            ("ZENTTY_PANE_TOKEN".to_owned(), token.clone()),
            ("ZENTTY_WINDOW_ID".to_owned(), window_id.to_owned()),
            ("ZENTTY_WORKLANE_ID".to_owned(), worklane_id.to_owned()),
            ("ZENTTY_PANE_ID".to_owned(), pane_id.to_owned()),
            ("ZENTTY_INSTANCE_ID".to_owned(), self.instance_id.clone()),
            (
                "COLORTERM".to_owned(),
                color_terminal_environment(std::env::var_os("COLORTERM").as_deref()),
            ),
        ];
        let active_wrapper_directories = self.active_wrapper_directories();
        eprintln!(
            "zentty-linux: agent-wrapper-export pane={} installed={} active={}",
            pane_id,
            wrapper_names(&self.wrapper_directories),
            wrapper_names(&active_wrapper_directories)
        );
        if !active_wrapper_directories.is_empty() {
            let wrappers = std::env::join_paths(&active_wrapper_directories)
                .map_err(|error| format!("agent wrapper path is invalid: {error}"))?
                .to_string_lossy()
                .into_owned();
            let path = std::env::join_paths(
                active_wrapper_directories
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
        environment.push((
            "ZENTTY_OPENCODE_SYNC_THEME_FILE".to_owned(),
            self.opencode_theme_source_path_for_token(&token)
                .to_string_lossy()
                .into_owned(),
        ));
        let pane_path = pane_path(&environment, &current_path());
        environment.extend(agent_teams_environment(
            &self.tmux_shim_directory,
            &self.runtime_directory,
            &self.instance_id,
            pane_id,
            self.agent_teams_enabled
                .then_some(std::ffi::OsStr::new("1")),
            std::env::var_os("TMUX").as_deref(),
            &pane_path,
        )?);
        environment.extend(shell_integration_environment(
            &self.shell_integration_directory,
            std::env::var_os("ZDOTDIR").as_deref(),
            std::env::var_os("PROMPT_COMMAND").as_deref(),
            std::env::var_os("XDG_DATA_DIRS").as_deref(),
        ));
        Ok(environment)
    }

    pub(crate) fn opencode_theme_source_path(&self, pane_id: &str) -> Option<PathBuf> {
        self.tokens_by_pane
            .get(pane_id)
            .map(|token| self.opencode_theme_source_path_for_token(token))
    }

    pub(crate) fn opencode_overlay_config_directory(&self, pane_id: &str) -> Option<PathBuf> {
        let token = self.tokens_by_pane.get(pane_id)?;
        Some(
            self.runtime_directory
                .join("agent-overlays")
                .join(format!("opencode-{}", pane_token_component(token)?))
                .join("config"),
        )
    }

    fn opencode_theme_source_path_for_token(&self, token: &str) -> PathBuf {
        let component = pane_token_component(token).unwrap_or("invalid-token");
        self.runtime_directory
            .join("opencode-theme-sources")
            .join(format!("{component}.json"))
    }

    pub(crate) fn available_integration_wrappers(&self) -> std::collections::BTreeSet<String> {
        self.wrapper_directories
            .iter()
            .filter_map(|directory| directory.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .collect()
    }

    pub(crate) fn set_agent_teams_enabled(&mut self, enabled: bool) {
        self.agent_teams_enabled = enabled;
    }

    pub(crate) fn set_agent_integrations(
        &mut self,
        states: std::collections::BTreeMap<String, zentty_core::AgentIntegrationState>,
    ) {
        self.integration_states = states;
    }

    fn active_wrapper_directories(&self) -> Vec<PathBuf> {
        self.wrapper_directories
            .iter()
            .filter(|directory| {
                let Some(tool) = directory.file_name().and_then(|name| name.to_str()) else {
                    return false;
                };
                integration_enabled(&self.integration_states, tool)
            })
            .cloned()
            .collect()
    }

    pub(crate) fn missing_codex_home(&self) -> PathBuf {
        self.runtime_directory.join("missing-codex-home")
    }

    pub(crate) fn unregister_pane(&mut self, pane_id: &str) {
        self.target_by_pane.remove(pane_id);
        if let Some(path) = self.credentials_by_pane.remove(pane_id)
            && let Err(error) = remove_pane_credential(&path)
        {
            eprintln!("zentty-linux: pane-credential-remove pane={pane_id} error={error}");
        }
        let Some(token) = self.tokens_by_pane.remove(pane_id) else {
            return;
        };
        if let Ok(mut registry) = self.registry.lock() {
            let _ = registry.unregister(&token);
            if self.automation_target_pane.as_deref() == Some(pane_id) {
                if let Some((replacement_pane, (window, worklane))) =
                    self.target_by_pane.iter().next()
                {
                    let _ = registry.retarget(
                        &self.automation_token,
                        AgentTarget::new(window, worklane, replacement_pane),
                    );
                    self.automation_target_pane = Some(replacement_pane.clone());
                } else {
                    let _ = registry.unregister(&self.automation_token);
                    self.automation_target_pane = None;
                    let _ = std::fs::remove_file(self.runtime_directory.join("instance.json"));
                    let _ = std::fs::remove_file(self.runtime_directory.join("automation.token"));
                }
            }
        }
    }

    pub(crate) fn retarget_registered_panes<'a>(
        &mut self,
        panes: impl IntoIterator<Item = (&'a str, &'a str, &'a str)>,
    ) -> Result<(), String> {
        for (window_id, worklane_id, pane_id) in panes {
            let requested_target = (window_id, worklane_id);
            if self
                .target_by_pane
                .get(pane_id)
                .map(|(window, worklane)| (window.as_str(), worklane.as_str()))
                == Some(requested_target)
            {
                continue;
            }
            let Some(token) = self.tokens_by_pane.get(pane_id) else {
                continue;
            };
            self.registry
                .lock()
                .map_err(|_| "agent pane registry is unavailable".to_owned())?
                .retarget(token, AgentTarget::new(window_id, worklane_id, pane_id))
                .map_err(|error| error.to_string())?;
            self.target_by_pane.insert(
                pane_id.to_owned(),
                (window_id.to_owned(), worklane_id.to_owned()),
            );
        }
        Ok(())
    }

    pub(crate) fn drain(&self) -> Vec<AuthenticatedAgentEvent> {
        self.log_ingress_pressure();
        self.receiver.drain_batch(32)
    }

    fn log_ingress_pressure(&self) {
        let now = Instant::now();
        if self
            .last_pressure_log
            .get()
            .is_some_and(|last| now.duration_since(last) < Duration::from_secs(5))
        {
            return;
        }
        self.last_pressure_log.set(Some(now));
        for (route, pressure) in [
            ("events", self.receiver.take_pressure()),
            ("tmux", self.tmux_receiver.take_pressure()),
            ("servers", self.server_receiver.take_pressure()),
            ("product", self.product_receiver.take_pressure()),
        ] {
            if pressure.rejected > 0 {
                eprintln!(
                    "tornadotty: ingress-pressure route={route} queued={} high-water={} rejected={} last-pane={:?}",
                    pressure.queued,
                    pressure.high_water,
                    pressure.rejected,
                    pressure.last_rejected_pane
                );
            }
        }
    }

    pub(crate) fn drain_tmux(&self) -> Vec<AuthenticatedTmuxRequest> {
        self.tmux_receiver.drain_batch(4)
    }

    pub(crate) fn drain_servers(&self) -> Vec<AuthenticatedServerRequest> {
        self.server_receiver.drain_batch(4)
    }

    pub(crate) fn drain_products(&self) -> Vec<AuthenticatedProductRequest> {
        self.product_receiver.drain_batch(4)
    }

    pub(crate) fn control_credential_for_pane(&self, pane_id: &str) -> Option<&Path> {
        self.credentials_by_pane.get(pane_id).map(PathBuf::as_path)
    }

    pub(crate) fn socket_path_for_cli(&self) -> String {
        self.socket_path.to_string_lossy().into_owned()
    }
}

fn installed_cli_path(current_executable: &Path) -> PathBuf {
    let packaged_cli = current_executable.with_file_name("tornadotty-cli");
    if packaged_cli.is_file() {
        packaged_cli
    } else {
        current_executable.with_file_name("zentty")
    }
}

fn integration_enabled(
    states: &std::collections::BTreeMap<String, zentty_core::AgentIntegrationState>,
    tool: &str,
) -> bool {
    states.get(tool) != Some(&zentty_core::AgentIntegrationState::Off)
}

fn current_path() -> std::ffi::OsString {
    std::env::var_os("PATH").unwrap_or_else(|| "/usr/bin:/bin".into())
}

fn pane_path(environment: &[(String, String)], fallback: &std::ffi::OsStr) -> std::ffi::OsString {
    environment
        .iter()
        .find_map(|(name, value)| (name == "PATH").then_some(value.as_str()))
        .map_or_else(|| fallback.to_owned(), std::ffi::OsString::from)
}

fn agent_teams_environment(
    shim_directory: &Path,
    runtime_directory: &Path,
    instance_id: &str,
    pane_id: &str,
    enabled: Option<&std::ffi::OsStr>,
    ambient_tmux: Option<&std::ffi::OsStr>,
    current_path: &std::ffi::OsStr,
) -> Result<Vec<(String, String)>, String> {
    if enabled != Some(std::ffi::OsStr::new("1"))
        || ambient_tmux.is_some_and(|value| !value.is_empty())
        || !is_executable(&shim_directory.join("tmux"))
    {
        return Ok(Vec::new());
    }
    let path = std::env::join_paths(
        std::iter::once(shim_directory.to_path_buf())
            .chain(std::env::split_paths(current_path).filter(|entry| entry != shim_directory)),
    )
    .map_err(|error| format!("tmux shim PATH is invalid: {error}"))?
    .to_string_lossy()
    .into_owned();
    Ok(vec![
        (
            "ZENTTY_TMUX_SHIM_DIR".to_owned(),
            shim_directory.to_string_lossy().into_owned(),
        ),
        ("PATH".to_owned(), path),
        (
            "TMUX".to_owned(),
            format!("{}/tmux-compat,0,{pane_id}", runtime_directory.display()),
        ),
        ("TMUX_PANE".to_owned(), format!("%{pane_id}")),
        (
            "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS".to_owned(),
            "1".to_owned(),
        ),
        ("ZENTTY_INSTANCE_ID".to_owned(), instance_id.to_owned()),
    ])
}

fn shell_integration_environment(
    integration_directory: &Path,
    original_zdotdir: Option<&std::ffi::OsStr>,
    original_prompt_command: Option<&std::ffi::OsStr>,
    original_xdg_data_directories: Option<&std::ffi::OsStr>,
) -> Vec<(String, String)> {
    let required = [
        ".zshenv",
        "zentty-bash-integration.bash",
        "zentty-zsh-integration.zsh",
        "fish/vendor_conf.d/zentty-shell-integration.fish",
        "nushell/vendor/autoload/zentty.nu",
    ];
    if required.iter().any(|relative| {
        std::fs::symlink_metadata(integration_directory.join(relative))
            .map_or(true, |metadata| !metadata.file_type().is_file())
    }) {
        return Vec::new();
    }
    let integration = integration_directory.to_string_lossy().into_owned();
    let original_xdg = original_xdg_data_directories
        .filter(|value| !value.is_empty())
        .map_or_else(
            || std::ffi::OsString::from("/usr/local/share:/usr/share"),
            std::ffi::OsString::from,
        );
    let xdg_data_directories =
        std::env::join_paths(std::iter::once(integration_directory.to_path_buf()).chain(
            std::env::split_paths(&original_xdg).filter(|entry| entry != integration_directory),
        ))
        .map_or_else(
            |_| format!("{integration}:/usr/local/share:/usr/share"),
            |paths| paths.to_string_lossy().into_owned(),
        );
    let mut environment = vec![
        (
            "ZENTTY_SHELL_INTEGRATION_DIR".to_owned(),
            integration.clone(),
        ),
        ("ZENTTY_SHELL_INTEGRATION".to_owned(), "1".to_owned()),
        ("ZDOTDIR".to_owned(), integration.clone()),
        (
            "PROMPT_COMMAND".to_owned(),
            ". \"$ZENTTY_SHELL_INTEGRATION_DIR/zentty-bash-integration.bash\"".to_owned(),
        ),
        ("ZENTTY_SHELL_INTEGRATION_XDG_DIR".to_owned(), integration),
        ("XDG_DATA_DIRS".to_owned(), xdg_data_directories),
    ];
    if let Some(value) = original_zdotdir.filter(|value| !value.is_empty()) {
        environment.push((
            "ZENTTY_ORIGINAL_ZDOTDIR".to_owned(),
            value.to_string_lossy().into_owned(),
        ));
    }
    if let Some(value) = original_prompt_command.filter(|value| !value.is_empty()) {
        environment.push((
            "ZENTTY_BASH_ORIGINAL_PROMPT_COMMAND".to_owned(),
            value.to_string_lossy().into_owned(),
        ));
    }
    if let Some(value) = original_xdg_data_directories.filter(|value| !value.is_empty()) {
        environment.push((
            "ZENTTY_ORIGINAL_XDG_DATA_DIRS".to_owned(),
            value.to_string_lossy().into_owned(),
        ));
    }
    environment
}

fn color_terminal_environment(inherited: Option<&std::ffi::OsStr>) -> String {
    inherited
        .filter(|value| !value.to_string_lossy().trim().is_empty())
        .map_or_else(
            || "truecolor".to_owned(),
            |value| value.to_string_lossy().into_owned(),
        )
}

fn pane_token_component(token: &str) -> Option<&str> {
    (token.len() >= 32 && token.bytes().all(|byte| byte.is_ascii_hexdigit())).then(|| &token[..32])
}

fn instance_runtime_directory(
    xdg_runtime_directory: Option<&std::ffi::OsStr>,
    temporary_directory: &std::path::Path,
    process_id: u32,
    nonce: &str,
) -> PathBuf {
    if let Some(runtime) = xdg_runtime_directory
        .map(std::path::Path::new)
        .filter(|path| path.is_absolute())
    {
        let root = runtime.join("zentty");
        let descriptive = root.join(format!("instance-{process_id}-{nonce}"));
        if socket_path_fits(&descriptive) {
            return descriptive;
        }
        // AF_UNIX paths are short on Linux. Keep discovery beneath the
        // authoritative XDG root when only the descriptive directory name
        // pushes the socket over that bound; the full process identity and
        // instance ID remain in the validated private descriptor.
        let compact = root.join(format!("i-{nonce}"));
        if socket_path_fits(&compact) {
            return compact;
        }
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

fn installed_wrapper_directories(wrapper_root: &std::path::Path) -> Vec<PathBuf> {
    [
        ("amp", &["amp"][..]),
        ("claude", &["claude"][..]),
        ("codex", &["codex"][..]),
        ("copilot", &["copilot"][..]),
        ("cursor", &["cursor-agent"][..]),
        ("droid", &["droid"][..]),
        ("gemini", &["gemini"][..]),
        ("opencode", &["opencode"][..]),
        ("pi", &["pi"][..]),
        ("omp", &["omp"][..]),
        ("kimi", &["kimi", "kimi-cli"][..]),
        ("grok", &["grok"][..]),
        ("agy", &["agy"][..]),
        ("hermes", &["hermes"][..]),
        ("vibe", &["vibe", "mistral-vibe"][..]),
        ("small-harness", &["small-harness"][..]),
    ]
    .into_iter()
    .filter_map(|(tool, binaries)| {
        let wrapper_directory = wrapper_root.join(tool);
        binaries
            .iter()
            .any(|binary| is_executable(&wrapper_directory.join(binary)))
            .then_some(wrapper_directory)
    })
    .collect()
}

fn wrapper_names(directories: &[PathBuf]) -> String {
    directories
        .iter()
        .filter_map(|directory| directory.file_name().and_then(|name| name.to_str()))
        .collect::<Vec<_>>()
        .join(",")
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
        // Per-launch agent overlays live under this instance-owned, randomly
        // named private directory and must not outlive the socket/application.
        let _ = std::fs::remove_dir_all(&self.runtime_directory);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        agent_teams_environment, color_terminal_environment, installed_cli_path,
        installed_wrapper_directories, instance_runtime_directory, integration_enabled, pane_path,
        shell_integration_environment, wrapper_names,
    };
    use std::collections::BTreeMap;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn installed_cli_is_the_tornadotty_sibling_of_the_resolved_gui() {
        let root =
            std::env::temp_dir().join(format!("tornadotty-cli-selection-{}", std::process::id()));
        let bin = root.join("bin");
        let gui = bin.join("tornadotty");
        let packaged_cli = bin.join("tornadotty-cli");
        let staging_cli = bin.join("zentty");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&bin).unwrap();
        fs::write(&staging_cli, "staging").unwrap();

        assert_eq!(installed_cli_path(&gui), staging_cli);
        fs::write(&packaged_cli, "packaged").unwrap();
        assert_eq!(installed_cli_path(&gui), packaged_cli);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn explicit_off_is_the_only_state_that_removes_an_installed_wrapper() {
        use zentty_core::AgentIntegrationState::{Ask, Off, On};

        let mut states = BTreeMap::new();
        assert!(integration_enabled(&states, "codex"));
        states.insert("codex".to_owned(), Ask);
        assert!(integration_enabled(&states, "codex"));
        states.insert("codex".to_owned(), On);
        assert!(integration_enabled(&states, "codex"));
        states.insert("codex".to_owned(), Off);
        assert!(!integration_enabled(&states, "codex"));
    }

    #[test]
    fn installed_wrappers_do_not_depend_on_the_launcher_path() {
        let root =
            std::env::temp_dir().join(format!("zentty-wrapper-selection-{}", std::process::id()));
        let wrappers = root.join("wrappers");
        let _ = fs::remove_dir_all(&root);
        for tool in ["claude", "codex", "gemini"] {
            fs::create_dir_all(wrappers.join(tool)).unwrap();
            let wrapper = wrappers.join(tool).join(tool);
            fs::write(&wrapper, "wrapper").unwrap();
            fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
        }
        fs::create_dir_all(wrappers.join("cursor")).unwrap();
        let cursor_wrapper = wrappers.join("cursor/cursor-agent");
        fs::write(&cursor_wrapper, "wrapper").unwrap();
        fs::set_permissions(&cursor_wrapper, fs::Permissions::from_mode(0o700)).unwrap();
        fs::create_dir_all(wrappers.join("kimi")).unwrap();
        for binary in ["kimi", "kimi-cli"] {
            let wrapper = wrappers.join("kimi").join(binary);
            fs::write(&wrapper, "wrapper").unwrap();
            fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let installed = installed_wrapper_directories(&wrappers);
        assert_eq!(
            installed,
            [
                wrappers.join("claude"),
                wrappers.join("codex"),
                wrappers.join("cursor"),
                wrappers.join("gemini"),
                wrappers.join("kimi")
            ]
        );
        assert_eq!(wrapper_names(&installed), "claude,codex,cursor,gemini,kimi");
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
        let constrained_runtime = format!("/tmp/{}", "r".repeat(43));
        let nonce = "0123456789abcdef0123456789abcdef";
        assert_eq!(
            instance_runtime_directory(
                Some(constrained_runtime.as_ref()),
                "/tmp".as_ref(),
                1_234_567,
                nonce,
            ),
            std::path::Path::new(&constrained_runtime)
                .join("zentty")
                .join(format!("i-{nonce}"))
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

    #[test]
    fn agent_teams_uses_only_an_executable_product_relative_shim_when_opted_in() {
        let root =
            std::env::temp_dir().join(format!("zentty-tmux-shim-selection-{}", std::process::id()));
        let shim = root.join("libexec/zentty/tmux-shim");
        let runtime = root.join("runtime/instance-private");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&shim).unwrap();
        let executable = shim.join("tmux");
        fs::write(&executable, "shim").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();

        let path =
            std::env::join_paths(["/real/bin".as_ref(), shim.as_path(), "/usr/bin".as_ref()])
                .unwrap();
        let overrides = agent_teams_environment(
            &shim,
            &runtime,
            "instance-secret",
            "pane-7",
            Some("1".as_ref()),
            None,
            &path,
        )
        .unwrap();
        let values = overrides.into_iter().collect::<BTreeMap<_, _>>();
        assert_eq!(values["ZENTTY_TMUX_SHIM_DIR"], shim.to_string_lossy());
        assert_eq!(values["ZENTTY_INSTANCE_ID"], "instance-secret");
        assert_eq!(
            values["TMUX"],
            format!("{}/tmux-compat,0,pane-7", runtime.display())
        );
        assert_eq!(values["TMUX_PANE"], "%pane-7");
        assert_eq!(values["CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS"], "1");
        assert_eq!(
            std::env::split_paths(values["PATH"].as_str()).collect::<Vec<_>>(),
            [shim.clone(), "/real/bin".into(), "/usr/bin".into()]
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pane_path_prefers_the_composed_pane_override_and_has_an_exact_fallback() {
        let environment = vec![
            ("IGNORED".to_owned(), "/wrong".to_owned()),
            ("PATH".to_owned(), "/wrappers:/usr/bin".to_owned()),
        ];
        assert_eq!(
            pane_path(&environment, "/fallback".as_ref()),
            "/wrappers:/usr/bin"
        );
        assert_eq!(pane_path(&[], "/fallback".as_ref()), "/fallback");
    }

    #[test]
    fn agent_teams_preserves_disabled_active_tmux_and_missing_shim_cases() {
        let root =
            std::env::temp_dir().join(format!("zentty-tmux-shim-negative-{}", std::process::id()));
        let shim = root.join("shim");
        fs::create_dir_all(&shim).unwrap();
        let executable = shim.join("tmux");
        fs::write(&executable, "shim").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let runtime = root.join("runtime");

        assert!(
            agent_teams_environment(
                &shim,
                &runtime,
                "instance",
                "pane",
                None,
                None,
                "/usr/bin".as_ref(),
            )
            .unwrap()
            .is_empty()
        );
        assert!(
            agent_teams_environment(
                &shim,
                &runtime,
                "instance",
                "pane",
                Some("1".as_ref()),
                Some("/real/tmux,1,2".as_ref()),
                "/usr/bin".as_ref(),
            )
            .unwrap()
            .is_empty()
        );
        fs::remove_file(executable).unwrap();
        assert!(
            agent_teams_environment(
                &shim,
                &runtime,
                "instance",
                "pane",
                Some("1".as_ref()),
                None,
                "/usr/bin".as_ref(),
            )
            .unwrap()
            .is_empty()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn complete_shell_tree_preserves_user_and_xdg_environment() {
        let root = std::env::temp_dir().join(format!(
            "zentty-shell-integration-environment-{}",
            std::process::id()
        ));
        let integration = root.join("share/zentty/shell-integration");
        let _ = fs::remove_dir_all(&root);
        for relative in [
            ".zshenv",
            "zentty-bash-integration.bash",
            "zentty-zsh-integration.zsh",
            "fish/vendor_conf.d/zentty-shell-integration.fish",
            "nushell/vendor/autoload/zentty.nu",
        ] {
            let path = integration.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "integration").unwrap();
        }

        let values = shell_integration_environment(
            &integration,
            Some("/user/zsh".as_ref()),
            Some("user_prompt".as_ref()),
            Some("/user/share:/usr/share".as_ref()),
        )
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        assert_eq!(values["ZENTTY_SHELL_INTEGRATION"], "1");
        assert_eq!(values["ZDOTDIR"], integration.to_string_lossy());
        assert_eq!(values["ZENTTY_ORIGINAL_ZDOTDIR"], "/user/zsh");
        assert_eq!(values["ZENTTY_BASH_ORIGINAL_PROMPT_COMMAND"], "user_prompt");
        assert_eq!(
            values["ZENTTY_ORIGINAL_XDG_DATA_DIRS"],
            "/user/share:/usr/share"
        );
        assert_eq!(
            std::env::split_paths(values["XDG_DATA_DIRS"].as_str()).collect::<Vec<_>>(),
            [
                integration.clone(),
                "/user/share".into(),
                "/usr/share".into()
            ]
        );
        assert_eq!(
            values["PROMPT_COMMAND"],
            ". \"$ZENTTY_SHELL_INTEGRATION_DIR/zentty-bash-integration.bash\""
        );
        let fallback = shell_integration_environment(&integration, None, None, None)
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            std::env::split_paths(fallback["XDG_DATA_DIRS"].as_str()).collect::<Vec<_>>(),
            [
                integration.clone(),
                "/usr/local/share".into(),
                "/usr/share".into(),
            ]
        );
        assert!(!fallback.contains_key("ZENTTY_ORIGINAL_XDG_DATA_DIRS"));

        fs::remove_file(integration.join(".zshenv")).unwrap();
        assert!(shell_integration_environment(&integration, None, None, None).is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn terminal_color_preserves_nonblank_values_and_defaults_to_truecolor() {
        assert_eq!(color_terminal_environment(None), "truecolor");
        assert_eq!(
            color_terminal_environment(Some(std::ffi::OsStr::new(" \n\t"))),
            "truecolor"
        );
        assert_eq!(
            color_terminal_environment(Some(std::ffi::OsStr::new("24bit"))),
            "24bit"
        );
    }
}
