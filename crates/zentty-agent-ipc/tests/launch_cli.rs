use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FAKE_TOOL: AtomicU64 = AtomicU64::new(1);

struct FakeTool {
    directory: std::path::PathBuf,
    receipt: std::path::PathBuf,
    binary: std::path::PathBuf,
}

impl FakeTool {
    fn new(name: &str) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "zentty-agent-launch-{}-{}-{name}",
            std::process::id(),
            NEXT_FAKE_TOOL.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let receipt = directory.join("receipt");
        let binary = directory.join(name);
        fs::write(
            &binary,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" >\"$ZENTTY_TEST_RECEIPT\"\nprintf 'AGENT=%s\\n' \"${ZENTTY_AGENT_TOOL:-}\" >>\"$ZENTTY_TEST_RECEIPT\"\nprintf 'CLAUDECODE=%s\\n' \"${CLAUDECODE:-}\" >>\"$ZENTTY_TEST_RECEIPT\"\nprintf 'GEMINI_SETTINGS=%s\\n' \"${GEMINI_CLI_SYSTEM_SETTINGS_PATH:-}\" >>\"$ZENTTY_TEST_RECEIPT\"\nif [ -n \"${GEMINI_CLI_SYSTEM_SETTINGS_PATH:-}\" ]; then cp \"$GEMINI_CLI_SYSTEM_SETTINGS_PATH\" \"$ZENTTY_TEST_RECEIPT.settings\"; stat -c 'MODE=%a' \"$GEMINI_CLI_SYSTEM_SETTINGS_PATH\" >>\"$ZENTTY_TEST_RECEIPT\"; fi\n",
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

#[test]
fn real_cli_execs_gemini_with_a_private_merged_overlay_without_mutating_source() {
    let tool = FakeTool::new("gemini");
    let runtime = tool.directory.join("runtime");
    let source = tool.directory.join("system-settings.json");
    fs::create_dir_all(&runtime).unwrap();
    fs::write(
        &source,
        r#"{"theme":"Dracula","general":{"enableNotifications":false}}"#,
    )
    .unwrap();
    let original = fs::read(&source).unwrap();
    let output = tool
        .command("gemini")
        .args(["--model", "gemini-2.5-pro"])
        .env("ZENTTY_INSTANCE_SOCKET", runtime.join("instance.sock"))
        .env("ZENTTY_PANE_TOKEN", "private-pane-token")
        .env("GEMINI_CLI_SYSTEM_SETTINGS_PATH", &source)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(&source).unwrap(), original);
    let receipt = fs::read_to_string(&tool.receipt).unwrap();
    assert!(receipt.contains("--model\ngemini-2.5-pro"), "{receipt}");
    assert!(receipt.contains("AGENT=gemini"), "{receipt}");
    assert!(receipt.contains("MODE=600"), "{receipt}");
    let settings: serde_json::Value =
        serde_json::from_slice(&fs::read(tool.receipt.with_extension("settings")).unwrap())
            .unwrap();
    assert_eq!(settings["theme"], "Dracula");
    assert_eq!(settings["general"]["enableNotifications"], true);
    assert!(
        settings["hooks"]["Notification"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("--adapter=gemini")
    );
    let overlay = receipt
        .lines()
        .find_map(|line| line.strip_prefix("GEMINI_SETTINGS="))
        .unwrap();
    assert!(std::path::Path::new(overlay).starts_with(&runtime));
    assert_ne!(std::path::Path::new(overlay), source);
}

#[test]
fn gemini_overlay_refuses_a_substituted_or_non_private_runtime_root() {
    let tool = FakeTool::new("gemini");
    let runtime = tool.directory.join("runtime-attack");
    let victim = tool.directory.join("victim");
    fs::create_dir_all(&runtime).unwrap();
    fs::create_dir_all(&victim).unwrap();
    symlink(&victim, runtime.join("agent-overlays")).unwrap();
    let output = tool
        .command("gemini")
        .env("ZENTTY_INSTANCE_SOCKET", runtime.join("instance.sock"))
        .env("ZENTTY_PANE_TOKEN", "private-pane-token")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("not a private directory"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(fs::read_dir(&victim).unwrap().next().is_none());
    assert!(!tool.receipt.exists(), "the real executable must not run");

    fs::remove_file(runtime.join("agent-overlays")).unwrap();
    fs::create_dir(runtime.join("agent-overlays")).unwrap();
    fs::set_permissions(
        runtime.join("agent-overlays"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let output = tool
        .command("gemini")
        .env("ZENTTY_INSTANCE_SOCKET", runtime.join("instance.sock"))
        .env("ZENTTY_PANE_TOKEN", "private-pane-token")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not a private directory"));
    assert!(!tool.receipt.exists(), "the real executable must not run");
}
