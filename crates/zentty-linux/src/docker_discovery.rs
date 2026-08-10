use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;
use zentty_core::{
    DetectedServer, DetectedServerConfidence, DetectedServerSource, normalize_server_url,
};

const DOCKER_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_DOCKER_OUTPUT_BYTES: u64 = 4 * 1024 * 1024;
const COMMON_WEB_PORTS: &[u16] = &[
    80, 443, 3000, 3001, 4000, 4200, 5000, 5173, 8000, 8080, 8888,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DockerPaneContext {
    pub(crate) worklane_id: String,
    pub(crate) pane_id: String,
    pub(crate) working_directory: PathBuf,
}

pub(crate) fn discover(
    panes: &[DockerPaneContext],
    now_ms: u64,
) -> Result<Vec<DetectedServer>, String> {
    if !Path::new("/var/run/docker.sock").exists()
        && std::env::var_os("HOME")
            .map(PathBuf::from)
            .is_none_or(|home| !home.join(".docker/run/docker.sock").exists())
    {
        return Ok(Vec::new());
    }
    let ids = run_docker(&["ps", "-q"])?;
    let ids = ids.split_whitespace().collect::<Vec<_>>();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut arguments = vec!["inspect"];
    arguments.extend(ids);
    parse_inspect_json(&run_docker(&arguments)?, panes, now_ms)
}

fn run_docker(arguments: &[&str]) -> Result<String, String> {
    let mut child = Command::new("/usr/bin/docker")
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("docker-launch-failed: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "docker-stdout-missing".to_owned())?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take(MAX_DOCKER_OUTPUT_BYTES + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let deadline = Instant::now() + DOCKER_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("docker-timeout".into());
            }
            Err(error) => return Err(format!("docker-wait-failed: {error}")),
        }
    };
    let bytes = reader
        .join()
        .map_err(|_| "docker-reader-panicked".to_owned())?
        .map_err(|error| format!("docker-read-failed: {error}"))?;
    if !status.success() {
        return Err(format!("docker-exit: {status}"));
    }
    if bytes.len() as u64 > MAX_DOCKER_OUTPUT_BYTES {
        return Err("docker-output-too-large".into());
    }
    String::from_utf8(bytes).map_err(|_| "docker-output-invalid-utf8".into())
}

fn parse_inspect_json(
    source: &str,
    panes: &[DockerPaneContext],
    now_ms: u64,
) -> Result<Vec<DetectedServer>, String> {
    let containers = serde_json::from_str::<Vec<Value>>(source)
        .map_err(|error| format!("docker-json-invalid: {error}"))?;
    let mut servers = Vec::new();
    for container in containers {
        let labels = container
            .pointer("/Config/Labels")
            .and_then(Value::as_object);
        let project_paths = labels
            .into_iter()
            .flat_map(|labels| {
                labels
                    .iter()
                    .filter(|(key, _)| {
                        matches!(
                            key.as_str(),
                            "com.docker.compose.project.working_dir"
                                | "com.docker.compose.project.config_files"
                        )
                    })
                    .flat_map(|(key, value)| {
                        value.as_str().into_iter().flat_map(move |value| {
                            value.split(',').map(move |path| {
                                if key.ends_with("config_files") {
                                    Path::new(path)
                                        .parent()
                                        .unwrap_or(Path::new("/"))
                                        .to_owned()
                                } else {
                                    PathBuf::from(path)
                                }
                            })
                        })
                    })
            })
            .collect::<Vec<_>>();
        let Some(pane) = deepest_matching_pane(&project_paths, panes) else {
            continue;
        };
        if !is_web_like(&container) {
            continue;
        }
        let id = container
            .get("Id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let Some(ports) = container
            .pointer("/NetworkSettings/Ports")
            .and_then(Value::as_object)
        else {
            continue;
        };
        for (container_target, bindings) in ports {
            let Some((container_port, protocol)) = container_target.split_once('/') else {
                continue;
            };
            let Ok(container_port) = container_port.parse::<u16>() else {
                continue;
            };
            if protocol != "tcp" || !COMMON_WEB_PORTS.contains(&container_port) {
                continue;
            }
            for binding in bindings.as_array().into_iter().flatten() {
                let host = binding
                    .get("HostIp")
                    .and_then(Value::as_str)
                    .unwrap_or("localhost");
                let Some(port) = binding.get("HostPort").and_then(Value::as_str) else {
                    continue;
                };
                let Ok(candidate) = normalize_server_url(&format!("{host}:{port}")) else {
                    continue;
                };
                servers.push(DetectedServer {
                    id: format!("docker:{id}:{}", candidate.origin),
                    origin: candidate.origin,
                    url: candidate.url,
                    display: candidate.display,
                    worklane_id: pane.worklane_id.clone(),
                    pane_id: Some(pane.pane_id.clone()),
                    source: DetectedServerSource::Docker,
                    ports: vec![candidate.port],
                    confidence: DetectedServerConfidence::Cwd,
                    updated_at_ms: now_ms,
                    first_seen_at_ms: now_ms,
                });
            }
        }
    }
    servers.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(servers)
}

fn deepest_matching_pane<'a>(
    project_paths: &[PathBuf],
    panes: &'a [DockerPaneContext],
) -> Option<&'a DockerPaneContext> {
    let mut best = None;
    let mut best_depth = 0;
    let mut ambiguous = false;
    for pane in panes.iter().filter(|pane| {
        project_paths.iter().any(|project| {
            pane.working_directory.starts_with(project)
                || project.starts_with(&pane.working_directory)
        })
    }) {
        let depth = pane.working_directory.components().count();
        if depth > best_depth {
            best = Some(pane);
            best_depth = depth;
            ambiguous = false;
        } else if depth == best_depth {
            ambiguous = true;
        }
    }
    (!ambiguous).then_some(best).flatten()
}

fn is_web_like(container: &Value) -> bool {
    let text = container.to_string().to_ascii_lowercase();
    let excluded = [
        "postgres", "mysql", "mariadb", "redis", "mongo", "rabbitmq", "kafka",
    ];
    if excluded.iter().any(|term| text.contains(term)) {
        return false;
    }
    [
        "web", "app", "frontend", "vite", "next", "node", "http", "nginx", "busybox",
    ]
    .iter()
    .any(|term| text.contains(term))
}

#[cfg(test)]
mod tests {
    use super::{DockerPaneContext, deepest_matching_pane, parse_inspect_json};
    use std::path::PathBuf;
    use zentty_core::{DetectedServerConfidence, DetectedServerSource};

    #[test]
    fn compose_project_requires_one_uniquely_deepest_matching_pane() {
        let panes = vec![
            DockerPaneContext {
                worklane_id: "lane-1".into(),
                pane_id: "pane-1".into(),
                working_directory: PathBuf::from("/tmp/project/frontend"),
            },
            DockerPaneContext {
                worklane_id: "lane-2".into(),
                pane_id: "pane-2".into(),
                working_directory: PathBuf::from("/tmp/project/backend"),
            },
        ];
        assert!(deepest_matching_pane(&[PathBuf::from("/tmp/project")], &panes).is_none());

        let nested = DockerPaneContext {
            worklane_id: "lane-3".into(),
            pane_id: "pane-3".into(),
            working_directory: PathBuf::from("/tmp/project/frontend/app"),
        };
        let mut uniquely_nested = panes;
        uniquely_nested.push(nested);
        assert_eq!(
            deepest_matching_pane(&[PathBuf::from("/tmp/project")], &uniquely_nested)
                .map(|pane| pane.pane_id.as_str()),
            Some("pane-3")
        );
    }

    #[test]
    fn compose_web_port_is_attributed_but_database_and_unrelated_paths_are_rejected() {
        let pane = DockerPaneContext {
            worklane_id: "lane-1".into(),
            pane_id: "pane-1".into(),
            working_directory: PathBuf::from("/tmp/project/frontend"),
        };
        let source = r#"[
          {"Id":"web","Name":"/project-web-1","Config":{"Image":"busybox:latest","Cmd":["httpd","-f","-p","8000"],"Labels":{"com.docker.compose.project.working_dir":"/tmp/project"}},"NetworkSettings":{"Ports":{"8000/tcp":[{"HostIp":"0.0.0.0","HostPort":"49152"}]}}},
          {"Id":"db","Name":"/postgres","Config":{"Image":"postgres:16","Labels":{"com.docker.compose.project.working_dir":"/tmp/project"}},"NetworkSettings":{"Ports":{"5432/tcp":[{"HostIp":"0.0.0.0","HostPort":"5432"}]}}},
          {"Id":"other","Name":"/other-web","Config":{"Image":"busybox:latest","Labels":{"com.docker.compose.project.working_dir":"/tmp/other"}},"NetworkSettings":{"Ports":{"8000/tcp":[{"HostIp":"0.0.0.0","HostPort":"49153"}]}}}
        ]"#;
        let servers = parse_inspect_json(source, &[pane], 100).unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].origin, "http://localhost:49152");
        assert_eq!(servers[0].source, DetectedServerSource::Docker);
        assert_eq!(servers[0].confidence, DetectedServerConfidence::Cwd);
        assert_eq!(servers[0].pane_id.as_deref(), Some("pane-1"));
    }
}
