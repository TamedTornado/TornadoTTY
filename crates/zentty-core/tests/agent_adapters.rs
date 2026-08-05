use zentty_core::{
    AgentPhase, AgentStatusStore, AgentTarget, AuthenticatedAgentEvent, adapt_claude_hook,
    adapt_codex_hook,
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
