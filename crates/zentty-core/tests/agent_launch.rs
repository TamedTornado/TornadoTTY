use std::collections::BTreeMap;
use zentty_core::{AgentLaunchTool, build_agent_launch_plan, build_gemini_settings};

const SESSION_ID: &str = "12345678-1234-4234-8234-123456789abc";

fn pane_environment() -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "ZENTTY_INSTANCE_SOCKET".to_owned(),
            "/run/zentty.sock".to_owned(),
        ),
        ("ZENTTY_PANE_TOKEN".to_owned(), "token".to_owned()),
        ("ZENTTY_WORKLANE_ID".to_owned(), "lane".to_owned()),
        ("ZENTTY_PANE_ID".to_owned(), "pane".to_owned()),
    ])
}

#[test]
fn persistent_source_agents_preserve_arguments_and_publish_exact_launch_policy() {
    for (tool, input, binary, target, pid_name) in [
        (
            AgentLaunchTool::Amp,
            "amp",
            "amp",
            "amp-hooks",
            "ZENTTY_AMP_PID",
        ),
        (
            AgentLaunchTool::Cursor,
            "cursor-agent",
            "cursor-agent",
            "cursor-hooks",
            "ZENTTY_CURSOR_PID",
        ),
        (
            AgentLaunchTool::Droid,
            "droid",
            "droid",
            "droid-hooks",
            "ZENTTY_DROID_PID",
        ),
        (
            AgentLaunchTool::Kimi,
            "kimi-cli",
            "kimi",
            "kimi-hooks",
            "ZENTTY_KIMI_PID",
        ),
        (
            AgentLaunchTool::Grok,
            "grok",
            "grok",
            "grok-hooks",
            "ZENTTY_GROK_PID",
        ),
        (
            AgentLaunchTool::Agy,
            "agy",
            "agy",
            "agy-hooks",
            "ZENTTY_AGY_PID",
        ),
        (
            AgentLaunchTool::Hermes,
            "hermes",
            "hermes",
            "hermes-hooks",
            "ZENTTY_HERMES_PID",
        ),
        (
            AgentLaunchTool::Vibe,
            "mistral-vibe",
            "vibe",
            "vibe-hooks",
            "ZENTTY_VIBE_PID",
        ),
    ] {
        assert_eq!(AgentLaunchTool::parse(input).unwrap(), tool);
        assert_eq!(tool.binary_name(), binary);
        assert_eq!(tool.persistent_integration_target(), Some(target));
        assert!(pid_name.starts_with("ZENTTY_") && pid_name.ends_with("_PID"));
        let plan = build_agent_launch_plan(
            tool,
            format!("/real/{binary}"),
            &["--project".to_owned(), "hostile path".to_owned()],
            "/stage/bin/zentty",
            SESSION_ID,
            &pane_environment(),
        )
        .unwrap();
        assert_eq!(plan.arguments, ["--project", "hostile path"]);
        assert!(!plan.set_environment.is_empty());
    }
    let vibe = build_agent_launch_plan(
        AgentLaunchTool::Vibe,
        "/real/vibe",
        &[],
        "/stage/bin/zentty",
        SESSION_ID,
        &pane_environment(),
    )
    .unwrap();
    assert_eq!(
        vibe.set_environment["VIBE_ENABLE_EXPERIMENTAL_HOOKS"],
        "true"
    );
}

#[test]
fn persistent_agent_management_commands_are_direct_but_grok_flags_remain_integrated() {
    for (tool, argument) in [
        (AgentLaunchTool::Amp, "permissions"),
        (AgentLaunchTool::Kimi, "plugin"),
        (AgentLaunchTool::Agy, "update"),
        (AgentLaunchTool::Hermes, "config"),
        (AgentLaunchTool::Vibe, "setup"),
    ] {
        let plan = build_agent_launch_plan(
            tool,
            format!("/real/{}", tool.binary_name()),
            &[argument.to_owned()],
            "/stage/bin/zentty",
            SESSION_ID,
            &pane_environment(),
        )
        .unwrap();
        assert!(plan.set_environment.is_empty(), "tool={tool:?}");
    }
    let grok = build_agent_launch_plan(
        AgentLaunchTool::Grok,
        "/real/grok",
        &["--help".to_owned()],
        "/stage/bin/zentty",
        SESSION_ID,
        &pane_environment(),
    )
    .unwrap();
    assert_eq!(grok.set_environment["ZENTTY_AGENT_TOOL"], "grok");
}

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
    assert!(
        plan.arguments
            .iter()
            .any(|value| value == r#"notify=["/stage/bin/zentty","codex-notify"]"#)
    );
}

#[test]
fn codex_notify_policy_preserves_user_overrides_and_only_unsets_managed_homes() {
    for arguments in [
        vec!["-c".to_owned(), "notify=[\"mine\"]".to_owned()],
        vec!["--config".to_owned(), "notify=[\"mine\"]".to_owned()],
        vec!["-cnotify=[\"mine\"]".to_owned()],
        vec!["--config=notify=[\"mine\"]".to_owned()],
    ] {
        let overridden = build_agent_launch_plan(
            AgentLaunchTool::Codex,
            "/real/codex",
            &arguments,
            "/stage/bin/zentty",
            "unused",
            &BTreeMap::from([(
                "CODEX_HOME".to_owned(),
                "/tmp/zentty-runtime/launch/worklane/pane/codex/home".to_owned(),
            )]),
        )
        .unwrap();
        assert_eq!(
            overridden
                .arguments
                .iter()
                .filter(|value| value.starts_with("notify="))
                .count(),
            usize::from(arguments.len() == 2),
            "{arguments:?}"
        );
        assert_eq!(overridden.unset_environment, ["CODEX_HOME"]);
    }

    let direct_runtime_home = build_agent_launch_plan(
        AgentLaunchTool::Codex,
        "/real/codex",
        &["--help".to_owned()],
        "/stage/bin/zentty",
        "unused",
        &BTreeMap::from([(
            "CODEX_HOME".to_owned(),
            "/run/user/1000/zentty/launch/lane/pane/codex/home".to_owned(),
        )]),
    )
    .unwrap();
    assert_eq!(direct_runtime_home.unset_environment, ["CODEX_HOME"]);

    let disabled = build_agent_launch_plan(
        AgentLaunchTool::Codex,
        "/real/codex",
        &["--help".to_owned()],
        "/stage/bin/zentty",
        "unused",
        &BTreeMap::from([
            ("ZENTTY_CODEX_NOTIFY_DISABLED".to_owned(), "1".to_owned()),
            ("CODEX_HOME".to_owned(), "/home/user/.codex".to_owned()),
        ]),
    )
    .unwrap();
    assert!(
        !disabled
            .arguments
            .iter()
            .any(|value| value.starts_with("notify="))
    );
    assert!(disabled.unset_environment.is_empty());

    let lookalike_home = build_agent_launch_plan(
        AgentLaunchTool::Codex,
        "/real/codex",
        &["--help".to_owned()],
        "/stage/bin/zentty",
        "unused",
        &BTreeMap::from([(
            "CODEX_HOME".to_owned(),
            "/tmp/zentty-runtime/not-a-launch/user-home".to_owned(),
        )]),
    )
    .unwrap();
    assert!(lookalike_home.unset_environment.is_empty());
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

#[test]
fn gemini_plan_preserves_arguments_and_selects_the_ephemeral_overlay() {
    let plan = build_agent_launch_plan(
        AgentLaunchTool::Gemini,
        "/real/gemini",
        &["--model".to_owned(), "gemini-2.5-pro".to_owned()],
        "/stage/bin/zentty",
        "unused",
        &BTreeMap::from([(
            "ZENTTY_GEMINI_SETTINGS_OVERLAY".to_owned(),
            "/runtime/pane/settings.json".to_owned(),
        )]),
    )
    .unwrap();
    assert_eq!(plan.arguments, ["--model", "gemini-2.5-pro"]);
    assert_eq!(plan.set_environment["ZENTTY_AGENT_TOOL"], "gemini");
    assert_eq!(
        plan.set_environment["GEMINI_CLI_SYSTEM_SETTINGS_PATH"],
        "/runtime/pane/settings.json"
    );
}

#[test]
fn gemini_settings_merge_source_values_and_append_each_exact_hook_once() {
    let existing = br#"{
        "theme": "Dracula",
        "general": {"enableNotifications": false, "vimMode": true},
        "hooks": {
            "SessionStart": [{"matcher":"user", "hooks":[{"type":"command","command":"existing", "timeout":7}]}]
        }
    }"#;
    let bytes = build_gemini_settings(Some(existing), "/stage/Zentty $CLI `beta`").unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["theme"], "Dracula");
    assert_eq!(value["general"]["vimMode"], true);
    assert_eq!(value["general"]["enableNotifications"], true);
    assert_eq!(value["hooks"]["SessionStart"].as_array().unwrap().len(), 2);
    for (event, timeout) in [
        ("SessionStart", 10_000),
        ("SessionEnd", 1_000),
        ("BeforeAgent", 10_000),
        ("AfterAgent", 10_000),
        ("Notification", 10_000),
        ("BeforeTool", 5_000),
    ] {
        let groups = value["hooks"][event].as_array().unwrap();
        let ours = groups
            .iter()
            .filter(|group| {
                group["hooks"][0]["command"]
                    .as_str()
                    .is_some_and(|command| command.contains("--adapter=gemini"))
            })
            .collect::<Vec<_>>();
        assert_eq!(ours.len(), 1, "{event}");
        assert_eq!(ours[0]["matcher"], "*");
        assert_eq!(ours[0]["hooks"][0]["timeout"], timeout);
        assert_eq!(ours[0]["hooks"][0]["type"], "command");
        assert_eq!(
            ours[0]["hooks"][0]["command"],
            r#""/stage/Zentty \$CLI \`beta\`" ipc agent-event --adapter=gemini || echo '{}'"#
        );
    }

    let merged_again = build_gemini_settings(Some(&bytes), "/stage/Zentty $CLI `beta`").unwrap();
    let twice: serde_json::Value = serde_json::from_slice(&merged_again).unwrap();
    assert_eq!(
        twice["hooks"]["SessionStart"].as_array().unwrap().len(),
        2,
        "rebuilding an overlay must not duplicate Zentty's hook"
    );
}

#[test]
fn gemini_settings_reject_non_object_or_malformed_sources() {
    assert!(build_gemini_settings(Some(b"[]"), "/stage/bin/zentty").is_err());
    assert!(build_gemini_settings(Some(b"not-json"), "/stage/bin/zentty").is_err());
}

#[test]
fn gemini_opt_out_is_a_direct_passthrough() {
    let plan = build_agent_launch_plan(
        AgentLaunchTool::Gemini,
        "/real/gemini",
        &["--version".to_owned()],
        "/stage/bin/zentty",
        "unused",
        &BTreeMap::from([("ZENTTY_GEMINI_HOOKS_DISABLED".to_owned(), "1".to_owned())]),
    )
    .unwrap();
    assert_eq!(plan.arguments, ["--version"]);
    assert!(plan.set_environment.is_empty());
}
