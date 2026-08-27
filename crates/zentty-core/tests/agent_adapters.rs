use zentty_core::{
    AgentPhase, AgentStatusStore, AgentTarget, AuthenticatedAgentEvent, adapt_agy_hook,
    adapt_claude_hook, adapt_codex_hook, adapt_codex_notify, adapt_copilot_hook, adapt_cursor_hook,
    adapt_droid_hook, adapt_gemini_hook, adapt_grok_hook, adapt_hermes_hook, adapt_kimi_hook,
    adapt_small_harness_hook, adapt_vibe_hook,
};

fn reduce(events: Vec<zentty_core::AgentEvent>) -> zentty_core::PaneAgentStatus {
    let target = AgentTarget::new("window", "lane", "pane");
    let mut store = AgentStatusStore::default();
    for (index, event) in events.into_iter().enumerate() {
        store.apply(
            AuthenticatedAgentEvent {
                target: target.clone(),
                pane_token: "token".to_owned(),
                event,
            },
            u64::try_from(index).unwrap(),
        );
    }
    store.status_for(&target).unwrap().clone()
}

fn apply_small_harness_payload(
    store: &mut AgentStatusStore,
    target: &AgentTarget,
    payload: &str,
    now: &mut u64,
) {
    for event in adapt_small_harness_hook(payload.as_bytes(), None).unwrap() {
        store.apply(
            AuthenticatedAgentEvent {
                target: target.clone(),
                pane_token: "token".to_owned(),
                event,
            },
            *now,
        );
        *now += 1;
    }
}

#[test]
fn codex_hooks_map_source_lifecycle_and_approval_semantics() {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../Zentty/AppState/Agent/EventAdapters/CodexEventAdapter.swift"
    ));
    let start = reduce(
        adapt_codex_hook(
            br#"{"hook_event_name":"SessionStart","session_id":"codex-a","cwd":"/tmp"}"#,
            Some(4242),
        )
        .unwrap(),
    );
    assert_eq!(start.agent_name, "Codex");
    assert_eq!(start.phase, AgentPhase::Starting);
    assert_eq!(start.tracked_pid, Some(4242));
    assert_eq!(start.working_directory.as_deref(), Some("/tmp"));

    let approval = reduce(
        adapt_codex_hook(
            br#"{"hook_event_name":"PermissionRequest","session_id":"codex-a","tool_name":"shell","message":"Run command?"}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(approval.phase, AgentPhase::NeedsInput);
    assert!(approval.requires_attention());
    assert_eq!(approval.text.as_deref(), Some("Run command?"));

    let question = reduce(
        adapt_codex_hook(
            br#"{"hook_event_name":"PreToolUse","session_id":"codex-a","tool_name":"request_user_input","tool_args":"{\"questions\":[{\"question\":\"Which database?\",\"options\":[{\"label\":\"Postgres\"},{\"label\":\"SQLite\"}]}]}"}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(question.phase, AgentPhase::NeedsInput);
    assert_eq!(
        question.text.as_deref(),
        Some("Which database?\n[Postgres] [SQLite]")
    );

    let running = reduce(
        adapt_codex_hook(
            br#"{"hook_event_name":"PreToolUse","session_id":"codex-a","tool_name":"shell"}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(running.phase, AgentPhase::Running);
    for tool in ["AskUserQuestion", "ask-user", "request_user_input"] {
        let payload = serde_json::json!({
            "hook_event_name": "PreToolUse",
            "session_id": "codex-a",
            "tool_name": tool,
            "tool_input": {"question": format!("Question from {tool}?")},
        });
        let question = reduce(adapt_codex_hook(payload.to_string().as_bytes(), None).unwrap());
        assert_eq!(question.phase, AgentPhase::NeedsInput, "{tool}");
    }
    let stopped = reduce(
        adapt_codex_hook(
            br#"{"hook_event_name":"Stop","session_id":"codex-a"}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(stopped.phase, AgentPhase::Idle);

    assert!(SOURCE.contains("case \"PermissionRequest\""));
    assert!(SOURCE.contains("case \"PreCompact\""));
    assert!(SOURCE.contains("case \"PostCompact\""));
    assert!(SOURCE.contains("case \"Stop\""));
}

#[test]
fn codex_compaction_hooks_preserve_canonical_transition_identity() {
    for (hook, expected, text) in [
        ("PreCompact", "agent.compacting", Some("Compacting")),
        ("PostCompact", "agent.compacted", None),
    ] {
        let payload = serde_json::json!({
            "hook_event_name": hook,
            "session_id": "codex-a",
        });
        let events = adapt_codex_hook(payload.to_string().as_bytes(), None).unwrap();
        let status = reduce(events.clone());
        assert_eq!(status.phase, AgentPhase::Running, "{hook}");
        assert_eq!(status.text.as_deref(), text, "{hook}");
        assert_eq!(
            serde_json::to_value(&events[0]).unwrap()["event"],
            expected,
            "{hook}"
        );
    }
}

#[test]
fn codex_positional_aliases_each_preserve_their_source_transition() {
    for (alias, expected) in [
        ("session-start", "session.start"),
        ("pre-tool-use", "agent.running"),
        ("permission-request", "agent.needs-input"),
        ("post-tool-use", "agent.running"),
        ("prompt-submit", "agent.running"),
        ("pre-compact", "agent.compacting"),
        ("post-compact", "agent.compacted"),
        ("stop", "agent.idle"),
    ] {
        let payload = serde_json::json!({
            "hook_event_name": alias,
            "session_id": "codex-positional",
            "tool_name": "shell",
            "message": "Allow command?",
        });
        let events = adapt_codex_hook(payload.to_string().as_bytes(), None).unwrap();
        assert_eq!(events.len(), 1, "{alias}");
        assert_eq!(
            serde_json::to_value(&events[0]).unwrap()["event"],
            expected,
            "{alias}"
        );
    }
}

#[test]
fn small_harness_plan_subagent_and_session_end_events_preserve_source_bookkeeping() {
    assert!(
        adapt_codex_hook(
            br#"{"hook_event_name":"PlanUpdated","session_id":"codex-plan","progress":{"done":1,"total":2}}"#,
            None,
        )
        .unwrap()
        .is_empty(),
        "PlanUpdated belongs to the source Small Harness adapter, not Codex"
    );
    let zero_total = adapt_small_harness_hook(
        br#"{"hook_event_name":"PlanUpdated","session_id":"codex-plan","progress":{"done":0,"total":0}}"#,
        None,
    )
    .unwrap();
    assert_eq!(zero_total.len(), 1);
    assert_eq!(
        serde_json::to_value(&zero_total[0]).unwrap()["event"],
        "agent.running"
    );
    let target = AgentTarget::new("window", "lane", "pane");
    let mut store = AgentStatusStore::default();
    let mut now = 0;

    apply_small_harness_payload(
        &mut store,
        &target,
        r#"{"hook_event_name":"PlanUpdated","session_id":"codex-plan","progress":{"doneCount":2,"totalCount":5}}"#,
        &mut now,
    );
    assert_eq!(
        store.status_for(&target).unwrap().progress,
        Some(zentty_core::AgentProgress { done: 2, total: 5 })
    );
    apply_small_harness_payload(
        &mut store,
        &target,
        r#"{"hook_event_name":"SubagentStart","session_id":"codex-plan","subagent_id":"worker-a"}"#,
        &mut now,
    );
    apply_small_harness_payload(
        &mut store,
        &target,
        r#"{"hook_event_name":"SubagentStart","session_id":"codex-plan","subagent_id":"worker-a"}"#,
        &mut now,
    );
    assert_eq!(
        store.status_for(&target).unwrap().progress,
        Some(zentty_core::AgentProgress { done: 2, total: 5 })
    );
    apply_small_harness_payload(
        &mut store,
        &target,
        r#"{"hook_event_name":"SubagentStop","session_id":"codex-plan","subagent_id":"worker-a"}"#,
        &mut now,
    );
    apply_small_harness_payload(
        &mut store,
        &target,
        r#"{"hook_event_name":"SubagentStop","session_id":"codex-plan","subagent_id":"worker-a"}"#,
        &mut now,
    );
    assert_eq!(
        store.status_for(&target).unwrap().progress,
        Some(zentty_core::AgentProgress { done: 2, total: 5 })
    );
    apply_small_harness_payload(
        &mut store,
        &target,
        r#"{"hook_event_name":"SessionEnd","session_id":"codex-plan"}"#,
        &mut now,
    );
    assert!(store.status_for(&target).is_none());
}

#[test]
fn small_harness_and_droid_task_aliases_preserve_started_and_completed_identity() {
    for (hook, expected) in [
        ("SubagentStart", "task.started"),
        ("SubagentStop", "task.completed"),
    ] {
        let payload = serde_json::json!({
            "hook_event_name": hook,
            "session_id": "small-tasks",
            "subagent_id": "worker-a",
        });
        let events = adapt_small_harness_hook(payload.to_string().as_bytes(), None).unwrap();
        assert_eq!(events.len(), 2, "{hook}");
        assert_eq!(
            serde_json::to_value(&events[1]).unwrap()["event"],
            expected,
            "{hook}"
        );
    }

    for (hook, tool, expected) in [
        ("PreToolUse", "Task", "task.started"),
        ("SubagentStop", "Task", "task.completed"),
    ] {
        let payload = serde_json::json!({
            "hook_event_name": hook,
            "session_id": "droid-tasks",
            "tool_name": tool,
            "task_id": "worker-a",
        });
        let events = adapt_droid_hook(payload.to_string().as_bytes(), None).unwrap();
        assert_eq!(events.len(), 2, "{hook}");
        assert_eq!(
            serde_json::to_value(&events[1]).unwrap()["event"],
            expected,
            "{hook}"
        );
    }
}

#[test]
fn codex_notify_maps_turn_completion_and_human_interaction_without_auto_review_noise() {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../Zentty/AppState/Agent/EventAdapters/CodexNotifyEventAdapter.swift"
    ));
    let idle = reduce(
        adapt_codex_notify(br#"{"type":"agent-turn-complete","session_id":"codex-notify-a"}"#)
            .unwrap(),
    );
    assert_eq!(idle.phase, AgentPhase::Idle);

    let installed_payload = reduce(
        adapt_codex_notify(
            br#"{"type":"agent-turn-complete","thread-id":"b5f6c1c2-1111-2222-3333-444455556666","turn-id":"turn-1","last-assistant-message":"Done"}"#,
        )
        .unwrap(),
    );
    assert_eq!(
        installed_payload.session_id,
        "b5f6c1c2-1111-2222-3333-444455556666"
    );

    for (payload, expected_kind, expected_text) in [
        (
            br#"{"type":"permission-requested","session_id":"codex-notify-a","message":"Allow editing src/main.rs?"}"#.as_slice(),
            zentty_core::AgentInteractionKind::Approval,
            "Allow editing src/main.rs?",
        ),
        (
            br#"{"type":"question","session_id":"codex-notify-a","body":"Choose database?\n1. Postgres\n2. SQLite"}"#.as_slice(),
            zentty_core::AgentInteractionKind::Decision,
            "Choose database?\n1. Postgres\n2. SQLite",
        ),
        (
            br#"{"type":"auth","session_id":"codex-notify-a","title":"Please sign in to Codex"}"#.as_slice(),
            zentty_core::AgentInteractionKind::Auth,
            "Please sign in to Codex",
        ),
    ] {
        let status = reduce(adapt_codex_notify(payload).unwrap());
        assert_eq!(status.phase, AgentPhase::NeedsInput);
        assert_eq!(status.interaction, expected_kind);
        assert_eq!(status.text.as_deref(), Some(expected_text));
    }

    for message in [
        "Auto approval review started",
        "Automatic approval review started",
        "Auto reviewer started",
        "Auto review returned",
        "Guardian approval review returned",
    ] {
        let payload = serde_json::json!({"type": "permission", "message": message});
        assert!(
            adapt_codex_notify(payload.to_string().as_bytes())
                .unwrap()
                .is_empty(),
            "{message}"
        );
    }
    for message in [
        "Guardian approval",
        "Guardian review permission",
        "Approval review",
    ] {
        let payload = serde_json::json!({"type": "notice", "message": message});
        assert!(
            !adapt_codex_notify(payload.to_string().as_bytes())
                .unwrap()
                .is_empty(),
            "{message} must not be mistaken for the three-part guardian lifecycle"
        );
    }
    assert!(
        adapt_codex_notify(br#"{"type":"informational","message":"Build finished"}"#)
            .unwrap()
            .is_empty()
    );
    assert!(SOURCE.contains("agent-turn-complete"));
    assert!(SOURCE.contains("isCodexAutoApprovalLifecycleMessage"));
}

#[test]
fn codex_notify_source_classification_branches_are_independently_observable() {
    for message in [
        "Log in to Codex",
        "Login to Codex",
        "Sign in to Codex",
        "Sign-in to Codex",
    ] {
        let status = reduce(
            adapt_codex_notify(
                serde_json::json!({"type": "auth", "message": message})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap(),
        );
        assert_eq!(status.interaction, zentty_core::AgentInteractionKind::Auth);
    }
    let typed_permission =
        reduce(adapt_codex_notify(br#"{"type":"permission","message":"Proceed"}"#).unwrap());
    assert_eq!(
        typed_permission.interaction,
        zentty_core::AgentInteractionKind::Approval
    );
    let plain_question =
        reduce(adapt_codex_notify(br#"{"type":"question","message":"Choose now"}"#).unwrap());
    assert_eq!(
        plain_question.interaction,
        zentty_core::AgentInteractionKind::GenericInput
    );
    let punctuation_question =
        reduce(adapt_codex_notify(br#"{"type":"notice","message":"Continue?"}"#).unwrap());
    assert_eq!(
        punctuation_question.interaction,
        zentty_core::AgentInteractionKind::GenericInput
    );
    for message in ["Continue? [Yes] [No]", "Continue?\n1. Yes\n2. No"] {
        let status = reduce(
            adapt_codex_notify(
                serde_json::json!({"type": "question", "message": message})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap(),
        );
        assert_eq!(
            status.interaction,
            zentty_core::AgentInteractionKind::Decision
        );
    }
    for message in [
        "Continue? [Yes",
        "Continue? No]",
        "Continue?\nX. Yes",
        "Continue?\n1.",
    ] {
        let status = reduce(
            adapt_codex_notify(
                serde_json::json!({"type": "question", "message": message})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap(),
        );
        assert_eq!(
            status.interaction,
            zentty_core::AgentInteractionKind::GenericInput,
            "{message}"
        );
    }
}

#[test]
fn codex_question_hook_falls_back_to_the_real_bounded_transcript_file() {
    let path = std::env::temp_dir().join(format!(
        "zentty-codex-adapter-transcript-{}.jsonl",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    std::fs::write(
        &path,
        r#"{"type":"response_item","payload":{"type":"function_call","name":"request_user_input","arguments":{"question":"Which migration strategy?"}}}"#,
    )
    .unwrap();
    let payload = serde_json::json!({
        "hook_event_name": "PreToolUse",
        "session_id": "codex-transcript",
        "tool_name": "request_user_input",
        "transcript_path": path,
    });
    let status = reduce(adapt_codex_hook(payload.to_string().as_bytes(), None).unwrap());
    assert_eq!(status.phase, AgentPhase::NeedsInput);
    assert_eq!(status.text.as_deref(), Some("Which migration strategy?"));
    assert_eq!(status.transcript_path.as_deref(), path.to_str());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn claude_hooks_map_questions_stop_and_session_end() {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../Zentty/AppState/Agent/EventAdapters/ClaudeEventAdapter.swift"
    ));
    let question = reduce(
        adapt_claude_hook(
            br#"{"hook_event_name":"PreToolUse","session_id":"claude-a","tool_name":"AskUserQuestion","tool_input":{"questions":[{"question":"Which database?"}]}}"#,
            Some(5252),
        )
        .unwrap(),
    );
    assert_eq!(question.agent_name, "Claude Code");
    assert_eq!(question.phase, AgentPhase::NeedsInput);
    assert_eq!(question.text.as_deref(), Some("Which database?"));

    let idle = reduce(
        adapt_claude_hook(
            br#"{"hook_event_name":"Stop","session_id":"claude-a"}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(idle.phase, AgentPhase::Idle);
    assert!(SOURCE.contains("case \"PreToolUse\""));
    assert!(SOURCE.contains("case \"SessionEnd\""));
}

#[test]
fn claude_hooks_map_the_source_lifecycle_and_ignore_non_action_chatter() {
    for hook in ["UserPromptSubmit", "SubagentStart", "PostCompact"] {
        let running = reduce(
            adapt_claude_hook(
                format!(r#"{{"hook_event_name":"{hook}","session_id":"claude-life"}}"#).as_bytes(),
                None,
            )
            .unwrap(),
        );
        assert_eq!(running.phase, AgentPhase::Running, "{hook}");
        assert_eq!(running.text, None, "{hook}");
    }

    let compacting = reduce(
        adapt_claude_hook(
            br#"{"hook_event_name":"PreCompact","session_id":"claude-life"}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(compacting.phase, AgentPhase::Running);
    assert_eq!(compacting.text.as_deref(), Some("Compacting"));

    let subagent_stopped = reduce(
        adapt_claude_hook(
            br#"{"hook_event_name":"SubagentStop","session_id":"claude-life"}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(subagent_stopped.phase, AgentPhase::Idle);

    let camel_idle = reduce(
        adapt_claude_hook(
            br#"{"hook_event_name":"Notification","notificationType":"idle_prompt","session_id":"claude-life"}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(camel_idle.phase, AgentPhase::Idle);

    for message in ["Claude needs your attention", "Choose a target?"] {
        let payload = serde_json::json!({
            "hook_event_name": "Notification",
            "session_id": "claude-life",
            "message": message,
        });
        let waiting = reduce(adapt_claude_hook(payload.to_string().as_bytes(), None).unwrap());
        assert_eq!(waiting.phase, AgentPhase::NeedsInput, "{message}");
        assert_eq!(waiting.text.as_deref(), Some(message), "{message}");
    }
    let error_message = reduce(
        adapt_claude_hook(
            br#"{"hook_event_name":"Notification","session_id":"claude-life","error":"Login required"}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(error_message.phase, AgentPhase::NeedsInput);
    assert_eq!(error_message.text.as_deref(), Some("Login required"));

    let ended = adapt_claude_hook(
        br#"{"hook_event_name":"SessionEnd","session_id":"claude-life"}"#,
        None,
    )
    .unwrap();
    assert_eq!(ended.len(), 1);
    assert_eq!(
        serde_json::to_value(&ended[0]).unwrap()["event"],
        "session.end"
    );

    assert!(
        adapt_claude_hook(
            br#"{"hook_event_name":"Notification","session_id":"claude-life","message":"Build finished"}"#,
            None,
        )
        .unwrap()
        .is_empty()
    );
    assert!(
        adapt_claude_hook(br#"{"hook_event_name":"FutureEvent"}"#, None)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn claude_task_hooks_are_idempotent_and_reordered_completion_is_deterministic() {
    let target = AgentTarget::new("window", "lane", "pane");
    let mut store = AgentStatusStore::default();
    let payloads = [
        r#"{"hook_event_name":"TaskCompleted","session_id":"claude-tasks","task_id":"task-b"}"#,
        r#"{"hook_event_name":"TaskCreated","session_id":"claude-tasks","task_id":"task-a"}"#,
        r#"{"hook_event_name":"TaskCreated","session_id":"claude-tasks","task_id":"task-b"}"#,
        r#"{"hook_event_name":"TaskCreated","session_id":"claude-tasks","task_id":"task-a"}"#,
        r#"{"hook_event_name":"TaskCompleted","session_id":"claude-tasks","task_id":"task-a"}"#,
    ];
    for (now, payload) in payloads.into_iter().enumerate() {
        for event in adapt_claude_hook(payload.as_bytes(), None).unwrap() {
            store.apply(
                AuthenticatedAgentEvent {
                    target: target.clone(),
                    pane_token: "token".to_owned(),
                    event,
                },
                u64::try_from(now).unwrap(),
            );
        }
    }
    let status = store.status_for(&target).unwrap();
    assert_eq!(status.phase, AgentPhase::Running);
    assert_eq!(
        status.progress,
        Some(zentty_core::AgentProgress { done: 2, total: 2 })
    );
    assert!(
        adapt_claude_hook(
            br#"{"hook_event_name":"TaskCreated","session_id":"claude-tasks"}"#,
            None,
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn claude_questions_preserve_options_and_permission_fallbacks() {
    let question = reduce(
        adapt_claude_hook(
            br#"{"hook_event_name":"PreToolUse","session_id":"claude-question","tool_name":"AskUserQuestion","tool_input":{"questions":[{"question":"Which database?","options":[{"label":"Postgres"},{"label":"SQLite"}]}]}}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(
        question.text.as_deref(),
        Some("Which database?\n[Postgres] [SQLite]")
    );

    let approval = reduce(
        adapt_claude_hook(
            br#"{"hook_event_name":"PermissionRequest","session_id":"claude-approval","tool_name":"Bash"}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(approval.text.as_deref(), Some("Claude needs your approval"));
    assert_eq!(
        approval.interaction,
        zentty_core::AgentInteractionKind::Approval
    );
}

#[test]
fn adapters_reject_malformed_or_unsupported_hook_payloads() {
    assert!(adapt_codex_hook(b"not-json", None).is_err());
    assert!(adapt_claude_hook(b"not-json", None).is_err());
    assert!(adapt_claude_hook(br"{}", None).is_err());
}

#[test]
fn newly_installed_hook_adapters_drive_real_status_transitions() {
    let cursor = reduce(
        adapt_cursor_hook(
            br#"{"hook_event_name":"SessionStart","conversation_id":"cursor-a"}"#,
            Some(101),
        )
        .unwrap(),
    );
    assert_eq!(cursor.agent_name, "Cursor");
    assert_eq!(cursor.phase, AgentPhase::Starting);
    assert_eq!(cursor.tracked_pid, Some(101));

    let droid = reduce(
        adapt_droid_hook(
            br#"{"hook_event_name":"PreToolUse","session_id":"droid-a","tool_name":"AskUser","tool_input":{"question":"Which branch?"}}"#,
            Some(202),
        )
        .unwrap(),
    );
    assert_eq!(droid.phase, AgentPhase::NeedsInput);
    assert_eq!(droid.text.as_deref(), Some("Which branch?"));

    let vibe = reduce(
        adapt_vibe_hook(
            br#"{"hook_event_name":"before_tool","session_id":"vibe-a","tool_name":"ask_user_question","tool_input":{"question":"Ship it?"}}"#,
        )
        .unwrap(),
    );
    assert_eq!(vibe.agent_name, "Mistral Vibe");
    assert_eq!(vibe.phase, AgentPhase::NeedsInput);
    assert_eq!(vibe.text.as_deref(), Some("Ship it?"));

    let progress = reduce(
        adapt_vibe_hook(
            br#"{"hook_event_name":"after_tool","session_id":"vibe-a","tool_name":"todo","tool_output":{"todos":[{"status":"completed"},{"status":"pending"}],"total_count":2}}"#,
        )
        .unwrap(),
    );
    assert_eq!(
        progress.progress.map(|value| (value.done, value.total)),
        Some((1, 2))
    );
}

#[test]
fn remaining_integration_adapters_cover_source_lifecycle_and_input_semantics() {
    for (event, expected_agent) in [
        (
            adapt_kimi_hook(
                br#"{"hook_event_name":"SessionStart","session_id":"a"}"#,
                Some(1),
            )
            .unwrap(),
            "Kimi",
        ),
        (
            adapt_grok_hook(
                br#"{"hook_event_name":"session_start","session_id":"b"}"#,
                Some(2),
            )
            .unwrap(),
            "Grok",
        ),
        (
            adapt_agy_hook(
                br#"{"hook_event_name":"PreInvocation","session_id":"c"}"#,
                Some(3),
            )
            .unwrap(),
            "Antigravity",
        ),
        (
            adapt_hermes_hook(
                br#"{"hook_event_name":"on_session_start","session_id":"d"}"#,
                Some(4),
            )
            .unwrap(),
            "Hermes",
        ),
    ] {
        let status = reduce(event);
        assert_eq!(status.agent_name, expected_agent);
        assert!(matches!(
            status.phase,
            AgentPhase::Starting | AgentPhase::Running
        ));
    }

    let approval = reduce(adapt_kimi_hook(
        br#"{"hook_event_name":"PreToolUse","session_id":"kimi","tool_name":"WriteFile","tool_input":{"path":"README.md"}}"#,
        None,
    ).unwrap());
    assert_eq!(approval.phase, AgentPhase::NeedsInput);

    let hermes = reduce(adapt_hermes_hook(
        br#"{"hook_event_name":"pre_approval_request","session_id":"hermes","message":"Allow deploy?"}"#,
        None,
    ).unwrap());
    assert_eq!(hermes.phase, AgentPhase::NeedsInput);
    assert_eq!(hermes.text.as_deref(), Some("Allow deploy?"));

    let kimi = reduce(
        adapt_kimi_hook(
            br#"{"hook_event_name":"PreToolUse","session_id":"kimi","tool_name":"WriteFile","tool_input":{"file_path":"README.md"}}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(kimi.text.as_deref(), Some("Allow WriteFile on README.md?"));
}

#[test]
fn claude_and_kimi_do_not_regress_into_generic_hook_guesses() {
    assert!(
        adapt_claude_hook(br#"{"event":"SessionStart","session_id":"claude-a"}"#, None,).is_err(),
        "Claude's source hook contract does not accept the Small Harness event key"
    );
    assert!(
        adapt_kimi_hook(
            br#"{"hook_event_name":"Notification","session_id":"kimi-a","notification_type":"turn_complete","message":"done"}"#,
            None,
        )
        .unwrap()
        .is_empty()
    );
    assert!(
        adapt_kimi_hook(
            br#"{"hook_event_name":"PreToolUse","session_id":"kimi-a","tool_name":"ReadFile"}"#,
            None,
        )
        .unwrap()
        .is_empty()
    );
    let kimi_approval = reduce(
        adapt_kimi_hook(
            br#"{"hook_event_name":"PreToolUse","session_id":"kimi-a","tool_name":"Shell","tool_input":{"command":"cargo test"}}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(kimi_approval.phase, AgentPhase::NeedsInput);
    assert_eq!(
        kimi_approval.interaction,
        zentty_core::AgentInteractionKind::Approval
    );
    let kimi_resolved = reduce(
        adapt_kimi_hook(
            br#"{"hook_event_name":"PostToolUse","session_id":"kimi-a","tool_name":"Shell"}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(kimi_resolved.phase, AgentPhase::Running);
}

#[test]
fn source_specific_adapter_exceptions_do_not_regress_into_generic_hook_guesses() {
    for payload in [
        br#"{"hook_event_name":"Notification","session_id":"grok-a","message":"turn complete"}"#.as_slice(),
        br#"{"hook_event_name":"PreToolUse","session_id":"grok-a","tool_name":"ask_user_question"}"#.as_slice(),
    ] {
        assert!(adapt_grok_hook(payload, None).unwrap().is_empty());
    }
    let grok_tool = reduce(
        adapt_grok_hook(
            br#"{"hook_event_name":"pre_tool_use","session_id":"grok-a","tool_name":"shell"}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(grok_tool.phase, AgentPhase::Running);

    let hermes_end = reduce(
        adapt_hermes_hook(
            br#"{"hook_event_name":"on_session_end","session_id":"hermes-a"}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(hermes_end.phase, AgentPhase::Idle);

    let agy_background = reduce(
        adapt_agy_hook(
            br#"{"hook_event_name":"Stop","session_id":"agy-a","fullyIdle":false,"message":"background work remains"}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(agy_background.phase, AgentPhase::UnresolvedStop);
    assert_eq!(
        agy_background.text.as_deref(),
        Some("background work remains")
    );

    let cursor_error = reduce(
        adapt_cursor_hook(
            br#"{"hook_event_name":"Stop","conversation_id":"cursor-a","status":"error","error":"worker failed"}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(cursor_error.phase, AgentPhase::UnresolvedStop);
    let cursor_aborted = adapt_cursor_hook(
        br#"{"hook_event_name":"Stop","conversation_id":"cursor-a","status":"aborted"}"#,
        None,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&cursor_aborted[0]).unwrap()["state"]["stopCandidate"],
        true
    );

    let droid_manual = reduce(
        adapt_droid_hook(
            br#"{"hook_event_name":"PreToolUse","session_id":"droid-a","permission_mode":"off","tool_name":"Write","tool_input":{"path":"src/main.rs"}}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(droid_manual.phase, AgentPhase::NeedsInput);
    assert_eq!(
        droid_manual.interaction,
        zentty_core::AgentInteractionKind::Approval
    );
    assert!(
        adapt_droid_hook(
            br#"{"hook_event_name":"PostToolUse","session_id":"droid-a","tool_name":"ExitSpecMode"}"#,
            None,
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn cursor_and_droid_todo_hooks_emit_observable_progress() {
    for (events, expected_agent) in [
        (
            adapt_cursor_hook(
                br#"{"hook_event_name":"PreToolUse","conversation_id":"cursor-tasks","tool_name":"TodoWrite","tool_input":{"todos":[{"status":"completed"},{"status":"in_progress"},{"status":"pending"}]}}"#,
                None,
            )
            .unwrap(),
            "Cursor",
        ),
        (
            adapt_droid_hook(
                br#"{"hook_event_name":"PostToolUse","session_id":"droid-tasks","tool_name":"TodoWrite","tool_input":{"todos":[{"status":"done"},{"status":"complete"},{"status":"pending"}]}}"#,
                None,
            )
            .unwrap(),
            "Droid",
        ),
    ] {
        let status = reduce(events);
        assert_eq!(status.agent_name, expected_agent);
        assert_eq!(
            status.progress.map(|progress| (progress.done, progress.total)),
            Some(if expected_agent == "Cursor" { (1, 3) } else { (2, 3) })
        );
    }
}

#[test]
fn droid_source_interactions_and_cwd_are_preserved() {
    let decision = reduce(
        adapt_droid_hook(
            br#"{"hook_event_name":"PreToolUse","session_id":"droid-source","tool_name":"AskUser","workingDirectory":"/tmp","toolInput":{"question":"Choose a target?","options":["Staging",{"label":"Production"}]}}"#,
            Some(6262),
        )
        .unwrap(),
    );
    assert_eq!(decision.phase, AgentPhase::NeedsInput);
    assert_eq!(
        decision.interaction,
        zentty_core::AgentInteractionKind::Decision
    );
    assert_eq!(
        decision.text.as_deref(),
        Some("Choose a target?\n- Staging\n- Production")
    );
    assert_eq!(decision.working_directory.as_deref(), Some("/tmp"));

    let spec = reduce(
        adapt_droid_hook(
            br#"{"hook_event_name":"PreToolUse","session_id":"droid-source","tool_name":"ExitSpecMode","project_dir":"/tmp","tool_input":{"plan":"Add the Droid feature\n\nDetails follow"}}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(spec.phase, AgentPhase::NeedsInput);
    assert_eq!(
        spec.text.as_deref(),
        Some("Droid proposed a spec: Add the Droid feature")
    );
}

#[test]
fn droid_anonymous_counters_yield_to_authoritative_todos() {
    let fallback_target = AgentTarget::new("window", "lane", "droid-fallback-pane");
    let mut fallback_store = AgentStatusStore::default();
    let mut fallback_now = 0;
    for payload in [
        br#"{"hook_event_name":"PreToolUse","session_id":"droid-fallback","tool_name":"Task"}"#
            .as_slice(),
        br#"{"hook_event_name":"SubagentStop","session_id":"droid-fallback"}"#.as_slice(),
    ] {
        for event in adapt_droid_hook(payload, Some(6262)).unwrap() {
            fallback_store.apply(
                AuthenticatedAgentEvent {
                    target: fallback_target.clone(),
                    pane_token: "token".to_owned(),
                    event,
                },
                fallback_now,
            );
            fallback_now += 1;
        }
    }
    assert_eq!(
        fallback_store
            .status_for(&fallback_target)
            .unwrap()
            .progress
            .map(|progress| (progress.done, progress.total)),
        Some((1, 1)),
        "anonymous Droid subagents must retain source counter semantics"
    );

    let target = AgentTarget::new("window", "lane", "droid-pane");
    let mut store = AgentStatusStore::default();
    let mut now = 0;
    for payload in [
        br#"{"hook_event_name":"PreToolUse","session_id":"droid-source","tool_name":"TodoWrite","tool_input":{"todos":"1. [completed] Review logs\n2. [in_progress] Patch adapter\n3. [pending] Run tests"}}"#.as_slice(),
        br#"{"hook_event_name":"PreToolUse","session_id":"droid-source","tool_name":"Task","tool_use_id":"subtask-late"}"#.as_slice(),
        br#"{"hook_event_name":"SubagentStop","session_id":"droid-source","tool_use_id":"subtask-late"}"#.as_slice(),
    ] {
        for event in adapt_droid_hook(payload, Some(6262)).unwrap() {
            store.apply(
                AuthenticatedAgentEvent {
                    target: target.clone(),
                    pane_token: "token".to_owned(),
                    event,
                },
                now,
            );
            now += 1;
        }
    }
    assert_eq!(
        store
            .status_for(&target)
            .unwrap()
            .progress
            .map(|progress| (progress.done, progress.total)),
        Some((1, 3)),
        "subagent events must not replace an authoritative TodoWrite snapshot"
    );

    for event in adapt_droid_hook(
        br#"{"hook_event_name":"PostToolUse","session_id":"droid-source","tool_name":"TodoWrite","tool_input":{"todos":[]}}"#,
        Some(6262),
    )
    .unwrap()
    {
        store.apply(
            AuthenticatedAgentEvent {
                target: target.clone(),
                pane_token: "token".to_owned(),
                event,
            },
            now,
        );
        now += 1;
    }
    assert_eq!(
        store
            .status_for(&target)
            .unwrap()
            .progress
            .map(|progress| (progress.done, progress.total)),
        Some((1, 1)),
        "the source-compatible complete sentinel must clear stale visible work"
    );
}

#[test]
fn cursor_todo_identity_snapshots_merge_in_the_canonical_status_store() {
    let target = AgentTarget::new("window", "lane", "pane");
    let mut store = AgentStatusStore::default();
    let mut now = 0;
    for payload in [
        br#"{"hook_event_name":"preToolUse","conversation_id":"cursor-merge","tool_name":"TodoWrite","tool_input":{"merge":false,"todos":[{"id":"one","status":"pending"},{"id":"two","status":"pending"},{"id":"three","status":"pending"},{"id":"four","status":"pending"},{"id":"five","status":"pending"}]}}"#.as_slice(),
        br#"{"hook_event_name":"preToolUse","conversation_id":"cursor-merge","tool_name":"TodoWrite","tool_input":{"merge":true,"todos":[{"id":"one","status":"completed"},{"id":"three","status":"completed"}]}}"#.as_slice(),
        br#"{"hook_event_name":"postToolUse","conversation_id":"cursor-merge","tool_name":"TodoWrite","tool_input":{"merge":true,"todos":[{"id":"six","status":"pending"}]}}"#.as_slice(),
    ] {
        for event in adapt_cursor_hook(payload, Some(8181)).unwrap() {
            store.apply(
                AuthenticatedAgentEvent {
                    target: target.clone(),
                    pane_token: "token".to_owned(),
                    event,
                },
                now,
            );
            now += 1;
        }
    }
    let status = store.status_for(&target).unwrap();
    assert_eq!(status.phase, AgentPhase::Running);
    assert_eq!(
        status
            .progress
            .map(|progress| (progress.done, progress.total)),
        Some((2, 6))
    );

    for event in adapt_cursor_hook(
        br#"{"hook_event_name":"stop","conversation_id":"cursor-merge","status":"completed"}"#,
        Some(8181),
    )
    .unwrap()
    {
        store.apply(
            AuthenticatedAgentEvent {
                target: target.clone(),
                pane_token: "token".to_owned(),
                event,
            },
            now,
        );
        now += 1;
    }
    let stopped = store.status_for(&target).unwrap();
    assert_eq!(stopped.phase, AgentPhase::Idle);
    assert_eq!(
        stopped
            .progress
            .map(|progress| (progress.done, progress.total)),
        Some((2, 6))
    );
}

#[test]
fn cursor_payload_identity_captures_cwd_transcript_and_suppresses_noise() {
    let cwd = std::env::temp_dir();
    let payload = format!(
        r#"{{"hookEventName":"sessionStart","conversationId":"cursor-camel","workspaceRoots":[{:?}],"transcriptPath":"/tmp/cursor-transcript.jsonl"}}"#,
        cwd.to_string_lossy()
    );
    let status = reduce(adapt_cursor_hook(payload.as_bytes(), Some(9191)).unwrap());
    assert_eq!(status.session_id, "cursor-camel");
    assert_eq!(status.tracked_pid, Some(9191));
    assert_eq!(
        status.working_directory.as_deref(),
        Some(cwd.to_string_lossy().as_ref())
    );
    assert_eq!(
        status.transcript_path.as_deref(),
        Some("/tmp/cursor-transcript.jsonl")
    );
    assert!(
        adapt_cursor_hook(
            br#"{"hook_event_name":"beforeShellExecution","conversation_id":"cursor-camel"}"#,
            None,
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn cursor_checklists_and_subagents_use_shared_progress_projection() {
    let checklist = reduce(
        adapt_cursor_hook(
            br#"{"hook_event_name":"postToolUse","conversationId":"cursor-checklist","toolName":"TodoWrite","toolInput":{"todos":"- [x] Review logs\n- [ ] Run tests"}}"#,
            Some(7272),
        )
        .unwrap(),
    );
    assert_eq!(checklist.phase, AgentPhase::Running);
    assert_eq!(
        checklist
            .progress
            .map(|progress| (progress.done, progress.total)),
        Some((1, 2))
    );

    let target = AgentTarget::new("window", "lane", "subagent-pane");
    let mut store = AgentStatusStore::default();
    let mut now = 0;
    for payload in [
        br#"{"hook_event_name":"subagentStart","parent_conversation_id":"cursor-parent","subagent_id":"worker-one"}"#.as_slice(),
        br#"{"hook_event_name":"subagentStop","parentConversationId":"cursor-parent","subagentId":"worker-one"}"#.as_slice(),
    ] {
        for event in adapt_cursor_hook(payload, Some(7373)).unwrap() {
            store.apply(
                AuthenticatedAgentEvent {
                    target: target.clone(),
                    pane_token: "token".to_owned(),
                    event,
                },
                now,
            );
            now += 1;
        }
    }
    let status = store.status_for(&target).unwrap();
    assert_eq!(status.session_id, "cursor-parent");
    assert_eq!(status.phase, AgentPhase::Running);
    assert_eq!(
        status
            .progress
            .map(|progress| (progress.done, progress.total)),
        Some((1, 1))
    );
}

#[test]
fn gemini_hooks_map_the_complete_source_lifecycle() {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../Zentty/AppState/Agent/EventAdapters/GeminiEventAdapter.swift"
    ));
    let start = reduce(
        adapt_gemini_hook(
            br#"{"hookEventName":"SessionStart","sessionId":"gemini-a","cwd":"/tmp/project"}"#,
            Some(6262),
        )
        .unwrap(),
    );
    assert_eq!(start.agent_name, "Gemini");
    assert_eq!(start.phase, AgentPhase::Starting);
    assert_eq!(start.tracked_pid, Some(6262));

    for hook in ["BeforeAgent", "BeforeTool"] {
        let running = reduce(
            adapt_gemini_hook(
                format!(r#"{{"hook_event_name":"{hook}","session_id":"gemini-a"}}"#).as_bytes(),
                None,
            )
            .unwrap(),
        );
        assert_eq!(running.phase, AgentPhase::Running);
    }

    let idle = reduce(
        adapt_gemini_hook(
            br#"{"hook_event_name":"AfterAgent","session_id":"gemini-a"}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(idle.phase, AgentPhase::Idle);

    let ended = adapt_gemini_hook(
        br#"{"hook_event_name":"SessionEnd","session_id":"gemini-a"}"#,
        None,
    )
    .unwrap();
    let target = AgentTarget::new("window", "lane", "pane");
    let mut store = AgentStatusStore::default();
    store.apply(
        AuthenticatedAgentEvent {
            target: target.clone(),
            pane_token: "token".to_owned(),
            event: adapt_gemini_hook(
                br#"{"hook_event_name":"SessionStart","session_id":"gemini-a"}"#,
                Some(6262),
            )
            .unwrap()
            .remove(0),
        },
        1,
    );
    for event in ended {
        store.apply(
            AuthenticatedAgentEvent {
                target: target.clone(),
                pane_token: "token".to_owned(),
                event,
            },
            2,
        );
    }
    assert!(store.status_for(&target).is_none());

    for event in [
        "SessionStart",
        "BeforeAgent",
        "BeforeTool",
        "AfterAgent",
        "SessionEnd",
        "Notification",
    ] {
        assert!(SOURCE.contains(&format!("\"{event}\"")));
    }
}

#[test]
fn gemini_permission_notifications_preserve_specific_text_or_derive_details() {
    let explicit = reduce(
        adapt_gemini_hook(
            br#"{"hook_event_name":"Notification","notification_type":"toolpermission","session_id":"gemini-a","message":"Allow shell command?","details":{"tool_name":"shell","path":"/ignored"}}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(explicit.phase, AgentPhase::NeedsInput);
    assert_eq!(explicit.text.as_deref(), Some("Allow shell command?"));
    assert!(explicit.requires_attention());

    let derived = reduce(
        adapt_gemini_hook(
            br#"{"hook_event_name":"Notification","notificationType":"ToolPermission","sessionId":"gemini-b","message":"Action required","details":{"toolName":"write_file","filePath":"/tmp/a b"}}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(
        derived.text.as_deref(),
        Some("Allow write_file on /tmp/a b?")
    );

    let fallback = reduce(
        adapt_gemini_hook(
            br#"{"hook_event_name":"Notification","notification_type":"ToolPermission","session_id":"gemini-c"}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(fallback.text.as_deref(), Some("Gemini needs your approval"));

    let string_details = reduce(
        adapt_gemini_hook(
            br#"{"hook_event_name":"Notification","notification_type":"ToolPermission","session_id":"gemini-d","message":"Action required","details":"Grant this tool once"}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(string_details.text.as_deref(), Some("Grant this tool once"));
}

#[test]
fn gemini_ignores_non_permission_notifications_and_future_events_but_rejects_bad_input() {
    assert!(
        adapt_gemini_hook(
            br#"{"hook_event_name":"Notification","notification_type":"SessionComplete"}"#,
            None,
        )
        .unwrap()
        .is_empty()
    );
    assert!(
        adapt_gemini_hook(br#"{"hook_event_name":"FutureEvent"}"#, None)
            .unwrap()
            .is_empty()
    );
    assert!(adapt_gemini_hook(b"not-json", None).is_err());
    assert!(adapt_gemini_hook(br"{}", None).is_err());
}

#[test]
fn copilot_source_aliases_preserve_session_and_question_lifecycle() {
    let starting = adapt_copilot_hook(
        br#"{"sessionId":"copilot-a","cwd":"/tmp/project"}"#,
        Some("session-start"),
        Some(991),
    )
    .unwrap();
    assert_eq!(starting.len(), 2);
    let started = serde_json::to_value(&starting[0]).unwrap();
    let seeded = serde_json::to_value(&starting[1]).unwrap();
    assert_eq!(started["event"], "session.start");
    assert_eq!(started["agent"]["pid"], 991);
    assert_eq!(seeded["event"], "agent.idle");
    assert_eq!(seeded["session"]["id"], "copilot-a");

    let question = reduce(
        adapt_copilot_hook(
            br#"{"session_id":"copilot-a","tool_name":"Ask_User-Question","tool_args":"{\"question\":\"Which file?\"}"}"#,
            Some("pre-tool-use"),
            None,
        )
        .unwrap(),
    );
    assert_eq!(question.phase, AgentPhase::NeedsInput);
    assert_eq!(question.text.as_deref(), Some("Which file?"));

    assert!(
        adapt_copilot_hook(
            br#"{"sessionId":"copilot-b","message":"Model request failed"}"#,
            Some("error-occurred"),
            None,
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn copilot_non_question_tools_are_noops_and_question_completion_returns_idle() {
    assert!(
        adapt_copilot_hook(br#"{"toolName":"ReadFile"}"#, Some("pre-tool-use"), None,)
            .unwrap()
            .is_empty()
    );
    let idle = adapt_copilot_hook(
        br#"{"sessionID":"copilot-a","toolName":"AskUserQuestion"}"#,
        Some("post-tool-use"),
        None,
    )
    .unwrap();
    assert_eq!(idle.len(), 1);
    assert_eq!(
        serde_json::to_value(&idle[0]).unwrap()["event"],
        "agent.idle"
    );

    let running = adapt_copilot_hook(
        br#"{"sessionId":"copilot-a"}"#,
        Some("user-prompt-submitted"),
        None,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&running[0]).unwrap()["event"],
        "agent.running"
    );
    let ended =
        adapt_copilot_hook(br#"{"sessionId":"copilot-a"}"#, Some("session-end"), None).unwrap();
    assert_eq!(
        serde_json::to_value(&ended[0]).unwrap()["event"],
        "session.end"
    );
}

#[test]
fn copilot_rejects_missing_or_unknown_events_and_malformed_payloads() {
    assert!(adapt_copilot_hook(br"{}", None, None).is_err());
    assert!(adapt_copilot_hook(b"not-json", Some("session-start"), None).is_err());
    assert!(adapt_copilot_hook(br"{}", Some("future-event"), None).is_err());
}
