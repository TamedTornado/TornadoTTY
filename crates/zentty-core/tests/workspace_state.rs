use zentty_core::{
    AgentEvent, AgentInteractionKind, AgentPhase, AgentTarget, AuthenticatedAgentEvent,
    ClosePaneOutcome, CodexTranscriptQuestion, NewWorklanePlacement, PaneMoveSplitAxis,
    PaneMoveTarget, PaneRecipe, PaneResizeDirection, SessionRestoreEnvelope, WindowRecipe,
    WorklaneColor, WorkspaceState, WorkspaceStateImportError,
};

#[test]
fn pane_drag_column_gap_uses_reduced_space_and_preserves_exact_identity() {
    let mut state = WorkspaceState::new("lane-a", "pane-a");
    assert!(state.split_focused_pane_right("pane-b"));
    assert!(state.move_pane_to_target(
        "pane-a",
        PaneMoveTarget::ColumnGap {
            worklane_id: "lane-a".to_owned(),
            column_index: 2,
        },
        900.0,
    ));
    let lane = state.active_worklane();
    assert_eq!(lane.columns.len(), 2);
    assert_eq!(lane.columns[0].panes[0].id, "pane-b");
    assert_eq!(lane.columns[1].panes[0].id, "pane-a");
    assert_eq!(state.focused_pane_id(), Some("pane-a"));
}

#[test]
fn pane_drag_split_supports_source_sibling_vertical_and_horizontal_outcomes() {
    let mut vertical = WorkspaceState::new("lane-a", "pane-a");
    assert!(vertical.split_focused_pane_below("pane-b"));
    assert!(vertical.move_pane_to_target(
        "pane-a",
        PaneMoveTarget::Split {
            worklane_id: "lane-a".to_owned(),
            target_pane_id: "pane-b".to_owned(),
            axis: PaneMoveSplitAxis::Vertical,
            leading: false,
        },
        900.0,
    ));
    assert_eq!(
        vertical.active_worklane().columns[0]
            .panes
            .iter()
            .map(|pane| pane.id.as_str())
            .collect::<Vec<_>>(),
        ["pane-b", "pane-a"]
    );

    let mut horizontal = WorkspaceState::new("lane-a", "pane-a");
    assert!(horizontal.split_focused_pane_below("pane-b"));
    assert!(horizontal.move_pane_to_target(
        "pane-a",
        PaneMoveTarget::Split {
            worklane_id: "lane-a".to_owned(),
            target_pane_id: "pane-b".to_owned(),
            axis: PaneMoveSplitAxis::Horizontal,
            leading: false,
        },
        900.0,
    ));
    assert_eq!(horizontal.active_worklane().columns.len(), 2);
    assert_eq!(
        horizontal.active_worklane().columns[0].panes[0].id,
        "pane-b"
    );
    assert_eq!(
        horizontal.active_worklane().columns[1].panes[0].id,
        "pane-a"
    );
}

#[test]
fn pane_drag_stack_gap_crosses_worklanes_and_rejection_is_mutation_free() {
    let mut state = WorkspaceState::new("lane-a", "pane-a");
    assert!(state.split_focused_pane_right("pane-b"));
    assert!(state.create_worklane("lane-b", "pane-c"));
    let target_column = state.active_worklane().columns[0].id.clone();
    assert!(state.move_pane_to_target(
        "pane-a",
        PaneMoveTarget::StackGap {
            worklane_id: "lane-b".to_owned(),
            column_id: target_column,
            pane_index: 1,
        },
        900.0,
    ));
    assert_eq!(state.active_worklane_id(), "lane-b");
    assert_eq!(
        state.active_worklane().columns[0]
            .panes
            .iter()
            .map(|pane| pane.id.as_str())
            .collect::<Vec<_>>(),
        ["pane-c", "pane-a"]
    );

    let before = state.clone();
    assert!(!state.move_pane_to_target(
        "pane-a",
        PaneMoveTarget::Split {
            worklane_id: "lane-b".to_owned(),
            target_pane_id: "pane-a".to_owned(),
            axis: PaneMoveSplitAxis::Vertical,
            leading: true,
        },
        900.0,
    ));
    assert_eq!(state, before);
}

#[test]
fn cross_window_pane_drag_inserts_at_exact_stack_and_rolls_back_invalid_targets() {
    let mut source = WorkspaceState::new("source-lane", "foreign-pane");
    let transfer = source
        .extract_pane_for_cross_window_transfer("foreign-pane")
        .expect("foreign pane should extract");
    let mut destination = WorkspaceState::new("lane-a", "pane-a");
    assert!(destination.split_focused_pane_below("pane-b"));
    assert!(destination.insert_cross_window_pane_at_target(
        transfer,
        PaneMoveTarget::StackGap {
            worklane_id: "lane-a".to_owned(),
            column_id: "column-pane-a".to_owned(),
            pane_index: 1,
        },
        640.0,
    ));
    assert_eq!(
        destination.active_columns()[0]
            .panes
            .iter()
            .map(|pane| pane.id.as_str())
            .collect::<Vec<_>>(),
        ["pane-a", "foreign-pane", "pane-b"]
    );

    let before = destination.clone();
    let mut second_source = WorkspaceState::new("source-2", "foreign-2");
    let invalid = second_source
        .extract_pane_for_cross_window_transfer("foreign-2")
        .expect("second foreign pane should extract");
    assert!(!destination.insert_cross_window_pane_at_target(
        invalid,
        PaneMoveTarget::Split {
            worklane_id: "missing".to_owned(),
            target_pane_id: "pane-a".to_owned(),
            axis: PaneMoveSplitAxis::Horizontal,
            leading: true,
        },
        640.0,
    ));
    assert_eq!(destination, before);
}

#[test]
fn new_worklane_placement_preserves_source_top_after_current_and_end_semantics() {
    for (placement, expected) in [
        (NewWorklanePlacement::Top, vec!["new", "a", "b"]),
        (NewWorklanePlacement::AfterCurrent, vec!["a", "new", "b"]),
        (NewWorklanePlacement::End, vec!["a", "b", "new"]),
    ] {
        let mut state = WorkspaceState::new("a", "pane-a");
        assert!(state.create_worklane("b", "pane-b"));
        assert!(state.select_worklane("a"));
        assert!(state.create_worklane_with_placement("new", "pane-new", placement));
        assert_eq!(state.worklane_ids(), expected);
        assert_eq!(state.active_worklane_id(), "new");
    }
}

const V3_ENVELOPE: &[u8] = include_bytes!("fixtures/session-restore-v3.json");

#[test]
fn live_ssh_identity_is_presentational_customizable_and_never_persisted() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    assert!(state.set_pane_ssh_connection_label("pane-a", Some(" deploy@example.test ")));
    assert_eq!(
        state.sidebar_summaries()[0].pane_rows[0].primary_text,
        "deploy@example.test"
    );
    assert!(!state.set_pane_ssh_connection_label("pane-a", Some("deploy@example.test")));

    assert!(state.set_pane_custom_title("pane-a", Some("Production")));
    assert_eq!(
        state.sidebar_summaries()[0].pane_rows[0].primary_text,
        "Production"
    );

    let recipe = state.to_window_recipe(&WindowRecipe {
        id: "window-a".to_owned(),
        frame: None,
        worklanes: Vec::new(),
        active_worklane_id: None,
    });
    let restored = WorkspaceState::from_window_recipe(&recipe).expect("restore recipe");
    assert_eq!(restored.pane("pane-a").unwrap().ssh_connection_label, None);
    assert!(!state.set_pane_ssh_connection_label("missing", Some("host")));
    assert!(state.set_pane_ssh_connection_label("pane-a", None));
}

#[test]
fn real_terminal_titles_reconcile_agent_state_used_by_sidebar_summaries() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    state.apply_agent_event(
        AuthenticatedAgentEvent {
            target: AgentTarget::new("window-a", "worklane-a", "pane-a"),
            pane_token: "token-a".to_owned(),
            event: AgentEvent::parse(
                br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"codex-title"}}"#,
            )
            .unwrap(),
        },
        1,
    );
    assert!(state.reconcile_terminal_title(
        "pane-a",
        "[ ! ] Action Required | zentty | Tasks 1/3",
        2
    ));
    let summaries = state.sidebar_summaries();
    let status = summaries[0].pane_rows[0].agent_status.clone().unwrap();
    assert_eq!(status.phase, zentty_core::AgentPhase::NeedsInput);
    assert_eq!(status.progress.unwrap().done, 1);
    assert!(status.requires_attention());
}

#[test]
fn agent_pid_signals_are_session_scoped_and_an_unscoped_clear_clears_every_session() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    assert!(state.apply_agent_pid_signal(
        "pane-a",
        Some("parent"),
        None,
        Some("Codex"),
        Some(4101),
        1,
    ));
    assert!(state.apply_agent_pid_signal(
        "pane-a",
        Some("child"),
        Some("parent"),
        Some("Codex"),
        Some(4102),
        2,
    ));
    state.apply_canonical_agent_event(
        AgentTarget::new("window-a", "worklane-a", "pane-a"),
        &AgentEvent::parse(br#"{"version":1,"event":"agent.needs-input","agent":{"name":"Codex"},"session":{"id":"child","parentId":"parent"},"state":{"interaction":{"kind":"question"}}}"#).unwrap(),
        2,
    );
    let child = state.sidebar_summaries()[0].pane_rows[0]
        .agent_status
        .clone()
        .unwrap();
    assert_eq!(child.session_id, "child");
    assert_eq!(child.parent_session_id.as_deref(), Some("parent"));
    assert_eq!(child.tracked_pid, Some(4102));

    assert!(state.apply_agent_pid_signal("pane-a", Some("child"), None, None, None, 3));
    let cleared_child = state.sidebar_summaries()[0].pane_rows[0]
        .agent_status
        .clone()
        .unwrap();
    assert_eq!(cleared_child.session_id, "child");
    assert!(cleared_child.tracked_pid.is_none());

    assert!(state.apply_agent_pid_signal("pane-a", None, None, None, None, 4));
    let remaining = state.sidebar_summaries()[0].pane_rows[0]
        .agent_status
        .clone()
        .unwrap();
    assert!(remaining.tracked_pid.is_none());
    assert!(!state.apply_agent_pid_signal("pane-a", None, None, None, None, 5));
}

#[test]
fn signal_priority_rejects_weaker_conflicts_and_prefers_an_active_root_over_its_child() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    let target = AgentTarget::new("window-a", "worklane-a", "pane-a");
    state.apply_agent_signal_event(
        target.clone(),
        &AgentEvent::parse(br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"root"}}"#).unwrap(),
        zentty_core::AgentSignalOrigin::ExplicitHook,
        zentty_core::AgentSignalConfidence::Explicit,
        1,
    );
    state.apply_agent_signal_event(
        target.clone(),
        &AgentEvent::parse(br#"{"version":1,"event":"agent.idle","session":{"id":"root"}}"#)
            .unwrap(),
        zentty_core::AgentSignalOrigin::Inferred,
        zentty_core::AgentSignalConfidence::Weak,
        2,
    );
    assert_eq!(
        state.sidebar_summaries()[0].pane_rows[0]
            .agent_status
            .as_ref()
            .unwrap()
            .phase,
        AgentPhase::Running
    );

    state.apply_agent_signal_event(
        target,
        &AgentEvent::parse(br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"child","parentId":"root"}}"#).unwrap(),
        zentty_core::AgentSignalOrigin::ExplicitHook,
        zentty_core::AgentSignalConfidence::Explicit,
        3,
    );
    assert_eq!(
        state.sidebar_summaries()[0].pane_rows[0]
            .agent_status
            .as_ref()
            .unwrap()
            .session_id,
        "root"
    );
}

#[test]
fn title_inferred_codex_questions_offer_and_validate_transcript_enrichment() {
    let envelope = SessionRestoreEnvelope::from_json(V3_ENVELOPE).unwrap();
    let mut state = WorkspaceState::from_window_recipe(&envelope.workspace.windows[0]).unwrap();
    state.apply_agent_event(
        AuthenticatedAgentEvent {
            target: AgentTarget::new("window-main", "worklane-main", "pane-agent"),
            pane_token: "token-pane-agent".to_owned(),
            event: AgentEvent::parse(
                br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"codex-enrichment"},"transcriptPath":"/tmp/explicit.jsonl"}"#,
            )
            .unwrap(),
        },
        1,
    );
    assert!(state.reconcile_terminal_title(
        "pane-agent",
        "[ ! ] Action Required | plan-mode-prompt",
        2,
    ));

    let candidate = state
        .codex_transcript_enrichment_candidate("pane-agent", None)
        .expect("title-inferred attention should request transcript enrichment");
    assert_eq!(candidate.pane_id, "pane-agent");
    assert_eq!(candidate.session_id, "codex-enrichment");
    assert_eq!(candidate.working_directory.as_deref(), Some("/tmp/project"));
    assert_eq!(
        candidate.transcript_path.as_deref(),
        Some("/tmp/explicit.jsonl")
    );

    assert!(state.apply_codex_transcript_enrichment(
        &candidate,
        &CodexTranscriptQuestion {
            text: "Which implementation?\n[Minimal] [Broad]".to_owned(),
            interaction: AgentInteractionKind::Decision,
        },
        3,
    ));
    let summaries = state.sidebar_summaries();
    let status = summaries[0].pane_rows[0].agent_status.as_ref().unwrap();
    assert_eq!(
        status.text.as_deref(),
        Some("Which implementation?\n[Minimal] [Broad]")
    );
    assert_eq!(status.interaction, AgentInteractionKind::Decision);
}

#[test]
fn transcript_enrichment_rejects_stale_session_and_resolved_status_results() {
    let envelope = SessionRestoreEnvelope::from_json(V3_ENVELOPE).unwrap();
    let seeded_state = || {
        let mut state = WorkspaceState::from_window_recipe(&envelope.workspace.windows[0]).unwrap();
        state.apply_agent_event(
            AuthenticatedAgentEvent {
                target: AgentTarget::new("window-main", "worklane-main", "pane-agent"),
                pane_token: "token-pane-agent".to_owned(),
                event: AgentEvent::parse(
                    br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"codex-enrichment"}}"#,
                )
                .unwrap(),
            },
            1,
        );
        assert!(state.reconcile_terminal_title(
            "pane-agent",
            "[ ! ] Action Required | plan-mode-prompt",
            2,
        ));
        state
    };
    let question = CodexTranscriptQuestion {
        text: "Choose?\n[One] [Two]".to_owned(),
        interaction: AgentInteractionKind::Decision,
    };

    let mut wrong_session = seeded_state();
    let mut candidate = wrong_session
        .codex_transcript_enrichment_candidate("pane-agent", None)
        .unwrap();
    candidate.session_id = "different-session".to_owned();
    assert!(!wrong_session.apply_codex_transcript_enrichment(&candidate, &question, 3));

    let mut resolved = seeded_state();
    let candidate = resolved
        .codex_transcript_enrichment_candidate("pane-agent", None)
        .unwrap();
    assert!(resolved.record_terminal_input_submitted("pane-agent", 500));
    assert!(!resolved.apply_codex_transcript_enrichment(&candidate, &question, 501));
}

#[test]
fn persisted_workspace_projection_excludes_agent_secrets_prompts_and_transcripts() {
    let envelope = SessionRestoreEnvelope::from_json(V3_ENVELOPE).unwrap();
    let template = &envelope.workspace.windows[0];
    let mut state = WorkspaceState::from_window_recipe(template).unwrap();
    state.apply_agent_event(
        AuthenticatedAgentEvent {
            target: AgentTarget::new("window-main", "worklane-main", "pane-agent"),
            pane_token: "private-pane-capability".to_owned(),
            event: AgentEvent::parse(
                br#"{"version":1,"event":"agent.needs-input","agent":{"name":"Codex"},"session":{"id":"private-session"},"transcriptPath":"/private/transcript.jsonl","state":{"interaction":{"kind":"approval","text":"SECRET_PROMPT_BODY"}}}"#,
            )
            .unwrap(),
        },
        1,
    );
    let persisted = serde_json::to_string(&state.to_window_recipe(template)).unwrap();

    for forbidden in [
        "private-pane-capability",
        "private-session",
        "/private/transcript.jsonl",
        "SECRET_PROMPT_BODY",
    ] {
        assert!(
            !persisted.contains(forbidden),
            "persisted workspace escaped agent-private value: {forbidden}"
        );
    }
}

#[test]
fn explicit_transcript_context_does_not_require_a_guessed_working_directory() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    state.apply_agent_event(
        AuthenticatedAgentEvent {
            target: AgentTarget::new("window-a", "worklane-a", "pane-a"),
            pane_token: "token-pane-a".to_owned(),
            event: AgentEvent::parse(
                br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"codex-explicit"},"transcriptPath":"/tmp/explicit.jsonl","context":{"workingDirectory":"/tmp"}}"#,
            )
            .unwrap(),
        },
        1,
    );
    assert!(state.reconcile_terminal_title(
        "pane-a",
        "[ ! ] Action Required | plan-mode-prompt",
        2,
    ));
    let candidate = state
        .codex_transcript_enrichment_candidate("pane-a", None)
        .unwrap();
    assert_eq!(candidate.working_directory.as_deref(), Some("/tmp"));
    assert_eq!(
        candidate.transcript_path.as_deref(),
        Some("/tmp/explicit.jsonl")
    );
}

#[test]
fn physical_terminal_lifecycle_events_reconcile_the_sidebar_status_store() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    let needs_input = || {
        AuthenticatedAgentEvent {
        target: AgentTarget::new("window-a", "worklane-a", "pane-a"),
        pane_token: "token-a".to_owned(),
        event: AgentEvent::parse(
            br#"{"version":1,"event":"agent.needs-input","agent":{"name":"Codex"},"session":{"id":"codex-lifecycle"},"state":{"interaction":{"kind":"question","text":"Proceed?"}}}"#,
        )
        .unwrap(),
    }
    };
    state.apply_agent_event(needs_input(), 1_000);
    assert!(!state.record_terminal_input_submitted("pane-a", 1_349));
    assert!(state.record_terminal_input_submitted("pane-a", 1_350));
    assert_eq!(
        state.sidebar_summaries()[0].pane_rows[0]
            .agent_status
            .as_ref()
            .unwrap()
            .phase,
        zentty_core::AgentPhase::Running
    );

    assert!(state.record_terminal_interrupt("pane-a", 2_000));
    assert!(
        state.sidebar_summaries()[0].pane_rows[0]
            .agent_status
            .is_none()
    );

    state.apply_agent_event(needs_input(), 6_000);
    assert!(state.reconcile_terminal_title("pane-a", "bash", 6_001));
    assert!(
        state.sidebar_summaries()[0].pane_rows[0]
            .agent_status
            .is_none()
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn active_supported_agents_produce_restorable_per_pane_drafts() {
    let envelope = SessionRestoreEnvelope::from_json(V3_ENVELOPE).unwrap();
    let mut state = WorkspaceState::from_window_recipe(&envelope.workspace.windows[0]).unwrap();
    let event = |pane_id: &str, payload: &[u8]| AuthenticatedAgentEvent {
        target: AgentTarget::new("window-main", "worklane-main", pane_id),
        pane_token: format!("token-{pane_id}"),
        event: AgentEvent::parse(payload).unwrap(),
    };
    state.apply_agent_event(
        event(
            "pane-agent",
            br#"{"version":1,"event":"session.start","agent":{"name":"Codex","pid":4242},"session":{"id":"session-codex"},"context":{"workingDirectory":"/tmp","launch":{"arguments":["codex","--ambient-secret","DO_NOT_PERSIST"],"environment":{"API_TOKEN":"DO_NOT_PERSIST"}}}}"#,
        ),
        10,
    );
    state.apply_agent_event(
        event(
            "pane-shell",
            br#"{"version":1,"event":"session.start","agent":{"name":"Claude Code","pid":4343},"session":{"id":"123e4567-e89b-12d3-a456-426614174000"}}"#,
        ),
        11,
    );
    for (now, payload) in [
        br#"{"version":1,"event":"task.started","agent":{"name":"Codex"},"session":{"id":"session-codex"},"task":{"id":"worker-a"}}"#.as_slice(),
        br#"{"version":1,"event":"task.completed","agent":{"name":"Codex"},"session":{"id":"session-codex"},"task":{"id":"worker-a"}}"#.as_slice(),
        br#"{"version":1,"event":"task.started","agent":{"name":"Codex"},"session":{"id":"session-codex"},"task":{"id":"worker-b"}}"#.as_slice(),
    ]
    .into_iter()
    .enumerate()
    {
        state.apply_agent_event(event("pane-agent", payload), 12 + u64::try_from(now).unwrap());
    }

    let drafts = state.agent_restore_drafts();
    assert_eq!(drafts.len(), 2);
    assert_eq!(drafts[0].pane_id, "pane-agent");
    assert_eq!(
        drafts[0].resume_command().as_deref(),
        Some("codex resume session-codex")
    );
    assert_eq!(drafts[0].tracked_pid, 4242);
    assert_eq!(drafts[0].working_directory.as_deref(), Some("/tmp"));
    assert_eq!(
        drafts[0].agent_launch_snapshot.as_ref().unwrap().arguments,
        ["codex", "resume", "session-codex"]
    );
    assert!(
        drafts[0]
            .agent_launch_snapshot
            .as_ref()
            .unwrap()
            .environment
            .is_none()
    );
    assert!(
        !serde_json::to_string(&drafts)
            .unwrap()
            .contains("DO_NOT_PERSIST")
    );
    assert_eq!(
        drafts[0].task_progress,
        Some(zentty_core::AgentProgress { done: 1, total: 2 })
    );
    assert_eq!(drafts[0].tasks.get("worker-a"), Some(&true));
    assert_eq!(drafts[0].tasks.get("worker-b"), Some(&false));
    assert!(!drafts[0].task_progress_authoritative);
    assert_eq!(drafts[1].pane_id, "pane-shell");
    assert_eq!(
        drafts[1].resume_command().as_deref(),
        Some("claude --resume 123e4567-e89b-12d3-a456-426614174000")
    );

    let mut gemini_state =
        WorkspaceState::from_window_recipe(&envelope.workspace.windows[0]).unwrap();
    gemini_state.apply_agent_event(
        event(
            "pane-shell",
            br#"{"version":1,"event":"session.start","agent":{"name":"Gemini","pid":4444},"session":{"id":"gemini-session"}}"#,
        ),
        12,
    );
    let gemini = gemini_state.agent_restore_drafts();
    assert_eq!(gemini.len(), 1);
    assert_eq!(gemini[0].pane_id, "pane-shell");
    assert_eq!(
        gemini[0].resume_command().as_deref(),
        Some("gemini --resume")
    );
    assert_eq!(
        gemini[0].agent_launch_snapshot.as_ref().unwrap().arguments,
        ["gemini", "--resume"]
    );

    let mut copilot_state =
        WorkspaceState::from_window_recipe(&envelope.workspace.windows[0]).unwrap();
    copilot_state.apply_agent_event(
        event(
            "pane-shell",
            br#"{"version":1,"event":"session.start","agent":{"name":"Copilot","pid":4545},"session":{"id":"123e4567-e89b-12d3-a456-426614174001"}}"#,
        ),
        13,
    );
    let copilot = copilot_state.agent_restore_drafts();
    assert_eq!(copilot.len(), 1);
    assert_eq!(
        copilot[0].resume_command().as_deref(),
        Some("copilot --resume=123e4567-e89b-12d3-a456-426614174001")
    );

    let mut relaunched =
        WorkspaceState::from_window_recipe(&envelope.workspace.windows[0]).unwrap();
    assert!(relaunched.seed_restored_agent(&gemini[0], 13));
    let summaries = relaunched.sidebar_summaries();
    let restored_status = summaries[0].pane_rows[1].agent_status.as_ref().unwrap();
    assert_eq!(restored_status.agent_name, "Gemini");
    assert_eq!(restored_status.phase, zentty_core::AgentPhase::Starting);
    assert_eq!(restored_status.session_id, "gemini-session");

    let mut codex_relaunched =
        WorkspaceState::from_window_recipe(&envelope.workspace.windows[0]).unwrap();
    assert!(codex_relaunched.seed_restored_agent(&drafts[0], 14));
    let restored_codex = codex_relaunched.sidebar_summaries()[0].pane_rows[0]
        .agent_status
        .as_ref()
        .unwrap()
        .clone();
    assert_eq!(
        restored_codex.progress,
        Some(zentty_core::AgentProgress { done: 1, total: 2 })
    );
    codex_relaunched.apply_agent_event(
        event(
            "pane-agent",
            br#"{"version":1,"event":"task.completed","agent":{"name":"Codex"},"session":{"id":"session-codex"},"task":{"id":"worker-b"}}"#,
        ),
        15,
    );
    assert_eq!(
        codex_relaunched.sidebar_summaries()[0].pane_rows[0]
            .agent_status
            .as_ref()
            .unwrap()
            .progress,
        Some(zentty_core::AgentProgress { done: 2, total: 2 })
    );

    assert!(codex_relaunched.clear_failed_agent_restore("pane-agent"));
    assert!(codex_relaunched.agent_restore_drafts().is_empty());
    assert!(codex_relaunched.pane("pane-agent").is_some());
    let projected = codex_relaunched.to_window_recipe(&envelope.workspace.windows[0]);
    assert_eq!(
        projected.worklanes.len(),
        envelope.workspace.windows[0].worklanes.len()
    );
    assert!(
        projected
            .worklanes
            .iter()
            .flat_map(|worklane| &worklane.columns)
            .flat_map(|column| &column.panes)
            .any(|pane| pane.id == "pane-agent")
    );
}

#[test]
fn remaining_managed_agents_produce_their_source_resume_invocations() {
    let envelope = SessionRestoreEnvelope::from_json(V3_ENVELOPE).unwrap();
    for (agent, session_id, expected) in [
        (
            "GitHub Copilot CLI",
            "123e4567-e89b-12d3-a456-426614174000",
            "copilot --resume=123e4567-e89b-12d3-a456-426614174000",
        ),
        ("OpenCode", "ses_AbC123", "opencode --session ses_AbC123"),
        ("Pi", "project-session", "pi -c"),
        ("OMP", "project-session", "omp -c"),
        (
            "Small Harness",
            "project-session",
            "small-harness --continue",
        ),
    ] {
        let mut state = WorkspaceState::from_window_recipe(&envelope.workspace.windows[0]).unwrap();
        let payload = format!(
            r#"{{"version":1,"event":"session.start","agent":{{"name":"{agent}","pid":4242}},"session":{{"id":"{session_id}"}},"context":{{"workingDirectory":"/tmp/project"}}}}"#
        );
        state.apply_agent_event(
            AuthenticatedAgentEvent {
                target: AgentTarget::new("window-main", "worklane-main", "pane-agent"),
                pane_token: "token-pane-agent".to_owned(),
                event: AgentEvent::parse(payload.as_bytes()).unwrap(),
            },
            10,
        );
        let drafts = state.agent_restore_drafts();
        assert_eq!(drafts.len(), 1, "agent={agent}");
        assert_eq!(
            drafts[0].resume_command().as_deref(),
            Some(expected),
            "agent={agent}"
        );
    }
}

#[test]
fn restored_explicit_task_progress_remains_authoritative() {
    let envelope = SessionRestoreEnvelope::from_json(V3_ENVELOPE).unwrap();
    let mut state = WorkspaceState::from_window_recipe(&envelope.workspace.windows[0]).unwrap();
    let pane_id = "pane-agent";
    let session_id = "session-codex";
    let draft = zentty_core::PaneRestoreDraft {
        pane_id: pane_id.to_owned(),
        kind: zentty_core::RestoreDraftKind::AgentResume,
        tool_name: "Codex".to_owned(),
        session_id: session_id.to_owned(),
        working_directory: Some("/tmp/project".to_owned()),
        tracked_pid: 0,
        agent_launch_snapshot: Some(zentty_core::AgentLaunchSnapshot {
            arguments: vec![
                "codex".to_owned(),
                "resume".to_owned(),
                session_id.to_owned(),
            ],
            environment: None,
        }),
        task_progress: Some(zentty_core::AgentProgress { done: 3, total: 4 }),
        tasks: std::collections::BTreeMap::default(),
        task_progress_authoritative: true,
    };
    assert!(state.seed_restored_agent(&draft, 16));
    state.apply_agent_event(
        AuthenticatedAgentEvent {
            target: AgentTarget::new("window-main", "worklane-main", pane_id),
            pane_token: "token-pane-agent".to_owned(),
            event: AgentEvent::parse(
                br#"{"version":1,"event":"task.completed","agent":{"name":"Codex"},"session":{"id":"session-codex"},"task":{"id":"late-counter"}}"#,
            )
            .unwrap(),
        },
        17,
    );
    assert_eq!(
        state.sidebar_summaries()[0].pane_rows[0]
            .agent_status
            .as_ref()
            .unwrap()
            .progress,
        Some(zentty_core::AgentProgress { done: 3, total: 4 })
    );
}

#[test]
fn worklane_commands_preserve_order_selection_and_source_title_semantics() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");

    assert!(state.create_worklane("worklane-b", "pane-b"));
    assert_eq!(state.worklane_ids(), ["worklane-a", "worklane-b"]);
    assert_eq!(state.active_worklane_id(), "worklane-b");

    assert!(state.set_worklane_title("worklane-a", Some("  API work  ")));
    assert_eq!(state.worklanes()[0].title.as_deref(), Some("API work"));
    assert!(state.set_worklane_title("worklane-a", Some("   ")));
    assert_eq!(state.worklanes()[0].title, None);
    assert!(!state.set_worklane_title("missing", Some("ignored")));

    assert!(state.set_worklane_color("worklane-a", Some(WorklaneColor::Blue)));
    assert_eq!(state.worklanes()[0].color, Some(WorklaneColor::Blue));
    assert!(state.move_worklane("worklane-a", 1));
    assert_eq!(state.worklane_ids(), ["worklane-b", "worklane-a"]);
    assert_eq!(state.active_worklane_id(), "worklane-b");

    assert!(state.select_worklane("worklane-a"));
    assert!(!state.select_worklane("missing"));
    assert!(state.close_active_worklane());
    assert_eq!(state.worklane_ids(), ["worklane-b"]);
    assert_eq!(state.active_worklane_id(), "worklane-b");
    assert!(!state.close_active_worklane());
}

#[test]
fn closing_a_named_worklane_preserves_unrelated_active_selection() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    assert!(state.create_worklane("worklane-b", "pane-b"));
    assert!(state.create_worklane("worklane-c", "pane-c"));
    assert!(state.select_worklane("worklane-b"));

    assert!(state.close_worklane("worklane-a"));
    assert_eq!(state.worklane_ids(), ["worklane-b", "worklane-c"]);
    assert_eq!(state.active_worklane_id(), "worklane-b");

    assert!(state.close_worklane("worklane-b"));
    assert_eq!(state.worklane_ids(), ["worklane-c"]);
    assert_eq!(state.active_worklane_id(), "worklane-c");
    assert!(!state.close_worklane("worklane-c"));
    assert!(!state.close_worklane("missing"));
}

#[test]
fn pane_commands_keep_real_terminal_identity_attached_to_stable_panes() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");

    assert!(state.split_focused_pane_right("pane-b"));
    assert_eq!(state.active_pane_ids(), ["pane-a", "pane-b"]);
    assert_eq!(state.focused_pane_id(), Some("pane-b"));

    assert!(state.select_pane("pane-a"));
    assert_eq!(state.focused_pane_id(), Some("pane-a"));
    assert_eq!(state.close_focused_pane(), ClosePaneOutcome::Closed);
    assert_eq!(state.active_pane_ids(), ["pane-b"]);
    assert_eq!(state.focused_pane_id(), Some("pane-b"));
    assert_eq!(state.close_focused_pane(), ClosePaneOutcome::CloseWindow);
    assert_eq!(state.active_pane_ids(), ["pane-b"]);
}

#[test]
fn closing_an_unfocused_pane_does_not_steal_focus() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    assert!(state.split_focused_pane_right("pane-b"));
    assert_eq!(state.focused_pane_id(), Some("pane-b"));

    assert_eq!(state.close_pane("pane-a"), ClosePaneOutcome::Closed);
    assert_eq!(state.active_pane_ids(), ["pane-b"]);
    assert_eq!(state.focused_pane_id(), Some("pane-b"));

    assert!(state.split_focused_pane_below("pane-c"));
    assert_eq!(state.close_pane("pane-b"), ClosePaneOutcome::Closed);
    assert_eq!(state.active_pane_ids(), ["pane-c"]);
    assert_eq!(state.focused_pane_id(), Some("pane-c"));
}

#[test]
fn closed_pane_restore_is_lifo_expiring_and_preserves_source_launch_context() {
    const SOURCE_STACK: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../Zentty/Restore/ClosedPaneStack.swift"
    ));
    const SOURCE_RESTORE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../Zentty/AppState/WorklaneStore+Restore.swift"
    ));
    let envelope = SessionRestoreEnvelope::from_json(V3_ENVELOPE).unwrap();
    let mut state = WorkspaceState::from_window_recipe(&envelope.workspace.windows[0]).unwrap();
    assert!(state.configure_pane_launch(
        "pane-agent",
        Some("/tmp".to_owned()),
        Some("cargo test".to_owned())
    ));

    assert_eq!(
        state.close_pane_at("pane-agent", 1_000),
        ClosePaneOutcome::Closed
    );
    let restored = state
        .restore_closed_pane_at("pane-restored", 1_001)
        .expect("recent local pane should restore");
    assert_eq!(restored.pane_id, "pane-restored");
    assert_eq!(restored.worklane_id, "worklane-main");
    assert_eq!(restored.working_directory.as_deref(), Some("/tmp"));
    assert_eq!(restored.prefill_text.as_deref(), Some("cargo test"));
    assert_eq!(state.active_pane_ids(), ["pane-restored", "pane-shell"]);
    assert_eq!(state.focused_pane_id(), Some("pane-restored"));
    assert_eq!(state.active_columns()[0].pane_heights, [420.0, 1.0]);
    assert_eq!(
        state.pane("pane-restored").unwrap().custom_title.as_deref(),
        Some("Agent")
    );

    assert_eq!(
        state.close_pane_at("pane-restored", 2_000),
        ClosePaneOutcome::Closed
    );
    assert!(
        state
            .restore_closed_pane_at("pane-expired", 2_000 + 60 * 60 + 1)
            .is_none()
    );
    assert!(SOURCE_STACK.contains("defaultCapacity: Int = 10"));
    assert!(SOURCE_STACK.contains("defaultExpiry: TimeInterval = 60 * 60"));
    assert!(SOURCE_RESTORE.contains("prefillText: composition.prefillText"));
    assert!(SOURCE_RESTORE.contains("let newPaneID = runtimeIdentity.makePaneID()"));

    let mut natural_exit = WorkspaceState::new("lane", "pane-a");
    assert!(natural_exit.split_focused_pane_right("pane-b"));
    assert_eq!(
        natural_exit.close_pane_after_child_exit("pane-b"),
        ClosePaneOutcome::Closed
    );
    assert!(natural_exit.restore_closed_pane_at("pane-c", 1).is_none());
}

#[test]
fn product_clock_close_and_restore_entry_points_share_the_deterministic_contract() {
    let mut state = WorkspaceState::new("lane", "pane-a");
    assert!(state.split_focused_pane_right("pane-b"));
    assert_eq!(state.close_pane("pane-b"), ClosePaneOutcome::Closed);
    let restored = state
        .restore_closed_pane("pane-restored")
        .expect("a pane closed on the product clock must be immediately restorable");
    assert_eq!(restored.pane_id, "pane-restored");
    assert_eq!(state.active_pane_ids(), ["pane-a", "pane-restored"]);
}

#[test]
fn restore_returns_an_inactive_pane_to_its_original_worklane() {
    let mut state = WorkspaceState::new("lane-a", "pane-a");
    assert!(state.create_worklane("lane-b", "pane-b"));
    assert!(state.split_focused_pane_right("pane-b-keep"));
    assert!(state.select_worklane("lane-a"));

    assert_eq!(
        state.close_pane_at("pane-b", 1_000),
        ClosePaneOutcome::Closed
    );
    let restored = state
        .restore_closed_pane_at("pane-b-restored", 1_001)
        .expect("inactive worklane pane should restore");
    assert_eq!(restored.worklane_id, "lane-b");
    assert_eq!(state.active_worklane_id(), "lane-b");
    assert_eq!(state.focused_pane_id(), Some("pane-b-restored"));
    assert_eq!(
        state.worklane_id_for_pane("pane-b-restored"),
        Some("lane-b")
    );
}

#[test]
fn closed_pane_history_keeps_exact_capacity_across_capture_and_rollback() {
    fn filled_history() -> WorkspaceState {
        let mut state = WorkspaceState::new("lane", "pane-base");
        for index in 0..10 {
            let pane_id = format!("pane-{index}");
            assert!(state.split_focused_pane_right(&pane_id));
            assert_eq!(
                state.close_pane_at(&pane_id, 1_000 + index),
                ClosePaneOutcome::Closed
            );
        }
        state
    }

    fn drain_history(state: &mut WorkspaceState) -> usize {
        let mut count = 0;
        while state
            .restore_closed_pane_at(format!("restored-{count}"), 2_003)
            .is_some()
        {
            count += 1;
        }
        count
    }

    let mut capture_at_capacity = filled_history();
    assert_eq!(drain_history(&mut capture_at_capacity), 10);

    let mut rollback_at_capacity = filled_history();
    let pending = rollback_at_capacity
        .restore_closed_pane_at("pane-pending", 2_000)
        .expect("latest pane should restore");
    assert_eq!(
        rollback_at_capacity.rollback_restored_pane_at(pending, 2_001),
        ClosePaneOutcome::Closed
    );
    assert_eq!(drain_history(&mut rollback_at_capacity), 10);

    let mut rollback_over_capacity = filled_history();
    let pending = rollback_over_capacity
        .restore_closed_pane_at("pane-pending", 2_000)
        .expect("latest pane should restore");
    assert!(rollback_over_capacity.split_focused_pane_right("pane-after-restore"));
    assert_eq!(
        rollback_over_capacity.close_pane_at("pane-after-restore", 2_001),
        ClosePaneOutcome::Closed
    );
    assert_eq!(
        rollback_over_capacity.rollback_restored_pane_at(pending, 2_002),
        ClosePaneOutcome::Closed
    );
    assert_eq!(drain_history(&mut rollback_over_capacity), 10);
}

#[test]
fn closed_pane_restore_prefers_agent_resume_and_preserves_scrollback() {
    let envelope = SessionRestoreEnvelope::from_json(V3_ENVELOPE).unwrap();
    let mut state = WorkspaceState::from_window_recipe(&envelope.workspace.windows[0]).unwrap();
    state.apply_agent_event(
        AuthenticatedAgentEvent {
            target: AgentTarget::new("window-main", "worklane-main", "pane-agent"),
            pane_token: "token-pane-agent".to_owned(),
            event: AgentEvent::parse(
                br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"agent-session-safe"},"context":{"workingDirectory":"/tmp"}}"#,
            )
            .unwrap(),
        },
        999,
    );

    assert_eq!(
        state.close_pane_with_scrollback_at(
            "pane-agent",
            1_000,
            Some("first line\nsecond line".to_owned())
        ),
        ClosePaneOutcome::Closed
    );
    let restored = state
        .restore_closed_pane_at_in_home("pane-restored", 1_001, "/tmp")
        .expect("agent pane should restore");
    assert_eq!(
        restored.prefill_text.as_deref(),
        Some("codex resume agent-session-safe")
    );
    assert_eq!(
        restored.scrollback_text.as_deref(),
        Some("first line\nsecond line")
    );
    assert!(!restored.original_directory_missing);
}

#[test]
fn closed_pane_restore_walks_a_missing_cwd_to_an_existing_ancestor() {
    let root = std::env::temp_dir().join(format!(
        "zentty-closed-pane-cwd-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let missing = root.join("deleted").join("project");
    std::fs::create_dir_all(&root).unwrap();

    let mut state = WorkspaceState::new("lane", "pane-a");
    assert!(state.split_focused_pane_right("pane-b"));
    assert!(state.configure_pane_launch(
        "pane-b",
        Some(missing.to_string_lossy().into_owned()),
        None
    ));
    assert_eq!(
        state.close_pane_at("pane-b", 1_000),
        ClosePaneOutcome::Closed
    );
    let restored = state
        .restore_closed_pane_at_in_home("pane-restored", 1_001, "/")
        .expect("missing-CWD pane should restore");
    assert_eq!(
        restored.working_directory.as_deref(),
        Some(root.to_string_lossy().as_ref())
    );
    assert!(restored.original_directory_missing);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_runtime_restore_can_be_rolled_back_and_retried_without_losing_history() {
    let mut state = WorkspaceState::new("lane", "pane-a");
    assert!(state.split_focused_pane_right("pane-b"));
    assert_eq!(
        state.close_pane_at("pane-b", 1_000),
        ClosePaneOutcome::Closed
    );

    let first_attempt = state
        .restore_closed_pane_at("pane-runtime-failed", 1_001)
        .expect("closed pane should be available for the first attempt");
    assert_eq!(first_attempt.pane_id, "pane-runtime-failed");
    assert_eq!(
        state.rollback_restored_pane_at(first_attempt, 1_001),
        ClosePaneOutcome::Closed
    );
    assert_eq!(state.active_pane_ids(), ["pane-a"]);

    let retry = state
        .restore_closed_pane_at("pane-retry", 1_002)
        .expect("runtime failure must not consume closed-pane history");
    assert_eq!(retry.pane_id, "pane-retry");
    assert_eq!(state.active_pane_ids(), ["pane-a", "pane-retry"]);
}

#[test]
fn vertical_pane_commands_preserve_column_identity_and_geometry() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");

    assert!(state.split_focused_pane_below("pane-b"));
    assert_eq!(state.active_columns().len(), 1);
    assert_eq!(state.active_pane_ids(), ["pane-a", "pane-b"]);
    assert_eq!(state.active_columns()[0].pane_heights, [0.5, 0.5]);
    assert!(state.move_focused_pane_up());
    assert_eq!(state.active_pane_ids(), ["pane-b", "pane-a"]);
    assert!(!state.move_focused_pane_up());
    assert!(state.move_focused_pane_down());
    assert_eq!(state.active_pane_ids(), ["pane-a", "pane-b"]);
}

#[test]
fn team_column_equalization_targets_only_the_column_containing_the_teammate() {
    let mut state = WorkspaceState::new("lane-1", "leader");
    assert!(state.split_focused_pane_right("teammate-1"));
    assert!(state.split_focused_pane_below("teammate-2"));
    assert!(state.split_focused_pane_below("teammate-3"));
    assert_eq!(
        state.active_worklane().columns[1].pane_heights,
        vec![0.5, 0.25, 0.25]
    );

    assert!(state.equalize_pane_heights_in_column("teammate-1"));
    assert_eq!(state.active_worklane().columns[0].pane_heights, vec![1.0]);
    assert_eq!(
        state.active_worklane().columns[1].pane_heights,
        vec![1.0 / 3.0; 3]
    );
    assert!(!state.equalize_pane_heights_in_column("teammate-1"));
}

#[test]
fn team_width_restoration_targets_the_leader_without_changing_focus() {
    let mut state = WorkspaceState::new("lane-1", "leader");
    assert!(state.split_focused_pane_right("teammate"));
    assert!(state.arrange_golden_width(true, 1000.0));
    assert!(state.select_worklane_and_pane("lane-1", "teammate"));

    assert!(state.restore_column_width("leader", 777.0));
    assert!((state.active_columns()[0].width - 777.0).abs() < f64::EPSILON);
    assert_eq!(state.focused_pane_id(), Some("teammate"));
    assert!(!state.restore_column_width("leader", 777.0));
    assert!(!state.restore_column_width("missing", 500.0));
    assert!(!state.restore_column_width("leader", f64::NAN));
}

#[test]
fn duplicate_ids_and_invalid_reorders_are_rejected_without_mutation() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    assert!(!state.create_worklane("worklane-a", "pane-b"));
    assert!(!state.create_worklane("worklane-b", "pane-a"));
    assert!(!state.split_focused_pane_right("pane-a"));
    assert!(!state.move_worklane("worklane-a", 1));
    assert_eq!(state.worklane_ids(), ["worklane-a"]);
    assert_eq!(state.active_pane_ids(), ["pane-a"]);
}

#[test]
fn closing_a_specific_inactive_pane_removes_its_lane_without_changing_selection() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    assert!(state.create_worklane("worklane-b", "pane-b"));
    assert!(state.select_worklane("worklane-a"));

    assert_eq!(state.close_pane("pane-b"), ClosePaneOutcome::Closed);
    assert_eq!(state.worklane_ids(), ["worklane-a"]);
    assert_eq!(state.active_worklane_id(), "worklane-a");
    assert_eq!(state.close_pane("missing"), ClosePaneOutcome::NotFound);
}

#[test]
fn focused_pane_moves_within_the_active_worklane_without_losing_focus() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    assert!(state.split_focused_pane_right("pane-b"));
    assert!(state.split_focused_pane_right("pane-c"));
    assert_eq!(state.active_pane_ids(), ["pane-a", "pane-b", "pane-c"]);

    assert!(state.move_focused_pane_left());
    assert_eq!(state.active_pane_ids(), ["pane-a", "pane-c", "pane-b"]);
    assert_eq!(state.focused_pane_id(), Some("pane-c"));
    assert!(state.move_focused_pane_left());
    assert_eq!(state.active_pane_ids(), ["pane-c", "pane-a", "pane-b"]);
    assert!(state.move_focused_pane_left());
    assert_eq!(state.active_columns().len(), 3);
    assert!(!state.move_focused_pane_left());
    assert!(state.move_focused_pane_right());
    assert_eq!(state.active_pane_ids(), ["pane-c", "pane-a", "pane-b"]);
    assert_eq!(state.active_columns().len(), 2);
}

#[test]
fn focused_pane_transfers_to_existing_worklane_as_a_focused_column() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    assert!(state.split_focused_pane_below("pane-b"));
    assert!(state.set_pane_title("pane-b", "agent review"));
    assert!(state.create_worklane("worklane-b", "pane-c"));
    assert!(state.select_worklane("worklane-a"));

    assert!(state.transfer_focused_pane_to_worklane("worklane-b"));
    assert_eq!(state.worklane_ids(), ["worklane-a", "worklane-b"]);
    assert_eq!(state.active_worklane_id(), "worklane-b");
    assert_eq!(state.active_pane_ids(), ["pane-c", "pane-b"]);
    assert_eq!(state.focused_pane_id(), Some("pane-b"));
    assert_eq!(state.active_columns().len(), 2);
    assert_eq!(state.active_columns()[1].id, "column-pane-b");
    assert_eq!(
        state.active_columns()[1].panes[0].live_title,
        "agent review"
    );
    assert_eq!(state.worklanes()[0].columns[0].pane_heights, [1.0]);
}

#[test]
fn cross_worklane_transfer_removes_an_empty_source_and_rejects_invalid_targets() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    assert!(state.create_worklane("worklane-b", "pane-b"));
    assert!(state.select_worklane("worklane-a"));

    let unchanged = state.clone();
    assert!(!state.transfer_focused_pane_to_worklane("worklane-a"));
    assert!(!state.transfer_focused_pane_to_worklane("missing"));
    assert_eq!(state, unchanged);

    assert!(state.transfer_focused_pane_to_worklane("worklane-b"));
    assert_eq!(state.worklane_ids(), ["worklane-b"]);
    assert_eq!(state.active_worklane_id(), "worklane-b");
    assert_eq!(state.active_pane_ids(), ["pane-b", "pane-a"]);
    assert_eq!(state.focused_pane_id(), Some("pane-a"));
}

#[test]
fn sidebar_summaries_are_compound_worklane_and_pane_presentations() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    assert!(state.split_focused_pane_right("pane-b"));
    assert!(state.configure_pane_launch("pane-a", Some("/repo/nimbu".to_owned()), None,));
    assert!(state.set_pane_title("pane-a", "project shell"));
    assert!(state.set_pane_title("pane-b", "cargo test"));
    assert!(state.set_worklane_title("worklane-a", Some("  Nimbu API  ")));
    assert!(state.set_worklane_color("worklane-a", Some(WorklaneColor::Blue)));

    let summaries = state.sidebar_summaries();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].top_label.as_deref(), Some("Nimbu API"));
    assert_eq!(summaries[0].primary_text, "cargo test");
    assert_eq!(summaries[0].color, Some(WorklaneColor::Blue));
    assert!(summaries[0].is_active);
    assert_eq!(summaries[0].pane_rows.len(), 2);
    assert_eq!(summaries[0].pane_rows[0].primary_text, "project shell");
    assert_eq!(
        summaries[0].pane_rows[0].working_directory.as_deref(),
        Some("/repo/nimbu")
    );
    assert_eq!(summaries[0].pane_rows[1].working_directory, None);
    assert!(!summaries[0].pane_rows[0].is_focused);
    assert_eq!(summaries[0].pane_rows[1].primary_text, "cargo test");
    assert!(summaries[0].pane_rows[1].is_focused);
}

#[test]
fn authenticated_agent_directory_temporarily_owns_all_pane_context_projection() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    assert!(state.configure_pane_launch("pane-a", Some("/tmp".to_owned()), None));
    let target = AgentTarget::new("window-a", "worklane-a", "pane-a");
    state.apply_agent_event(
        AuthenticatedAgentEvent {
            target: target.clone(),
            pane_token: "authenticated".to_owned(),
            event: AgentEvent::parse(
                br#"{"version":1,"event":"session.start","agent":{"name":"Codex"},"session":{"id":"bro"},"context":{"workingDirectory":"/usr"}}"#,
            )
            .unwrap(),
        },
        1_000,
    );

    assert_eq!(
        state.effective_working_directory_for_pane("pane-a"),
        Some("/usr")
    );
    assert_eq!(
        state.sidebar_summaries()[0].pane_rows[0]
            .working_directory
            .as_deref(),
        Some("/usr")
    );
    assert_eq!(
        state.pane("pane-a").unwrap().working_directory.as_deref(),
        Some("/tmp"),
        "agent context must not overwrite the parent shell directory"
    );

    state.apply_agent_event(
        AuthenticatedAgentEvent {
            target,
            pane_token: "authenticated".to_owned(),
            event: AgentEvent::parse(
                br#"{"version":1,"event":"session.end","agent":{"name":"Codex"},"session":{"id":"bro"}}"#,
            )
            .unwrap(),
        },
        2_000,
    );
    assert_eq!(
        state.effective_working_directory_for_pane("pane-a"),
        Some("/tmp")
    );
    assert_eq!(
        state.sidebar_summaries()[0].pane_rows[0]
            .working_directory
            .as_deref(),
        Some("/tmp")
    );
}

#[test]
fn source_window_round_trip_preserves_metadata_while_applying_product_state() {
    let envelope = SessionRestoreEnvelope::from_json(V3_ENVELOPE).unwrap();
    let template = &envelope.workspace.windows[0];
    let mut state = WorkspaceState::from_window_recipe(template).unwrap();

    assert_eq!(state.worklane_ids(), ["worklane-main"]);
    assert_eq!(state.active_pane_ids(), ["pane-agent", "pane-shell"]);
    assert_eq!(state.focused_pane_id(), Some("pane-agent"));
    assert!(state.set_worklane_title("worklane-main", Some("  Linux port  ")));
    assert!(state.set_worklane_color("worklane-main", Some(WorklaneColor::Purple)));
    assert!(state.select_pane("pane-shell"));

    let projected = state.to_window_recipe(template);
    assert_eq!(projected.worklanes[0].title.as_deref(), Some("Linux port"));
    assert_eq!(projected.worklanes[0].color.as_deref(), Some("purple"));
    assert_eq!(
        projected.worklanes[0].columns[0].focused_pane_id.as_deref(),
        Some("pane-shell")
    );
    assert_eq!(
        projected.worklanes[0].columns[0].pane_heights,
        [420.0, 280.0]
    );
    assert_eq!(
        projected.worklanes[0].columns[0].panes[0]
            .last_run_command
            .as_deref(),
        Some("cargo test")
    );
    assert_eq!(
        projected.worklanes[0].bookmark_origin_id.as_deref(),
        Some("bookmark-main")
    );
}

#[test]
fn explicit_bookmark_unlink_overrides_preserved_recipe_metadata() {
    let original = WorkspaceState::new("linked", "pane");
    let mut recipe = original.to_window_recipe(&WindowRecipe {
        id: "window-bookmark".to_owned(),
        frame: None,
        worklanes: Vec::new(),
        active_worklane_id: None,
    });
    recipe.worklanes[0].bookmark_origin_id = Some("template-1".to_owned());
    let mut state = WorkspaceState::from_window_recipe(&recipe).unwrap();
    assert!(state.set_bookmark_origin("linked", None));
    recipe = state.to_window_recipe(&recipe);
    assert_eq!(recipe.worklanes[0].bookmark_origin_id, None);
}

#[test]
fn pane_custom_identity_survives_runtime_titles_and_clears_to_live_fallback() {
    let envelope = SessionRestoreEnvelope::from_json(V3_ENVELOPE).unwrap();
    let template = &envelope.workspace.windows[0];
    let mut state = WorkspaceState::from_window_recipe(template).unwrap();

    assert_eq!(
        state.sidebar_summaries()[0].pane_rows[0].primary_text,
        "Agent"
    );
    assert!(state.set_pane_title("pane-agent", "  compiling  "));
    assert_eq!(
        state.sidebar_summaries()[0].pane_rows[0].primary_text,
        "Agent"
    );
    assert!(state.set_pane_custom_title("pane-agent", Some("  Review Agent  ")));
    assert_eq!(
        state.sidebar_summaries()[0].pane_rows[0].primary_text,
        "Review Agent"
    );
    assert!(state.set_pane_custom_title("pane-agent", Some("   ")));
    assert_eq!(
        state.sidebar_summaries()[0].pane_rows[0].primary_text,
        "compiling"
    );

    let projected = state.to_window_recipe(template);
    let pane = &projected.worklanes[0].columns[0].panes[0];
    assert_eq!(pane.custom_title, None);
    assert_eq!(pane.last_activity_title.as_deref(), Some("compiling"));
    assert_eq!(pane.title_seed.as_deref(), Some("Codex"));
}

#[test]
fn pane_launch_context_updates_without_changing_identity_or_focus() {
    let mut state = WorkspaceState::new("lane-1", "pane-1");
    assert!(state.split_focused_pane_right("pane-2"));
    assert!(state.select_pane("pane-1"));

    assert!(state.configure_pane_launch(
        "pane-2",
        Some("/repo/team".to_owned()),
        Some("claude --agent-id worker".to_owned()),
    ));
    assert_eq!(state.focused_pane_id(), Some("pane-1"));
    let pane = state.pane("pane-2").unwrap();
    assert_eq!(pane.working_directory.as_deref(), Some("/repo/team"));
    assert_eq!(
        pane.last_run_command.as_deref(),
        Some("claude --agent-id worker")
    );
    assert!(!state.configure_pane_launch("missing", None, None));
}

#[test]
fn shell_history_requires_physical_submission_but_explicit_launches_do_not() {
    let mut state = WorkspaceState::new("lane-1", "pane-1");

    assert!(!state.record_submitted_shell_command(
        "pane-1",
        "source /usr/share/zentty/shell-integration/bash/zentty.bash"
    ));
    assert_eq!(state.pane("pane-1").unwrap().last_run_command, None);

    assert!(!state.record_terminal_input_submitted("pane-1", 100));
    assert!(state.record_submitted_shell_command("pane-1", "cargo test"));
    assert_eq!(
        state.pane("pane-1").unwrap().last_run_command.as_deref(),
        Some("cargo test")
    );
    assert!(!state.record_submitted_shell_command(
        "pane-1",
        "_zentty_prompt_bootstrap"
    ));
    assert_eq!(
        state.pane("pane-1").unwrap().last_run_command.as_deref(),
        Some("cargo test")
    );

    assert!(state.split_focused_pane_right("pane-2"));
    assert!(state.configure_pane_launch(
        "pane-2",
        Some("/repo/bro".to_owned()),
        Some("codex resume bro".to_owned()),
    ));
    assert_eq!(
        state.pane("pane-2").unwrap().last_run_command.as_deref(),
        Some("codex resume bro")
    );
}

#[test]
fn pane_navigation_history_crosses_worklanes_and_preserves_browser_semantics() {
    let mut state = WorkspaceState::new("lane-1", "pane-1");
    assert!(state.split_focused_pane_right("pane-2"));
    assert!(state.create_worklane("lane-2", "pane-3"));

    assert!(state.can_navigate_back());
    assert!(!state.can_navigate_forward());
    assert!(state.navigate_back());
    assert_eq!(state.active_worklane_id(), "lane-1");
    assert_eq!(state.focused_pane_id(), Some("pane-2"));
    assert!(state.navigate_back());
    assert_eq!(state.focused_pane_id(), Some("pane-1"));
    assert!(state.can_navigate_forward());
    assert!(state.navigate_forward());
    assert_eq!(state.focused_pane_id(), Some("pane-2"));

    assert!(state.select_worklane_and_pane("lane-2", "pane-3"));
    assert!(!state.can_navigate_forward());
    assert!(state.navigate_back());
    assert_eq!(state.active_worklane_id(), "lane-1");
    assert_eq!(state.focused_pane_id(), Some("pane-2"));
}

#[test]
fn adjacent_pane_traversal_follows_sidebar_order_and_wraps_across_worklanes() {
    const SOURCE: &str =
        include_str!("../../../Zentty/UI/WorklanePeek/WorklanePeekSelectionState.swift");

    let mut state = WorkspaceState::new("lane-1", "pane-1");
    assert!(state.split_focused_pane_below("pane-2"));
    assert!(state.create_worklane("lane-2", "pane-3"));

    assert!(state.select_adjacent_pane(true));
    assert_eq!(state.active_worklane_id(), "lane-1");
    assert_eq!(state.focused_pane_id(), Some("pane-1"));
    assert!(state.select_adjacent_pane(true));
    assert_eq!(state.focused_pane_id(), Some("pane-2"));
    assert!(state.select_adjacent_pane(true));
    assert_eq!(state.active_worklane_id(), "lane-2");
    assert_eq!(state.focused_pane_id(), Some("pane-3"));
    assert!(state.select_adjacent_pane(false));
    assert_eq!(state.active_worklane_id(), "lane-1");
    assert_eq!(state.focused_pane_id(), Some("pane-2"));

    assert!(SOURCE.contains("paneStripState.panes.map"));
    assert!(SOURCE.contains("(currentIndex + direction.offset + count) % count"));
}

#[test]
fn adjacent_worklane_traversal_wraps_without_changing_each_lanes_focused_pane() {
    let mut state = WorkspaceState::new("worklane-a", "pane-a");
    assert!(state.create_worklane("worklane-b", "pane-b"));
    assert!(state.select_worklane("worklane-a"));
    let first_pane = state.focused_pane_id().map(str::to_owned);

    assert!(state.select_adjacent_worklane(true));
    let second_id = state.active_worklane_id().to_owned();
    let second_pane = state.focused_pane_id().map(str::to_owned);
    assert!(state.select_adjacent_worklane(true));
    assert_eq!(state.focused_pane_id(), first_pane.as_deref());
    assert!(state.select_adjacent_worklane(false));
    assert_eq!(state.active_worklane_id(), second_id);
    assert_eq!(state.focused_pane_id(), second_pane.as_deref());
}

#[test]
fn drag_insertion_reorders_stable_worklane_ids_without_changing_selection() {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../Zentty/UI/Sidebar/SidebarWorklaneReorderModel.swift"
    ));
    let mut state = WorkspaceState::new("a", "pane-a");
    assert!(state.create_worklane("b", "pane-b"));
    assert!(state.create_worklane("c", "pane-c"));
    assert_eq!(state.active_worklane_id(), "c");

    assert!(state.reorder_worklane("a", 2));
    assert_eq!(
        state
            .worklanes()
            .iter()
            .map(|worklane| worklane.id.as_str())
            .collect::<Vec<_>>(),
        ["b", "c", "a"]
    );
    assert_eq!(state.active_worklane_id(), "c");
    assert!(!state.reorder_worklane("missing", 0));
    assert!(!state.reorder_worklane("a", 3));
    assert!(SOURCE.contains("currentOrder.filter { $0 != draggedID }"));
    assert!(SOURCE.contains("order.insert(draggedID, at: insertionIndex)"));
}

#[test]
fn right_insertion_commands_preserve_their_distinct_width_contracts() {
    let mut added = WorkspaceState::new("lane-1", "pane-1");
    assert!(added.add_pane_right_without_resizing("pane-2", 719.0));
    assert!(
        added
            .active_columns()
            .iter()
            .all(|column| (column.width - 719.0).abs() < f64::EPSILON)
    );

    let mut split = WorkspaceState::new("lane-1", "pane-1");
    assert!(split.split_focused_pane_right_visibly("pane-2", 359.0));
    assert!(
        split
            .active_columns()
            .iter()
            .all(|column| (column.width - 359.0).abs() < f64::EPSILON)
    );

    let mut split_left = WorkspaceState::new("lane-1", "pane-1");
    assert!(split_left.insert_focused_pane_left("pane-left", 359.0));
    assert_eq!(split_left.active_pane_ids(), ["pane-left", "pane-1"]);
    assert!(
        split_left
            .active_columns()
            .iter()
            .all(|column| (column.width - 359.0).abs() < f64::EPSILON)
    );
}

#[test]
fn multi_column_recipe_round_trip_preserves_source_topology() {
    let envelope = SessionRestoreEnvelope::from_json(V3_ENVELOPE).unwrap();
    let mut window = envelope.workspace.windows[0].clone();
    window.worklanes[0].columns.push(zentty_core::ColumnRecipe {
        id: "column-right".to_owned(),
        width: 320.0,
        focused_pane_id: Some("pane-review".to_owned()),
        last_focused_pane_id: Some("pane-review".to_owned()),
        pane_heights: vec![700.0],
        panes: vec![PaneRecipe {
            id: "pane-review".to_owned(),
            custom_title: Some("Review".to_owned()),
            title_seed: None,
            working_directory: Some("/tmp/project".to_owned()),
            last_activity_title: None,
            last_run_command: None,
        }],
    });
    window.worklanes[0].focused_column_id = Some("column-right".to_owned());

    let state = WorkspaceState::from_window_recipe(&window).unwrap();
    assert_eq!(state.active_columns().len(), 2);
    assert_eq!(
        state.active_pane_ids(),
        ["pane-agent", "pane-shell", "pane-review"]
    );
    assert_eq!(state.focused_pane_id(), Some("pane-review"));
    let projected = state.to_window_recipe(&window);
    assert_eq!(projected.worklanes[0].columns, window.worklanes[0].columns);
}

#[test]
fn cross_column_move_matches_source_height_reconciliation() {
    let envelope = SessionRestoreEnvelope::from_json(V3_ENVELOPE).unwrap();
    let mut window = envelope.workspace.windows[0].clone();
    window.worklanes[0].columns.push(zentty_core::ColumnRecipe {
        id: "column-right".to_owned(),
        width: 320.0,
        focused_pane_id: Some("pane-review".to_owned()),
        last_focused_pane_id: Some("pane-review".to_owned()),
        pane_heights: vec![700.0],
        panes: vec![PaneRecipe {
            id: "pane-review".to_owned(),
            custom_title: Some("Review".to_owned()),
            title_seed: None,
            working_directory: Some("/tmp/project".to_owned()),
            last_activity_title: None,
            last_run_command: None,
        }],
    });

    let mut state = WorkspaceState::from_window_recipe(&window).unwrap();
    assert!(state.move_focused_pane_right());
    assert_eq!(state.active_columns()[0].pane_heights, [700.0]);
    assert_eq!(state.active_columns()[1].pane_heights, [1.0, 1.0]);
    assert_eq!(
        state.active_pane_ids(),
        ["pane-shell", "pane-agent", "pane-review"]
    );
    assert_eq!(state.focused_pane_id(), Some("pane-agent"));
}

#[test]
fn duplicate_column_identity_is_rejected() {
    let envelope = SessionRestoreEnvelope::from_json(V3_ENVELOPE).unwrap();
    let mut window = envelope.workspace.windows[0].clone();
    let duplicate = window.worklanes[0].columns[0].clone();
    window.worklanes[0].columns.push(duplicate);

    assert_eq!(
        WorkspaceState::from_window_recipe(&window),
        Err(WorkspaceStateImportError::DuplicateColumn(
            "column-left".to_owned()
        ))
    );
}

#[test]
fn source_directional_focus_preserves_each_columns_last_focused_pane() {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../Zentty/Layout/PaneStripState.swift"
    ));
    let mut state = WorkspaceState::new("lane", "left-top");
    assert!(state.split_focused_pane_below("left-bottom"));
    assert!(state.select_pane("left-top"));
    assert!(state.split_focused_pane_right("right-top"));
    assert!(state.split_focused_pane_below("right-bottom"));

    assert!(state.focus_pane_left());
    assert_eq!(state.focused_pane_id(), Some("left-top"));
    assert!(state.focus_pane_right());
    assert_eq!(state.focused_pane_id(), Some("right-bottom"));
    assert!(state.focus_pane_up());
    assert_eq!(state.focused_pane_id(), Some("right-top"));
    assert!(!state.focus_pane_up());
    assert!(state.focus_pane_down());
    assert_eq!(state.focused_pane_id(), Some("right-bottom"));
    assert!(!state.focus_pane_down());

    for source_contract in [
        "mutating func moveFocusLeft()",
        "mutating func moveFocusRight()",
        "mutating func moveFocusUp()",
        "mutating func moveFocusDown()",
    ] {
        assert!(SOURCE.contains(source_contract));
    }
}

#[test]
fn source_before_insertion_commands_target_the_focused_slot() {
    let mut state = WorkspaceState::new("lane", "pane-a");
    assert!(state.insert_focused_pane_left("pane-left", 320.0));
    assert_eq!(state.active_pane_ids(), ["pane-left", "pane-a"]);
    assert_eq!(state.focused_pane_id(), Some("pane-left"));
    assert!(state.split_focused_pane_below("pane-lower"));
    assert!(state.insert_focused_pane_above("pane-upper"));
    assert_eq!(
        state.active_pane_ids(),
        ["pane-left", "pane-upper", "pane-lower", "pane-a"]
    );
    assert_eq!(state.focused_pane_id(), Some("pane-upper"));
}

#[test]
fn source_grid_isolates_a_selected_pane_without_replacing_its_live_identity() {
    let mut state = WorkspaceState::new("lane", "source");
    assert!(state.split_focused_pane_right("neighbor"));
    assert!(state.select_pane("source"));
    assert!(state.isolate_focused_pane_in_new_worklane(
        "grid-lane",
        NewWorklanePlacement::AfterCurrent,
        720.0,
    ));
    assert_eq!(state.worklane_ids(), ["lane", "grid-lane"]);
    assert_eq!(state.active_worklane_id(), "grid-lane");
    assert_eq!(state.active_pane_ids(), ["source"]);
    assert_eq!(state.worklanes()[0].columns[0].panes[0].id, "neighbor");
    assert_eq!(state.pane("source").unwrap().id, "source");
    assert!(!state.isolate_focused_pane_in_new_worklane(
        "unused",
        NewWorklanePlacement::AfterCurrent,
        720.0,
    ));

    let mut last_column = WorkspaceState::new("lane", "first");
    assert!(last_column.split_focused_pane_right("middle"));
    assert!(last_column.split_focused_pane_right("last"));
    assert!(last_column.isolate_focused_pane_in_new_worklane(
        "grid-lane",
        NewWorklanePlacement::End,
        720.0,
    ));
    assert_eq!(
        last_column.worklanes()[0].focused_column_id,
        "column-middle"
    );
    assert_eq!(last_column.worklanes()[0].columns.len(), 2);
}

#[test]
fn source_arrangement_presets_reflow_stable_panes_and_preserve_focus() {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../Zentty/Input/PaneCommand.swift"
    ));
    let mut state = WorkspaceState::new("lane", "pane-1");
    for pane in ["pane-2", "pane-3", "pane-4", "pane-5"] {
        assert!(state.split_focused_pane_right(pane));
    }
    assert!(state.select_pane("pane-3"));

    assert!(state.arrange_columns(3, 900.0));
    assert!(
        state
            .active_columns()
            .iter()
            .all(|column| (column.width - (898.0 / 3.0)).abs() < f64::EPSILON)
    );
    assert!(state.arrange_panes_per_column(2));
    assert_eq!(state.active_columns().len(), 3);
    assert_eq!(state.active_columns()[0].panes.len(), 2);
    assert_eq!(state.active_columns()[1].panes.len(), 2);
    assert_eq!(state.active_columns()[2].panes.len(), 1);
    assert_eq!(state.focused_pane_id(), Some("pane-3"));
    assert_eq!(
        state.active_pane_ids(),
        ["pane-1", "pane-2", "pane-3", "pane-4", "pane-5"]
    );

    for source_contract in [
        "case fullWidth = 1",
        "case halfWidth = 2",
        "case thirds = 3",
        "case quarters = 4",
        "case fullHeight = 1",
        "case twoPerColumn = 2",
        "case threePerColumn = 3",
        "case fourPerColumn = 4",
    ] {
        assert!(SOURCE.contains(source_contract));
    }
}

#[test]
fn source_readable_width_changes_scale_every_multi_column_worklane_proportionally() {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../Zentty/AppState/WorklaneStore.swift"
    ));
    let mut state = WorkspaceState::new("lane-a", "pane-a1");
    assert!(state.split_focused_pane_right("pane-a2"));
    assert!(state.restore_column_width("pane-a1", 400.0));
    assert!(state.restore_column_width("pane-a2", 600.0));
    assert!(state.create_worklane("lane-b", "pane-b1"));
    assert!(state.split_focused_pane_right("pane-b2"));
    assert!(state.restore_column_width("pane-b1", 300.0));
    assert!(state.restore_column_width("pane-b2", 700.0));

    assert!(state.scale_multi_column_widths(1.5));
    assert_eq!(state.worklanes()[0].columns[0].width, 600.0);
    assert_eq!(state.worklanes()[0].columns[1].width, 900.0);
    assert_eq!(state.worklanes()[1].columns[0].width, 450.0);
    assert_eq!(state.worklanes()[1].columns[1].width, 1_050.0);
    assert!(!state.scale_multi_column_widths(1.0));
    assert!(!state.scale_multi_column_widths(f64::NAN));
    assert!(SOURCE.contains("scalePaneWidths(by: readableWidthScaleFactor)"));
    assert!(SOURCE.contains("nextReadableWidth / previousReadableWidth"));
}

#[test]
fn divider_resize_updates_only_adjacent_columns_and_clamps_to_source_minimums() {
    let mut state = WorkspaceState::new("lane", "pane-left");
    assert!(state.split_focused_pane_right("pane-middle"));
    assert!(state.split_focused_pane_right("pane-right"));
    for (pane_id, width) in [
        ("pane-left", 400.0),
        ("pane-middle", 350.0),
        ("pane-right", 250.0),
    ] {
        assert!(state.restore_column_width(pane_id, width));
    }

    let left_column = state.active_columns()[0].id.clone();
    assert!(state.resize_column_divider(&left_column, 75.0, 160.0));
    assert!((state.active_columns()[0].width - 475.0).abs() < f64::EPSILON);
    assert!((state.active_columns()[1].width - 275.0).abs() < f64::EPSILON);
    assert!((state.active_columns()[2].width - 250.0).abs() < f64::EPSILON);

    assert!(state.resize_column_divider(&left_column, 1_000.0, 160.0));
    assert!((state.active_columns()[0].width - 590.0).abs() < f64::EPSILON);
    assert!((state.active_columns()[1].width - 160.0).abs() < f64::EPSILON);
    assert!(!state.resize_column_divider(&left_column, 1.0, 160.0));
    assert!(state.equalize_column_divider(&left_column, 160.0));
    assert!((state.active_columns()[0].width - 375.0).abs() < f64::EPSILON);
    assert!((state.active_columns()[1].width - 375.0).abs() < f64::EPSILON);
}

#[test]
fn divider_resize_updates_only_adjacent_panes_and_preserves_total_weight() {
    let mut state = WorkspaceState::new("lane", "pane-top");
    assert!(state.split_focused_pane_below("pane-middle"));
    assert!(state.split_focused_pane_below("pane-bottom"));
    let column_id = state.active_columns()[0].id.clone();
    let before = state.active_columns()[0].pane_heights.clone();

    assert!(state.resize_pane_divider(&column_id, "pane-top", 30.0, 600.0, 80.0,));
    let heights = &state.active_columns()[0].pane_heights;
    assert!((heights.iter().sum::<f64>() - 1.0).abs() < f64::EPSILON);
    assert!((heights[0] - (before[0] + 0.05)).abs() < f64::EPSILON);
    assert!((heights[1] - (before[1] - 0.05)).abs() < f64::EPSILON);
    assert!((heights[2] - before[2]).abs() < f64::EPSILON);

    assert!(state.resize_pane_divider(&column_id, "pane-top", 1_000.0, 600.0, 80.0,));
    let heights = &state.active_columns()[0].pane_heights;
    assert!((heights[1] - (80.0 / 600.0)).abs() < f64::EPSILON);
    assert!(!state.resize_pane_divider(&column_id, "pane-top", 1.0, 600.0, 80.0,));
    assert!(state.equalize_pane_divider(&column_id, "pane-top"));
    let heights = &state.active_columns()[0].pane_heights;
    assert!((heights[0] - heights[1]).abs() < f64::EPSILON);
    assert!((heights[2] - before[2]).abs() < f64::EPSILON);
}

#[test]
fn absolute_focused_pane_height_matches_source_fraction_and_preserves_other_proportions() {
    let mut state = WorkspaceState::new("lane", "pane-top");
    assert!(state.split_focused_pane_below("pane-middle"));
    assert!(state.split_focused_pane_below("pane-bottom"));

    assert!(state.resize_focused_pane_height_to_fraction(0.60));
    let heights = &state.active_columns()[0].pane_heights;
    let total = heights.iter().sum::<f64>();
    assert!((heights[2] / total - 0.60).abs() < 1e-12);
    assert!((heights[0] / heights[1] - 2.0).abs() < 1e-12);

    assert!(state.resize_focused_pane_height_to_fraction(0.0));
    let heights = &state.active_columns()[0].pane_heights;
    let total = heights.iter().sum::<f64>();
    assert!((heights[2] / total - 0.05).abs() < 1e-12);
    assert!(!WorkspaceState::new("lane", "only").resize_focused_pane_height_to_fraction(0.5));
}

#[test]
fn divider_geometry_round_trips_exactly_through_the_workspace_recipe() {
    let envelope = SessionRestoreEnvelope::from_json(V3_ENVELOPE).unwrap();
    let mut window = envelope.workspace.windows[0].clone();
    window.worklanes[0].columns.push(zentty_core::ColumnRecipe {
        id: "column-right".to_owned(),
        width: 360.0,
        focused_pane_id: Some("pane-review".to_owned()),
        last_focused_pane_id: Some("pane-review".to_owned()),
        pane_heights: vec![1.0],
        panes: vec![PaneRecipe {
            id: "pane-review".to_owned(),
            custom_title: None,
            title_seed: None,
            working_directory: Some("/tmp".to_owned()),
            last_activity_title: None,
            last_run_command: None,
        }],
    });
    let left_column_id = window.worklanes[0].columns[0].id.clone();
    let top_pane_id = window.worklanes[0].columns[0].panes[0].id.clone();
    let mut state = WorkspaceState::from_window_recipe(&window).unwrap();

    assert!(state.resize_column_divider(&left_column_id, 47.25, 160.0));
    assert!(state.resize_pane_divider(&left_column_id, &top_pane_id, -33.5, 700.0, 80.0,));
    let projected = state.to_window_recipe(&window);
    let restored = WorkspaceState::from_window_recipe(&projected).unwrap();

    assert_eq!(restored.active_columns(), state.active_columns());
}

#[test]
fn keyboard_horizontal_resize_changes_only_the_focused_source_column() {
    let mut state = WorkspaceState::new("lane", "left");
    assert!(state.split_focused_pane_right("middle"));
    assert!(state.split_focused_pane_right("right"));
    assert!(state.focus_pane_left());
    for (pane, width) in [("left", 400.0), ("middle", 500.0), ("right", 600.0)] {
        assert!(state.restore_column_width(pane, width));
    }

    assert!(state.resize_focused_column(PaneResizeDirection::Left, 9.0, 160.0, 900.0));
    assert!((state.active_columns()[0].width - 400.0).abs() < f64::EPSILON);
    assert!((state.active_columns()[1].width - 509.0).abs() < f64::EPSILON);
    assert!((state.active_columns()[2].width - 600.0).abs() < f64::EPSILON);
    assert!(state.focus_pane_right());
    assert!(state.resize_focused_column(PaneResizeDirection::Right, 9.0, 160.0, 900.0));
    assert!((state.active_columns()[2].width - 591.0).abs() < f64::EPSILON);
}

#[test]
fn keyboard_horizontal_resize_obeys_source_edge_and_bounds_policy() {
    let mut state = WorkspaceState::new("lane", "left");
    assert!(state.split_focused_pane_right("middle"));
    assert!(state.split_focused_pane_right("right"));
    assert!(state.focus_pane_left());
    assert!(state.focus_pane_left());
    for (pane, width) in [("left", 200.0), ("middle", 300.0), ("right", 400.0)] {
        assert!(state.restore_column_width(pane, width));
    }

    assert!(state.resize_focused_column(PaneResizeDirection::Left, 50.0, 160.0, 500.0));
    assert!((state.active_columns()[0].width - 160.0).abs() < f64::EPSILON);
    assert!(!state.resize_focused_column(PaneResizeDirection::Left, 50.0, 160.0, 500.0));
    assert!(state.resize_focused_column(PaneResizeDirection::Right, 50.0, 160.0, 500.0));
    assert!((state.active_columns()[0].width - 210.0).abs() < f64::EPSILON);

    assert!(state.focus_pane_right());
    assert!(state.resize_focused_column(PaneResizeDirection::Right, 250.0, 160.0, 500.0));
    assert!((state.active_columns()[1].width - 500.0).abs() < f64::EPSILON);
    assert!(!state.resize_focused_column(PaneResizeDirection::Right, 1.0, 160.0, 500.0));

    assert!(state.focus_pane_right());
    assert!(state.resize_focused_column(PaneResizeDirection::Right, 300.0, 160.0, 500.0));
    assert!((state.active_columns()[2].width - 160.0).abs() < f64::EPSILON);
    assert!(!state.resize_focused_column(PaneResizeDirection::Up, 10.0, 160.0, 500.0));
    assert!(!state.resize_focused_column(PaneResizeDirection::Left, f64::NAN, 160.0, 500.0));
}

#[test]
fn keyboard_vertical_resize_uses_an_adjacent_last_interacted_divider() {
    let mut state = WorkspaceState::new("lane", "top");
    assert!(state.split_focused_pane_below("middle"));
    assert!(state.split_focused_pane_below("bottom"));
    assert!(state.focus_pane_up());
    let before = state.active_columns()[0].pane_heights.clone();

    assert!(state.resize_focused_pane_vertically(
        PaneResizeDirection::Up,
        20.0,
        600.0,
        80.0,
        Some("top"),
    ));
    let heights = &state.active_columns()[0].pane_heights;
    assert!(heights[0] < before[0]);
    assert!(heights[1] > before[1]);
    assert!((heights[2] - before[2]).abs() < f64::EPSILON);

    assert!(state.resize_focused_pane_vertically(
        PaneResizeDirection::Down,
        20.0,
        600.0,
        80.0,
        Some("top"),
    ));
    for (restored, original) in state.active_columns()[0].pane_heights.iter().zip(&before) {
        assert!((restored - original).abs() < f64::EPSILON);
    }

    assert!(!state.resize_focused_pane_vertically(
        PaneResizeDirection::Left,
        20.0,
        600.0,
        80.0,
        None,
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn split_out_pane_to_new_window_preserves_source_metadata_and_normalizes_destination() {
    let mut state = WorkspaceState::new("build", "shell");
    assert!(state.set_worklane_title("build", Some("Build")));
    assert!(state.set_worklane_color("build", Some(WorklaneColor::Teal)));
    assert!(state.split_focused_pane_below("agent"));
    assert!(state.set_pane_custom_title("agent", Some("Reviewer")));
    assert!(state.configure_pane_launch(
        "agent",
        Some("/tmp/project".to_owned()),
        Some("codex resume abc".to_owned()),
    ));
    state.apply_agent_event(
        AuthenticatedAgentEvent {
            target: AgentTarget::new("source-window", "build", "agent"),
            pane_token: "token-agent".to_owned(),
            event: AgentEvent::parse(
                br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"session-agent"}}"#,
            )
            .unwrap(),
        },
        1,
    );
    state.apply_agent_event(
        AuthenticatedAgentEvent {
            target: AgentTarget::new("source-window", "build", "agent"),
            pane_token: "token-agent".to_owned(),
            event: AgentEvent::parse(
                br#"{"version":1,"event":"agent.idle","session":{"id":"session-agent"},"state":{"stopCandidate":true}}"#,
            )
            .unwrap(),
        },
        2,
    );
    let mut source_recipe = state.to_window_recipe(&WindowRecipe {
        id: "source-window".to_owned(),
        frame: None,
        worklanes: Vec::new(),
        active_worklane_id: None,
    });
    source_recipe.worklanes[0].next_pane_number = 42;
    source_recipe.worklanes[0].bookmark_origin_id = Some("bookmark-build".to_owned());

    let mut transfer = state
        .split_pane_to_new_window("agent", "destination-lane")
        .expect("a pane in a multi-pane worklane can move to a new window");

    assert_eq!(transfer.moved_pane_id, "agent");
    assert!(!transfer.source_window_should_close);
    assert_eq!(state.active_pane_ids(), ["shell"]);
    assert_eq!(state.active_worklane().title.as_deref(), Some("Build"));
    assert_eq!(state.active_worklane().color, Some(WorklaneColor::Teal));
    assert_eq!(transfer.destination.worklane_ids(), ["destination-lane"]);
    let destination = transfer.destination.active_worklane();
    assert_eq!(destination.title.as_deref(), Some("Build"));
    assert_eq!(destination.color, Some(WorklaneColor::Teal));
    assert_eq!(destination.columns.len(), 1);
    assert!((destination.columns[0].width - 1.0).abs() < f64::EPSILON);
    assert_eq!(destination.columns[0].pane_heights, [1.0]);
    let moved = transfer.destination.pane("agent").unwrap();
    assert_eq!(moved.custom_title.as_deref(), Some("Reviewer"));
    assert_eq!(moved.working_directory.as_deref(), Some("/tmp/project"));
    assert_eq!(moved.last_run_command.as_deref(), Some("codex resume abc"));
    assert!(
        state.sidebar_summaries()[0].pane_rows[0]
            .agent_status
            .is_none()
    );
    assert_eq!(
        transfer.destination.sidebar_summaries()[0].pane_rows[0]
            .agent_status
            .as_ref()
            .map(|status| status.session_id.as_str()),
        Some("session-agent")
    );
    assert!(transfer.destination.sweep_agent_lifecycle(2_002, |_| true));
    assert_eq!(
        transfer.destination.sidebar_summaries()[0].pane_rows[0]
            .agent_status
            .as_ref()
            .map(|status| status.phase),
        Some(AgentPhase::Idle)
    );
    assert!(state.split_focused_pane_below("agent"));
    state.apply_agent_event(
        AuthenticatedAgentEvent {
            target: AgentTarget::new("source-window", "build", "agent"),
            pane_token: "replacement-token".to_owned(),
            event: AgentEvent::parse(
                br#"{"version":1,"event":"task.progress","session":{"id":"session-agent"},"progress":{"done":1,"total":2}}"#,
            )
            .unwrap(),
        },
        1_000,
    );
    assert!(!state.sweep_agent_lifecycle(2_002, |_| true));
    assert_eq!(
        state.sidebar_summaries()[0].pane_rows[1]
            .agent_status
            .as_ref()
            .map(|status| status.phase),
        Some(AgentPhase::Starting)
    );
    let destination_recipe = transfer
        .destination_window_recipe(&source_recipe, "destination-window")
        .expect("source metadata can project the destination recipe");
    assert_eq!(destination_recipe.id, "destination-window");
    assert_eq!(destination_recipe.frame, None);
    assert_eq!(destination_recipe.worklanes[0].id, "destination-lane");
    assert_eq!(destination_recipe.worklanes[0].next_pane_number, 42);
    assert_eq!(
        destination_recipe.worklanes[0]
            .bookmark_origin_id
            .as_deref(),
        Some("bookmark-build")
    );
    assert_eq!(
        destination_recipe.worklanes[0].columns[0].panes[0]
            .working_directory
            .as_deref(),
        Some("/tmp/project")
    );
}

#[test]
fn split_out_only_pane_transfers_complete_worklane_and_rejects_final_pane() {
    let mut state = WorkspaceState::new("main", "main-pane");
    assert!(state.create_worklane("review", "review-pane"));
    assert!(state.set_worklane_title("review", Some("Review")));
    assert!(state.set_worklane_color("review", Some(WorklaneColor::Purple)));
    let mut source_recipe = state.to_window_recipe(&WindowRecipe {
        id: "source-window".to_owned(),
        frame: None,
        worklanes: Vec::new(),
        active_worklane_id: None,
    });
    let review_recipe = source_recipe
        .worklanes
        .iter_mut()
        .find(|worklane| worklane.id == "review")
        .unwrap();
    review_recipe.next_pane_number = 17;
    review_recipe.bookmark_origin_id = Some("bookmark-review".to_owned());

    let transfer = state
        .split_pane_to_new_window("review-pane", "unused-generated-id")
        .expect("a complete worklane can leave a multi-worklane window");

    assert_eq!(state.worklane_ids(), ["main"]);
    assert_eq!(state.active_worklane_id(), "main");
    assert_eq!(transfer.destination.worklane_ids(), ["review"]);
    assert_eq!(transfer.destination.active_worklane_id(), "review");
    assert_eq!(
        transfer.destination.active_worklane().title.as_deref(),
        Some("Review")
    );
    assert_eq!(
        transfer.destination.active_worklane().color,
        Some(WorklaneColor::Purple)
    );
    let destination_recipe = transfer
        .destination_window_recipe(&source_recipe, "destination-window")
        .expect("complete worklane metadata can project the destination recipe");
    assert_eq!(destination_recipe.worklanes[0].id, "review");
    assert_eq!(destination_recipe.worklanes[0].next_pane_number, 17);
    assert_eq!(
        destination_recipe.worklanes[0]
            .bookmark_origin_id
            .as_deref(),
        Some("bookmark-review")
    );

    assert!(
        state
            .split_pane_to_new_window("main-pane", "destination")
            .is_none()
    );
    assert_eq!(state.worklane_ids(), ["main"]);
    assert_eq!(state.active_pane_ids(), ["main-pane"]);
}

#[test]
fn split_out_rejects_stale_or_colliding_identity_without_mutation() {
    let mut state = WorkspaceState::new("main", "left");
    assert!(state.split_focused_pane_right("right"));
    let before = state.clone();

    assert!(
        state
            .split_pane_to_new_window("missing", "destination")
            .is_none()
    );
    assert_eq!(state, before);
    assert!(state.split_pane_to_new_window("right", "").is_none());
    assert_eq!(state, before);
    assert!(state.split_pane_to_new_window("right", "main").is_none());
    assert_eq!(state, before);
}

#[test]
fn cross_window_transfer_preserves_pane_agent_state_and_uses_destination_geometry() {
    let mut source = WorkspaceState::new("source", "shell");
    assert!(source.split_focused_pane_right("agent"));
    assert!(source.configure_pane_launch(
        "agent",
        Some("/tmp/project".to_owned()),
        Some("codex resume session-1".to_owned()),
    ));
    assert!(source.set_pane_custom_title("agent", Some("Reviewer")));
    source.apply_agent_event(
        AuthenticatedAgentEvent {
            target: AgentTarget::new("source-window", "source", "agent"),
            pane_token: "agent-token".to_owned(),
            event: AgentEvent::parse(
                br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"session-1"}}"#,
            )
            .unwrap(),
        },
        1,
    );
    let transfer = source
        .extract_pane_for_cross_window_transfer("agent")
        .expect("a live pane can leave for an existing window");

    assert!(!transfer.source_window_should_close);
    assert_eq!(source.active_pane_ids(), ["shell"]);
    assert!(
        source.sidebar_summaries()[0].pane_rows[0]
            .agent_status
            .is_none()
    );

    let mut destination = WorkspaceState::new("destination", "destination-pane");
    assert!(destination.insert_cross_window_pane(transfer, "destination", 240.0,));
    assert_eq!(destination.active_worklane_id(), "destination");
    assert_eq!(destination.focused_pane_id(), Some("agent"));
    let moved = destination.pane("agent").expect("moved pane exists");
    assert_eq!(moved.custom_title.as_deref(), Some("Reviewer"));
    assert_eq!(moved.working_directory.as_deref(), Some("/tmp/project"));
    assert_eq!(
        moved.last_run_command.as_deref(),
        Some("codex resume session-1")
    );
    let column = destination
        .active_columns()
        .last()
        .expect("moved pane owns a destination column");
    assert!((column.width - 240.0).abs() < f64::EPSILON);
    assert_eq!(column.pane_heights, [1.0]);
    assert_eq!(
        destination.sidebar_summaries()[0].pane_rows[1]
            .agent_status
            .as_ref()
            .map(|status| status.session_id.as_str()),
        Some("session-1")
    );
}

#[test]
fn cross_window_transfer_allows_the_final_source_pane_and_rejects_invalid_destinations() {
    let mut multi_worklane_source = WorkspaceState::new("one", "pane-one");
    assert!(multi_worklane_source.create_worklane("two", "pane-two"));
    assert!(multi_worklane_source.create_worklane("three", "pane-three"));
    let ordered_ids = multi_worklane_source
        .worklane_ids()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let retained_active_id = ordered_ids[0].clone();
    let departing_id = ordered_ids[1].clone();
    let departing_pane_id = multi_worklane_source
        .sidebar_summaries()
        .into_iter()
        .find(|summary| summary.worklane_id == departing_id)
        .and_then(|summary| summary.pane_rows.into_iter().next())
        .map(|pane| pane.pane_id)
        .expect("middle worklane owns one pane");
    assert!(multi_worklane_source.select_worklane(&retained_active_id));
    let inactive_transfer = multi_worklane_source
        .extract_pane_for_cross_window_transfer(&departing_pane_id)
        .expect("an inactive single-pane worklane can leave");
    assert!(!inactive_transfer.source_window_should_close);
    assert_eq!(multi_worklane_source.worklane_ids().len(), 2);
    assert!(
        !multi_worklane_source
            .worklane_ids()
            .contains(&departing_id.as_str())
    );
    assert_eq!(
        multi_worklane_source.active_worklane_id(),
        retained_active_id
    );

    let mut source = WorkspaceState::new("source", "only");
    let transfer = source
        .extract_pane_for_cross_window_transfer("only")
        .expect("the destination keeps the application alive");
    assert!(transfer.source_window_should_close);
    assert!(source.worklane_ids().is_empty());

    let mut missing_target = WorkspaceState::new("destination", "existing");
    let missing_before = missing_target.clone();
    assert!(!missing_target.insert_cross_window_pane(transfer.clone(), "missing", 200.0,));
    assert_eq!(missing_target, missing_before);

    let mut duplicate = WorkspaceState::new("destination", "only");
    let duplicate_before = duplicate.clone();
    assert!(!duplicate.insert_cross_window_pane(transfer.clone(), "destination", 200.0));
    assert_eq!(duplicate, duplicate_before);

    let mut destination = WorkspaceState::new("destination", "existing");
    assert!(destination.insert_cross_window_pane(transfer, "destination", 200.0));
    assert_eq!(destination.active_pane_ids(), ["existing", "only"]);
}

#[test]
fn golden_and_reset_layout_presets_change_only_geometry() {
    let mut state = WorkspaceState::new("lane", "pane-1");
    assert!(state.split_focused_pane_right("pane-2"));
    assert!(state.split_focused_pane_below("pane-3"));
    let pane_order = state
        .active_pane_ids()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();

    assert!(state.arrange_golden_width(true, 1000.0));
    assert!(state.active_columns()[1].width > state.active_columns()[0].width);
    assert!(
        (state.active_columns()[0].width + state.active_columns()[1].width - 999.0).abs()
            < f64::EPSILON
    );
    assert!(state.arrange_golden_height(false));
    assert!(state.active_columns()[1].pane_heights[1] < state.active_columns()[1].pane_heights[0]);
    assert!(state.reset_active_layout(480.0));
    assert!(
        state
            .active_columns()
            .iter()
            .all(|column| (column.width - 480.0).abs() < f64::EPSILON)
    );
    assert_eq!(state.active_columns()[1].pane_heights, [0.5, 0.5]);
    assert_eq!(state.active_pane_ids(), pane_order);
    assert_eq!(state.focused_pane_id(), Some("pane-3"));
}

#[test]
fn targeted_golden_layout_does_not_activate_its_background_worklane() {
    let mut state = WorkspaceState::new("lane-1", "leader");
    assert!(state.split_focused_pane_right("teammate"));
    assert!(state.create_worklane("lane-2", "foreground"));
    let active_before = state.active_worklane_id().to_owned();
    let focused_before = state.focused_pane_id().map(str::to_owned);
    let major = (1.0 + 5.0_f64.sqrt()) / (3.0 + 5.0_f64.sqrt());
    let pair_width = 999.0;

    assert!(state.arrange_golden_width_for_pane("leader", true, 1000.0));
    let lane = state
        .worklanes()
        .iter()
        .find(|worklane| worklane.id == "lane-1")
        .unwrap();
    assert!((lane.columns[0].width - pair_width * major).abs() < f64::EPSILON);
    assert!((lane.columns[1].width - pair_width * (1.0 - major)).abs() < f64::EPSILON);
    assert_eq!(state.active_worklane_id(), active_before);
    assert_eq!(state.focused_pane_id(), focused_before.as_deref());

    assert!(state.restore_column_width("leader", 123.0));
    assert!(state.arrange_golden_width_for_pane("leader", true, 1000.0));
    assert!(state.restore_column_width("teammate", 123.0));
    assert!(state.arrange_golden_width_for_pane("leader", true, 1000.0));
    assert!(!state.arrange_golden_width_for_pane("leader", true, 1000.0));

    assert!(state.arrange_golden_width_for_pane("leader", true, 1.0));
    assert!(state.restore_column_width("leader", major + f64::EPSILON));
    assert!(!state.arrange_golden_width_for_pane("leader", true, 1.0));
    assert!(state.restore_column_width("teammate", 1.0 - major + f64::EPSILON));
    assert!(!state.arrange_golden_width_for_pane("leader", true, 1.0));

    assert!(state.arrange_golden_width_for_pane("teammate", true, 1000.0));
    let lane = state
        .worklanes()
        .iter()
        .find(|worklane| worklane.id == "lane-1")
        .unwrap();
    assert!((lane.columns[0].width - pair_width * (1.0 - major)).abs() < f64::EPSILON);
    assert!((lane.columns[1].width - pair_width * major).abs() < f64::EPSILON);

    assert!(state.arrange_golden_width_for_pane("teammate", false, 1000.0));
    let lane = state
        .worklanes()
        .iter()
        .find(|worklane| worklane.id == "lane-1")
        .unwrap();
    assert!((lane.columns[0].width - pair_width * major).abs() < f64::EPSILON);
    assert!((lane.columns[1].width - pair_width * (1.0 - major)).abs() < f64::EPSILON);
    assert!(!state.arrange_golden_width_for_pane("leader", true, f64::NAN));
    assert!(!state.arrange_golden_width_for_pane("missing", true, 1000.0));

    let mut single = WorkspaceState::new("single", "only");
    assert!(!single.arrange_golden_width_for_pane("only", true, 1000.0));
}
