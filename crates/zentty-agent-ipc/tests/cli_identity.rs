use std::process::Command;

#[test]
fn public_usage_and_error_prefix_name_tornadotty_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_zentty"))
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "tornadotty-cli: usage: tornadotty-cli ipc \
<agent-event|agent-signal|agent-status> [arguments...]\n"
    );
}
