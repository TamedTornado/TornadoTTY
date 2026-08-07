use zentty_core::{AgentPhase, PaneAgentStatus};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentStatusPresentation {
    pub(crate) text: String,
    pub(crate) requires_attention: bool,
}

pub(crate) fn present(status: &PaneAgentStatus) -> AgentStatusPresentation {
    let state = match status.phase {
        AgentPhase::Starting => "Starting".to_owned(),
        AgentPhase::Running => status.progress.map_or_else(
            || status.text.clone().unwrap_or_else(|| "Running".to_owned()),
            |progress| format!("Running ({}/{})", progress.done, progress.total),
        ),
        AgentPhase::NeedsInput => status.text.as_deref().map_or_else(
            || "Needs input".to_owned(),
            |text| format!("Needs input: {text}"),
        ),
        AgentPhase::Idle => "Idle".to_owned(),
        AgentPhase::UnresolvedStop => "Stopped unexpectedly".to_owned(),
    };
    AgentStatusPresentation {
        text: format!("{} · {state}", status.agent_name),
        requires_attention: status.requires_attention(),
    }
}

#[cfg(test)]
mod tests {
    use super::present;
    use zentty_core::{AgentInteractionKind, AgentPhase, AgentProgress, PaneAgentStatus};

    fn status(phase: AgentPhase) -> PaneAgentStatus {
        PaneAgentStatus {
            session_id: "session".to_owned(),
            parent_session_id: None,
            agent_name: "Claude Code".to_owned(),
            phase,
            text: None,
            interaction: AgentInteractionKind::None,
            progress: None,
            tracked_pid: None,
            transcript_path: None,
            updated_at: 1,
        }
    }

    #[test]
    fn presents_running_progress_without_hiding_the_agent() {
        let mut value = status(AgentPhase::Running);
        value.progress = Some(AgentProgress { done: 2, total: 5 });
        assert_eq!(present(&value).text, "Claude Code · Running (2/5)");
    }

    #[test]
    fn presents_attention_reason_and_attention_state() {
        let mut value = status(AgentPhase::NeedsInput);
        value.interaction = AgentInteractionKind::Approval;
        value.text = Some("Run cargo test?".to_owned());
        let presentation = present(&value);
        assert_eq!(
            presentation.text,
            "Claude Code · Needs input: Run cargo test?"
        );
        assert!(presentation.requires_attention);
    }

    #[test]
    fn idle_and_unresolved_stop_remain_distinct() {
        assert_eq!(
            present(&status(AgentPhase::Idle)).text,
            "Claude Code · Idle"
        );
        assert_eq!(
            present(&status(AgentPhase::UnresolvedStop)).text,
            "Claude Code · Stopped unexpectedly"
        );
    }
}
