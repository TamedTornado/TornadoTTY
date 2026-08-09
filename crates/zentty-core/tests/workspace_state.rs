use zentty_core::{
    AgentEvent, AgentInteractionKind, AgentTarget, AuthenticatedAgentEvent, ClosePaneOutcome,
    CodexTranscriptQuestion, PaneRecipe, SessionRestoreEnvelope, WorklaneColor, WorkspaceState,
    WorkspaceStateImportError,
};

const V3_ENVELOPE: &[u8] = include_bytes!("fixtures/session-restore-v3.json");

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
                br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"codex-explicit"},"transcriptPath":"/tmp/explicit.jsonl"}"#,
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
    assert_eq!(candidate.working_directory, None);
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
            br#"{"version":1,"event":"session.start","agent":{"name":"Codex","pid":4242},"session":{"id":"session-codex"}}"#,
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

    let drafts = state.agent_restore_drafts();
    assert_eq!(drafts.len(), 2);
    assert_eq!(drafts[0].pane_id, "pane-agent");
    assert_eq!(
        drafts[0].resume_command().as_deref(),
        Some("codex resume session-codex")
    );
    assert_eq!(drafts[0].tracked_pid, 4242);
    assert_eq!(drafts[0].working_directory.as_deref(), Some("/tmp/project"));
    assert_eq!(
        drafts[0].agent_launch_snapshot.as_ref().unwrap().arguments,
        ["codex", "resume", "session-codex"]
    );
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

    let mut relaunched =
        WorkspaceState::from_window_recipe(&envelope.workspace.windows[0]).unwrap();
    assert!(relaunched.seed_restored_agent(&gemini[0], 13));
    let summaries = relaunched.sidebar_summaries();
    let restored_status = summaries[0].pane_rows[1].agent_status.as_ref().unwrap();
    assert_eq!(restored_status.agent_name, "Gemini");
    assert_eq!(restored_status.phase, zentty_core::AgentPhase::Starting);
    assert_eq!(restored_status.session_id, "gemini-session");
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

    assert_eq!(
        state.close_pane_at("pane-agent", 1_000),
        ClosePaneOutcome::Closed
    );
    let restored = state
        .restore_closed_pane_at("pane-restored", 1_001)
        .expect("recent local pane should restore");
    assert_eq!(restored.pane_id, "pane-restored");
    assert_eq!(restored.worklane_id, "worklane-main");
    assert_eq!(restored.working_directory.as_deref(), Some("/tmp/project"));
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
        state.rollback_restored_pane_at("pane-runtime-failed", 1_001),
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
    assert!(!summaries[0].pane_rows[0].is_focused);
    assert_eq!(summaries[0].pane_rows[1].primary_text, "cargo test");
    assert!(summaries[0].pane_rows[1].is_focused);
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
