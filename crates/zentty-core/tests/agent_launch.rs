use std::collections::BTreeMap;
use zentty_core::{
    AgentLaunchTool, build_agent_launch_plan, build_copilot_config, build_cursor_hooks,
    build_gemini_settings, build_small_harness_hooks, sanitize_amp_resume_arguments,
};

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
fn amp_resume_arguments_match_the_source_allow_drop_and_reject_policy() {
    let strings = |values: &[&str]| {
        values
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>()
    };
    for (input, expected) in [
        (
            vec![
                "amp",
                "--mode",
                "smart",
                "--effort=high",
                "--settings-file",
                "/tmp/amp settings.json",
            ],
            Some(vec![
                "--mode",
                "smart",
                "--effort=high",
                "--settings-file",
                "/tmp/amp settings.json",
            ]),
        ),
        (
            vec![
                "threads",
                "continue",
                "T-old",
                "-m",
                "rush",
                "--visibility",
                "private",
            ],
            Some(vec!["-m", "rush", "--visibility", "private"]),
        ),
        (
            vec!["t", "c", "--log-level=debug", "--mcp-config", "mcp.json"],
            Some(vec!["--log-level=debug", "--mcp-config", "mcp.json"]),
        ),
        (
            vec![
                "--label",
                "old",
                "-l",
                "discard",
                "--archive",
                "--json",
                "--output-format",
                "json",
                "--log-file",
                "amp.log",
            ],
            Some(vec!["--log-file", "amp.log"]),
        ),
        (vec!["--mode"], Some(vec![])),
        (
            vec!["--mode", "safe", "hostile; touch /tmp/no"],
            Some(vec!["--mode", "safe"]),
        ),
    ] {
        assert_eq!(
            sanitize_amp_resume_arguments(&strings(&input)),
            expected.map(|values| strings(&values)),
            "input={input:?}"
        );
    }
    for input in [
        vec!["login"],
        vec!["amp", "permissions"],
        vec!["--execute", "echo hi"],
        vec!["--print=secret"],
        vec!["-x"],
        vec!["--help"],
        vec!["-V"],
        vec!["--jetbrains"],
    ] {
        assert_eq!(
            sanitize_amp_resume_arguments(&strings(&input)),
            None,
            "input={input:?}"
        );
    }
}

#[test]
fn amp_plan_activates_the_owned_plugin_and_publishes_ordered_launch_identity() {
    let arguments = [
        "amp".to_owned(),
        "threads".to_owned(),
        "continue".to_owned(),
        "T-old".to_owned(),
        "--mode".to_owned(),
        "smart".to_owned(),
        "--label".to_owned(),
        "discard".to_owned(),
    ];
    let plan = build_agent_launch_plan(
        AgentLaunchTool::Amp,
        "/real/amp",
        &arguments,
        "/stage/bin/zentty",
        SESSION_ID,
        &pane_environment(),
    )
    .unwrap();

    assert_eq!(plan.arguments, arguments);
    assert_eq!(plan.set_environment["ZENTTY_AGENT_TOOL"], "amp");
    assert_eq!(plan.set_environment["PLUGINS"], "all");
    assert_eq!(
        serde_json::from_str::<Vec<String>>(
            &plan.set_environment["ZENTTY_AMP_RESUME_ARGUMENTS_JSON"]
        )
        .unwrap(),
        ["--mode", "smart"]
    );
    assert_eq!(plan.pre_launch_actions.len(), 2);
    for (action, event) in plan
        .pre_launch_actions
        .iter()
        .zip(["session.start", "agent.running"])
    {
        let rendered = action.standard_input.replace("__ZENTTY_SELF_PID__", "4242");
        let payload: serde_json::Value = serde_json::from_str(&rendered).unwrap();
        assert_eq!(payload["version"], 1);
        assert_eq!(payload["event"], event);
        assert_eq!(payload["agent"]["name"], "Amp");
        assert_eq!(payload["agent"]["pid"], 4242);
        assert_eq!(
            payload["context"]["launch"]["arguments"],
            serde_json::json!(["--mode", "smart"])
        );
    }

    let rejected_snapshot = build_agent_launch_plan(
        AgentLaunchTool::Amp,
        "/real/amp",
        &["--execute=echo hi".to_owned()],
        "/stage/bin/zentty",
        SESSION_ID,
        &pane_environment(),
    )
    .unwrap();
    assert!(
        !rejected_snapshot
            .set_environment
            .contains_key("ZENTTY_AMP_RESUME_ARGUMENTS_JSON")
    );
    assert!(
        rejected_snapshot
            .pre_launch_actions
            .iter()
            .all(|action| action.standard_input.contains("\"arguments\":[]"))
    );
}

#[test]
fn cursor_plan_selects_the_private_overlay_and_preserves_original_arguments() {
    let mut environment = pane_environment();
    environment.insert(
        "ZENTTY_CURSOR_CONFIG_OVERLAY".to_owned(),
        "/runtime/agent-overlays/cursor-private/.cursor".to_owned(),
    );
    let arguments = ["--model".to_owned(), "cursor-fast".to_owned()];
    let plan = build_agent_launch_plan(
        AgentLaunchTool::Cursor,
        "/real/cursor-agent",
        &arguments,
        "/stage/bin/zentty",
        SESSION_ID,
        &environment,
    )
    .unwrap();

    assert_eq!(plan.arguments, arguments);
    assert_eq!(plan.set_environment["ZENTTY_AGENT_TOOL"], "cursor");
    assert_eq!(
        plan.set_environment["CURSOR_CONFIG_DIR"],
        "/runtime/agent-overlays/cursor-private/.cursor"
    );
    assert!(plan.pre_launch_actions.is_empty());
}

#[test]
fn cursor_hooks_are_complete_shell_safe_and_todo_scoped() {
    let hooks: serde_json::Value =
        serde_json::from_slice(&build_cursor_hooks("/stage/bin/zentty;$(must-not-run)").unwrap())
            .unwrap();
    assert_eq!(hooks["version"], 1);
    let groups = hooks["hooks"].as_object().unwrap();
    assert_eq!(groups.len(), 10);
    for event in [
        "sessionStart",
        "sessionEnd",
        "beforeSubmitPrompt",
        "stop",
        "beforeShellExecution",
        "afterShellExecution",
        "subagentStart",
        "subagentStop",
    ] {
        assert_eq!(groups[event].as_array().unwrap().len(), 1, "event={event}");
        assert!(groups[event][0].get("matcher").is_none(), "event={event}");
    }
    for event in ["preToolUse", "postToolUse"] {
        assert_eq!(groups[event][0]["matcher"], "TodoWrite");
    }
    let command = groups["sessionStart"][0]["command"].as_str().unwrap();
    assert_eq!(
        command,
        r#""/stage/bin/zentty;\$(must-not-run)" ipc agent-event --adapter=cursor"#
    );
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

#[test]
fn remaining_source_tools_publish_exact_names_and_no_persistent_install_target() {
    for (input, tool, binary) in [
        ("copilot", AgentLaunchTool::Copilot, "copilot"),
        ("opencode", AgentLaunchTool::OpenCode, "opencode"),
        ("pi", AgentLaunchTool::Pi, "pi"),
        ("omp", AgentLaunchTool::Omp, "omp"),
        (
            "small-harness",
            AgentLaunchTool::SmallHarness,
            "small-harness",
        ),
    ] {
        assert_eq!(AgentLaunchTool::parse(input).unwrap(), tool);
        assert_eq!(tool.binary_name(), binary);
        assert_eq!(tool.binary_names(), &[binary]);
        assert_eq!(tool.persistent_integration_target(), None);
    }
}

#[test]
fn remaining_source_tools_are_direct_outside_a_routed_pane() {
    for tool in [
        AgentLaunchTool::Copilot,
        AgentLaunchTool::OpenCode,
        AgentLaunchTool::Pi,
        AgentLaunchTool::Omp,
        AgentLaunchTool::SmallHarness,
    ] {
        let arguments = vec!["--literal".to_owned(), "hostile path;$HOME".to_owned()];
        let environment = match tool {
            AgentLaunchTool::Copilot => BTreeMap::from([(
                "ZENTTY_COPILOT_HOME_OVERLAY".to_owned(),
                "/private/copilot".to_owned(),
            )]),
            AgentLaunchTool::Pi => {
                BTreeMap::from([("ZENTTY_PI_EXTENSION".to_owned(), "/stage/pi.js".to_owned())])
            }
            AgentLaunchTool::Omp => BTreeMap::from([(
                "ZENTTY_OMP_EXTENSION".to_owned(),
                "/stage/omp.js".to_owned(),
            )]),
            AgentLaunchTool::SmallHarness => BTreeMap::from([(
                "ZENTTY_SMALL_HARNESS_HOOKS_FILE".to_owned(),
                "/private/small-hooks.json".to_owned(),
            )]),
            AgentLaunchTool::OpenCode => BTreeMap::new(),
            _ => unreachable!(),
        };
        let plan = build_agent_launch_plan(
            tool,
            format!("/real/{}", tool.binary_name()),
            &arguments,
            "/stage/bin/zentty",
            SESSION_ID,
            &environment,
        )
        .unwrap();
        assert_eq!(plan.arguments, arguments, "tool={tool:?}");
        assert!(plan.set_environment.is_empty(), "tool={tool:?}");
        assert!(plan.pre_launch_actions.is_empty(), "tool={tool:?}");
    }
}

#[test]
fn pi_family_management_and_early_exit_arguments_never_load_extensions() {
    for (tool, arguments) in [
        (AgentLaunchTool::Pi, vec!["install"]),
        (AgentLaunchTool::Pi, vec!["--version=full"]),
        (AgentLaunchTool::Pi, vec!["--profile", "work", "config"]),
        (AgentLaunchTool::Omp, vec!["plugin", "list"]),
        (AgentLaunchTool::Omp, vec!["--cwd=/hostile path", "models"]),
        (AgentLaunchTool::Omp, vec!["--alias=value"]),
    ] {
        let arguments = arguments.into_iter().map(str::to_owned).collect::<Vec<_>>();
        let mut environment = pane_environment();
        let extension_key = match tool {
            AgentLaunchTool::Pi => "ZENTTY_PI_EXTENSION",
            AgentLaunchTool::Omp => "ZENTTY_OMP_EXTENSION",
            _ => unreachable!(),
        };
        environment.insert(extension_key.to_owned(), "/stage/extension.js".to_owned());
        let plan = build_agent_launch_plan(
            tool,
            format!("/real/{}", tool.binary_name()),
            &arguments,
            "/stage/bin/zentty",
            SESSION_ID,
            &environment,
        )
        .unwrap();
        assert_eq!(plan.arguments, arguments, "tool={tool:?}");
        assert!(plan.set_environment.is_empty(), "tool={tool:?}");
        assert!(plan.pre_launch_actions.is_empty(), "tool={tool:?}");
    }
}

#[test]
fn pi_family_managed_plans_prepend_only_the_matching_staged_extension() {
    for (tool, canonical, variable, extension) in [
        (
            AgentLaunchTool::Pi,
            "Pi",
            "ZENTTY_PI_EXTENSION",
            "/stage/share/zentty/pi/extensions/zentty-pi-zentty.js",
        ),
        (
            AgentLaunchTool::Omp,
            "OMP",
            "ZENTTY_OMP_EXTENSION",
            "/stage/share/zentty/omp/extensions/zentty-omp-zentty.js",
        ),
    ] {
        let mut environment = pane_environment();
        environment.insert(variable.to_owned(), extension.to_owned());
        let plan = build_agent_launch_plan(
            tool,
            format!("/real/{}", tool.binary_name()),
            &["--prompt".to_owned(), "hostile path;$HOME".to_owned()],
            "/stage/bin/zentty",
            SESSION_ID,
            &environment,
        )
        .unwrap();
        assert_eq!(&plan.arguments[..2], ["-e", extension]);
        assert_eq!(&plan.arguments[2..], ["--prompt", "hostile path;$HOME"]);
        assert_eq!(
            plan.set_environment["ZENTTY_AGENT_TOOL"],
            tool.binary_name()
        );
        assert_eq!(
            plan.set_environment["ZENTTY_AGENT_CANONICAL_NAME"],
            canonical
        );
        assert_eq!(plan.pre_launch_actions.len(), 1);
        assert!(
            plan.pre_launch_actions[0]
                .standard_input
                .contains(canonical)
        );
        assert!(
            plan.pre_launch_actions[0]
                .standard_input
                .contains("__ZENTTY_SELF_PID__")
        );
    }
}

#[test]
fn small_harness_uses_only_the_ephemeral_managed_hook_file_and_clears_stale_inline_state() {
    let mut environment = pane_environment();
    environment.insert(
        "ZENTTY_SMALL_HARNESS_HOOKS_FILE".to_owned(),
        "/run/zentty/launch/lane/pane/small-harness/managed-hooks.json".to_owned(),
    );
    let plan = build_agent_launch_plan(
        AgentLaunchTool::SmallHarness,
        "/real/small-harness",
        &["--continue".to_owned()],
        "/stage/bin/zentty",
        SESSION_ID,
        &environment,
    )
    .unwrap();
    assert_eq!(plan.arguments, ["--continue"]);
    assert_eq!(plan.set_environment["ZENTTY_AGENT_TOOL"], "small-harness");
    assert_eq!(
        plan.set_environment["SMALL_HARNESS_MANAGED_HOOKS_FILE"],
        "/run/zentty/launch/lane/pane/small-harness/managed-hooks.json"
    );
    assert_eq!(plan.unset_environment, ["SMALL_HARNESS_MANAGED_HOOKS_JSON"]);

    for arguments in [vec!["completions"], vec!["--help=plain"]] {
        let arguments = arguments.into_iter().map(str::to_owned).collect::<Vec<_>>();
        let direct = build_agent_launch_plan(
            AgentLaunchTool::SmallHarness,
            "/real/small-harness",
            &arguments,
            "/stage/bin/zentty",
            SESSION_ID,
            &environment,
        )
        .unwrap();
        assert!(direct.set_environment.is_empty());
        assert_eq!(
            direct.unset_environment,
            [
                "SMALL_HARNESS_MANAGED_HOOKS_FILE",
                "SMALL_HARNESS_MANAGED_HOOKS_JSON"
            ]
        );
    }
}

#[test]
fn copilot_plan_consumes_config_override_and_selects_private_overlay() {
    let mut environment = pane_environment();
    environment.insert(
        "ZENTTY_COPILOT_HOME_OVERLAY".to_owned(),
        "/run/zentty/launch/lane/pane/copilot/home".to_owned(),
    );
    let plan = build_agent_launch_plan(
        AgentLaunchTool::Copilot,
        "/real/copilot",
        &[
            "--config-dir".to_owned(),
            "/hostile config;$HOME".to_owned(),
            "--resume=session-safe".to_owned(),
        ],
        "/stage/bin/zentty",
        SESSION_ID,
        &environment,
    )
    .unwrap();
    assert_eq!(plan.arguments, ["--resume=session-safe"]);
    assert_eq!(plan.set_environment["ZENTTY_AGENT_TOOL"], "copilot");
    assert_eq!(
        plan.set_environment["COPILOT_HOME"],
        "/run/zentty/launch/lane/pane/copilot/home"
    );
}

#[test]
fn opencode_plan_selects_overlay_and_emits_one_session_start_before_exec() {
    let mut environment = pane_environment();
    environment.insert(
        "ZENTTY_OPENCODE_CONFIG_OVERLAY".to_owned(),
        "/run/zentty/launch/lane/pane/opencode/config".to_owned(),
    );
    environment.insert(
        "ZENTTY_OPENCODE_BASE_CONFIG_DIR".to_owned(),
        "/home/user/.config/opencode".to_owned(),
    );
    let plan = build_agent_launch_plan(
        AgentLaunchTool::OpenCode,
        "/real/.opencode",
        &["--session".to_owned(), "session-safe".to_owned()],
        "/stage/bin/zentty",
        SESSION_ID,
        &environment,
    )
    .unwrap();
    assert_eq!(plan.executable_path, "/real/.opencode");
    assert_eq!(plan.arguments, ["--session", "session-safe"]);
    assert_eq!(plan.set_environment["ZENTTY_AGENT_TOOL"], "opencode");
    assert_eq!(
        plan.set_environment["OPENCODE_CONFIG_DIR"],
        "/run/zentty/launch/lane/pane/opencode/config"
    );
    assert_eq!(plan.pre_launch_actions.len(), 1);
    assert!(
        plan.pre_launch_actions[0]
            .standard_input
            .contains("OpenCode")
    );
}

#[test]
fn opencode_without_a_staged_plugin_still_emits_source_prelaunch_status() {
    let plan = build_agent_launch_plan(
        AgentLaunchTool::OpenCode,
        "/real/opencode",
        &["run".to_owned()],
        "/stage/bin/zentty",
        SESSION_ID,
        &pane_environment(),
    )
    .unwrap();
    assert_eq!(plan.set_environment["ZENTTY_AGENT_TOOL"], "opencode");
    assert!(!plan.set_environment.contains_key("OPENCODE_CONFIG_DIR"));
    assert_eq!(plan.pre_launch_actions.len(), 1);
}

#[test]
fn copilot_config_merge_preserves_user_fields_and_adds_each_hook_once() {
    let existing = br#"{
        // user comment
        "model": "gpt-5",
        "hooks": {"sessionStart": [{"type":"command","bash":"mine","timeoutSec":7},],},
    }"#;
    let merged = build_copilot_config(Some(existing), "/stage/Zentty $CLI `beta`").unwrap();
    let value: serde_json::Value = serde_json::from_slice(&merged).unwrap();
    assert_eq!(value["model"], "gpt-5");
    assert_eq!(value["version"], 1);
    for (name, timeout) in [
        ("sessionStart", 10),
        ("sessionEnd", 10),
        ("userPromptSubmitted", 10),
        ("preToolUse", 5),
        ("postToolUse", 5),
        ("errorOccurred", 10),
    ] {
        let hooks = value["hooks"][name].as_array().unwrap();
        assert_eq!(
            hooks
                .iter()
                .filter(|hook| hook["bash"]
                    .as_str()
                    .is_some_and(|command| command.contains("--adapter=copilot")))
                .count(),
            1,
            "{name}"
        );
        let ours = hooks.last().unwrap();
        assert_eq!(ours["type"], "command");
        assert_eq!(ours["timeoutSec"], timeout);
    }
    let again = build_copilot_config(Some(&merged), "/stage/Zentty $CLI `beta`").unwrap();
    assert_eq!(merged, again);

    let replaced = build_copilot_config(
        Some(br#"{"hooks":{"sessionStart":"invalid"}}"#),
        "/stage/bin/zentty",
    )
    .unwrap();
    let replaced: serde_json::Value = serde_json::from_slice(&replaced).unwrap();
    assert!(replaced["hooks"]["sessionStart"].is_array());
    assert!(build_copilot_config(Some(b"not-json"), "/stage/bin/zentty").is_err());
}

#[test]
fn small_harness_hooks_are_complete_bounded_and_idempotently_serialized() {
    let hooks = build_small_harness_hooks("/stage/Zentty $CLI `beta`").unwrap();
    let value: serde_json::Value = serde_json::from_slice(&hooks).unwrap();
    assert_eq!(value["source"], "zentty");
    assert_eq!(value["hooks"].as_object().unwrap().len(), 12);
    for event in [
        "SessionStart",
        "UserPromptSubmit",
        "PreToolUse",
        "PermissionRequest",
        "PostToolUse",
        "PreCompact",
        "PostCompact",
        "PlanUpdated",
        "SubagentStart",
        "SubagentStop",
        "Stop",
        "SessionEnd",
    ] {
        let hook = &value["hooks"][event][0]["hooks"][0];
        assert_eq!(hook["type"], "command");
        assert!(
            hook["command"]
                .as_str()
                .unwrap()
                .contains("--adapter=small-harness")
        );
        assert_eq!(
            hook["timeoutSec"],
            if event == "SessionEnd" { 1 } else { 10 }
        );
        assert_eq!(hook["envVars"].as_array().unwrap().len(), 7);
    }
}
