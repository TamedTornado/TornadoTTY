use zentty_core::{AgentEvent, AgentTarget, PaneTokenError, PaneTokenRegistry};

fn idle_event() -> AgentEvent {
    AgentEvent::parse(br#"{"version":1,"event":"agent.idle"}"#).unwrap()
}

#[test]
fn token_authentication_uses_the_registered_target_not_client_claims() {
    let target = AgentTarget::new("window-real", "lane-real", "pane-real");
    let mut registry = PaneTokenRegistry::default();
    registry.register("secret-token", target.clone()).unwrap();
    let authenticated = registry.authenticate("secret-token", idle_event()).unwrap();
    assert_eq!(authenticated.target, target);
    assert_eq!(authenticated.pane_token, "secret-token");
}

#[test]
fn retarget_follows_a_pane_move_without_changing_its_child_capability() {
    let mut registry = PaneTokenRegistry::default();
    registry
        .register(
            "stable-token",
            AgentTarget::new("window", "lane-before", "pane"),
        )
        .unwrap();
    registry
        .retarget(
            "stable-token",
            AgentTarget::new("window", "lane-after", "pane"),
        )
        .unwrap();
    assert_eq!(
        registry
            .authenticate("stable-token", idle_event())
            .unwrap()
            .target,
        AgentTarget::new("window", "lane-after", "pane")
    );
}

#[test]
fn duplicate_unknown_and_unregistered_tokens_are_rejected() {
    let mut registry = PaneTokenRegistry::default();
    registry
        .register("token", AgentTarget::new("window", "lane", "pane"))
        .unwrap();
    assert!(matches!(
        registry.register("token", AgentTarget::new("other", "other", "other")),
        Err(PaneTokenError::DuplicateToken)
    ));
    assert!(matches!(
        registry.authenticate("wrong", idle_event()),
        Err(PaneTokenError::InvalidToken)
    ));
    assert!(registry.unregister("token"));
    assert!(matches!(
        registry.authenticate("token", idle_event()),
        Err(PaneTokenError::InvalidToken)
    ));
}
