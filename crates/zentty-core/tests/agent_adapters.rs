use zentty_core::{
    AgentPhase, AgentStatusStore, AgentTarget, AuthenticatedAgentEvent, adapt_claude_hook,
    adapt_codex_hook, adapt_codex_notify, adapt_gemini_hook,
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

#[test]
fn codex_hooks_map_source_lifecycle_and_approval_semantics() {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../Zentty/AppState/Agent/EventAdapters/CodexEventAdapter.swift"
    ));
    let start = reduce(
        adapt_codex_hook(
            br#"{"hook_event_name":"SessionStart","session_id":"codex-a","cwd":"/tmp/project"}"#,
            Some(4242),
        )
        .unwrap(),
    );
    assert_eq!(start.agent_name, "Codex");
    assert_eq!(start.phase, AgentPhase::Starting);
    assert_eq!(start.tracked_pid, Some(4242));

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

    let compacting = reduce(
        adapt_codex_hook(
            br#"{"hook_event_name":"PreCompact","session_id":"codex-a"}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(compacting.phase, AgentPhase::Running);
    assert_eq!(compacting.text.as_deref(), Some("Compacting"));
    let compacted = reduce(
        adapt_codex_hook(
            br#"{"hook_event_name":"PostCompact","session_id":"codex-a"}"#,
            None,
        )
        .unwrap(),
    );
    assert_eq!(compacted.phase, AgentPhase::Running);
    assert_eq!(compacted.text, None);

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
fn adapters_reject_malformed_or_unsupported_hook_payloads() {
    assert!(adapt_codex_hook(b"not-json", None).is_err());
    assert!(adapt_claude_hook(br#"{"hook_event_name":"FutureEvent"}"#, None).is_err());
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
