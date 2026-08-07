use std::fs;
use std::os::unix::fs::symlink;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use zentty_core::{
    AgentInteractionKind, codex_question_from_transcript_path, codex_question_from_transcript_text,
    codex_transcript_cache_key, locate_recent_codex_transcript_path,
};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

fn fixture() -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "zentty-codex-transcript-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir(&path).unwrap();
    path
}

#[test]
fn extracts_the_newest_source_shaped_question_and_formats_decision_options() {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../Zentty/AppState/Agent/CodexTranscriptQuestionExtractor.swift"
    ));
    let transcript = r#"
{"type":"function_call","name":"request_user_input","arguments":"{\"question\":\"Old question?\"}"}
not-json
{"type":"response_item","payload":{"type":"function_call","name":"ask_user_question_tool","arguments":{"questions":[{"header":"Deployment target","options":[{"label":"Staging"},{"label":"Production"}]}]}}}
"#;
    let question = codex_question_from_transcript_text(transcript).unwrap();
    assert_eq!(question.text, "Deployment target\n[Staging] [Production]");
    assert_eq!(question.interaction, AgentInteractionKind::Decision);
    let free_form = codex_question_from_transcript_text(
        r#"{"type":"function_call","name":"request_user_input","arguments":{"question":"What should change?"}}"#,
    )
    .unwrap();
    assert_eq!(free_form.interaction, AgentInteractionKind::Question);
    assert!(SOURCE.contains("maxTailBytes: UInt64 = 256 * 1024"));
    assert!(SOURCE.contains("response_item"));
}

#[test]
fn bounded_real_file_tail_ignores_a_partial_first_line_and_rejects_symlinks() {
    let root = fixture();
    let transcript = root.join("rollout.jsonl");
    let padding = format!("{{\"padding\":\"{}\"}}\n", "x".repeat(270 * 1024));
    let final_line = r#"{"type":"function_call","name":"request_user_input","arguments":{"question":"Use the bounded tail?"}}"#;
    fs::write(&transcript, format!("{padding}{final_line}\n")).unwrap();
    assert_eq!(
        codex_question_from_transcript_path(&transcript)
            .unwrap()
            .text,
        "Use the bounded tail?"
    );

    let link = root.join("linked.jsonl");
    symlink(&transcript, &link).unwrap();
    assert!(codex_question_from_transcript_path(&link).is_none());
    assert!(codex_question_from_transcript_path(&root).is_none());

    let boundary = root.join("boundary.jsonl");
    let mut boundary_event = serde_json::json!({
        "type": "function_call",
        "name": "request_user_input",
        "arguments": {"question": "Must be discarded", "padding": ""},
    });
    let empty_length = boundary_event.to_string().len();
    let target_line_length = 256 * 1024 - 1;
    boundary_event["arguments"]["padding"] =
        serde_json::Value::String("x".repeat(target_line_length - empty_length));
    let encoded = boundary_event.to_string();
    assert_eq!(encoded.len(), target_line_length);
    fs::write(&boundary, format!("prefix-without-newline{encoded}\n")).unwrap();
    assert!(
        codex_question_from_transcript_path(&boundary).is_none(),
        "the first partial line at the exact tail boundary must be discarded"
    );

    let newline_boundary = root.join("newline-boundary.jsonl");
    let mut boundary_question = serde_json::json!({
        "type": "function_call",
        "name": "request_user_input",
        "arguments": {"question": "Preserve the complete boundary line", "padding": ""},
    });
    let empty_length = boundary_question.to_string().len();
    let target_event_length = 256 * 1024 - 2;
    boundary_question["arguments"]["padding"] =
        serde_json::Value::String("x".repeat(target_event_length - empty_length));
    let encoded = boundary_question.to_string();
    assert_eq!(encoded.len(), target_event_length);
    fs::write(&newline_boundary, format!("prefix\n{encoded}\n")).unwrap();
    assert_eq!(
        codex_question_from_transcript_path(&newline_boundary)
            .unwrap()
            .text,
        "Preserve the complete boundary line"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn malformed_or_non_question_transcripts_have_no_false_attention_signal() {
    assert!(codex_question_from_transcript_text("not-json\n{}").is_none());
    assert!(
        codex_question_from_transcript_text(
            r#"{"type":"function_call","name":"shell","arguments":{"question":"Not for the user"}}"#
        )
        .is_none()
    );
}

#[test]
fn recent_discovery_is_bounded_and_requires_the_normalized_working_directory() {
    let root = fixture();
    let codex_home = root.join("codex-home");
    let sessions = codex_home.join("sessions");
    let cwd = root.join("project");
    fs::create_dir(&cwd).unwrap();

    for day in [
        "2026/08/01",
        "2026/08/02",
        "2026/08/03",
        "2026/08/04",
        "2026/08/05",
    ] {
        fs::create_dir_all(sessions.join(day)).unwrap();
    }
    fs::write(
        sessions.join("2026/08/01/older.jsonl"),
        format!(
            "{{\"cwd\":{:?}}}\n{{\"type\":\"function_call\",\"name\":\"request_user_input\",\"arguments\":{{\"question\":\"Too old\"}}}}\n",
            cwd.to_string_lossy()
        ),
    )
    .unwrap();
    assert_eq!(
        locate_recent_codex_transcript_path(&codex_home, cwd.to_str().unwrap()),
        None,
        "only the four newest session-day directories may be searched"
    );

    let newest_day = sessions.join("2026/08/05");
    let matching = newest_day.join("00-matching.jsonl");
    fs::write(
        &matching,
        format!(
            "{{\"payload\":{{\"currentWorkingDirectory\":{:?}}}}}\n{{\"type\":\"function_call\",\"name\":\"request_user_input\",\"arguments\":{{\"question\":\"Bounded candidate\"}}}}\n",
            cwd.join(".").to_string_lossy()
        ),
    )
    .unwrap();
    for index in 0..12 {
        std::thread::sleep(Duration::from_millis(2));
        fs::write(
            newest_day.join(format!("{index:02}-newer.jsonl")),
            "{\"cwd\":\"/different/project\"}\n",
        )
        .unwrap();
    }
    assert_eq!(
        locate_recent_codex_transcript_path(&codex_home, cwd.to_str().unwrap()),
        None,
        "only the twelve newest transcript candidates may be searched"
    );

    std::thread::sleep(Duration::from_millis(2));
    let newest_matching = newest_day.join("99-newest-matching.jsonl");
    fs::write(
        &newest_matching,
        format!(
            "{{\"cwd\":{:?}}}\n{{\"type\":\"function_call\",\"name\":\"ask_user_question\",\"arguments\":{{\"question\":\"Use this transcript?\"}}}}\n",
            cwd.to_string_lossy()
        ),
    )
    .unwrap();
    assert_eq!(
        locate_recent_codex_transcript_path(&codex_home, cwd.to_str().unwrap()),
        Some(newest_matching)
    );
    assert_eq!(
        locate_recent_codex_transcript_path(&codex_home, "/different/project"),
        None,
        "a question from another working directory must not leak attention"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn transcript_cache_identity_changes_with_file_identity_and_rejects_symlinks() {
    let root = fixture();
    let transcript = root.join("rollout.jsonl");
    fs::write(&transcript, "{}\n").unwrap();
    let first = codex_transcript_cache_key(&transcript).unwrap();
    assert_eq!(first.path, transcript);
    assert_eq!(first.file_size, 3);

    fs::write(&transcript, "{}\n{}\n").unwrap();
    let second = codex_transcript_cache_key(&transcript).unwrap();
    assert_ne!(first, second);
    assert_eq!(second.file_size, 6);

    let link = root.join("linked.jsonl");
    symlink(&transcript, &link).unwrap();
    assert!(codex_transcript_cache_key(&link).is_none());
    fs::remove_dir_all(root).unwrap();
}
