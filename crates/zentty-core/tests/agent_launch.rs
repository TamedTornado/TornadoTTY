use std::collections::BTreeMap;
use zentty_core::{AgentLaunchTool, build_agent_launch_plan};

const SESSION_ID: &str = "12345678-1234-4234-8234-123456789abc";

#[test]
fn claude_plan_injects_ephemeral_source_hooks_without_writing_config() {
    let plan = build_agent_launch_plan(
        AgentLaunchTool::Claude,
        "/real/claude",
        &["hello".to_owned()],
        "/stage/bin/zentty",
        SESSION_ID,
        &BTreeMap::new(),
    )
    .unwrap();
    assert_eq!(plan.executable_path, "/real/claude");
    assert_eq!(&plan.arguments[..2], ["--session-id", SESSION_ID]);
    let settings_index = plan
        .arguments
        .iter()
        .position(|value| value == "--settings")
        .unwrap();
    let settings: serde_json::Value =
        serde_json::from_str(&plan.arguments[settings_index + 1]).unwrap();
    assert_eq!(
        settings["hooks"]["SessionStart"].as_array().unwrap().len(),
        4
    );
    assert_eq!(settings["hooks"]["PreToolUse"].as_array().unwrap().len(), 2);
    assert!(plan.arguments[settings_index + 1].contains("ipc agent-event --adapter=claude"));
    assert_eq!(plan.arguments.last().unwrap(), "hello");
    assert_eq!(plan.set_environment["ZENTTY_AGENT_TOOL"], "claude");
    assert_eq!(plan.unset_environment, ["CLAUDECODE"]);
}

#[test]
fn claude_resume_preserves_session_and_explicit_color_policy() {
    let environment = BTreeMap::from([
        ("FORCE_COLOR".to_owned(), "1".to_owned()),
        ("COLORTERM".to_owned(), "24bit".to_owned()),
    ]);
    let plan = build_agent_launch_plan(
        AgentLaunchTool::Claude,
        "/real/claude",
        &["--resume=session".to_owned()],
        "/stage/bin/zentty",
        SESSION_ID,
        &environment,
    )
    .unwrap();
    assert!(!plan.arguments.iter().any(|value| value == "--session-id"));
    assert!(!plan.set_environment.contains_key("FORCE_COLOR"));
    assert!(!plan.set_environment.contains_key("COLORTERM"));
}

#[test]
fn codex_plan_injects_all_source_hook_events_and_trust_state() {
    let plan = build_agent_launch_plan(
        AgentLaunchTool::Codex,
        "/real/codex",
        &["--help".to_owned()],
        "/stage/bin/zentty",
        "unused",
        &BTreeMap::new(),
    )
    .unwrap();
    for event in [
        "SessionStart",
        "PreToolUse",
        "PermissionRequest",
        "PostToolUse",
        "UserPromptSubmit",
        "PreCompact",
        "PostCompact",
        "Stop",
    ] {
        assert!(
            plan.arguments
                .iter()
                .any(|value| value.starts_with(&format!("hooks.{event}=")))
        );
        let hook = plan
            .arguments
            .iter()
            .find(|value| value.starts_with(&format!("hooks.{event}=")))
            .unwrap();
        assert!(
            hook.contains("|| true; echo '{}'"),
            "Codex requires every successful hook to return a JSON object: {hook}"
        );
    }
    let state = plan
        .arguments
        .iter()
        .find(|value| value.starts_with("hooks.state="))
        .unwrap();
    assert_eq!(state.matches("trusted_hash=\"sha256:").count(), 8);
    assert!(
        plan.arguments
            .iter()
            .any(|value| value == "features.hooks=true")
    );
    assert_eq!(plan.arguments.last().unwrap(), "--help");
}

#[test]
fn codex_subcommands_receive_session_config_in_their_own_argument_scope() {
    for arguments in [
        vec!["exec".to_owned(), "prompt".to_owned()],
        vec!["resume".to_owned(), "session-a".to_owned()],
    ] {
        let plan = build_agent_launch_plan(
            AgentLaunchTool::Codex,
            "/real/codex",
            &arguments,
            "/stage/bin/zentty",
            "unused",
            &BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(plan.arguments[0], arguments[0]);
        assert_eq!(plan.arguments[1], "-c");
        assert_eq!(plan.arguments[2], "features.hooks=true");
        assert_eq!(plan.arguments.last(), arguments.last());
    }
}

#[test]
fn disabled_and_management_launches_are_direct_passthroughs() {
    let disabled = BTreeMap::from([("ZENTTY_CODEX_HOOKS_DISABLED".to_owned(), "1".to_owned())]);
    let codex = build_agent_launch_plan(
        AgentLaunchTool::Codex,
        "/real/codex",
        &["--version".to_owned()],
        "/stage/bin/zentty",
        "unused",
        &disabled,
    )
    .unwrap();
    assert_eq!(codex.arguments, ["--version"]);
    assert!(codex.set_environment.is_empty());

    let claude = build_agent_launch_plan(
        AgentLaunchTool::Claude,
        "/real/claude",
        &["config".to_owned(), "get".to_owned()],
        "/stage/bin/zentty",
        SESSION_ID,
        &BTreeMap::new(),
    )
    .unwrap();
    assert_eq!(claude.arguments, ["config", "get"]);
    assert_eq!(claude.unset_environment, ["CLAUDECODE"]);
}
