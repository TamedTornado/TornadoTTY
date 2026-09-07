//! Close policy inputs captured on GTK, with process I/O confined to a worker.
use std::collections::BTreeMap;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};

use zentty_core::{CloseEvidence, ClosePaneEvidence, CloseTarget};

use super::super::pane_context::PaneContext;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClosePaneSnapshot {
    pub(super) evidence: ClosePaneEvidence,
    pub(super) identity: PaneContext,
    pub(super) window_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CloseSnapshot {
    pub(crate) target: CloseTarget,
    pub(crate) panes: Vec<ClosePaneSnapshot>,
}

impl CloseSnapshot {
    pub(crate) fn new(target: CloseTarget, mut panes: Vec<ClosePaneSnapshot>) -> Self {
        panes.sort_by(|a, b| {
            (&a.window_id, &a.identity.pane_id).cmp(&(&b.window_id, &b.identity.pane_id))
        });
        Self { target, panes }
    }

    pub(super) fn inspect(&self) -> InspectedClose {
        let mut processes = BTreeMap::new();
        let panes = self
            .panes
            .iter()
            .map(|pane| {
                let mut evidence = pane.evidence.clone();
                if let Some(pid) = pane.identity.foreground_pid {
                    let process = processes.entry(pid).or_insert_with(|| inspect_process(pid));
                    evidence.has_running_process =
                        process.as_ref().is_none_or(|(comm, command_line)| {
                            !super::looks_like_idle_shell(comm.trim(), command_line)
                        });
                }
                evidence
            })
            .collect();
        InspectedClose {
            evidence: CloseEvidence::new(self.target.clone(), panes),
            processes,
        }
    }
}

#[derive(Eq, PartialEq)]
pub(super) struct InspectedClose {
    pub(super) evidence: CloseEvidence,
    // Never log process arguments. Retain them only across this confirmation,
    // so an exec in the same foreground PID is not mistaken for unchanged work.
    processes: BTreeMap<u64, Option<(String, Vec<u8>)>>,
}

fn inspect_process(pid: u64) -> Option<(String, Vec<u8>)> {
    let pid = u32::try_from(pid).ok()?;
    let root = std::path::Path::new("/proc").join(pid.to_string());
    Some((
        fs::read_to_string(root.join("comm")).ok()?,
        fs::read(root.join("cmdline")).ok()?,
    ))
}

static BUSY: AtomicBool = AtomicBool::new(false);

pub(super) struct Permit;
impl Permit {
    pub(super) fn acquire() -> Option<Self> {
        BUSY.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| Self)
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(pid: Option<u64>) -> CloseSnapshot {
        CloseSnapshot::new(
            CloseTarget::Window {
                window_id: "window".into(),
            },
            vec![ClosePaneSnapshot {
                window_id: "window".into(),
                identity: PaneContext {
                    pane_id: "pane".into(),
                    worklane_id: "lane".into(),
                    topology_generation: 1,
                    working_directory: None,
                    foreground_pid: pid,
                    remote: false,
                },
                evidence: ClosePaneEvidence {
                    pane_id: "pane".into(),
                    has_running_process: false,
                    has_active_agent: false,
                    has_session_history: false,
                },
            }],
        )
    }

    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[test]
    fn real_proc_inspection_keeps_idle_live_and_missing_process_policy() {
        let idle = Child(
            std::process::Command::new("/bin/bash")
                .args(["--noprofile", "--norc"])
                .stdin(std::process::Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let source = snapshot(Some(u64::from(idle.0.id())));
        assert!(!source.inspect().evidence.panes[0].has_running_process);
        let running = Child(
            std::process::Command::new("/bin/sleep")
                .arg("30")
                .spawn()
                .unwrap(),
        );
        assert!(
            snapshot(Some(u64::from(running.0.id())))
                .inspect()
                .evidence
                .panes[0]
                .has_running_process
        );
        let before = source.inspect();
        drop(idle);
        let after = source.inspect();
        assert!(
            after.evidence.panes[0].has_running_process,
            "unreadable is not idle"
        );
        assert!(
            before != after,
            "accepted process identity must be revalidated"
        );
        assert!(!snapshot(None).inspect().evidence.panes[0].has_running_process);
        assert!(snapshot(Some(u64::MAX)).inspect().evidence.panes[0].has_running_process);
    }

    #[test]
    fn worker_capacity_is_released_after_completion_and_panic() {
        let permit = Permit::acquire().unwrap();
        let (release, blocked) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let _permit = permit;
            blocked.recv().unwrap();
        });
        assert!(Permit::acquire().is_none());
        release.send(()).unwrap();
        worker.join().unwrap();
        let result = std::panic::catch_unwind(|| {
            let _permit = Permit::acquire().unwrap();
            panic!("controlled worker panic");
        });
        assert!(result.is_err());
        assert!(Permit::acquire().is_some());
    }
}
