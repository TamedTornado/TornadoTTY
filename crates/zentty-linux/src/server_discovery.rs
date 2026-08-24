use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Read;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use zentty_core::DetectedServerConfidence;

const MAX_NET_TABLE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_STAT_BYTES: u64 = 4096;
const MAX_PROCESSES: usize = 32_768;
const MAX_FDS_PER_PROCESS: usize = 4096;
const MAX_ANCESTRY_DEPTH: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PaneProcessContext {
    pub(crate) pane_id: String,
    pub(crate) foreground_pid: u32,
    pub(crate) foreground_start_time: u64,
    pub(crate) working_directory: PathBuf,
    pub(crate) repository_root: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ListenerObservation {
    pub(crate) pane_id: Option<String>,
    pub(crate) listener_pid: u32,
    pub(crate) listener_start_time: u64,
    pub(crate) local_host: String,
    pub(crate) port: u16,
    pub(crate) owned_by_pane: bool,
    pub(crate) confidence: Option<DetectedServerConfidence>,
}

#[derive(Debug)]
pub(crate) enum DiscoveryError {
    Read(String),
    TooLarge(PathBuf),
}

impl std::fmt::Display for DiscoveryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(detail) => write!(formatter, "read-failed {detail}"),
            Self::TooLarge(path) => write!(formatter, "input-too-large {}", path.display()),
        }
    }
}

#[derive(Clone, Debug)]
struct ProcessInfo {
    parent_pid: u32,
    start_time: u64,
    working_directory: Option<PathBuf>,
}

#[derive(Clone, Debug)]
struct SocketRecord {
    host: String,
    port: u16,
    inode: u64,
}

pub(crate) fn scan_listeners_at(
    proc_root: &Path,
    panes: &[PaneProcessContext],
) -> Result<Vec<ListenerObservation>, DiscoveryError> {
    let mut sockets = parse_socket_table(
        &read_bounded(&proc_root.join("net/tcp"), MAX_NET_TABLE_BYTES)?,
        false,
    );
    sockets.extend(parse_socket_table(
        &read_bounded(&proc_root.join("net/tcp6"), MAX_NET_TABLE_BYTES)?,
        true,
    ));
    let socket_by_inode = sockets
        .into_iter()
        .map(|socket| (socket.inode, socket))
        .collect::<BTreeMap<_, _>>();
    let mut processes = BTreeMap::new();
    let mut owners = Vec::new();
    let entries = fs::read_dir(proc_root)
        .map_err(|error| DiscoveryError::Read(format!("{}: {error}", proc_root.display())))?;
    for entry in entries.take(MAX_PROCESSES).flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        let process_root = entry.path();
        let Ok(stat) = read_bounded(&process_root.join("stat"), MAX_STAT_BYTES) else {
            continue;
        };
        let Some(info) = parse_process_info(&stat, &process_root) else {
            continue;
        };
        processes.insert(pid, info);
        let Ok(fds) = fs::read_dir(process_root.join("fd")) else {
            continue;
        };
        for fd in fds.take(MAX_FDS_PER_PROCESS).flatten() {
            let Ok(target) = fs::read_link(fd.path()) else {
                continue;
            };
            if let Some(inode) = socket_inode(&target)
                && socket_by_inode.contains_key(&inode)
            {
                owners.push((inode, pid));
            }
        }
    }
    let mut observations = Vec::new();
    for (inode, pid) in owners {
        let Some(socket) = socket_by_inode.get(&inode) else {
            continue;
        };
        let Some(listener) = processes.get(&pid) else {
            continue;
        };
        let pid_pane = panes.iter().find(|pane| {
            processes
                .get(&pane.foreground_pid)
                .is_some_and(|root| root.start_time == pane.foreground_start_time)
                && is_descendant(pid, pane.foreground_pid, &processes)
        });
        let attributed_pane = pid_pane.or_else(|| {
            listener
                .working_directory
                .as_deref()
                .and_then(|cwd| unique_cwd_pane(cwd, panes))
        });
        let confidence = if pid_pane.is_some() {
            Some(DetectedServerConfidence::Pid)
        } else if attributed_pane.is_some() {
            Some(DetectedServerConfidence::Cwd)
        } else {
            None
        };
        observations.push(ListenerObservation {
            pane_id: attributed_pane.map(|pane| pane.pane_id.clone()),
            listener_pid: pid,
            listener_start_time: listener.start_time,
            local_host: socket.host.clone(),
            port: socket.port,
            owned_by_pane: pid_pane.is_some(),
            confidence,
        });
    }
    observations.sort_by_key(|observation| (observation.port, observation.listener_pid));
    observations.dedup_by_key(|observation| (observation.port, observation.listener_pid));
    Ok(observations)
}

fn read_bounded(path: &Path, limit: u64) -> Result<String, DiscoveryError> {
    let mut file = File::open(path)
        .map_err(|error| DiscoveryError::Read(format!("{}: {error}", path.display())))?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| DiscoveryError::Read(format!("{}: {error}", path.display())))?;
    if bytes.len() as u64 > limit {
        return Err(DiscoveryError::TooLarge(path.to_owned()));
    }
    String::from_utf8(bytes)
        .map_err(|_| DiscoveryError::Read(format!("{}: invalid UTF-8", path.display())))
}

fn parse_socket_table(table: &str, ipv6: bool) -> Vec<SocketRecord> {
    table
        .lines()
        .skip(1)
        .filter_map(|line| {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            if fields.len() < 10 || fields[3] != "0A" {
                return None;
            }
            let (address, port) = fields[1].split_once(':')?;
            let port = u16::from_str_radix(port, 16).ok()?;
            if port == 0 {
                return None;
            }
            let host = if ipv6 {
                decode_ipv6(address)?
            } else {
                decode_ipv4(address)?
            };
            Some(SocketRecord {
                host,
                port,
                inode: fields[9].parse().ok()?,
            })
        })
        .collect()
}

fn decode_ipv4(hex: &str) -> Option<String> {
    let value = u32::from_str_radix(hex, 16).ok()?;
    Some(Ipv4Addr::from(value.to_le_bytes()).to_string())
}

fn decode_ipv6(hex: &str) -> Option<String> {
    if hex.len() != 32 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut bytes = [0_u8; 16];
    for (word_index, chunk) in hex.as_bytes().as_chunks::<8>().0.iter().enumerate() {
        let word = u32::from_str_radix(std::str::from_utf8(chunk).ok()?, 16).ok()?;
        bytes[word_index * 4..word_index * 4 + 4].copy_from_slice(&word.to_le_bytes());
    }
    Some(std::net::Ipv6Addr::from(bytes).to_string())
}

fn parse_process_info(stat: &str, process_root: &Path) -> Option<ProcessInfo> {
    let close = stat.rfind(')')?;
    let fields = stat
        .get(close + 1..)?
        .split_whitespace()
        .collect::<Vec<_>>();
    Some(ProcessInfo {
        parent_pid: fields.get(1)?.parse().ok()?,
        start_time: fields.get(19)?.parse().ok()?,
        working_directory: fs::read_link(process_root.join("cwd")).ok(),
    })
}

pub(crate) fn process_start_time(pid: u32) -> Option<u64> {
    let process_root = Path::new("/proc").join(pid.to_string());
    let stat = read_bounded(&process_root.join("stat"), MAX_STAT_BYTES).ok()?;
    parse_process_info(&stat, &process_root).map(|process| process.start_time)
}

fn unique_cwd_pane<'a>(
    cwd: &Path,
    panes: &'a [PaneProcessContext],
) -> Option<&'a PaneProcessContext> {
    let cwd = fs::canonicalize(cwd).ok()?;
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let broad = [Path::new("/"), Path::new("/tmp"), Path::new("/var/tmp")];
    let mut matches = panes
        .iter()
        .filter_map(|pane| {
            let pane_cwd = fs::canonicalize(&pane.working_directory).ok()?;
            if broad.contains(&pane_cwd.as_path()) || home.as_ref() == Some(&pane_cwd) {
                return None;
            }
            let repository = fs::canonicalize(pane.repository_root.as_ref()?).ok()?;
            (cwd.starts_with(&pane_cwd) && cwd.starts_with(&repository))
                .then_some((pane, pane_cwd.components().count()))
        })
        .collect::<Vec<_>>();
    matches.sort_by_key(|(_, depth)| std::cmp::Reverse(*depth));
    let (pane, depth) = matches.first().copied()?;
    (matches
        .iter()
        .filter(|(_, candidate)| candidate == &depth)
        .count()
        == 1)
        .then_some(pane)
}

fn socket_inode(target: &Path) -> Option<u64> {
    let target = target.to_str()?;
    target
        .strip_prefix("socket:[")?
        .strip_suffix(']')?
        .parse()
        .ok()
}

fn is_descendant(pid: u32, ancestor: u32, processes: &BTreeMap<u32, ProcessInfo>) -> bool {
    if pid == ancestor {
        return true;
    }
    let mut current = pid;
    let mut visited = BTreeSet::from([pid]);
    for _ in 0..MAX_ANCESTRY_DEPTH {
        let Some(parent) = processes.get(&current).map(|process| process.parent_pid) else {
            return false;
        };
        if parent == ancestor {
            return true;
        }
        if parent == 0 || !visited.insert(parent) {
            return false;
        }
        current = parent;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{PaneProcessContext, parse_process_info, scan_listeners_at};
    use std::fs;
    use std::net::TcpListener;
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn private_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "zentty-server-discovery-{name}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn stat(pid: u32, command: &str, parent: u32, start_time: u64) -> String {
        format!("{pid} ({command}) S {parent} 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 {start_time} 0\n")
    }

    fn create_process(root: &Path, pid: u32, parent: u32, start_time: u64, cwd: &Path) {
        let process = root.join(pid.to_string());
        fs::create_dir_all(process.join("fd")).unwrap();
        fs::write(
            process.join("stat"),
            stat(pid, "node dev server", parent, start_time),
        )
        .unwrap();
        symlink(cwd, process.join("cwd")).unwrap();
    }

    #[test]
    fn fake_proc_scan_attributes_ipv4_listener_to_exact_pane_ancestry() {
        let root = private_root("ipv4");
        let project = root.join("project");
        fs::create_dir_all(root.join("net")).unwrap();
        fs::create_dir_all(&project).unwrap();
        fs::write(
            root.join("net/tcp"),
            "sl local_address rem_address st tx_queue:rx_queue tr:tm->when retrnsmt uid timeout inode\n\
             0: 0100007F:1439 00000000:0000 0A 00000000:00000000 00:00000000 00000000 1000 0 12345\n",
        )
        .unwrap();
        fs::write(
            root.join("net/tcp6"),
            "sl local_address rem_address st inode\n",
        )
        .unwrap();
        create_process(&root, 4000, 1, 100, &project);
        create_process(&root, 4242, 4000, 200, &project);
        symlink("socket:[12345]", root.join("4242/fd/7")).unwrap();

        let observations = scan_listeners_at(
            &root,
            &[PaneProcessContext {
                pane_id: "pane-1".into(),
                foreground_pid: 4000,
                foreground_start_time: 100,
                working_directory: project.clone(),
                repository_root: Some(project.clone()),
            }],
        )
        .unwrap();
        assert_eq!(observations.len(), 1);
        let observation = &observations[0];
        assert_eq!(observation.pane_id, Some("pane-1".into()));
        assert_eq!(observation.listener_pid, 4242);
        assert_eq!(observation.listener_start_time, 200);
        assert_eq!(observation.local_host, "127.0.0.1");
        assert_eq!(observation.port, 5177);
        assert!(observation.owned_by_pane);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pid_reuse_cycle_and_unrelated_listener_are_not_owned() {
        let root = private_root("unowned");
        let project = root.join("project");
        let unrelated = root.join("unrelated");
        fs::create_dir_all(root.join("net")).unwrap();
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&unrelated).unwrap();
        fs::write(
            root.join("net/tcp"),
            "sl local_address rem_address st tx_queue:rx_queue tr:tm->when retrnsmt uid timeout inode\n\
             0: 00000000:0BB8 00000000:0000 0A 00000000:00000000 00:00000000 00000000 1000 0 555\n",
        )
        .unwrap();
        fs::write(root.join("net/tcp6"), "").unwrap();
        create_process(&root, 5000, 5001, 999, &project);
        create_process(&root, 5001, 5000, 501, &unrelated);
        symlink("socket:[555]", root.join("5001/fd/4")).unwrap();
        let observations = scan_listeners_at(
            &root,
            &[PaneProcessContext {
                pane_id: "pane-reused".into(),
                foreground_pid: 5000,
                foreground_start_time: 100,
                working_directory: project.clone(),
                repository_root: Some(project),
            }],
        )
        .unwrap();
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].pane_id, None);
        assert!(!observations[0].owned_by_pane);
        assert_eq!(observations[0].local_host, "0.0.0.0");
        assert_eq!(observations[0].port, 3000);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn real_proc_scan_correlates_a_live_kernel_listener_to_this_process() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let pid = std::process::id();
        let process_root = Path::new("/proc").join(pid.to_string());
        let stat = fs::read_to_string(process_root.join("stat")).unwrap();
        let process = parse_process_info(&stat, &process_root).unwrap();
        let observations = scan_listeners_at(
            Path::new("/proc"),
            &[PaneProcessContext {
                pane_id: "real-pane".into(),
                foreground_pid: pid,
                foreground_start_time: process.start_time,
                working_directory: std::env::current_dir().unwrap(),
                repository_root: None,
            }],
        )
        .unwrap();
        let observation = observations
            .iter()
            .find(|observation| observation.port == port && observation.listener_pid == pid)
            .expect("live listener missing from /proc observation");
        assert_eq!(observation.local_host, "127.0.0.1");
        assert_eq!(observation.pane_id.as_deref(), Some("real-pane"));
        assert!(observation.owned_by_pane);
        drop(listener);
    }
}
