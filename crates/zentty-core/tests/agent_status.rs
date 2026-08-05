use zentty_core::{
    AgentEvent, AgentInteractionKind, AgentPhase, AgentStatusStore, AgentTarget,
    AuthenticatedAgentEvent, PaneTokenRegistry,
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
        2,
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
