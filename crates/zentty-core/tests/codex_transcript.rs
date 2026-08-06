use std::fs;
use std::os::unix::fs::symlink;
use std::sync::atomic::{AtomicU64, Ordering};
use zentty_core::{codex_question_from_transcript_path, codex_question_from_transcript_text};

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
