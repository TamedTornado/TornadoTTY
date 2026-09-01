use std::collections::HashSet;
use zentty_core::{
    AgentInteractionKind, AgentPhase, AgentSignalConfidence, AgentSignalOrigin, AttentionInbox,
    AttentionState, AttentionTarget, PaneAgentStatus,
};

#[test]
fn pane_notification_is_recorded_without_fabricating_an_agent_delivery() {
    let mut inbox = AttentionInbox::default();
    let target = AttentionTarget::new("window-1", "lane-1", "pane-1");

    assert!(inbox.record_pane_notification(
        target.clone(),
        "Build complete",
        "Review the result",
        42,
    ));
    assert!(inbox.drain_deliveries().is_empty());
    let item = &inbox.items()[0];
    assert_eq!(item.target, target);
    assert_eq!(item.agent_name, zentty_core::PRODUCT_NAME);
    assert_eq!(item.state, AttentionState::Ready);
    assert_eq!(item.status_text, "Build complete");
    assert_eq!(item.primary_text, "Review the result");
    assert_eq!(item.created_at_ms, 42);
    assert!(!inbox.observe(target, None, 43));
    assert!(!inbox.items()[0].is_resolved());
}

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
        artifact_link: None,
        working_directory: None,
        agent_launch_snapshot: None,
        signal_origin: AgentSignalOrigin::ExplicitHook,
        signal_confidence: AgentSignalConfidence::Explicit,
        updated_at,
    }
}

fn target(pane: &str) -> AttentionTarget {
    AttentionTarget::new("window-1", "lane-1", pane)
}

fn phase_status(
    phase: AgentPhase,
    interaction: AgentInteractionKind,
    text: Option<&str>,
    updated_at: u64,
) -> PaneAgentStatus {
    let mut status = status(interaction, text, updated_at);
    status.phase = phase;
    status
}

#[test]
fn new_and_changed_attention_coalesce_by_pane_without_losing_history() {
    let mut inbox = AttentionInbox::default();
    assert!(!inbox.observe(
        target("pane-1"),
        Some(&status(
            AgentInteractionKind::Approval,
            Some("Allow write?"),
            1
        )),
        10,
    ));
    assert!(inbox.advance(3_010));
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
    assert!(inbox.advance(3_012));

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
    assert!(inbox.items().is_empty());
}

#[test]
fn ordering_dismiss_clear_and_window_scoping_are_deterministic() {
    let mut inbox = AttentionInbox::default();
    inbox.observe(
        target("pane-1"),
        Some(&status(AgentInteractionKind::Approval, Some("First"), 1)),
        10,
    );
    inbox.advance(3_010);
    let other = AttentionTarget::new("window-2", "lane-2", "pane-2");
    inbox.observe(
        other.clone(),
        Some(&status(AgentInteractionKind::Question, Some("Second"), 2)),
        11,
    );
    inbox.advance(3_011);
    assert_eq!(inbox.most_urgent_unresolved().unwrap().target, other);
    let newest_id = inbox.items()[0].id;
    assert!(inbox.dismiss(newest_id));
    let deliveries = inbox.drain_deliveries();
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].item.primary_text, "First");
    assert!(!inbox.dismiss(newest_id));
    assert_eq!(inbox.unresolved_count(), 1);
    assert_eq!(inbox.items()[0].primary_text, "First");
    assert!(inbox.clear());
    assert!(!inbox.clear());
}

#[test]
fn needs_input_is_debounced_and_resolution_cancels_pending_delivery() {
    let mut inbox = AttentionInbox::default();
    let waiting = status(
        AgentInteractionKind::Approval,
        Some("Approve deployment?"),
        1,
    );
    assert!(!inbox.observe_with_context(
        target("pane-1"),
        Some(&waiting),
        false,
        Some("Frontend · deploy".to_owned()),
        10,
    ));
    assert_eq!(inbox.pending_count(), 1);
    assert!(!inbox.advance(3_009));
    assert!(inbox.observe(target("pane-1"), None, 100));
    assert_eq!(inbox.pending_count(), 0);
    assert!(!inbox.advance(4_000));
    assert!(inbox.items().is_empty());
    assert!(inbox.drain_deliveries().is_empty());
}

#[test]
fn completion_and_unresolved_stop_are_immediate_and_source_shaped() {
    let mut inbox = AttentionInbox::default();
    assert!(inbox.observe_with_context(
        target("ready"),
        Some(&phase_status(
            AgentPhase::Idle,
            AgentInteractionKind::None,
            None,
            1,
        )),
        false,
        Some("Frontend · tests".to_owned()),
        10,
    ));
    assert!(inbox.observe_with_context(
        target("stopped"),
        Some(&phase_status(
            AgentPhase::UnresolvedStop,
            AgentInteractionKind::None,
            None,
            2,
        )),
        false,
        Some("Backend · server".to_owned()),
        11,
    ));

    assert_eq!(inbox.items()[0].state, AttentionState::UnresolvedStop);
    assert_eq!(inbox.items()[0].status_text, "Stopped early");
    assert_eq!(inbox.items()[1].state, AttentionState::Ready);
    assert_eq!(inbox.items()[1].primary_text, "Agent is ready.");
    let deliveries = inbox.drain_deliveries();
    assert_eq!(deliveries.len(), 2);
    assert!(deliveries.iter().all(|delivery| delivery.desktop_allowed));
}

#[test]
fn ready_text_enriches_one_completion_but_a_new_running_cycle_delivers_again() {
    let mut inbox = AttentionInbox::default();
    let pane = target("pane-1");
    let ready = phase_status(AgentPhase::Idle, AgentInteractionKind::None, None, 1);

    assert!(inbox.observe(pane.clone(), Some(&ready), 10));
    assert_eq!(inbox.drain_deliveries().len(), 1);

    let enriched = phase_status(
        AgentPhase::Idle,
        AgentInteractionKind::None,
        Some("The completed response from the terminal notification."),
        2,
    );
    assert!(inbox.observe(pane.clone(), Some(&enriched), 11));
    assert_eq!(inbox.items().len(), 1);
    assert_eq!(
        inbox.items()[0].primary_text,
        "The completed response from the terminal notification."
    );
    assert!(inbox.drain_deliveries().is_empty());

    let running = phase_status(AgentPhase::Running, AgentInteractionKind::None, None, 3);
    assert!(inbox.observe(pane.clone(), Some(&running), 12));
    let ready_again = phase_status(AgentPhase::Idle, AgentInteractionKind::None, None, 4);
    assert!(inbox.observe(pane, Some(&ready_again), 13));
    assert_eq!(inbox.items().len(), 2);
    assert_eq!(inbox.drain_deliveries().len(), 1);
}

#[test]
fn focused_pane_keeps_inbox_history_but_suppresses_desktop_and_focus_resolves_existing() {
    let mut inbox = AttentionInbox::default();
    let ready = phase_status(AgentPhase::Idle, AgentInteractionKind::None, None, 1);
    assert!(inbox.observe_with_context(target("pane-1"), Some(&ready), true, None, 10,));
    let deliveries = inbox.drain_deliveries();
    assert_eq!(deliveries.len(), 1);
    assert!(!deliveries[0].desktop_allowed);
    assert_eq!(inbox.unresolved_count(), 1);

    assert!(!inbox.observe_with_context(target("pane-1"), Some(&ready), false, None, 11,));
    assert!(inbox.observe_with_context(target("pane-1"), Some(&ready), true, None, 12,));
    assert_eq!(inbox.unresolved_count(), 0);
}

#[test]
fn clear_and_stale_cleanup_cancel_pending_items_without_late_delivery() {
    let mut inbox = AttentionInbox::default();
    inbox.observe(
        target("pane-1"),
        Some(&status(
            AgentInteractionKind::Question,
            Some("Continue?"),
            1,
        )),
        10,
    );
    assert!(inbox.clear());
    assert_eq!(inbox.pending_count(), 0);
    assert!(!inbox.advance(10_000));

    inbox.observe(
        target("pane-2"),
        Some(&status(AgentInteractionKind::Question, Some("Retry?"), 2)),
        20,
    );
    assert!(inbox.resolve_stale("window-1", &HashSet::new(), 21));
    assert_eq!(inbox.pending_count(), 0);
    assert!(!inbox.advance(10_000));
}

#[test]
fn resolution_clear_and_stale_cleanup_cancel_queued_desktop_delivery() {
    let mut inbox = AttentionInbox::default();
    let ready = phase_status(AgentPhase::Idle, AgentInteractionKind::None, None, 1);

    assert!(inbox.observe(target("resolved"), Some(&ready), 10));
    assert!(inbox.resolve_target(&target("resolved"), 11));
    assert!(inbox.drain_deliveries().is_empty());

    assert!(inbox.observe(target("cleared"), Some(&ready), 12));
    assert!(inbox.clear());
    assert!(inbox.drain_deliveries().is_empty());

    assert!(inbox.observe(target("stale"), Some(&ready), 13));
    assert!(inbox.resolve_stale("window-1", &HashSet::new(), 14));
    assert!(inbox.drain_deliveries().is_empty());
}

#[test]
fn stale_cleanup_is_scoped_to_missing_targets_in_one_window() {
    let mut inbox = AttentionInbox::default();
    let ready = phase_status(AgentPhase::Idle, AgentInteractionKind::None, None, 1);
    let stale = target("stale");
    let live = target("live");
    let other = AttentionTarget::new("window-2", "lane-2", "other");

    assert!(inbox.observe_with_context(stale.clone(), Some(&ready), false, None, 10));
    assert!(inbox.observe_with_context(live.clone(), Some(&ready), false, None, 11));
    assert!(inbox.observe_with_context(other.clone(), Some(&ready), true, None, 12));

    let waiting = status(AgentInteractionKind::Question, Some("Still live?"), 2);
    let pending_stale = target("pending-stale");
    let pending_live = target("pending-live");
    let pending_other = AttentionTarget::new("window-2", "lane-2", "pending-other");
    assert!(!inbox.observe(pending_stale.clone(), Some(&waiting), 20));
    assert!(!inbox.observe(pending_live.clone(), Some(&waiting), 21));
    assert!(!inbox.observe(pending_other.clone(), Some(&waiting), 22));

    assert!(inbox.resolve_stale(
        "window-1",
        &HashSet::from([live.clone(), pending_live.clone()]),
        30,
    ));
    assert_eq!(inbox.pending_count(), 2);
    assert_eq!(
        inbox
            .items()
            .iter()
            .find(|item| item.target == stale)
            .unwrap()
            .resolved_at_ms,
        Some(30)
    );
    assert!(
        !inbox
            .items()
            .iter()
            .find(|item| item.target == live)
            .unwrap()
            .is_resolved()
    );
    assert!(
        !inbox
            .items()
            .iter()
            .find(|item| item.target == other)
            .unwrap()
            .is_resolved()
    );

    // State for live targets in this and other windows must survive pruning.
    assert!(!inbox.observe_with_context(live.clone(), Some(&ready), false, None, 31));
    assert!(!inbox.observe_with_context(other.clone(), Some(&ready), true, None, 32));
    assert!(inbox.advance(3_022));
    let deliveries = inbox.drain_deliveries();
    assert_eq!(deliveries.len(), 4);
    assert!(
        deliveries
            .iter()
            .any(|delivery| delivery.item.target == live)
    );
    assert!(
        deliveries
            .iter()
            .any(|delivery| delivery.item.target == other)
    );
    assert!(
        deliveries
            .iter()
            .any(|delivery| delivery.item.target == pending_live)
    );
    assert!(
        deliveries
            .iter()
            .any(|delivery| delivery.item.target == pending_other)
    );
    assert!(
        !deliveries
            .iter()
            .any(|delivery| delivery.item.target == pending_stale)
    );
}

#[test]
fn needs_input_without_an_interaction_is_not_attention_and_fallbacks_are_exact() {
    let mut inbox = AttentionInbox::default();
    assert!(!inbox.observe(
        target("none"),
        Some(&status(
            AgentInteractionKind::None,
            Some("informational"),
            1
        )),
        10,
    ));
    assert_eq!(inbox.pending_count(), 0);

    for (index, interaction) in [
        AgentInteractionKind::Approval,
        AgentInteractionKind::Decision,
        AgentInteractionKind::Question,
        AgentInteractionKind::Auth,
        AgentInteractionKind::GenericInput,
    ]
    .into_iter()
    .enumerate()
    {
        let pane = format!("fallback-{index}");
        assert!(!inbox.observe(target(&pane), Some(&status(interaction, None, 2)), 20));
    }
    assert!(inbox.advance(3_020));
    let observed = inbox
        .items()
        .iter()
        .map(|item| item.primary_text.as_str())
        .collect::<HashSet<_>>();
    assert_eq!(
        observed,
        HashSet::from([
            "Approval required.",
            "Decision required.",
            "Question pending.",
            "Sign-in required.",
            "Input required.",
        ])
    );
}

#[test]
fn stale_cleanup_reports_no_change_when_every_target_is_live_or_in_another_window() {
    let mut inbox = AttentionInbox::default();
    let ready = phase_status(AgentPhase::Idle, AgentInteractionKind::None, None, 1);
    let live = target("live");
    let other = AttentionTarget::new("window-2", "lane-2", "other");
    assert!(inbox.observe(live.clone(), Some(&ready), 10));
    assert!(inbox.observe(other.clone(), Some(&ready), 11));

    assert!(!inbox.resolve_stale("window-1", &HashSet::from([live]), 12));
    let deliveries = inbox.drain_deliveries();
    assert_eq!(deliveries.len(), 2);
    assert!(
        deliveries
            .iter()
            .any(|delivery| delivery.item.target == other)
    );
}
