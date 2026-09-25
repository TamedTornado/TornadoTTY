use zentty_core::{NotificationCapability, codex_notification_capability};

fn known() -> Vec<String> {
    [
        "-c",
        "tui.notification_method=osc9",
        "-c",
        "tui.notifications=['approval-requested','agent-turn-complete']",
        "-c",
        "tui.notification_condition='always'",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

#[test]
fn only_explicit_unambiguous_tui_contract_is_recognized() {
    for kinds in [
        "['other','agent-turn-complete']",
        "['approval-requested','other']",
        "['approval-requested','approval-requested']",
        "['agent-turn-complete','agent-turn-complete']",
    ] {
        let mut args = known();
        args.extend(["-c".into(), format!("tui.notifications={kinds}")]);
        assert_eq!(
            codex_notification_capability(&args),
            NotificationCapability::Unknown
        );
    }
    for prefix in ["-c", "--config="] {
        let packed = known()
            .chunks_exact(2)
            .map(|pair| format!("{prefix}{}", pair[1]))
            .collect::<Vec<_>>();
        assert_eq!(
            codex_notification_capability(&packed),
            NotificationCapability::CodexTuiAttentionV1
        );
    }
    assert_eq!(
        codex_notification_capability(&known()),
        NotificationCapability::CodexTuiAttentionV1
    );
    for extra in [
        "tui.notifications=true",
        "tui.notifications=['agent-turn-complete']",
        "tui.notification_method='bel'",
        "tui.notification_condition='unfocused'",
        "tui={notifications=true}",
        "tui.notifications=[broken",
        "tui.notifications=['approval-requested','agent-turn-complete','other']",
    ] {
        let mut args = known();
        args.extend(["--config".into(), extra.into()]);
        assert_eq!(
            codex_notification_capability(&args),
            NotificationCapability::Unknown,
            "{extra}"
        );
    }
    assert_eq!(
        codex_notification_capability(&[]),
        NotificationCapability::Unknown
    );
    assert_eq!(
        codex_notification_capability(&known()[..4]),
        NotificationCapability::Unknown
    );
    let mut args = known();
    args.extend(["--".into(), "-c".into(), "tui.notifications=true".into()]);
    assert_eq!(
        codex_notification_capability(&args),
        NotificationCapability::CodexTuiAttentionV1
    );
}

#[test]
fn real_launch_plan_and_conflicting_overrides_are_classified() {
    for args in [vec![], vec!["resume".into(), "session".into()]] {
        let plan = zentty_core::build_agent_launch_plan(
            zentty_core::AgentLaunchTool::Codex,
            "/bin/codex",
            &args,
            "/bin/tornadotty-cli",
            "launch",
            &Default::default(),
        )
        .unwrap();
        assert_eq!(
            codex_notification_capability(&plan.arguments),
            NotificationCapability::CodexTuiAttentionV1
        );
    }
    let plan = zentty_core::build_agent_launch_plan(
        zentty_core::AgentLaunchTool::Codex,
        "/bin/codex",
        &["--config=tui.notifications=true".into()],
        "/bin/tornadotty-cli",
        "launch",
        &Default::default(),
    )
    .unwrap();
    assert_eq!(
        codex_notification_capability(&plan.arguments),
        NotificationCapability::Unknown
    );
}
