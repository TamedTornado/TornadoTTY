use std::collections::BTreeMap;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
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
    tmux_shim_directory: PathBuf,
    shell_integration_directory: PathBuf,
    instance_id: String,
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
            tokens_by_pane: BTreeMap::new(),
            worklane_by_pane: BTreeMap::new(),
            runtime_directory,
            socket_path,
            cli_path,
            wrapper_directories,
            tmux_shim_directory,
            shell_integration_directory,
            instance_id: instance,
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
            ("ZENTTY_INSTANCE_ID".to_owned(), self.instance_id.clone()),
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
        let pane_path = pane_path(&environment, &current_path());
        environment.extend(agent_teams_environment(
            &self.tmux_shim_directory,
            &self.runtime_directory,
            &self.instance_id,
            pane_id,
            std::env::var_os("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS").as_deref(),
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

    pub(crate) fn missing_codex_home(&self) -> PathBuf {
        self.runtime_directory.join("missing-codex-home")
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
    ["claude", "codex", "gemini"]
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
        // Per-launch agent overlays live under this instance-owned, randomly
        // named private directory and must not outlive the socket/application.
        let _ = std::fs::remove_dir_all(&self.runtime_directory);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        agent_teams_environment, enabled_wrapper_directories, instance_runtime_directory,
        pane_path, shell_integration_environment,
    };
    use std::collections::BTreeMap;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn wrappers_are_enabled_only_for_installed_real_tools() {
        let root =
            std::env::temp_dir().join(format!("zentty-wrapper-selection-{}", std::process::id()));
        let wrappers = root.join("wrappers");
        let real = root.join("real");
        let _ = fs::remove_dir_all(&root);
        for tool in ["claude", "codex", "gemini"] {
            fs::create_dir_all(wrappers.join(tool)).unwrap();
            let wrapper = wrappers.join(tool).join(tool);
            fs::write(&wrapper, "wrapper").unwrap();
            fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
        }
        fs::create_dir_all(&real).unwrap();
        let codex = real.join("codex");
        fs::write(&codex, "real").unwrap();
        fs::set_permissions(&codex, fs::Permissions::from_mode(0o700)).unwrap();
        let gemini = real.join("gemini");
        fs::write(&gemini, "real").unwrap();
        fs::set_permissions(&gemini, fs::Permissions::from_mode(0o700)).unwrap();

        assert_eq!(
            enabled_wrapper_directories(&wrappers, real.as_os_str()),
            [wrappers.join("codex"), wrappers.join("gemini")]
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
}
