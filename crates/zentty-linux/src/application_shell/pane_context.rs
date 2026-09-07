//! In-memory identity for work crossing the GTK/background boundary.
//! This is a snapshot, not a second source of pane state. No filesystem reads.
use std::path::PathBuf;

use super::ApplicationShell;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PaneContext {
    pub(super) pane_id: String,
    pub(super) worklane_id: String,
    pub(super) topology_generation: u64,
    pub(super) working_directory: Option<PathBuf>,
    pub(super) foreground_pid: Option<u64>,
    pub(super) remote: bool,
}

impl PaneContext {
    pub(super) fn capture(shell: &ApplicationShell, pane_id: &str) -> Option<Self> {
        let pane = shell.state.pane(pane_id)?;
        Some(Self {
            pane_id: pane_id.to_owned(),
            worklane_id: shell.state.worklane_id_for_pane(pane_id)?.to_owned(),
            topology_generation: shell.topology_generation(),
            working_directory: shell
                .state
                .effective_working_directory_for_pane(pane_id)
                .map(PathBuf::from),
            foreground_pid: shell
                .pane_runtime
                .surface(pane_id)
                .and_then(zentty_ghostty::GhosttySurface::foreground_process_id),
            remote: pane.ssh_connection_label.is_some()
                || shell.remote_panes.identities.contains_key(pane_id),
        })
    }

    pub(super) fn is_current(&self, shell: &ApplicationShell) -> bool {
        !shell.shutting_down && self.matches(Self::capture(shell, &self.pane_id).as_ref())
    }

    pub(super) fn matches(&self, current: Option<&Self>) -> bool {
        current == Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_moved_replaced_remote_or_changed_panes_reject_pending_work() {
        let original = PaneContext {
            pane_id: "pane-1".into(),
            worklane_id: "lane-1".into(),
            topology_generation: 4,
            working_directory: Some("/project".into()),
            foreground_pid: Some(100),
            remote: false,
        };
        assert!(original.matches(Some(&original)));
        assert!(!original.matches(None));
        let changes = [
            PaneContext {
                pane_id: "pane-2".into(),
                ..original.clone()
            },
            PaneContext {
                worklane_id: "lane-2".into(),
                ..original.clone()
            },
            PaneContext {
                topology_generation: 5,
                ..original.clone()
            },
            PaneContext {
                working_directory: Some("/other".into()),
                ..original.clone()
            },
            PaneContext {
                foreground_pid: Some(101),
                ..original.clone()
            },
            PaneContext {
                remote: true,
                ..original.clone()
            },
        ];
        for current in changes {
            assert!(!original.matches(Some(&current)), "{current:?}");
        }
    }
}
