use zentty_core::{
    AgentEvent, AgentInteractionKind, AgentPhase, AgentStatusStore, AgentTarget,
    AuthenticatedAgentEvent, PaneAgentStatus, PaneTokenRegistry,
};

fn target() -> AgentTarget {
    AgentTarget::new("window-a", "worklane-a", "pane-a")
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
    assert!(store.apply_codex_title("pane-a", "Working ⠙ codex-stale", 21));
    assert_eq!(
        store.status_for(&target()).unwrap().phase,
        AgentPhase::Running
    );
}
