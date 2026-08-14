use zentty_core::{
    AgentEvent, AgentInteractionKind, AgentPhase, AgentStatusStore, AgentTarget,
    AuthenticatedAgentEvent, PaneAgentStatus, PaneTokenRegistry, TerminalProgressState,
};

fn target() -> AgentTarget {
    AgentTarget::new("window-a", "worklane-a", "pane-a")
}

fn event_for(pane_id: &str, json: &[u8]) -> AuthenticatedAgentEvent {
    AuthenticatedAgentEvent {
        target: AgentTarget::new("window-a", "worklane-a", pane_id),
        pane_token: format!("token-{pane_id}"),
        event: AgentEvent::parse(json).unwrap(),
    }
}

fn seed_other_pane_codex_tracking(store: &mut AgentStatusStore) {
    store.apply(
        event_for(
            "pane-inferred",
            br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"inferred"}}"#,
        ),
        1_000,
    );
    assert!(store.apply_codex_title("pane-inferred", "[ ! ] Action Required | tracking", 1_100));

    for (pane, session) in [
        ("pane-suppressed", "suppressed"),
        ("pane-observed", "observed"),
    ] {
        store.apply(
            event_for(
                pane,
                format!(
                    r#"{{"version":1,"event":"agent.running","agent":{{"name":"Codex"}},"session":{{"id":"{session}"}}}}"#
                )
                .as_bytes(),
            ),
            1_000,
        );
        store.apply(
            event_for(
                pane,
                format!(r#"{{"version":1,"event":"agent.idle","session":{{"id":"{session}"}}}}"#)
                    .as_bytes(),
            ),
            1_100,
        );
    }
}

fn lifecycle_event(
    pane_id: &str,
    session_id: &str,
    kind: &str,
    extra: &str,
) -> AuthenticatedAgentEvent {
    event_for(
        pane_id,
        format!(
            r#"{{"version":1,"event":"{kind}","agent":{{"name":"Codex"}},"session":{{"id":"{session_id}"}}{extra}}}"#
        )
        .as_bytes(),
    )
}

#[test]
fn stop_candidate_observes_exact_grace_and_new_activity_cancels_it() {
    let mut store = AgentStatusStore::default();
    store.apply(
        lifecycle_event("pane-a", "session-a", "agent.running", ""),
        1_000,
    );
    store.apply(
        lifecycle_event(
            "pane-a",
            "session-a",
            "agent.idle",
            r#","state":{"stopCandidate":true}"#,
        ),
        2_000,
    );
    assert_eq!(
        store.status_for_pane("pane-a").unwrap().phase,
        AgentPhase::Running
    );
    assert!(!store.sweep(3_999, |_| true));
    assert_eq!(
        store.status_for_pane("pane-a").unwrap().phase,
        AgentPhase::Running
    );
    assert!(store.sweep(4_000, |_| true));
    assert_eq!(
        store.status_for_pane("pane-a").unwrap().phase,
        AgentPhase::Idle
    );

    store.apply(
        lifecycle_event("pane-a", "session-a", "agent.running", ""),
        5_000,
    );
    store.apply(
        lifecycle_event(
            "pane-a",
            "session-a",
            "agent.idle",
            r#","state":{"stopCandidate":true}"#,
        ),
        6_000,
    );
    store.apply(
        lifecycle_event("pane-a", "session-a", "agent.running", ""),
        6_500,
    );
    assert!(!store.sweep(8_000, |_| true));
    assert_eq!(
        store.status_for_pane("pane-a").unwrap().phase,
        AgentPhase::Running
    );

    store.apply(
        lifecycle_event(
            "pane-a",
            "session-a",
            "agent.idle",
            r#","state":{"stopCandidate":true}"#,
        ),
        9_000,
    );
    assert!(store.apply_terminal_progress("pane-a", TerminalProgressState::Indeterminate, 9_100));
    assert!(!store.sweep(11_000, |_| true));

    store.apply(
        lifecycle_event(
            "pane-a",
            "session-a",
            "agent.idle",
            r#","state":{"stopCandidate":true}"#,
        ),
        12_000,
    );
    assert!(store.apply_codex_title("pane-a", "Working ⠋ zentty", 12_100));
    assert!(!store.sweep(14_000, |_| true));
}

#[test]
fn unobserved_stop_candidate_is_removed_after_its_grace() {
    let mut store = AgentStatusStore::default();
    store.apply(
        lifecycle_event(
            "pane-a",
            "session-a",
            "agent.idle",
            r#","state":{"stopCandidate":true}"#,
        ),
        1_000,
    );
    assert_eq!(
        store.status_for_pane("pane-a").unwrap().phase,
        AgentPhase::Running
    );
    assert!(store.sweep(3_000, |_| true));
    assert!(store.status_for_pane("pane-a").is_none());
}

#[test]
fn process_death_and_visibility_deadlines_match_the_source_lifecycle() {
    let mut store = AgentStatusStore::default();
    store.apply(
        event_for(
            "pane-running",
            br#"{"version":1,"event":"agent.running","agent":{"name":"Codex","pid":4242},"session":{"id":"running"}}"#,
        ),
        1_000,
    );
    store.apply(
        lifecycle_event(
            "pane-running",
            "running",
            "agent.needs-input",
            r#","state":{"text":"Allow?","interaction":{"kind":"approval","text":"Allow?"}}"#,
        ),
        1_100,
    );
    assert!(store.sweep(2_000, |pid| pid != 4242));
    let stopped = store.status_for_pane("pane-running").unwrap();
    assert_eq!(stopped.phase, AgentPhase::UnresolvedStop);
    assert_eq!(stopped.interaction, AgentInteractionKind::None);
    assert_eq!(stopped.text, None);
    assert_eq!(stopped.tracked_pid, None);
    assert!(!store.sweep(601_999, |_| false));
    assert!(store.sweep(602_000, |_| false));
    assert!(store.status_for_pane("pane-running").is_none());

    store.apply(
        event_for(
            "pane-idle",
            br#"{"version":1,"event":"agent.running","agent":{"name":"Codex","pid":4343},"session":{"id":"idle"}}"#,
        ),
        700_000,
    );
    store.apply(
        lifecycle_event("pane-idle", "idle", "agent.idle", ""),
        700_100,
    );
    assert!(store.sweep(700_200, |pid| pid != 4343));
    assert!(store.status_for_pane("pane-idle").is_none());
}

#[test]
fn dead_ephemeral_starts_are_silent_only_through_the_exact_source_window() {
    let starting = |pane: &str| {
        event_for(
            pane,
            br#"{"version":1,"event":"session.start","agent":{"name":"Codex","pid":4444},"session":{"id":"starting"}}"#,
        )
    };

    let mut exact = AgentStatusStore::default();
    exact.apply(starting("pane-exact"), 1_000);
    assert!(exact.sweep(2_000, |pid| pid != 4444));
    assert!(exact.status_for_pane("pane-exact").is_none());

    let mut expired = AgentStatusStore::default();
    expired.apply(starting("pane-expired"), 1_000);
    assert!(expired.sweep(2_001, |pid| pid != 4444));
    assert_eq!(
        expired.status_for_pane("pane-expired").unwrap().phase,
        AgentPhase::UnresolvedStop
    );
}

#[test]
fn idle_and_inactive_sessions_expire_without_environmental_assumptions() {
    let mut store = AgentStatusStore::default();
    store.apply(
        lifecycle_event("pane-idle", "idle", "agent.running", ""),
        1_000,
    );
    store.apply(
        lifecycle_event("pane-idle", "idle", "agent.idle", ""),
        2_000,
    );
    assert!(!store.sweep(121_999, |_| true));
    assert!(store.sweep(122_000, |_| true));
    assert!(store.status_for_pane("pane-idle").is_none());

    store.apply(
        lifecycle_event("pane-stale", "stale", "agent.running", ""),
        200_000,
    );
    assert!(!store.sweep(1_999_999, |_| true));
    assert!(store.sweep(2_000_000, |_| true));
    assert!(store.status_for_pane("pane-stale").is_none());
}

#[test]
fn lifecycle_clocks_are_removed_or_transferred_with_the_pane() {
    let mut removed = AgentStatusStore::default();
    removed.apply(
        lifecycle_event("pane-a", "session-a", "agent.running", ""),
        1_000,
    );
    removed.apply(
        lifecycle_event(
            "pane-a",
            "session-a",
            "agent.idle",
            r#","state":{"stopCandidate":true}"#,
        ),
        2_000,
    );
    removed.remove_pane("pane-a");
    assert!(!removed.sweep(4_000, |_| true));
}

fn assert_other_pane_codex_tracking_was_preserved(store: &mut AgentStatusStore) {
    assert!(store.apply_codex_title("pane-inferred", "Working ⠋ tracking", 1_200));
    assert!(!store.apply_codex_title("pane-suppressed", "Working ⠋ tracking", 1_200));
    assert!(store.apply_codex_user_submitted("pane-observed", 2_000));
}

#[test]
fn gemini_terminal_notifications_reconcile_only_source_owned_attention_and_completion() {
    let mut store = AgentStatusStore::default();

    assert!(store.apply_terminal_notification(
        "pane-a",
        Some("Gemini"),
        Some("Action required"),
        1_000,
    ));
    let status = store.status_for_pane("pane-a").unwrap();
    assert_eq!(status.agent_name, "Gemini");
    assert_eq!(status.phase, AgentPhase::NeedsInput);
    assert_eq!(status.interaction, AgentInteractionKind::Approval);
    assert_eq!(status.text.as_deref(), Some("Action required"));

    assert!(store.apply_terminal_notification(
        "pane-a",
        Some("Gemini"),
        Some("Session complete"),
        1_001,
    ));
    let status = store.status_for_pane("pane-a").unwrap();
    assert_eq!(status.phase, AgentPhase::Idle);
    assert_eq!(status.interaction, AgentInteractionKind::None);
    assert_eq!(status.text, None);

    assert!(!store.apply_terminal_notification(
        "pane-shell",
        Some("Backup"),
        Some("Session complete"),
        1_002,
    ));
    assert!(store.status_for_pane("pane-shell").is_none());
}

#[test]
fn gemini_terminal_completion_preserves_the_installed_hook_session_identity() {
    let mut store = AgentStatusStore::default();
    store.apply(
        event_for(
            "pane-a",
            br#"{"version":1,"event":"agent.running","agent":{"name":"Gemini"},"session":{"id":"gemini-real"}}"#,
        ),
        1_000,
    );

    assert!(store.apply_terminal_notification(
        "pane-a",
        Some("Gemini"),
        Some("Session complete"),
        1_001,
    ));
    let status = store.status_for_pane("pane-a").unwrap();
    assert_eq!(status.session_id, "gemini-real");
    assert_eq!(status.phase, AgentPhase::Idle);
}

#[test]
fn gemini_terminal_notification_change_receipt_tracks_each_visible_field() {
    let mut phase_only = AgentStatusStore::default();
    phase_only.apply(
        event_for(
            "pane-phase",
            br#"{"version":1,"event":"session.start","agent":{"name":"Gemini"},"session":{"id":"phase"}}"#,
        ),
        1_000,
    );
    assert!(phase_only.apply_terminal_notification(
        "pane-phase",
        Some("Gemini"),
        Some("Session complete"),
        1_001,
    ));

    let mut interaction_only = AgentStatusStore::default();
    interaction_only.apply(
        event_for(
            "pane-interaction",
            br#"{"version":1,"event":"agent.needs-input","agent":{"name":"Gemini"},"session":{"id":"interaction"},"state":{"text":"Action required","interaction":{"kind":"generic-input","text":"Action required"}}}"#,
        ),
        1_000,
    );
    assert!(interaction_only.apply_terminal_notification(
        "pane-interaction",
        Some("Gemini"),
        Some("Action required"),
        1_001,
    ));

    let mut text_only = AgentStatusStore::default();
    text_only.apply(
        event_for(
            "pane-text",
            br#"{"version":1,"event":"agent.needs-input","agent":{"name":"Gemini"},"session":{"id":"text"},"state":{"text":"Old approval","interaction":{"kind":"approval","text":"Old approval"}}}"#,
        ),
        1_000,
    );
    assert!(text_only.apply_terminal_notification(
        "pane-text",
        Some("Gemini"),
        Some("Action required"),
        1_001,
    ));
    assert!(!text_only.apply_terminal_notification(
        "pane-text",
        Some("Gemini"),
        Some("Action required"),
        1_002,
    ));
}

#[test]
fn canonical_events_drive_attention_and_progress_without_trusting_payload_routing() {
    let mut tokens = PaneTokenRegistry::default();
    tokens.register("token-a", target()).unwrap();
    tokens
        .register(
            "token-b",
            AgentTarget::new("window-a", "worklane-a", "pane-b"),
        )
        .unwrap();
    let mut statuses = AgentStatusStore::default();

    let starting = AgentEvent::parse(
        br#"{"version":1,"event":"session.start","agent":{"name":"Codex","pid":4242},"session":{"id":"session-a"}}"#,
    )
    .unwrap();
    let canonical = tokens.authenticate("token-a", starting).unwrap();
    assert_eq!(canonical.target, target());
    statuses.apply(canonical, 1_000);

    let approval = AgentEvent::parse(
        br#"{"version":1,"event":"agent.needs-input","session":{"id":"session-a"},"state":{"text":"Allow write?","interaction":{"kind":"approval","text":"Allow write?"}}}"#,
    )
    .unwrap();
    statuses.apply(tokens.authenticate("token-a", approval).unwrap(), 1_001);
    let status = statuses.status_for(&target()).unwrap();
    assert_eq!(status.phase, AgentPhase::NeedsInput);
    assert_eq!(status.interaction, AgentInteractionKind::Approval);
    assert_eq!(status.text.as_deref(), Some("Allow write?"));
    assert!(status.requires_attention());

    let progress = AgentEvent::parse(
        br#"{"version":1,"event":"task.progress","session":{"id":"session-a"},"progress":{"done":9,"total":4}}"#,
    )
    .unwrap();
    statuses.apply(tokens.authenticate("token-a", progress).unwrap(), 1_002);
    let status = statuses.status_for(&target()).unwrap();
    assert_eq!(status.phase, AgentPhase::NeedsInput);
    assert_eq!(status.progress.unwrap().done, 4);
    assert_eq!(status.progress.unwrap().total, 4);

    assert!(
        tokens
            .authenticate("token-b", AgentEvent::idle("session-a"))
            .is_ok()
    );
    assert!(
        tokens
            .authenticate("wrong-token", AgentEvent::idle("session-a"))
            .is_err()
    );
}

#[test]
fn protocol_rejects_bad_versions_unknown_events_and_oversized_requests() {
    assert!(AgentEvent::parse(br#"{"version":2,"event":"agent.idle"}"#).is_err());
    assert!(AgentEvent::parse(br#"{"version":1,"event":"agent.teleported"}"#).is_err());
    assert!(AgentEvent::parse(b"not-json").is_err());
    assert!(AgentEvent::parse(&vec![b' '; AgentEvent::MAX_WIRE_BYTES + 1]).is_err());
}

#[test]
fn attention_requires_both_the_needs_input_phase_and_a_real_interaction() {
    let status = |phase, interaction| PaneAgentStatus {
        session_id: "session".to_owned(),
        parent_session_id: None,
        agent_name: "Codex".to_owned(),
        phase,
        text: None,
        interaction,
        progress: None,
        tracked_pid: None,
        transcript_path: None,
        updated_at: 1,
    };
    assert!(!status(AgentPhase::Running, AgentInteractionKind::Approval).requires_attention());
    assert!(!status(AgentPhase::NeedsInput, AgentInteractionKind::None).requires_attention());
    assert!(status(AgentPhase::NeedsInput, AgentInteractionKind::Question).requires_attention());
}

#[test]
fn reducer_prioritizes_attention_and_session_end_clears_only_its_session() {
    let mut store = AgentStatusStore::default();
    let event = |token: &str, json: &[u8]| AuthenticatedAgentEvent {
        target: target(),
        pane_token: token.to_owned(),
        event: AgentEvent::parse(json).unwrap(),
    };

    store.apply(
        event(
            "token-a",
            br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"codex"}}"#,
        ),
        1,
    );
    store.apply(
        event(
            "token-a",
            br#"{"version":1,"event":"agent.needs-input","agent":{"name":"Claude Code"},"session":{"id":"claude"},"state":{"interaction":{"kind":"decision"}}}"#,
        ),
        0,
    );
    assert_eq!(store.status_for(&target()).unwrap().session_id, "claude");

    store.apply(
        event(
            "token-a",
            br#"{"version":1,"event":"session.end","session":{"id":"claude"}}"#,
        ),
        3,
    );
    let remaining = store.status_for(&target()).unwrap();
    assert_eq!(remaining.session_id, "codex");
    assert_eq!(remaining.phase, AgentPhase::Running);
}

#[test]
fn pane_move_does_not_strand_a_session_under_its_previous_worklane_target() {
    let mut store = AgentStatusStore::default();
    store.apply(
        AuthenticatedAgentEvent {
            target: target(),
            pane_token: "stable-token".to_owned(),
            event: AgentEvent::parse(
                br#"{"version":1,"event":"agent.running","session":{"id":"moving"}}"#,
            )
            .unwrap(),
        },
        1,
    );
    let moved_target = AgentTarget::new("window-a", "worklane-b", "pane-a");
    store.apply(
        AuthenticatedAgentEvent {
            target: moved_target.clone(),
            pane_token: "stable-token".to_owned(),
            event: AgentEvent::parse(
                br#"{"version":1,"event":"session.end","session":{"id":"moving"}}"#,
            )
            .unwrap(),
        },
        2,
    );
    assert!(store.status_for(&target()).is_none());
    assert!(store.status_for(&moved_target).is_none());
}

#[test]
fn removing_a_pane_removes_all_title_reconciliation_state() {
    let mut store = AgentStatusStore::default();
    store.apply(
        AuthenticatedAgentEvent {
            target: target(),
            pane_token: "token".to_owned(),
            event: AgentEvent::parse(
                br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"codex"}}"#,
            )
            .unwrap(),
        },
        1,
    );
    assert!(store.apply_codex_title("pane-a", "[ ! ] Action Required | zentty", 2));
    let mut inferred_store = store.clone();
    inferred_store.remove_pane("pane-a");
    assert_eq!(inferred_store, AgentStatusStore::default());
    assert!(store.apply_codex_title("pane-a", "Ready | zentty", 3));
    store.remove_pane("pane-a");
    assert_eq!(store, AgentStatusStore::default());
}

#[test]
fn codex_titles_reconcile_the_canonical_status_without_overriding_explicit_attention() {
    let mut store = AgentStatusStore::default();
    let event = |json: &[u8]| AuthenticatedAgentEvent {
        target: target(),
        pane_token: "token-a".to_owned(),
        event: AgentEvent::parse(json).unwrap(),
    };
    store.apply(
        event(
            br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"codex-title"}}"#,
        ),
        1,
    );

    assert!(store.apply_codex_title(
        "pane-a",
        "[ ! ] Action Required | codex-title | Tasks 2/5",
        2
    ));
    let inferred = store.status_for(&target()).unwrap();
    assert_eq!(inferred.phase, AgentPhase::NeedsInput);
    assert_eq!(inferred.interaction, AgentInteractionKind::GenericInput);
    assert_eq!(inferred.progress.unwrap().done, 2);

    assert!(store.apply_codex_title("pane-a", "Working ⠋ codex-title | Tasks 3/5", 3));
    let resumed = store.status_for(&target()).unwrap();
    assert_eq!(resumed.phase, AgentPhase::Running);
    assert_eq!(resumed.interaction, AgentInteractionKind::None);
    assert_eq!(resumed.progress.unwrap().done, 3);
    let unchanged_at = resumed.updated_at;
    assert!(!store.apply_codex_title("pane-a", "Working ⠙ codex-title | Tasks 3/5", 4));
    assert_eq!(
        store.status_for(&target()).unwrap().updated_at,
        unchanged_at
    );

    store.apply(
        event(
            br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"codex-title"},"state":{"text":"Compacting"}}"#,
        ),
        3,
    );
    assert_eq!(
        store.status_for(&target()).unwrap().text.as_deref(),
        Some("Compacting")
    );
    assert!(store.apply_codex_title("pane-a", "Working ⠹ codex-title", 3));
    assert_eq!(store.status_for(&target()).unwrap().text, None);

    store.apply(
        event(
            br#"{"version":1,"event":"agent.needs-input","agent":{"name":"Codex"},"session":{"id":"codex-title"},"state":{"interaction":{"kind":"decision","text":"Which database?"}}}"#,
        ),
        4,
    );
    assert!(!store.apply_codex_title("pane-a", "Working ⠙ codex-title", 5));
    let explicit = store.status_for(&target()).unwrap().clone();
    assert_eq!(explicit.phase, AgentPhase::NeedsInput);
    assert_eq!(explicit.interaction, AgentInteractionKind::Decision);
    assert_eq!(explicit.text.as_deref(), Some("Which database?"));

    assert!(!store.apply_codex_title("pane-a", "Waiting · codex-title", 6));
    assert_eq!(store.status_for(&target()).unwrap(), &explicit);
    assert!(!store.apply_codex_title("pane-a", "Ready | codex-title", 7));
    assert_eq!(store.status_for(&target()).unwrap(), &explicit);

    store.apply(
        event(
            br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"codex-title"}}"#,
        ),
        8,
    );
    assert!(store.apply_codex_title("pane-a", "[ . ] Action Required | codex-title", 9));
    assert!(store.apply_codex_title("pane-a", "Ready | codex-title", 10));
    assert_eq!(store.status_for(&target()).unwrap().phase, AgentPhase::Idle);
}

#[test]
fn recent_authoritative_idle_suppresses_one_stale_running_title_tick() {
    let mut store = AgentStatusStore::default();
    let event = |json: &[u8]| AuthenticatedAgentEvent {
        target: target(),
        pane_token: "token-a".to_owned(),
        event: AgentEvent::parse(json).unwrap(),
    };
    store.apply(
        event(
            br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"codex-stale"}}"#,
        ),
        10,
    );
    store.apply(
        event(br#"{"version":1,"event":"agent.idle","session":{"id":"codex-stale"}}"#),
        20,
    );
    assert!(!store.apply_codex_title("pane-a", "Working ⠋ codex-stale", 20));
    assert_eq!(store.status_for(&target()).unwrap().phase, AgentPhase::Idle);
    assert!(!store.apply_codex_title("pane-a", "Working ⠙ codex-stale", 1_019));
    assert!(store.apply_codex_title("pane-a", "Working ⠙ codex-stale", 1_020));
    assert_eq!(
        store.status_for(&target()).unwrap().phase,
        AgentPhase::Running
    );
}

#[test]
fn skipped_codex_title_transitions_do_not_write_back_partial_state() {
    let mut idle = AgentStatusStore::default();
    idle.apply(
        event_for(
            "pane-a",
            br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"codex-idle"}}"#,
        ),
        10,
    );
    idle.apply(
        event_for(
            "pane-a",
            br#"{"version":1,"event":"agent.idle","session":{"id":"codex-idle"}}"#,
        ),
        20,
    );
    let idle_before = idle.clone();
    assert!(!idle.apply_codex_title("pane-a", "Working ⠋ stale", 21));
    assert_eq!(idle, idle_before);

    let mut attention = AgentStatusStore::default();
    attention.apply(
        event_for(
            "pane-a",
            br#"{"version":1,"event":"agent.needs-input","agent":{"name":"Codex"},"session":{"id":"codex-attention"},"state":{"text":"Choose?","interaction":{"kind":"decision"}}}"#,
        ),
        30,
    );
    let attention_before = attention.clone();
    assert!(!attention.apply_codex_title("pane-a", "Working ⠋ stale", 31));
    assert_eq!(attention, attention_before);
}

#[test]
fn codex_osc_progress_resumes_idle_without_overriding_attention_or_interrupts() {
    assert!(!TerminalProgressState::Remove.indicates_activity());
    for state in [
        TerminalProgressState::Set,
        TerminalProgressState::Error,
        TerminalProgressState::Indeterminate,
        TerminalProgressState::Pause,
    ] {
        assert!(state.indicates_activity());
    }

    let mut store = AgentStatusStore::default();
    store.apply(
        event_for(
            "pane-a",
            br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"codex-progress"}}"#,
        ),
        1_000,
    );
    store.apply(
        event_for(
            "pane-a",
            br#"{"version":1,"event":"agent.idle","session":{"id":"codex-progress"}}"#,
        ),
        1_100,
    );

    assert!(store.apply_terminal_progress("pane-a", TerminalProgressState::Indeterminate, 1_101,));
    assert_eq!(
        store.status_for(&target()).unwrap().phase,
        AgentPhase::Running
    );

    store.apply(
        event_for(
            "pane-a",
            br#"{"version":1,"event":"agent.needs-input","session":{"id":"codex-progress"},"state":{"text":"Approve?","interaction":{"kind":"approval"}}}"#,
        ),
        1_200,
    );
    assert!(!store.apply_terminal_progress("pane-a", TerminalProgressState::Set, 1_201));
    assert_eq!(
        store.status_for(&target()).unwrap().phase,
        AgentPhase::NeedsInput
    );

    assert!(store.apply_codex_user_interrupted("pane-a", 1_300));
    assert!(!store.apply_terminal_progress("pane-a", TerminalProgressState::Pause, 1_301));
    assert!(!store.apply_terminal_progress("pane-a", TerminalProgressState::Remove, 1_302));
    assert!(store.status_for(&target()).is_none());
}

#[test]
fn sessionless_codex_completion_reconciles_the_existing_pane_session() {
    let mut store = AgentStatusStore::default();
    store.apply(
        event_for(
            "pane-a",
            br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"codex-real-session"}}"#,
        ),
        10,
    );
    store.apply(
        event_for(
            "pane-a",
            br#"{"version":1,"event":"agent.idle","agent":{"name":"Codex"}}"#,
        ),
        20,
    );

    let status = store.status_for(&target()).unwrap();
    assert_eq!(status.session_id, "codex-real-session");
    assert_eq!(status.phase, AgentPhase::Idle);
}

#[test]
fn codex_user_submit_resumes_attention_only_after_the_source_stabilization_window() {
    let event = |json: &[u8]| AuthenticatedAgentEvent {
        target: target(),
        pane_token: "token-a".to_owned(),
        event: AgentEvent::parse(json).unwrap(),
    };
    let needs_input = br#"{"version":1,"event":"agent.needs-input","agent":{"name":"Codex"},"session":{"id":"codex-input"},"state":{"interaction":{"kind":"question","text":"Continue?"}}}"#;

    let mut early = AgentStatusStore::default();
    early.apply(event(needs_input), 1_000);
    assert!(!early.apply_codex_user_submitted("pane-a", 1_349));
    assert_eq!(
        early.status_for(&target()).unwrap().phase,
        AgentPhase::NeedsInput
    );

    let mut boundary = AgentStatusStore::default();
    boundary.apply(event(needs_input), 1_000);
    assert!(boundary.apply_codex_user_submitted("pane-a", 1_350));
    let resumed = boundary.status_for(&target()).unwrap();
    assert_eq!(resumed.phase, AgentPhase::Running);
    assert_eq!(resumed.interaction, AgentInteractionKind::None);
    assert_eq!(resumed.text, None);
    assert_eq!(resumed.updated_at, 1_350);

    let mut independent_panes = AgentStatusStore::default();
    seed_other_pane_codex_tracking(&mut independent_panes);
    independent_panes.apply(event(needs_input), 1_000);
    assert!(independent_panes.apply_codex_user_submitted("pane-a", 1_350));
    assert!(!independent_panes.apply_codex_title("pane-suppressed", "Working ⠋ tracking", 1_200));
}

#[test]
fn codex_interrupt_clears_only_codex_and_suppresses_late_idle_until_real_activity() {
    let event = |json: &[u8]| AuthenticatedAgentEvent {
        target: target(),
        pane_token: "token-a".to_owned(),
        event: AgentEvent::parse(json).unwrap(),
    };
    let mut store = AgentStatusStore::default();
    store.apply(
        event(
            br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"codex-interrupt"}}"#,
        ),
        1_000,
    );
    store.apply(
        event(
            br#"{"version":1,"event":"agent.running","agent":{"name":"Claude Code"},"session":{"id":"claude-survives"}}"#,
        ),
        1_001,
    );

    assert!(store.apply_codex_user_interrupted("pane-a", 2_000));
    assert_eq!(
        store.status_for(&target()).unwrap().session_id,
        "claude-survives"
    );

    store.apply(
        event(br#"{"version":1,"event":"agent.idle","session":{"id":"codex-interrupt"}}"#),
        2_500,
    );
    assert_eq!(
        store.status_for(&target()).unwrap().session_id,
        "claude-survives"
    );
    assert!(!store.apply_codex_title("pane-a", "Working ⠋ zentty", 2_600));

    store.apply(
        event(
            br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"codex-interrupt"}}"#,
        ),
        2_700,
    );
    assert_eq!(
        store.status_for(&target()).unwrap().session_id,
        "codex-interrupt"
    );
    assert_eq!(
        store.status_for(&target()).unwrap().phase,
        AgentPhase::Running
    );

    let mut exact_deadline = AgentStatusStore::default();
    exact_deadline.apply(
        event(
            br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"deadline"}}"#,
        ),
        1_000,
    );
    assert!(exact_deadline.apply_codex_user_interrupted("pane-a", 2_000));
    exact_deadline.apply(
        event(br#"{"version":1,"event":"agent.idle","session":{"id":"deadline"}}"#),
        4_999,
    );
    assert!(exact_deadline.status_for(&target()).is_none());
    exact_deadline.apply(
        event(br#"{"version":1,"event":"agent.idle","session":{"id":"deadline"}}"#),
        5_000,
    );
    let expired = exact_deadline.status_for(&target()).unwrap();
    assert_eq!(expired.agent_name, "Codex");
    assert_eq!(expired.phase, AgentPhase::Idle);
    assert!(!exact_deadline.clear_codex_after_shell_return("pane-a", "bash"));

    let mut independent_panes = AgentStatusStore::default();
    seed_other_pane_codex_tracking(&mut independent_panes);
    independent_panes.apply(
        event(
            br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"interrupt-cleanup"}}"#,
        ),
        1_000,
    );
    assert!(independent_panes.apply_codex_user_interrupted("pane-a", 1_200));
    assert_other_pane_codex_tracking_was_preserved(&mut independent_panes);
}

#[test]
fn known_shell_titles_clear_stale_codex_without_clearing_other_agents() {
    let event = |json: &[u8]| AuthenticatedAgentEvent {
        target: target(),
        pane_token: "token-a".to_owned(),
        event: AgentEvent::parse(json).unwrap(),
    };
    let mut store = AgentStatusStore::default();
    store.apply(
        event(
            br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"codex-shell"}}"#,
        ),
        1_000,
    );
    store.apply(
        event(
            br#"{"version":1,"event":"agent.running","agent":{"name":"Gemini"},"session":{"id":"gemini-survives"}}"#,
        ),
        1_001,
    );

    assert!(!store.clear_codex_after_shell_return("pane-a", "project shell"));
    assert!(store.clear_codex_after_shell_return("pane-a", "/usr/bin/bash"));
    assert_eq!(
        store.status_for(&target()).unwrap().session_id,
        "gemini-survives"
    );
    assert!(!store.clear_codex_after_shell_return("pane-a", "bash"));

    let mut independent_panes = AgentStatusStore::default();
    seed_other_pane_codex_tracking(&mut independent_panes);
    independent_panes.apply(
        event(
            br#"{"version":1,"event":"agent.running","agent":{"name":"Codex"},"session":{"id":"shell-cleanup"}}"#,
        ),
        1_000,
    );
    assert!(independent_panes.clear_codex_after_shell_return("pane-a", "zsh"));
    assert_other_pane_codex_tracking_was_preserved(&mut independent_panes);
}

#[test]
fn claude_late_generic_notification_cannot_undo_an_explicit_stop() {
    let event = |json: &[u8]| AuthenticatedAgentEvent {
        target: target(),
        pane_token: "token-a".to_owned(),
        event: AgentEvent::parse(json).unwrap(),
    };
    let mut store = AgentStatusStore::default();
    store.apply(
        event(
            br#"{"version":1,"event":"agent.running","agent":{"name":"Claude Code"},"session":{"id":"claude-race"}}"#,
        ),
        1_000,
    );
    store.apply(
        event(
            br#"{"version":1,"event":"agent.idle","agent":{"name":"Claude Code"},"session":{"id":"claude-race"}}"#,
        ),
        2_000,
    );
    store.apply(
        event(
            br#"{"version":1,"event":"agent.needs-input","agent":{"name":"Claude Code"},"session":{"id":"claude-race"},"state":{"text":"Claude is waiting for your input","interaction":{"kind":"generic-input","text":"Claude is waiting for your input"}}}"#,
        ),
        2_100,
    );
    assert_eq!(store.status_for(&target()).unwrap().phase, AgentPhase::Idle);

    store.apply(
        event(
            br#"{"version":1,"event":"agent.needs-input","agent":{"name":"Claude Code"},"session":{"id":"claude-race"},"state":{"text":"Allow Bash?","interaction":{"kind":"approval","text":"Allow Bash?"}}}"#,
        ),
        2_200,
    );
    assert_eq!(
        store.status_for(&target()).unwrap().phase,
        AgentPhase::NeedsInput
    );

    let mut expired = AgentStatusStore::default();
    expired.apply(
        event(
            br#"{"version":1,"event":"agent.idle","agent":{"name":"Claude Code"},"session":{"id":"claude-expired"}}"#,
        ),
        2_000,
    );
    expired.apply(
        event(
            br#"{"version":1,"event":"agent.needs-input","agent":{"name":"Claude Code"},"session":{"id":"claude-expired"},"state":{"interaction":{"kind":"generic-input"}}}"#,
        ),
        7_000,
    );
    assert_eq!(
        expired.status_for(&target()).unwrap().phase,
        AgentPhase::NeedsInput
    );
}
