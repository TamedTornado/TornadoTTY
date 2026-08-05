use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use zentty_agent_ipc::{AgentIpcServer, generate_pane_token};
use zentty_core::{AgentTarget, AuthenticatedAgentEvent, PaneTokenRegistry};

pub(crate) struct AgentRuntime {
    server: Option<AgentIpcServer>,
    registry: Arc<Mutex<PaneTokenRegistry>>,
    receiver: mpsc::Receiver<AuthenticatedAgentEvent>,
    tokens_by_pane: BTreeMap<String, String>,
    worklane_by_pane: BTreeMap<String, String>,
    runtime_directory: PathBuf,
    socket_path: PathBuf,
    cli_path: PathBuf,
    window_id: String,
}

impl AgentRuntime {
    pub(crate) fn start(window_id: impl Into<String>) -> Result<Self, String> {
        let window_id = window_id.into();
        let instance = generate_pane_token().map_err(|error| error.to_string())?;
        let runtime_directory = std::env::temp_dir().join(format!(
            "zentty-agent-{}-{}",
            std::process::id(),
            &instance[..32]
        ));
        let socket_path = runtime_directory.join("instance.sock");
        let registry = Arc::new(Mutex::new(PaneTokenRegistry::default()));
        let (sender, receiver) = mpsc::channel();
        let server = AgentIpcServer::start(&socket_path, Arc::clone(&registry), sender)
            .map_err(|error| error.to_string())?;
        eprintln!(
            "zentty-linux: agent-runtime socket={}",
            socket_path.display()
        );
        let cli_path = std::env::current_exe()
            .map_err(|error| format!("could not resolve Zentty executable: {error}"))?
            .with_file_name("zentty");
        Ok(Self {
            server: Some(server),
            registry,
            receiver,
            tokens_by_pane: BTreeMap::new(),
            worklane_by_pane: BTreeMap::new(),
            runtime_directory,
            socket_path,
            cli_path,
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
        Ok(vec![
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
        ])
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
}

impl Drop for AgentRuntime {
    fn drop(&mut self) {
        if let Some(server) = self.server.take() {
            let _ = server.shutdown();
        }
        let _ = std::fs::remove_dir(&self.runtime_directory);
    }
}
