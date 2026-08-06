use zentty_core::{
    AgentPhase, AgentStatusStore, AgentTarget, AuthenticatedAgentEvent, adapt_claude_hook,
    adapt_codex_hook, adapt_gemini_hook,
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

    assert!(SOURCE.contains("case \"PermissionRequest\""));
    assert!(SOURCE.contains("case \"Stop\""));
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
