#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CloseTarget {
    Pane {
        window_id: String,
        worklane_id: String,
        pane_id: String,
    },
    Worklane {
        window_id: String,
        worklane_id: String,
    },
    Window {
        window_id: String,
    },
    Application,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CloseReason {
    SessionHistory,
    ActiveAgent,
    RunningProcess,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClosePaneEvidence {
    pub pane_id: String,
    pub has_running_process: bool,
    pub has_active_agent: bool,
    pub has_session_history: bool,
}

impl ClosePaneEvidence {
    #[must_use]
    pub fn reason(&self) -> Option<CloseReason> {
        if self.has_running_process {
            Some(CloseReason::RunningProcess)
        } else if self.has_active_agent {
            Some(CloseReason::ActiveAgent)
        } else if self.has_session_history {
            Some(CloseReason::SessionHistory)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CloseEvidence {
    pub target: CloseTarget,
    pub panes: Vec<ClosePaneEvidence>,
}

impl CloseEvidence {
    #[must_use]
    pub fn new(target: CloseTarget, mut panes: Vec<ClosePaneEvidence>) -> Self {
        panes.sort_by(|left, right| left.pane_id.cmp(&right.pane_id));
        Self { target, panes }
    }

    #[must_use]
    pub fn strongest_reason(&self) -> Option<CloseReason> {
        self.panes
            .iter()
            .filter_map(ClosePaneEvidence::reason)
            .max()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CloseDecision {
    StaleTarget,
    CloseNow,
    Confirm {
        reason: CloseReason,
        evidence: CloseEvidence,
    },
}

#[must_use]
pub fn decide_close(evidence: CloseEvidence, confirmation_enabled: bool) -> CloseDecision {
    if evidence.panes.is_empty() {
        return CloseDecision::StaleTarget;
    }
    let Some(reason) = evidence.strongest_reason() else {
        return CloseDecision::CloseNow;
    };
    if !confirmation_enabled {
        return CloseDecision::CloseNow;
    }
    CloseDecision::Confirm { reason, evidence }
}

#[cfg(test)]
mod tests {
    use super::{
        CloseDecision, CloseEvidence, ClosePaneEvidence, CloseReason, CloseTarget, decide_close,
    };

    fn pane(
        pane_id: &str,
        has_running_process: bool,
        has_active_agent: bool,
        has_session_history: bool,
    ) -> ClosePaneEvidence {
        ClosePaneEvidence {
            pane_id: pane_id.to_owned(),
            has_running_process,
            has_active_agent,
            has_session_history,
        }
    }

    fn window(panes: Vec<ClosePaneEvidence>) -> CloseEvidence {
        CloseEvidence::new(
            CloseTarget::Window {
                window_id: "window-1".to_owned(),
            },
            panes,
        )
    }

    #[test]
    fn missing_target_fails_closed_as_stale() {
        assert_eq!(
            decide_close(window(Vec::new()), true),
            CloseDecision::StaleTarget
        );
    }

    #[test]
    fn idle_history_free_shell_closes_without_a_dialog() {
        assert_eq!(
            decide_close(window(vec![pane("pane-1", false, false, false)]), true),
            CloseDecision::CloseNow
        );
    }

    #[test]
    fn reasons_follow_source_risk_precedence() {
        let evidence = window(vec![
            pane("pane-history", false, false, true),
            pane("pane-agent", false, true, true),
            pane("pane-process", true, false, false),
        ]);
        assert!(matches!(
            decide_close(evidence, true),
            CloseDecision::Confirm {
                reason: CloseReason::RunningProcess,
                ..
            }
        ));
    }

    #[test]
    fn active_agent_outranks_session_history() {
        let evidence = window(vec![
            pane("pane-history", false, false, true),
            pane("pane-agent", false, true, false),
        ]);
        assert!(matches!(
            decide_close(evidence, true),
            CloseDecision::Confirm {
                reason: CloseReason::ActiveAgent,
                ..
            }
        ));
    }

    #[test]
    fn disabled_confirmation_closes_even_with_live_work() {
        assert_eq!(
            decide_close(window(vec![pane("pane-1", true, true, true)]), false),
            CloseDecision::CloseNow
        );
    }

    #[test]
    fn evidence_order_is_canonical_for_stale_callback_comparison() {
        let left = window(vec![
            pane("pane-b", false, false, true),
            pane("pane-a", true, false, false),
        ]);
        let right = window(vec![
            pane("pane-a", true, false, false),
            pane("pane-b", false, false, true),
        ]);
        assert_eq!(left, right);
    }
}
