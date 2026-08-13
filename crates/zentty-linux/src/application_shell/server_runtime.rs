use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gtk::gio::prelude::AppInfoExt;
use gtk::{gio, glib};
use zentty_agent_ipc::{ServerCommand, ServerIpcReply, ServerIpcRequest};
use zentty_core::{
    DetectedServer, DetectedServerConfidence, DetectedServerSource, SYSTEM_DEFAULT_BROWSER_ID,
    ServerBrowserCatalog, ServerBrowserLaunchPlan, ServerBrowserLauncher, ServerBrowserTarget,
    ServerDetectionConfig, ServerPortRule, ServerRegistry, ServerTerminationObservation,
    authorize_server_termination, normalize_server_url,
};

use super::ApplicationShell;
use crate::config_store::ConfigStore;
use crate::server_discovery::{PaneProcessContext, process_start_time, scan_listeners_at};

const PROBE_INTERVAL: Duration = Duration::from_secs(2);

pub(super) struct ServerRuntime {
    pub(super) probe_source: Option<glib::SourceId>,
    probe_in_flight: bool,
    pub(super) servers: Vec<DetectedServer>,
    registry: ServerRegistry,
    docker_scan_tick: u8,
    pub(super) browser_catalog: ServerBrowserCatalog,
    pub(super) browser_actions: BTreeMap<String, (String, String)>,
}

impl ServerRuntime {
    pub(super) fn discover(config: &ServerDetectionConfig) -> Self {
        let browser_catalog = ServerBrowserCatalog::resolve(
            config,
            discover_browser_targets(config, std::env::var_os("PATH").as_deref()),
        );
        eprintln!(
            "zentty-linux: server-browser-discovery available={} preferred={} unavailable={}",
            browser_catalog.enabled.len(),
            browser_catalog
                .preferred
                .as_ref()
                .map_or("none", |target| target.id.as_str()),
            browser_catalog.unavailable_ids.join(",")
        );
        Self {
            probe_source: None,
            probe_in_flight: false,
            servers: Vec::new(),
            registry: ServerRegistry::default(),
            docker_scan_tick: 0,
            browser_catalog,
            browser_actions: BTreeMap::new(),
        }
    }
}

const BUILTIN_BROWSERS: &[(&str, &str, &[&str])] = &[
    ("firefox", "Firefox", &["firefox"]),
    (
        "chrome",
        "Google Chrome",
        &["google-chrome", "google-chrome-stable"],
    ),
    ("chromium", "Chromium", &["chromium", "chromium-browser"]),
    ("brave", "Brave", &["brave-browser"]),
    (
        "edge",
        "Microsoft Edge",
        &["microsoft-edge", "microsoft-edge-stable"],
    ),
    ("vivaldi", "Vivaldi", &["vivaldi", "vivaldi-stable"]),
    ("opera", "Opera", &["opera"]),
    ("zen", "Zen", &["zen-browser", "zen"]),
    ("floorp", "Floorp", &["floorp"]),
    ("tor-browser", "Tor Browser", &["tor-browser"]),
];

pub(crate) fn discover_browser_targets(
    config: &ServerDetectionConfig,
    path: Option<&std::ffi::OsStr>,
) -> Vec<ServerBrowserTarget> {
    let mut targets = vec![ServerBrowserTarget {
        id: SYSTEM_DEFAULT_BROWSER_ID.into(),
        name: "System Default".into(),
        launcher: ServerBrowserLauncher::SystemDefault,
    }];
    targets.extend(
        BUILTIN_BROWSERS
            .iter()
            .filter_map(|(id, name, candidates)| {
                super::open_with_runtime::resolve_executable(candidates, path).map(|executable| {
                    ServerBrowserTarget {
                        id: (*id).into(),
                        name: (*name).into(),
                        launcher: ServerBrowserLauncher::Executable {
                            path: executable.to_string_lossy().into_owned(),
                        },
                    }
                })
            }),
    );

    let mut desktop_apps = gio::AppInfo::all_for_type("x-scheme-handler/http")
        .into_iter()
        .filter_map(|app| Some((app.id()?.to_string(), app.display_name().to_string())))
        .collect::<Vec<_>>();
    desktop_apps.sort_by(|left, right| left.0.cmp(&right.0));
    let mut known_desktop_ids = HashSet::new();
    let mut known_names = targets
        .iter()
        .map(|target| normalized_catalog_name(&target.name))
        .collect::<HashSet<_>>();
    for (application_id, name) in desktop_apps {
        if !known_desktop_ids.insert(application_id.clone())
            || !known_names.insert(normalized_catalog_name(&name))
        {
            continue;
        }
        targets.push(ServerBrowserTarget {
            id: format!("desktop:{application_id}"),
            name,
            launcher: ServerBrowserLauncher::DesktopApplication { application_id },
        });
    }

    let mut custom_paths = HashSet::new();
    targets.extend(config.custom_browsers.iter().filter_map(|browser| {
        let executable = super::open_with_runtime::canonical_executable(Path::new(&browser.path))?;
        if !custom_paths.insert(executable.clone()) {
            return None;
        }
        Some(ServerBrowserTarget {
            id: browser.id.clone(),
            name: browser.name.clone(),
            launcher: ServerBrowserLauncher::Executable {
                path: executable.to_string_lossy().into_owned(),
            },
        })
    }));
    targets
}

fn normalized_catalog_name(name: &str) -> String {
    name.split_whitespace()
        .flat_map(str::chars)
        .flat_map(char::to_lowercase)
        .collect()
}

#[derive(Clone)]
struct PaneProbeSource {
    pane_id: String,
    worklane_id: String,
    foreground_pid: u32,
    working_directory: PathBuf,
}

struct ProbeResult {
    worklane_ids: Vec<String>,
    scanner_servers: Vec<DetectedServer>,
    docker_servers: Option<Vec<DetectedServer>>,
    docker_error: Option<String>,
}

pub(super) fn install(shell: &Rc<RefCell<ApplicationShell>>) -> Option<glib::SourceId> {
    if !shell
        .borrow()
        .config
        .server_detection
        .passive_detection_enabled
    {
        return None;
    }
    let weak = Rc::downgrade(shell);
    Some(glib::timeout_add_local(PROBE_INTERVAL, move || {
        let Some(shell) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        if shell.borrow().shutting_down {
            return glib::ControlFlow::Break;
        }
        request_probe(&shell);
        glib::ControlFlow::Continue
    }))
}

pub(super) fn refresh_servers(shell: &Rc<RefCell<ApplicationShell>>) {
    eprintln!("zentty-linux: action=refresh-servers result=requested");
    request_probe(shell);
}

pub(crate) fn handle_ipc(
    shell: &Rc<RefCell<ApplicationShell>>,
    target: &zentty_core::AgentTarget,
    request: &ServerIpcRequest,
) -> ServerIpcReply {
    let command_name = request.subcommand().trim_start_matches("server-");
    let mut arguments = vec![command_name.to_owned()];
    arguments.extend(request.arguments().iter().cloned());
    let command = match ServerCommand::parse(&arguments) {
        Ok(command) => command,
        Err(error) => return ipc_failure("invalid_command", error.to_string()),
    };
    if !shell.borrow().has_worklane(&target.worklane_id)
        || shell.borrow().state.pane(&target.pane_id).is_none()
    {
        return ipc_failure("stale_target", "pane is no longer available".to_owned());
    }
    let operation = match command {
        ServerCommand::Set { raw_url, pid, .. } => {
            register_ipc_server(shell, target, &raw_url, pid, DetectedServerSource::Manual)
        }
        ServerCommand::WatchSet { raw_url, pid, .. } => {
            register_ipc_server(shell, target, &raw_url, pid, DetectedServerSource::Watch)
        }
        ServerCommand::Clear { .. } => {
            clear_ipc_servers(shell, target, None);
            Ok(())
        }
        ServerCommand::WatchClear { .. } => {
            clear_ipc_servers(shell, target, Some(DetectedServerSource::Watch));
            Ok(())
        }
        ServerCommand::List { .. } => Ok(()),
        ServerCommand::Open {
            raw_url, browser, ..
        } => open_ipc_server(
            shell,
            &target.worklane_id,
            raw_url.as_deref(),
            browser.as_deref(),
        ),
        ServerCommand::Watch { .. } => Err("watch must run in the CLI".to_owned()),
    };
    if let Err(error) = operation {
        return ipc_failure("server_command_failed", error);
    }
    match server_state_json(&shell.borrow(), &target.worklane_id) {
        Ok(json) => ServerIpcReply::success(json)
            .unwrap_or_else(|error| ipc_failure("response_too_large", error.to_string())),
        Err(error) => ipc_failure("serialization_failed", error.to_string()),
    }
}

fn register_ipc_server(
    shell: &Rc<RefCell<ApplicationShell>>,
    target: &zentty_core::AgentTarget,
    raw_url: &str,
    pid: Option<u32>,
    source: DetectedServerSource,
) -> Result<(), String> {
    let candidate = normalize_server_url(raw_url).map_err(|error| format!("{error:?}"))?;
    let now = now_ms()?;
    let mut shell_state = shell.borrow_mut();
    shell_state.server_runtime.registry.upsert(DetectedServer {
        id: format!(
            "{}|{}|{:?}|{}",
            target.worklane_id, target.pane_id, source, candidate.origin
        ),
        origin: candidate.origin,
        url: candidate.url,
        display: candidate.display,
        worklane_id: target.worklane_id.clone(),
        pane_id: Some(target.pane_id.clone()),
        source,
        ports: vec![candidate.port],
        confidence: if pid.is_some() {
            DetectedServerConfidence::Pid
        } else {
            DetectedServerConfidence::Explicit
        },
        updated_at_ms: now,
        first_seen_at_ms: now,
    });
    refresh_registry_projection(&mut shell_state);
    drop(shell_state);
    shell.borrow().render_sidebar();
    Ok(())
}

fn clear_ipc_servers(
    shell: &Rc<RefCell<ApplicationShell>>,
    target: &zentty_core::AgentTarget,
    source: Option<DetectedServerSource>,
) {
    let mut shell_state = shell.borrow_mut();
    shell_state
        .server_runtime
        .registry
        .clear_pane(&target.worklane_id, &target.pane_id, source);
    refresh_registry_projection(&mut shell_state);
    drop(shell_state);
    shell.borrow().render_sidebar();
}

fn open_ipc_server(
    shell: &Rc<RefCell<ApplicationShell>>,
    worklane_id: &str,
    raw_url: Option<&str>,
    browser: Option<&str>,
) -> Result<(), String> {
    let normalized = raw_url
        .map(normalize_server_url)
        .transpose()
        .map_err(|error| format!("{error:?}"))?;
    let (server, target) = {
        let shell = shell.borrow();
        let ranked = shell
            .ranked_servers()
            .into_iter()
            .filter(|ranked| ranked.server.worklane_id == worklane_id)
            .collect::<Vec<_>>();
        let server = normalized
            .as_ref()
            .and_then(|candidate| {
                ranked
                    .iter()
                    .find(|ranked| ranked.server.origin == candidate.origin)
            })
            .or_else(|| ranked.first())
            .map(|ranked| ranked.server.clone());
        let server = server.ok_or_else(|| "no matching development server".to_owned())?;
        let target = select_browser(&shell.server_runtime.browser_catalog, browser)?;
        (server, target)
    };
    launch_url(server.url, target)?;
    Ok(())
}

fn server_state_json(
    shell: &ApplicationShell,
    worklane_id: &str,
) -> Result<String, serde_json::Error> {
    let ranked = shell
        .ranked_servers()
        .into_iter()
        .filter(|ranked| ranked.server.worklane_id == worklane_id)
        .collect::<Vec<_>>();
    let primary = ranked.first().map(|ranked| ranked.server.id.clone());
    let servers = ranked
        .into_iter()
        .map(|ranked| {
            serde_json::json!({
                "id": ranked.server.id,
                "origin": ranked.server.origin,
                "url": ranked.server.url,
                "display": ranked.server.display,
                "worklaneID": ranked.server.worklane_id,
                "paneID": ranked.server.pane_id,
                "source": source_name(ranked.server.source),
                "ports": ranked.server.ports,
                "confidence": confidence_name(ranked.server.confidence),
                "tier": format!("{:?}", ranked.tier).to_ascii_lowercase(),
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&serde_json::json!({
        "version": 2,
        "primaryServerID": primary,
        "servers": servers,
    }))
}

const fn source_name(source: DetectedServerSource) -> &'static str {
    match source {
        DetectedServerSource::Manual => "manual",
        DetectedServerSource::Watch => "watch",
        DetectedServerSource::Docker => "docker",
        DetectedServerSource::Scanner => "scanner",
    }
}

const fn confidence_name(confidence: DetectedServerConfidence) -> &'static str {
    match confidence {
        DetectedServerConfidence::Explicit => "explicit",
        DetectedServerConfidence::Pid => "pid",
        DetectedServerConfidence::Cwd => "cwd",
        DetectedServerConfidence::Worklane => "worklane",
    }
}

fn ipc_failure(code: &str, message: String) -> ServerIpcReply {
    ServerIpcReply::failure(code, message).unwrap_or_else(|_| {
        ServerIpcReply::failure("internal_error", "invalid server IPC failure").unwrap()
    })
}

fn now_ms() -> Result<u64, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    u64::try_from(millis).map_err(|error| error.to_string())
}

fn refresh_registry_projection(shell: &mut ApplicationShell) {
    shell.server_runtime.servers = shell
        .state
        .worklanes()
        .iter()
        .flat_map(|worklane| shell.server_runtime.registry.servers_in(&worklane.id))
        .collect();
}

pub(super) fn clear_passive_servers(shell: &mut ApplicationShell) -> usize {
    let removed = shell
        .server_runtime
        .registry
        .remove_sources(&[DetectedServerSource::Docker, DetectedServerSource::Scanner]);
    refresh_registry_projection(shell);
    removed
}

pub(super) fn set_port_ignored(shell: &Rc<RefCell<ApplicationShell>>, origin: &str, ignored: bool) {
    let (port, rules) = {
        let shell = shell.borrow();
        let Some(server) = shell
            .server_runtime
            .servers
            .iter()
            .find(|server| server.origin == origin)
        else {
            eprintln!("zentty-linux: action=server-port-policy origin={origin} error=not-found");
            return;
        };
        if server.source == DetectedServerSource::Manual {
            eprintln!("zentty-linux: action=server-port-policy origin={origin} error=manual");
            return;
        }
        let Some(port) = server.ports.first().copied() else {
            eprintln!("zentty-linux: action=server-port-policy origin={origin} error=no-port");
            return;
        };
        (
            port,
            shell.config.server_detection.ignored_port_rules.clone(),
        )
    };
    let references = rules.iter().map(String::as_str).collect::<Vec<_>>();
    let updated = if ignored {
        ServerPortRule::adding_port(port, &references)
    } else {
        ServerPortRule::removing_port(port, &references)
    };
    let weak = Rc::downgrade(shell);
    glib::spawn_future_local(async move {
        let persisted = updated.clone();
        let result =
            gio::spawn_blocking(move || ConfigStore::update_default_ignored_port_rules(&persisted))
                .await;
        let Some(shell) = weak.upgrade() else {
            return;
        };
        match result {
            Ok(Ok(path)) => {
                shell
                    .borrow_mut()
                    .config
                    .server_detection
                    .ignored_port_rules = updated;
                shell.borrow().render_sidebar();
                eprintln!(
                    "zentty-linux: action=server-port-policy port={port} ignored={ignored} path={} result=persisted",
                    path.display()
                );
            }
            Ok(Err(error)) => eprintln!(
                "zentty-linux: action=server-port-policy port={port} ignored={ignored} error={error}"
            ),
            Err(_) => eprintln!(
                "zentty-linux: action=server-port-policy port={port} ignored={ignored} error=worker-panic"
            ),
        }
    });
}

fn request_probe(shell: &Rc<RefCell<ApplicationShell>>) {
    let (sources, discover_docker) = {
        let mut shell = shell.borrow_mut();
        if shell.shutting_down
            || shell.server_runtime.probe_in_flight
            || !shell.config.server_detection.passive_detection_enabled
        {
            return;
        }
        let mut sources = Vec::new();
        for worklane in shell.state.worklanes() {
            for pane in worklane.columns.iter().flat_map(|column| &column.panes) {
                let Some(pid) = shell
                    .pane_runtime
                    .surface(&pane.id)
                    .and_then(zentty_ghostty::GhosttySurface::foreground_process_id)
                    .and_then(|pid| u32::try_from(pid).ok())
                else {
                    continue;
                };
                sources.push(PaneProbeSource {
                    pane_id: pane.id.clone(),
                    worklane_id: worklane.id.clone(),
                    foreground_pid: pid,
                    working_directory: pane
                        .working_directory
                        .as_deref()
                        .map_or_else(|| PathBuf::from("/"), PathBuf::from),
                });
            }
        }
        if sources.is_empty() {
            return;
        }
        let discover_docker = shell.server_runtime.docker_scan_tick == 0;
        shell.server_runtime.docker_scan_tick = (shell.server_runtime.docker_scan_tick + 1) % 3;
        shell.server_runtime.probe_in_flight = true;
        (sources, discover_docker)
    };
    let weak = Rc::downgrade(shell);
    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(move || probe(&sources, discover_docker)).await;
        let Some(shell) = weak.upgrade() else {
            return;
        };
        let mut shell = shell.borrow_mut();
        shell.server_runtime.probe_in_flight = false;
        if shell.shutting_down || !shell.config.server_detection.passive_detection_enabled {
            return;
        }
        match result {
            Ok(Ok(servers)) => apply_servers(&mut shell, servers),
            Ok(Err(error)) => eprintln!("zentty-linux: server-scan error={error}"),
            Err(_) => eprintln!("zentty-linux: server-scan error=worker-panic"),
        }
    });
}

fn probe(sources: &[PaneProbeSource], discover_docker: bool) -> Result<ProbeResult, String> {
    let contexts = sources
        .iter()
        .filter_map(|source| {
            Some(PaneProcessContext {
                pane_id: source.pane_id.clone(),
                foreground_pid: source.foreground_pid,
                foreground_start_time: process_start_time(source.foreground_pid)?,
                working_directory: source.working_directory.clone(),
                repository_root: repository_root(&source.working_directory),
            })
        })
        .collect::<Vec<_>>();
    let observations =
        scan_listeners_at(Path::new("/proc"), &contexts).map_err(|error| error.to_string())?;
    let worklane_by_pane = sources
        .iter()
        .map(|source| (source.pane_id.as_str(), source.worklane_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    let now = u64::try_from(now).map_err(|error| error.to_string())?;
    let mut scanner_servers = observations
        .into_iter()
        .filter_map(|observation| {
            let pane_id = observation.pane_id?;
            let worklane_id = worklane_by_pane.get(pane_id.as_str())?;
            let host = if observation.local_host.contains(':') {
                format!("[{}]", observation.local_host)
            } else {
                observation.local_host
            };
            let candidate = normalize_server_url(&format!("{host}:{}", observation.port)).ok()?;
            Some(DetectedServer {
                id: format!("{}|{}", worklane_id, candidate.origin),
                origin: candidate.origin,
                url: candidate.url,
                display: candidate.display,
                worklane_id: (*worklane_id).to_owned(),
                pane_id: Some(pane_id),
                source: DetectedServerSource::Scanner,
                ports: vec![candidate.port],
                confidence: observation
                    .confidence
                    .unwrap_or(DetectedServerConfidence::Worklane),
                updated_at_ms: now,
                first_seen_at_ms: now,
            })
        })
        .collect::<Vec<_>>();
    scanner_servers.sort_by(|left, right| left.id.cmp(&right.id));
    let worklane_ids = sources
        .iter()
        .map(|source| source.worklane_id.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let (docker_servers, docker_error) = if discover_docker {
        let panes = sources
            .iter()
            .map(|source| crate::docker_discovery::DockerPaneContext {
                worklane_id: source.worklane_id.clone(),
                pane_id: source.pane_id.clone(),
                working_directory: source.working_directory.clone(),
            })
            .collect::<Vec<_>>();
        match crate::docker_discovery::discover(&panes, now) {
            Ok(servers) => (Some(servers), None),
            Err(error) => (None, Some(error)),
        }
    } else {
        (None, None)
    };
    Ok(ProbeResult {
        worklane_ids,
        scanner_servers,
        docker_servers,
        docker_error,
    })
}

fn apply_servers(shell: &mut ApplicationShell, result: ProbeResult) {
    if let Some(error) = result.docker_error {
        eprintln!("zentty-linux: docker-server-scan error={error}");
    }
    for worklane_id in &result.worklane_ids {
        shell.server_runtime.registry.replace_source(
            DetectedServerSource::Scanner,
            worklane_id,
            result
                .scanner_servers
                .iter()
                .filter(|server| &server.worklane_id == worklane_id)
                .cloned()
                .collect(),
        );
        if let Some(docker_servers) = &result.docker_servers {
            shell.server_runtime.registry.replace_source(
                DetectedServerSource::Docker,
                worklane_id,
                docker_servers
                    .iter()
                    .filter(|server| &server.worklane_id == worklane_id)
                    .cloned()
                    .collect(),
            );
        }
    }
    let servers = result
        .worklane_ids
        .iter()
        .flat_map(|worklane_id| shell.server_runtime.registry.servers_in(worklane_id))
        .collect::<Vec<_>>();
    let previous_ids = shell
        .server_runtime
        .servers
        .iter()
        .map(|server| (&server.id, &server.pane_id))
        .collect::<Vec<_>>();
    let next_ids = servers
        .iter()
        .map(|server| (&server.id, &server.pane_id))
        .collect::<Vec<_>>();
    let changed = previous_ids != next_ids;
    if changed {
        for server in &servers {
            eprintln!(
                "zentty-linux: server-detected worklane={} pane={} origin={} source={:?} confidence={:?}",
                server.worklane_id,
                server.pane_id.as_deref().unwrap_or("none"),
                server.origin,
                server.source,
                server.confidence
            );
        }
    }
    shell.server_runtime.servers = servers;
    if changed {
        shell.render_sidebar();
    }
}

pub(super) fn open_server(shell: &Rc<RefCell<ApplicationShell>>, origin: &str) {
    let request = {
        let shell = shell.borrow();
        shell
            .server_runtime
            .servers
            .iter()
            .find(|server| server.origin == origin)
            .map(|server| server.url.clone())
            .zip(shell.server_runtime.browser_catalog.preferred.clone())
    };
    let Some((url, target)) = request else {
        eprintln!("zentty-linux: action=open-server origin={origin} error=not-found");
        return;
    };
    if let Err(error) = launch_url(url, target) {
        eprintln!("zentty-linux: action=open-server origin={origin} error={error}");
    }
}

pub(super) fn open_server_in_browser(shell: &Rc<RefCell<ApplicationShell>>, action_id: &str) {
    let request = {
        let shell = shell.borrow();
        let Some((origin, browser_id)) = shell.server_runtime.browser_actions.get(action_id) else {
            eprintln!(
                "zentty-linux: action=open-server-browser id={action_id:?} error=stale-action"
            );
            return;
        };
        let Some(url) = shell
            .server_runtime
            .servers
            .iter()
            .find(|server| server.origin == *origin)
            .map(|server| server.url.clone())
        else {
            eprintln!(
                "zentty-linux: action=open-server-browser id={action_id:?} error=stale-server"
            );
            return;
        };
        let Some(target) = shell
            .server_runtime
            .browser_catalog
            .target(browser_id)
            .cloned()
        else {
            eprintln!(
                "zentty-linux: action=open-server-browser id={action_id:?} error=stale-browser"
            );
            return;
        };
        (url, target)
    };
    let browser_id = request.1.id.clone();
    if let Err(error) = launch_url(request.0, request.1.clone()) {
        eprintln!("zentty-linux: action=open-server-browser id={action_id:?} error={error}");
        return;
    }
    {
        let mut shell = shell.borrow_mut();
        shell
            .config
            .server_detection
            .preferred_browser_id
            .clone_from(&browser_id);
        shell.server_runtime.browser_catalog.preferred = Some(request.1);
    }
    glib::spawn_future_local(async move {
        let saved_id = browser_id.clone();
        match gio::spawn_blocking(move || ConfigStore::update_default_preferred_browser(&saved_id))
            .await
        {
            Ok(Ok(path)) => eprintln!(
                "zentty-linux: action=remember-server-browser id={browser_id} path={} result=persisted",
                path.display()
            ),
            Ok(Err(error)) => eprintln!(
                "zentty-linux: action=remember-server-browser id={browser_id} error={error}"
            ),
            Err(_) => eprintln!(
                "zentty-linux: action=remember-server-browser id={browser_id} error=worker-panic"
            ),
        }
    });
}

fn select_browser(
    catalog: &ServerBrowserCatalog,
    requested: Option<&str>,
) -> Result<ServerBrowserTarget, String> {
    let Some(requested) = requested else {
        return catalog
            .preferred
            .clone()
            .ok_or_else(|| "no development-server browser is available".to_owned());
    };
    let normalized = if requested == "system" {
        SYSTEM_DEFAULT_BROWSER_ID
    } else {
        requested
    };
    catalog
        .target(normalized)
        .or_else(|| {
            catalog.enabled.iter().find(|target| {
                matches!(
                    &target.launcher,
                    ServerBrowserLauncher::DesktopApplication { application_id }
                        if application_id == normalized
                )
            })
        })
        .cloned()
        .ok_or_else(|| format!("unknown or unavailable browser target {requested:?}"))
}

fn launch_url(url: String, target: ServerBrowserTarget) -> Result<(), String> {
    let plan = target
        .launch_plan(&url)
        .map_err(|error| format!("invalid server URL for browser launch: {error:?}"))?;
    let target_id = target.id;
    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(move || launch_browser_plan(plan)).await;
        match result {
            Ok(Ok(())) => eprintln!(
                "zentty-linux: action=open-server browser={target_id} url={url} result=opened"
            ),
            Ok(Err(error)) => eprintln!(
                "zentty-linux: action=open-server browser={target_id} url={url} error={error}"
            ),
            Err(_) => eprintln!("zentty-linux: action=open-server url={url} error=worker-panic"),
        }
    });
    Ok(())
}

fn launch_browser_plan(plan: ServerBrowserLaunchPlan) -> Result<(), String> {
    match plan {
        ServerBrowserLaunchPlan::SystemDefault { url } => {
            gio::AppInfo::launch_default_for_uri(&url, None::<&gio::AppLaunchContext>)
                .map_err(|error| error.to_string())
        }
        ServerBrowserLaunchPlan::DesktopApplication {
            application_id,
            url,
        } => {
            let app = gio::AppInfo::all()
                .into_iter()
                .find(|app| app.id().as_deref() == Some(application_id.as_str()))
                .ok_or_else(|| "desktop browser disappeared after discovery".to_owned())?;
            app.launch_uris(&[url.as_str()], None::<&gio::AppLaunchContext>)
                .map_err(|error| error.to_string())
        }
        ServerBrowserLaunchPlan::Executable {
            executable,
            arguments,
        } => {
            let mut child = std::process::Command::new(&executable)
                .args(arguments)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|error| format!("could not launch {executable}: {error}"))?;
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            Ok(())
        }
    }
}

pub(super) fn stop_server(shell: &Rc<RefCell<ApplicationShell>>, origin: &str) {
    let request = {
        let shell = shell.borrow();
        let Some(server) = shell
            .server_runtime
            .servers
            .iter()
            .find(|server| server.origin == origin)
            .cloned()
        else {
            eprintln!("zentty-linux: action=stop-server origin={origin} error=not-found");
            return;
        };
        let Some(pane_id) = server.pane_id.as_deref() else {
            eprintln!("zentty-linux: action=stop-server origin={origin} error=not-owned");
            return;
        };
        let Some(source) = pane_probe_source(&shell, pane_id) else {
            eprintln!("zentty-linux: action=stop-server origin={origin} error=no-live-pane");
            return;
        };
        (server, source)
    };
    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(move || stop_owned_server(&request.0, &request.1)).await;
        match result {
            Ok(Ok(pid)) => eprintln!("zentty-linux: action=stop-server pid={pid} result=stopped"),
            Ok(Err(error)) => eprintln!("zentty-linux: action=stop-server error={error}"),
            Err(_) => eprintln!("zentty-linux: action=stop-server error=worker-panic"),
        }
    });
}

fn pane_probe_source(shell: &ApplicationShell, pane_id: &str) -> Option<PaneProbeSource> {
    let (worklane_id, pane) = shell
        .state
        .worklanes()
        .iter()
        .flat_map(|worklane| {
            worklane.columns.iter().flat_map(move |column| {
                column
                    .panes
                    .iter()
                    .map(move |pane| (worklane.id.as_str(), pane))
            })
        })
        .find(|(_, pane)| pane.id == pane_id)?;
    let foreground_pid = shell
        .pane_runtime
        .surface(pane_id)?
        .foreground_process_id()
        .and_then(|pid| u32::try_from(pid).ok())?;
    Some(PaneProbeSource {
        pane_id: pane_id.to_owned(),
        worklane_id: worklane_id.to_owned(),
        foreground_pid,
        working_directory: pane
            .working_directory
            .as_deref()
            .map_or_else(|| PathBuf::from("/"), PathBuf::from),
    })
}

fn stop_owned_server(server: &DetectedServer, source: &PaneProbeSource) -> Result<u32, String> {
    let start_time = process_start_time(source.foreground_pid)
        .ok_or_else(|| "pane-process-disappeared".to_owned())?;
    let context = PaneProcessContext {
        pane_id: source.pane_id.clone(),
        foreground_pid: source.foreground_pid,
        foreground_start_time: start_time,
        working_directory: source.working_directory.clone(),
        repository_root: repository_root(&source.working_directory),
    };
    let observations =
        scan_listeners_at(Path::new("/proc"), &[context]).map_err(|error| error.to_string())?;
    let target = observations
        .into_iter()
        .find_map(|observation| {
            authorize_server_termination(
                server,
                &source.pane_id,
                source.foreground_pid,
                &ServerTerminationObservation {
                    pane_id: observation.pane_id,
                    listener_pid: observation.listener_pid,
                    listener_start_time: observation.listener_start_time,
                    port: observation.port,
                    owned_by_pane: observation.owned_by_pane,
                },
            )
        })
        .ok_or_else(|| "not-owned-or-not-running".to_owned())?;
    signal_process(target.pid, "INT")?;
    eprintln!(
        "zentty-linux: action=stop-server pid={} signal=INT result=delivered",
        target.pid
    );
    std::thread::sleep(Duration::from_secs(2));
    if process_start_time(target.pid) == Some(target.start_time) {
        signal_process(target.pid, "KILL")?;
    }
    Ok(target.pid)
}

fn signal_process(pid: u32, signal: &str) -> Result<(), String> {
    let status = std::process::Command::new("/bin/kill")
        .args(["-s", signal, "--", &pid.to_string()])
        .status()
        .map_err(|error| format!("signal-{signal}-failed: {error}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("signal-{signal}-exit: {status}"))
}

fn repository_root(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|candidate| candidate.join(".git").exists())
        .map(Path::to_owned)
}

#[cfg(test)]
mod tests {
    use super::normalized_catalog_name;

    #[test]
    fn browser_catalog_names_deduplicate_case_and_desktop_spacing() {
        assert_eq!(normalized_catalog_name("Google Chrome"), "googlechrome");
        assert_eq!(
            normalized_catalog_name("  google   CHROME "),
            "googlechrome"
        );
        assert_ne!(normalized_catalog_name("Chrome Beta"), "googlechrome");
    }
}
