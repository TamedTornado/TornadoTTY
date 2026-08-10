use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gtk::{gio, glib};
use zentty_core::{
    DetectedServer, DetectedServerConfidence, DetectedServerSource, ServerPortRule, ServerRegistry,
    ServerTerminationObservation, authorize_server_termination, normalize_server_url,
};

use super::ApplicationShell;
use crate::config_store::ConfigStore;
use crate::server_discovery::{PaneProcessContext, process_start_time, scan_listeners_at};

const PROBE_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Default)]
pub(super) struct ServerRuntime {
    pub(super) probe_source: Option<glib::SourceId>,
    probe_in_flight: bool,
    pub(super) servers: Vec<DetectedServer>,
    registry: ServerRegistry,
    docker_scan_tick: u8,
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
        if shell.shutting_down || shell.server_runtime.probe_in_flight {
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
        if shell.shutting_down {
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
    let url = shell
        .borrow()
        .server_runtime
        .servers
        .iter()
        .find(|server| server.origin == origin)
        .map(|server| server.url.clone());
    let Some(url) = url else {
        eprintln!("zentty-linux: action=open-server origin={origin} error=not-found");
        return;
    };
    glib::spawn_future_local(async move {
        let launched_url = url.clone();
        let result = gio::spawn_blocking(move || {
            std::process::Command::new("xdg-open")
                .arg(&launched_url)
                .spawn()
                .and_then(|mut child| child.wait())
        })
        .await;
        match result {
            Ok(Ok(status)) if status.success() => {
                eprintln!("zentty-linux: action=open-server url={url} result=opened");
            }
            Ok(Ok(status)) => eprintln!(
                "zentty-linux: action=open-server url={url} error=launcher-exit status={status}"
            ),
            Ok(Err(error)) => {
                eprintln!("zentty-linux: action=open-server url={url} error={error}");
            }
            Err(_) => eprintln!("zentty-linux: action=open-server url={url} error=worker-panic"),
        }
    });
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
