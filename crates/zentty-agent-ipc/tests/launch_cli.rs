use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

struct FakeTool {
    directory: std::path::PathBuf,
    receipt: std::path::PathBuf,
    binary: std::path::PathBuf,
}

impl FakeTool {
    fn new(name: &str) -> Self {
        let directory =
            std::env::temp_dir().join(format!("zentty-agent-launch-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let receipt = directory.join("receipt");
        let binary = directory.join(name);
        fs::write(
            &binary,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" >\"$ZENTTY_TEST_RECEIPT\"\nprintf 'AGENT=%s\\n' \"${ZENTTY_AGENT_TOOL:-}\" >>\"$ZENTTY_TEST_RECEIPT\"\nprintf 'CLAUDECODE=%s\\n' \"${CLAUDECODE:-}\" >>\"$ZENTTY_TEST_RECEIPT\"\n",
        )
        .unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
        Self {
            directory,
            receipt,
            binary,
        }
    }

    fn command(&self, tool: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_zentty"));
        command
            .arg("launch")
            .arg(tool)
            .env("ZENTTY_REAL_BINARY", &self.binary)
            .env("ZENTTY_CLI_BIN", env!("CARGO_BIN_EXE_zentty"))
            .env("ZENTTY_TEST_RECEIPT", &self.receipt);
        command
    }
}

impl Drop for FakeTool {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn real_cli_execs_codex_with_ephemeral_hooks_and_original_arguments() {
    let tool = FakeTool::new("codex");
    let output = tool.command("codex").arg("--help").output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt = fs::read_to_string(&tool.receipt).unwrap();
    assert!(receipt.contains("features.hooks=true"), "{receipt}");
    assert!(receipt.contains("hooks.PermissionRequest="), "{receipt}");
    assert!(receipt.contains("hooks.state="), "{receipt}");
    assert!(receipt.contains("--help"), "{receipt}");
    assert!(receipt.contains("AGENT=codex"), "{receipt}");
}

#[test]
fn real_cli_execs_claude_with_settings_and_clears_nested_marker() {
    let tool = FakeTool::new("claude");
    let output = tool
        .command("claude")
        .arg("hello")
        .env("CLAUDECODE", "nested")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt = fs::read_to_string(&tool.receipt).unwrap();
    assert!(receipt.contains("--session-id"), "{receipt}");
    assert!(receipt.contains("--settings"), "{receipt}");
    assert!(
        receipt.contains("agent-event --adapter=claude"),
        "{receipt}"
    );
    assert!(receipt.contains("AGENT=claude"), "{receipt}");
    assert!(receipt.contains("CLAUDECODE=\n"), "{receipt}");
    assert!(receipt.contains("hello"), "{receipt}");
}
