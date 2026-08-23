use zentty_core::{
    AgentInteractionKind, AgentPhase, AgentProgress, AgentSignalConfidence, AgentSignalOrigin,
    FleetState, FleetSummary, FleetWindowSource, PaneAgentStatus, SidebarPaneSummary,
    SidebarWorklaneSummary, build_fleet_snapshots,
};

fn status(
    name: &str,
    phase: AgentPhase,
    interaction: AgentInteractionKind,
    text: Option<&str>,
    updated_at: u64,
) -> PaneAgentStatus {
    PaneAgentStatus {
        session_id: format!("session-{updated_at}"),
        parent_session_id: None,
        agent_name: name.to_owned(),
        phase,
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

fn pane(id: &str, title: &str, agent_status: Option<PaneAgentStatus>) -> SidebarPaneSummary {
    SidebarPaneSummary {
        pane_id: id.to_owned(),
        primary_text: title.to_owned(),
        custom_title: None,
        working_directory: None,
        is_focused: false,
        agent_status,
        project_context: None,
        project_icon_path: None,
    }
}

fn lane(id: &str, title: Option<&str>, panes: Vec<SidebarPaneSummary>) -> SidebarWorklaneSummary {
    SidebarWorklaneSummary {
        worklane_id: id.to_owned(),
        top_label: title.map(str::to_owned),
        primary_text: "shell".to_owned(),
        pane_rows: panes,
        is_active: false,
        color: None,
    }
}

#[test]
fn source_priority_and_labels_match_the_fleet_contract() {
    let worklanes = vec![lane(
        "lane-1",
        Some("Frontend"),
        vec![
            pane(
                "idle",
                "Idle task",
                Some(status(
                    "Codex",
                    AgentPhase::Idle,
                    AgentInteractionKind::None,
                    None,
                    50,
                )),
            ),
            pane(
                "active",
                "Running task",
                Some(status(
                    "Claude Code",
                    AgentPhase::Running,
                    AgentInteractionKind::None,
                    Some("Running tools"),
                    30,
                )),
            ),
            pane(
                "compact",
                "Context task",
                Some(status(
                    "Codex",
                    AgentPhase::Running,
                    AgentInteractionKind::None,
                    Some("Summarizing context"),
                    20,
                )),
            ),
            pane(
                "stopped",
                "Stopped task",
                Some(status(
                    "Gemini",
                    AgentPhase::UnresolvedStop,
                    AgentInteractionKind::None,
                    None,
                    10,
                )),
            ),
            pane(
                "waiting",
                "Approval task",
                Some(status(
                    "Codex",
                    AgentPhase::NeedsInput,
                    AgentInteractionKind::Approval,
                    Some("Allow deployment?"),
                    40,
                )),
            ),
        ],
    )];
    let snapshots = build_fleet_snapshots(&[FleetWindowSource {
        window_id: "window-1",
        window_title: "Zentty",
        worklanes: &worklanes,
    }]);

    assert_eq!(
        snapshots.iter().map(|item| item.state).collect::<Vec<_>>(),
        vec![
            FleetState::Waiting,
            FleetState::Stopped,
            FleetState::Compacting,
            FleetState::Active,
            FleetState::Idle,
        ]
    );
    assert_eq!(snapshots[0].status_label, "Requires approval");
    assert_eq!(snapshots[0].context_text, "Zentty · Frontend");
}

#[test]
fn cross_window_order_is_urgency_then_recency_then_stable_identity() {
    let first = vec![lane(
        "lane-a",
        None,
        vec![pane(
            "pane-a",
            "Zulu",
            Some(status(
                "Codex",
                AgentPhase::NeedsInput,
                AgentInteractionKind::Question,
                None,
                10,
            )),
        )],
    )];
    let second = vec![lane(
        "lane-b",
        None,
        vec![
            pane("plain", "shell", None),
            pane(
                "pane-b",
                "Alpha",
                Some(status(
                    "Claude Code",
                    AgentPhase::NeedsInput,
                    AgentInteractionKind::Decision,
                    None,
                    20,
                )),
            ),
        ],
    )];
    let snapshots = build_fleet_snapshots(&[
        FleetWindowSource {
            window_id: "window-1",
            window_title: "One",
            worklanes: &first,
        },
        FleetWindowSource {
            window_id: "window-2",
            window_title: "Two",
            worklanes: &second,
        },
    ]);

    assert_eq!(snapshots.len(), 2);
    assert_eq!(snapshots[0].target.window_id, "window-2");
    assert_eq!(snapshots[0].target.pane_id, "pane-b");
    assert_eq!(snapshots[1].target.window_id, "window-1");
}

#[test]
fn summary_collapses_sections_and_exposes_source_accessibility_copy() {
    let snapshot = |state: FleetState| zentty_core::FleetPaneSnapshot {
        target: zentty_core::AttentionTarget::new("window", "lane", format!("{state:?}")),
        window_title: "Zentty".to_owned(),
        worklane_title: "Lane".to_owned(),
        agent_name: "Codex".to_owned(),
        primary_text: "Agent".to_owned(),
        context_text: "Zentty · Lane".to_owned(),
        status_label: state.status_label(AgentInteractionKind::None).to_owned(),
        state,
        updated_at_ms: 1,
        progress: None,
    };
    let summary = FleetSummary::from_snapshots(&[
        snapshot(FleetState::Waiting),
        snapshot(FleetState::Stopped),
        snapshot(FleetState::Compacting),
        snapshot(FleetState::Active),
        snapshot(FleetState::Idle),
    ]);

    assert_eq!(summary.total_count(), 5);
    assert_eq!(summary.waiting_section_count(), 2);
    assert_eq!(summary.running_section_count(), 2);
    assert_eq!(
        summary.header(),
        "1 waiting · 1 stopped · 1 compacting · 1 active · 1 idle"
    );
    assert_eq!(
        summary.accessibility_label(),
        "Agent status: waiting for input. 1 waiting, 1 stopped, 2 running, 1 idle"
    );
}

#[test]
fn completed_progress_is_hidden_and_generic_shell_uses_agent_identity() {
    let mut complete = status(
        "Codex",
        AgentPhase::Idle,
        AgentInteractionKind::None,
        None,
        1,
    );
    complete.progress = Some(AgentProgress { done: 3, total: 3 });
    let mut incomplete = status(
        "Claude Code",
        AgentPhase::Running,
        AgentInteractionKind::None,
        None,
        2,
    );
    incomplete.progress = Some(AgentProgress { done: 2, total: 5 });
    let worklanes = vec![lane(
        "lane",
        None,
        vec![
            pane("complete", "shell", Some(complete)),
            pane("incomplete", "", Some(incomplete)),
        ],
    )];
    let snapshots = build_fleet_snapshots(&[FleetWindowSource {
        window_id: "window",
        window_title: "Zentty",
        worklanes: &worklanes,
    }]);

    assert_eq!(snapshots[0].primary_text, "Claude Code");
    assert_eq!(
        snapshots[0].progress,
        Some(AgentProgress { done: 2, total: 5 })
    );
    assert_eq!(snapshots[1].primary_text, "Codex");
    assert_eq!(snapshots[1].progress, None);
}
