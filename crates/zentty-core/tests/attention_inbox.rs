use std::collections::HashSet;
use zentty_core::{
    AgentInteractionKind, AgentPhase, AttentionInbox, AttentionTarget, PaneAgentStatus,
};

fn status(
    interaction: AgentInteractionKind,
    text: Option<&str>,
    updated_at: u64,
) -> PaneAgentStatus {
    PaneAgentStatus {
        session_id: "session".to_owned(),
        parent_session_id: None,
        agent_name: "Codex".to_owned(),
        phase: AgentPhase::NeedsInput,
        text: text.map(str::to_owned),
        interaction,
        progress: None,
        tracked_pid: None,
        transcript_path: None,
        updated_at,
    }
}

fn target(pane: &str) -> AttentionTarget {
    AttentionTarget::new("window-1", "lane-1", pane)
}

#[test]
fn new_and_changed_attention_coalesce_by_pane_without_losing_history() {
    let mut inbox = AttentionInbox::default();
    assert!(inbox.observe(
        target("pane-1"),
        Some(&status(
            AgentInteractionKind::Approval,
            Some("Allow write?"),
            1
        )),
        10,
    ));
    assert!(!inbox.observe(
        target("pane-1"),
        Some(&status(
            AgentInteractionKind::Approval,
            Some("Allow write?"),
            2
        )),
        11,
    ));
    assert!(inbox.observe(
        target("pane-1"),
        Some(&status(
            AgentInteractionKind::Question,
            Some("Which branch?"),
            3
        )),
        12,
    ));

    assert_eq!(inbox.items().len(), 2);
    assert_eq!(inbox.unresolved_count(), 1);
    assert_eq!(inbox.items()[0].primary_text, "Which branch?");
    assert_eq!(inbox.items()[0].status_text, "Has a question");
    assert_eq!(inbox.items()[1].resolved_at_ms, Some(12));
}

#[test]
fn resolved_and_stale_panes_remain_as_resolved_history() {
    let mut inbox = AttentionInbox::default();
    inbox.observe(
        target("pane-1"),
        Some(&status(AgentInteractionKind::Decision, None, 1)),
        10,
    );
    inbox.observe(
        target("pane-2"),
        Some(&status(AgentInteractionKind::Auth, Some("Sign in"), 1)),
        11,
    );
    assert!(inbox.observe(target("pane-1"), None, 12));
    assert!(inbox.resolve_stale("window-1", &HashSet::from([target("pane-1")]), 13));
    assert_eq!(inbox.unresolved_count(), 0);
    assert_eq!(inbox.items()[0].resolved_at_ms, Some(13));
    assert_eq!(inbox.items()[1].resolved_at_ms, Some(12));
}

#[test]
fn ordering_dismiss_clear_and_window_scoping_are_deterministic() {
    let mut inbox = AttentionInbox::default();
    inbox.observe(
        target("pane-1"),
        Some(&status(AgentInteractionKind::Approval, Some("First"), 1)),
        10,
    );
    let other = AttentionTarget::new("window-2", "lane-2", "pane-2");
    inbox.observe(
        other.clone(),
        Some(&status(AgentInteractionKind::Question, Some("Second"), 2)),
        11,
    );
    assert_eq!(inbox.most_urgent_unresolved().unwrap().target, other);
    let newest_id = inbox.items()[0].id;
    assert!(inbox.dismiss(newest_id));
    assert!(!inbox.dismiss(newest_id));
    assert_eq!(inbox.unresolved_count(), 1);
    assert!(inbox.clear());
    assert!(!inbox.clear());
}
