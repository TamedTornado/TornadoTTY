use zentty_core::{AgentInteractionKind, CodexTitlePhase, classify_codex_terminal_title};

#[test]
fn source_codex_titles_preserve_phase_subject_interaction_and_task_progress() {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../Zentty/AppState/Agent/TerminalMetadataChangeClassifier.swift"
    ));
    for (title, phase, subject) in [
        ("Working ⠋ zentty", CodexTitlePhase::Running, "zentty"),
        (
            "Thinking · Feature Audit",
            CodexTitlePhase::Running,
            "feature audit",
        ),
        ("Starting | zentty", CodexTitlePhase::Starting, "zentty"),
        ("Ready | zentty", CodexTitlePhase::Idle, "zentty"),
        (
            "Waiting · zentty main",
            CodexTitlePhase::Idle,
            "zentty main",
        ),
    ] {
        let signal = classify_codex_terminal_title(title).unwrap();
        assert_eq!(signal.phase, phase, "{title}");
        assert_eq!(signal.subject, subject, "{title}");
        assert_eq!(signal.interaction, AgentInteractionKind::None, "{title}");
        assert_eq!(
            signal.background_wait,
            title.starts_with("Waiting"),
            "{title}"
        );
    }

    let action =
        classify_codex_terminal_title("[ ! ] Action Required | codex-question | Tasks 7/5")
            .unwrap();
    assert_eq!(action.phase, CodexTitlePhase::NeedsInput);
    assert_eq!(action.subject, "action required | codex-question");
    assert_eq!(action.interaction, AgentInteractionKind::GenericInput);
    assert_eq!(action.progress.unwrap().done, 5);
    assert_eq!(action.progress.unwrap().total, 5);
    assert!(!action.background_wait);

    let thread = classify_codex_terminal_title("Main needs input | codex-question").unwrap();
    assert_eq!(thread.phase, CodexTitlePhase::NeedsInput);
    assert_eq!(thread.interaction, AgentInteractionKind::GenericInput);
    assert!(SOURCE.contains("parseCodexThreadStatusTitle"));
    assert!(SOURCE.contains("stripTrailingCodexTaskProgress"));
}

#[test]
fn waiting_titles_distinguish_background_wait_from_real_human_input() {
    let waiting = classify_codex_terminal_title("Waiting for your input | zentty").unwrap();
    assert_eq!(waiting.phase, CodexTitlePhase::NeedsInput);
    assert_eq!(waiting.interaction, AgentInteractionKind::GenericInput);

    let approval = classify_codex_terminal_title("Waiting for approval | zentty").unwrap();
    assert_eq!(approval.phase, CodexTitlePhase::NeedsInput);
    assert_eq!(approval.interaction, AgentInteractionKind::Approval);

    for title in [
        "Waiting plan-mode-prompt | zentty",
        "Waiting plan mode prompt | zentty",
    ] {
        assert_eq!(
            classify_codex_terminal_title(title).unwrap().interaction,
            AgentInteractionKind::Approval,
            "{title}"
        );
    }
    for title in [
        "Waiting question requested | zentty",
        "Waiting questions requested | zentty",
    ] {
        assert_eq!(
            classify_codex_terminal_title(title).unwrap().interaction,
            AgentInteractionKind::Decision,
            "{title}"
        );
    }
    for title in ["Waiting to log in | zentty", "Waiting for login | zentty"] {
        assert_eq!(
            classify_codex_terminal_title(title).unwrap().interaction,
            AgentInteractionKind::Auth,
            "{title}"
        );
    }

    let plain_question = classify_codex_terminal_title("Waiting: Continue? | zentty").unwrap();
    assert_eq!(plain_question.interaction, AgentInteractionKind::Question);
    for title in [
        "Waiting: Continue? [Yes] [No] | zentty",
        "Waiting: Continue?\n1. Yes\n2. No",
    ] {
        assert_eq!(
            classify_codex_terminal_title(title).unwrap().interaction,
            AgentInteractionKind::Decision,
            "{title}"
        );
    }
    for title in [
        "Waiting: Continue? [Yes | zentty",
        "Waiting: Continue? No] | zentty",
        "Waiting: Continue?\nX. Yes",
        "Waiting: Continue?\n1.",
    ] {
        assert_eq!(
            classify_codex_terminal_title(title).unwrap().interaction,
            AgentInteractionKind::Question,
            "{title}"
        );
    }

    for title in [
        "Side needs input | zentty",
        "Main wants input | zentty",
        "Main needs approval | zentty",
        "Parent needs approval | zentty",
    ] {
        assert!(classify_codex_terminal_title(title).is_none(), "{title}");
    }
    assert!(classify_codex_terminal_title("Waiting").is_none());
}

#[test]
fn malformed_badges_progress_and_unrelated_titles_do_not_create_false_state() {
    for title in [
        "[x] Action Required | zentty",
        "Actionable work | zentty",
        "Working",
        "bash",
        "Ready | zentty | Tasks nope/2",
        "Ready | zentty | Tasks 1/0",
    ] {
        if title == "Ready | zentty | Tasks nope/2" {
            let signal = classify_codex_terminal_title(title).unwrap();
            assert_eq!(signal.subject, "zentty | tasks nope/2");
            assert!(signal.progress.is_none());
        } else if title == "Ready | zentty | Tasks 1/0" {
            let signal = classify_codex_terminal_title(title).unwrap();
            assert_eq!(signal.subject, "zentty | tasks 1/0");
            assert!(signal.progress.is_none());
        } else {
            assert!(classify_codex_terminal_title(title).is_none(), "{title}");
        }
    }
}
