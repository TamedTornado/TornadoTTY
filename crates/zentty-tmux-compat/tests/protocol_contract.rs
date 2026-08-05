use zentty_tmux_compat::{Command, ProtocolError, TmuxCompatReply, TmuxCompatRequest};

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn request_payload_canonicalizes_commands_and_preserves_bounded_input() {
    let request = TmuxCompatRequest::new(
        1,
        "splitw",
        strings(&["-h", "-P"]),
        Some("input".to_owned()),
    )
    .unwrap();
    assert_eq!(TmuxCompatRequest::VERSION, 1);
    assert_eq!(request.command(), Command::SplitWindow);
    assert_eq!(request.arguments(), strings(&["-h", "-P"]));
    assert_eq!(request.standard_input(), Some("input"));
}

#[test]
fn request_payload_rejects_versions_counts_and_individual_or_total_sizes() {
    assert_eq!(TmuxCompatRequest::MAX_ARGUMENTS, 256);
    assert_eq!(TmuxCompatRequest::MAX_ARGUMENT_BYTES, 16_384);
    assert_eq!(TmuxCompatRequest::MAX_ARGUMENT_TOTAL_BYTES, 65_536);
    assert_eq!(TmuxCompatRequest::MAX_STANDARD_INPUT_BYTES, 262_144);
    assert_eq!(
        TmuxCompatRequest::new(2, "split-window", vec![], None).unwrap_err(),
        ProtocolError::UnsupportedVersion(2)
    );
    assert!(matches!(
        TmuxCompatRequest::new(1, "unknown", vec![], None),
        Err(ProtocolError::UnsupportedCommand(_))
    ));

    let exact_count = vec!["x".to_owned(); TmuxCompatRequest::MAX_ARGUMENTS];
    TmuxCompatRequest::new(1, "send-keys", exact_count, None).unwrap();
    let over_count = vec!["x".to_owned(); TmuxCompatRequest::MAX_ARGUMENTS + 1];
    assert_eq!(
        TmuxCompatRequest::new(1, "send-keys", over_count, None).unwrap_err(),
        ProtocolError::LimitExceeded("argument count")
    );

    let exact_argument = "x".repeat(TmuxCompatRequest::MAX_ARGUMENT_BYTES);
    TmuxCompatRequest::new(1, "send-keys", vec![exact_argument], None).unwrap();
    let over_argument = "x".repeat(TmuxCompatRequest::MAX_ARGUMENT_BYTES + 1);
    assert_eq!(
        TmuxCompatRequest::new(1, "send-keys", vec![over_argument], None).unwrap_err(),
        ProtocolError::LimitExceeded("argument bytes")
    );

    let piece = "x".repeat(TmuxCompatRequest::MAX_ARGUMENT_BYTES);
    let exact_total =
        vec![piece.clone(); TmuxCompatRequest::MAX_ARGUMENT_TOTAL_BYTES / piece.len()];
    TmuxCompatRequest::new(1, "send-keys", exact_total, None).unwrap();
    let over_total =
        vec![
            piece;
            TmuxCompatRequest::MAX_ARGUMENT_TOTAL_BYTES / TmuxCompatRequest::MAX_ARGUMENT_BYTES + 1
        ];
    assert_eq!(
        TmuxCompatRequest::new(1, "send-keys", over_total, None).unwrap_err(),
        ProtocolError::LimitExceeded("total argument bytes")
    );
}

#[test]
fn request_standard_input_accepts_the_source_ceiling_and_rejects_one_over() {
    let exact = "x".repeat(TmuxCompatRequest::MAX_STANDARD_INPUT_BYTES);
    TmuxCompatRequest::new(1, "set-buffer", vec![], Some(exact)).unwrap();
    let over = "x".repeat(TmuxCompatRequest::MAX_STANDARD_INPUT_BYTES + 1);
    assert_eq!(
        TmuxCompatRequest::new(1, "set-buffer", vec![], Some(over)).unwrap_err(),
        ProtocolError::LimitExceeded("standard input bytes")
    );
}

#[test]
fn reply_payload_maps_success_and_failure_to_source_cli_exit_status() {
    let success = TmuxCompatReply::success("done\n".to_owned()).unwrap();
    assert_eq!(TmuxCompatReply::VERSION, 1);
    assert!(success.is_ok());
    assert_eq!(success.stdout(), Some("done\n"));
    assert_eq!(success.error(), None);
    assert_eq!(success.exit_code(), 0);

    let failure = TmuxCompatReply::failure("unsupported", "popup is unsupported").unwrap();
    assert_eq!(TmuxCompatReply::VERSION, 1);
    assert!(!failure.is_ok());
    assert_eq!(failure.stdout(), None);
    assert_eq!(
        failure.error().map(|error| (error.code(), error.message())),
        Some(("unsupported", "popup is unsupported"))
    );
    assert_eq!(failure.exit_code(), 1);
}

#[test]
fn reply_payload_rejects_oversized_output_and_diagnostics() {
    assert_eq!(TmuxCompatReply::MAX_STDOUT_BYTES, 262_144);
    assert_eq!(TmuxCompatReply::MAX_ERROR_CODE_BYTES, 64);
    assert_eq!(TmuxCompatReply::MAX_ERROR_MESSAGE_BYTES, 16_384);
    TmuxCompatReply::success("x".repeat(TmuxCompatReply::MAX_STDOUT_BYTES)).unwrap();
    assert_eq!(
        TmuxCompatReply::success("x".repeat(TmuxCompatReply::MAX_STDOUT_BYTES + 1)).unwrap_err(),
        ProtocolError::LimitExceeded("stdout bytes")
    );
    assert_eq!(
        TmuxCompatReply::failure(
            "x".repeat(TmuxCompatReply::MAX_ERROR_CODE_BYTES + 1),
            "message"
        )
        .unwrap_err(),
        ProtocolError::LimitExceeded("error code bytes")
    );
    assert_eq!(
        TmuxCompatReply::failure(
            "code",
            "x".repeat(TmuxCompatReply::MAX_ERROR_MESSAGE_BYTES + 1)
        )
        .unwrap_err(),
        ProtocolError::LimitExceeded("error message bytes")
    );
    TmuxCompatReply::failure(
        "x".repeat(TmuxCompatReply::MAX_ERROR_CODE_BYTES),
        "x".repeat(TmuxCompatReply::MAX_ERROR_MESSAGE_BYTES),
    )
    .unwrap();
}

#[test]
fn protocol_errors_are_diagnostic() {
    assert_eq!(
        ProtocolError::UnsupportedVersion(2).to_string(),
        "unsupported tmux compatibility protocol version: 2"
    );
    assert_eq!(
        ProtocolError::LimitExceeded("stdout bytes").to_string(),
        "tmux compatibility stdout bytes limit exceeded"
    );
    assert!(
        TmuxCompatRequest::new(1, "unknown", vec![], None)
            .unwrap_err()
            .to_string()
            .contains("unsupported tmux compatibility command: unknown")
    );
}
