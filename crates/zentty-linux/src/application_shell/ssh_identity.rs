use std::cell::RefCell;
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gtk::{gio, glib};

use zentty_core::{SshDestination, parse_ssh_destination};

const MAX_PROC_FILE_BYTES: u64 = 64 * 1024;
const MAX_PROCESS_TREE_NODES: usize = 256;
const PROBE_INTERVAL: Duration = Duration::from_millis(500);

use crate::application_shell::ApplicationShell;

pub(crate) fn install(shell: &Rc<RefCell<ApplicationShell>>) -> glib::SourceId {
    let weak = Rc::downgrade(shell);
    glib::timeout_add_local(PROBE_INTERVAL, move || {
        let Some(shell) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        let mut shell = shell.borrow_mut();
        if shell.shutting_down {
            return glib::ControlFlow::Break;
        }
        if shell.ssh_probe_in_flight {
            return glib::ControlFlow::Continue;
        }
        let sources = shell
            .pane_runtime
            .live_pane_ids()
            .iter()
            .map(|pane_id| {
                let process_id = shell
                    .pane_runtime
                    .surface(pane_id)
                    .and_then(zentty_ghostty::GhosttySurface::foreground_process_id);
                (pane_id.clone(), process_id)
            })
            .collect::<Vec<_>>();
        shell.ssh_probe_in_flight = true;
        drop(shell);
        let weak = weak.clone();
        glib::spawn_future_local(async move {
            let observations = gio::spawn_blocking(move || {
                sources
                    .into_iter()
                    .map(|(pane_id, process_id)| {
                        let destination = process_id.and_then(probe_ssh_destination);
                        (pane_id, destination)
                    })
                    .collect::<Vec<_>>()
            })
            .await;
            let Some(shell) = weak.upgrade() else {
                return;
            };
            let mut shell = shell.borrow_mut();
            shell.ssh_probe_in_flight = false;
            if shell.shutting_down {
                return;
            }
            match observations {
                Ok(observations) => apply_observations(&mut shell, observations),
                Err(_) => eprintln!("zentty-linux: ssh-identity-probe state=worker-panic"),
            }
        });
        glib::ControlFlow::Continue
    })
}

fn apply_observations(
    shell: &mut ApplicationShell,
    observations: Vec<(String, Option<SshDestination>)>,
) {
    let mut changed = false;
    for (pane_id, destination) in observations {
        let label = destination
            .as_ref()
            .map(|destination| destination.target.as_str());
        if shell.state.set_pane_ssh_connection_label(&pane_id, label) {
            changed = true;
            eprintln!(
                "zentty-linux: ssh-identity pane={pane_id} state={}",
                if destination.is_some() {
                    "remote"
                } else {
                    "local"
                }
            );
        }
    }
    if changed {
        shell.render_sidebar();
        shell.refresh_pane_presentation();
    }
}

pub(crate) fn probe_ssh_destination(process_id: u64) -> Option<SshDestination> {
    probe_ssh_destination_at(Path::new("/proc"), process_id)
}

fn probe_ssh_destination_at(proc_root: &Path, process_id: u64) -> Option<SshDestination> {
    let process_id = u32::try_from(process_id).ok()?;
    if process_id == 0 {
        return None;
    }
    let process_root = proc_root.join(process_id.to_string());
    let root_start = read_start_time(&process_root)?;
    let mut processes = process_tree(proc_root, process_id)?;
    processes.sort_by_key(|(_, depth)| *depth);
    let destination = processes
        .into_iter()
        .rev()
        .find_map(|(process_id, _)| probe_process(&proc_root.join(process_id.to_string())));
    (read_start_time(&process_root)? == root_start)
        .then_some(destination)
        .flatten()
}

fn probe_process(process_root: &Path) -> Option<SshDestination> {
    let first_start = read_start_time(process_root)?;
    let name = read_bounded(process_root.join("comm"))?;
    if std::str::from_utf8(&name).ok()?.trim() != "ssh" {
        return None;
    }
    let command_line = read_bounded(process_root.join("cmdline"))?;
    let arguments = command_line
        .split(|byte| *byte == 0)
        .filter(|argument| !argument.is_empty())
        .map(std::str::from_utf8)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let destination = parse_ssh_destination(&arguments)?;
    (read_start_time(process_root)? == first_start).then_some(destination)
}

fn process_tree(proc_root: &Path, root_process_id: u32) -> Option<Vec<(u32, usize)>> {
    let mut processes = Vec::new();
    let mut pending = vec![(root_process_id, 0)];
    let mut visited = HashSet::new();
    while let Some((process_id, depth)) = pending.pop() {
        if !visited.insert(process_id) {
            continue;
        }
        if visited.len() > MAX_PROCESS_TREE_NODES {
            return None;
        }
        processes.push((process_id, depth));
        let children_path = proc_root
            .join(process_id.to_string())
            .join("task")
            .join(process_id.to_string())
            .join("children");
        let children = match read_bounded(children_path) {
            Some(children) => children,
            None if process_id == root_process_id => return None,
            None => continue,
        };
        let children = std::str::from_utf8(&children).ok()?;
        for child in children.split_whitespace() {
            pending.push((child.parse().ok()?, depth + 1));
        }
    }
    Some(processes)
}

fn read_bounded(path: PathBuf) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)
        .ok()?
        .take(MAX_PROC_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= MAX_PROC_FILE_BYTES).then_some(bytes)
}

fn read_start_time(process_root: &Path) -> Option<u64> {
    let stat = read_bounded(process_root.join("stat"))?;
    let stat = std::str::from_utf8(&stat).ok()?;
    let after_name = stat.rsplit_once(") ")?.1;
    after_name.split_whitespace().nth(19)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "zentty-ssh-probe-{}-{nonce}-{name}",
            std::process::id()
        ))
    }

    fn process(root: &Path, pid: u32, comm: &[u8], argv: &[u8], children: &[u32]) {
        let process = root.join(pid.to_string());
        let task = process.join("task").join(pid.to_string());
        fs::create_dir_all(&task).expect("fixture directory");
        fs::write(process.join("comm"), comm).expect("comm");
        fs::write(process.join("cmdline"), argv).expect("cmdline");
        fs::write(process.join("stat"), stat(pid, u64::from(pid) + 9000)).expect("stat");
        fs::write(
            task.join("children"),
            children
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(" "),
        )
        .expect("children");
    }

    fn stat(pid: u32, start_time: u64) -> String {
        let mut fields = vec!["S".to_owned()];
        fields.extend((4..=21).map(|field| field.to_string()));
        fields.push(start_time.to_string());
        format!("{pid} (ssh worker) {}\n", fields.join(" "))
    }

    #[test]
    fn real_proc_shape_requires_exact_ssh_identity_and_stable_start_time() {
        let root = fixture("valid");
        process(
            &root,
            42,
            b"ssh\n",
            b"/usr/bin/ssh\0-p\x002222\0deploy@example.test\0",
            &[],
        );
        assert_eq!(
            probe_ssh_destination_at(&root, 42),
            Some(SshDestination::new(
                "deploy@example.test",
                Some("deploy"),
                "example.test",
                Some(2222),
            ))
        );
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn walks_wrappers_and_prefers_the_deepest_ssh_descendant() {
        let root = fixture("tree");
        process(&root, 42, b"wrapper\n", b"wrapper\0", &[43]);
        process(&root, 43, b"ssh\n", b"ssh\0jump.example.test\0", &[44]);
        process(
            &root,
            44,
            b"ssh\n",
            b"ssh\0-l\0deploy\0prod.example.test\0",
            &[],
        );
        assert_eq!(
            probe_ssh_destination_at(&root, 42),
            Some(SshDestination::new(
                "deploy@prod.example.test",
                Some("deploy"),
                "prod.example.test",
                None,
            ))
        );
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn rejects_titles_non_ssh_processes_invalid_utf8_and_oversized_argv() {
        for (name, comm, argv) in [
            ("title", b"bash\n".as_slice(), b"ssh\0host\0".as_slice()),
            ("substring", b"my-ssh\n", b"ssh\0host\0"),
            ("utf8", b"ssh\n", b"ssh\0\xff\0"),
        ] {
            let root = fixture(name);
            process(&root, 42, comm, argv, &[]);
            assert_eq!(probe_ssh_destination_at(&root, 42), None);
            fs::remove_dir_all(root).expect("remove fixture");
        }
        let oversized =
            vec![b'x'; usize::try_from(MAX_PROC_FILE_BYTES).expect("proc bound fits usize") + 1];
        let root = fixture("oversized");
        process(&root, 42, b"ssh\n", &oversized, &[]);
        assert_eq!(probe_ssh_destination_at(&root, 42), None);
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn rejects_zero_missing_and_out_of_range_process_ids() {
        assert_eq!(probe_ssh_destination_at(Path::new("/missing"), 42), None);
        assert_eq!(probe_ssh_destination_at(Path::new("/proc"), 0), None);
        assert_eq!(
            probe_ssh_destination_at(Path::new("/proc"), u64::from(u32::MAX) + 1),
            None
        );
    }
}
