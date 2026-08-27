use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use std::os::unix::process::ExitStatusExt;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;
use zentty_agent_ipc::AgentIpcServer;
use zentty_core::{AgentTarget, PaneTokenRegistry};

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
            "#!/bin/sh\nprintf '%s\\n' \"$@\" >\"$ZENTTY_TEST_RECEIPT\"\nprintf 'AGENT=%s\\n' \"${ZENTTY_AGENT_TOOL:-}\" >>\"$ZENTTY_TEST_RECEIPT\"\nprintf 'CANONICAL=%s\\n' \"${ZENTTY_AGENT_CANONICAL_NAME:-}\" >>\"$ZENTTY_TEST_RECEIPT\"\nprintf 'CLAUDECODE=%s\\n' \"${CLAUDECODE:-}\" >>\"$ZENTTY_TEST_RECEIPT\"\nprintf 'GEMINI_SETTINGS=%s\\n' \"${GEMINI_CLI_SYSTEM_SETTINGS_PATH:-}\" >>\"$ZENTTY_TEST_RECEIPT\"\nprintf 'COPILOT_HOME=%s\\n' \"${COPILOT_HOME:-}\" >>\"$ZENTTY_TEST_RECEIPT\"\nprintf 'CURSOR_CONFIG=%s\\n' \"${CURSOR_CONFIG_DIR:-}\" >>\"$ZENTTY_TEST_RECEIPT\"\nprintf 'OPENCODE_CONFIG=%s\\n' \"${OPENCODE_CONFIG_DIR:-}\" >>\"$ZENTTY_TEST_RECEIPT\"\nprintf 'SMALL_HOOKS=%s\\n' \"${SMALL_HARNESS_MANAGED_HOOKS_FILE:-}\" >>\"$ZENTTY_TEST_RECEIPT\"\nprintf 'CURSOR_PID=%s DROID_PID=%s KIMI_PID=%s GROK_PID=%s AGY_PID=%s HERMES_PID=%s VIBE_PID=%s COPILOT_PID=%s SMALL_PID=%s\\n' \"${ZENTTY_CURSOR_PID:-}\" \"${ZENTTY_DROID_PID:-}\" \"${ZENTTY_KIMI_PID:-}\" \"${ZENTTY_GROK_PID:-}\" \"${ZENTTY_AGY_PID:-}\" \"${ZENTTY_HERMES_PID:-}\" \"${ZENTTY_VIBE_PID:-}\" \"${ZENTTY_COPILOT_PID:-}\" \"${ZENTTY_SMALL_HARNESS_PID:-}\" >>\"$ZENTTY_TEST_RECEIPT\"\nprintf 'VIBE_HOOKS=%s\\n' \"${VIBE_ENABLE_EXPERIMENTAL_HOOKS:-}\" >>\"$ZENTTY_TEST_RECEIPT\"\nif [ -n \"${GEMINI_CLI_SYSTEM_SETTINGS_PATH:-}\" ]; then cp \"$GEMINI_CLI_SYSTEM_SETTINGS_PATH\" \"$ZENTTY_TEST_RECEIPT.settings\"; stat -c 'MODE=%a' \"$GEMINI_CLI_SYSTEM_SETTINGS_PATH\" >>\"$ZENTTY_TEST_RECEIPT\"; fi\nif [ -n \"${COPILOT_HOME:-}\" ]; then cp \"$COPILOT_HOME/config.json\" \"$ZENTTY_TEST_RECEIPT.copilot\"; stat -c 'COPILOT_MODE=%a' \"$COPILOT_HOME/config.json\" >>\"$ZENTTY_TEST_RECEIPT\"; fi\nif [ -n \"${CURSOR_CONFIG_DIR:-}\" ]; then cp \"$CURSOR_CONFIG_DIR/hooks.json\" \"$ZENTTY_TEST_RECEIPT.cursor\"; stat -c 'CURSOR_MODE=%a' \"$CURSOR_CONFIG_DIR/hooks.json\" >>\"$ZENTTY_TEST_RECEIPT\"; fi\nif [ -n \"${OPENCODE_CONFIG_DIR:-}\" ]; then cp \"$OPENCODE_CONFIG_DIR/plugins/zentty-opencode-zentty.js\" \"$ZENTTY_TEST_RECEIPT.opencode\"; stat -c 'OPENCODE_MODE=%a' \"$OPENCODE_CONFIG_DIR/plugins/zentty-opencode-zentty.js\" >>\"$ZENTTY_TEST_RECEIPT\"; fi\nif [ -n \"${SMALL_HARNESS_MANAGED_HOOKS_FILE:-}\" ]; then cp \"$SMALL_HARNESS_MANAGED_HOOKS_FILE\" \"$ZENTTY_TEST_RECEIPT.small\"; stat -c 'SMALL_MODE=%a' \"$SMALL_HARNESS_MANAGED_HOOKS_FILE\" >>\"$ZENTTY_TEST_RECEIPT\"; fi\n",
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
            .env("ZENTTY_TEST_RECEIPT", &self.receipt)
            .env(
                "ZENTTY_INSTANCE_SOCKET",
                self.directory.join("instance.sock"),
            )
            .env(
                "ZENTTY_PANE_TOKEN",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
            .env("ZENTTY_WORKLANE_ID", "test-lane")
            .env("ZENTTY_PANE_ID", "test-pane")
            .env("HOME", &self.directory)
            .env("XDG_CONFIG_HOME", self.directory.join(".config"))
            .env_remove("KIMI_CODE_HOME")
            .env_remove("HERMES_HOME");
        command
    }

    fn resource(&self, relative: &str, contents: &str) -> std::path::PathBuf {
        let path = self.directory.join("resources").join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, contents).unwrap();
        path
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

#[test]
fn real_cli_launches_every_persistent_agent_with_managed_hooks_and_pid() {
    for (tool_name, binary_name, installed_path, pid_key) in [
        (
            "droid",
            "droid",
            ".factory/settings.local.json",
            "DROID_PID=",
        ),
        ("kimi", "kimi", ".kimi-code/config.toml", "KIMI_PID="),
        (
            "grok",
            "grok",
            ".grok/hooks/zentty-status.json",
            "GROK_PID=",
        ),
        ("agy", "agy", ".gemini/config/hooks.json", "AGY_PID="),
        ("hermes", "hermes", ".hermes/config.yaml", "HERMES_PID="),
        ("vibe", "vibe", ".vibe/hooks.toml", "VIBE_PID="),
    ] {
        let tool = FakeTool::new(binary_name);
        let output = tool
            .command(tool_name)
            .args(["chat", "hello world"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "tool={tool_name} stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let receipt = fs::read_to_string(&tool.receipt).unwrap();
        assert!(
            receipt.starts_with("chat\nhello world\n"),
            "tool={tool_name} receipt={receipt}"
        );
        assert!(
            receipt.contains(&format!("AGENT={tool_name}")),
            "tool={tool_name} receipt={receipt}"
        );
        let pid_line = receipt.lines().find(|line| line.contains(pid_key)).unwrap();
        assert!(
            !pid_line.contains(&format!("{pid_key} ")),
            "tool={tool_name} receipt={receipt}"
        );
        assert!(
            tool.directory.join(installed_path).is_file(),
            "tool={tool_name}"
        );
        if tool_name == "vibe" {
            assert!(receipt.contains("VIBE_HOOKS=true"));
        }
    }
}

#[test]
fn cursor_launch_uses_private_hooks_without_mutating_user_configuration() {
    let tool = FakeTool::new("cursor-agent");
    let runtime = tool.directory.join("runtime");
    let source = tool.directory.join(".cursor");
    fs::create_dir_all(&runtime).unwrap();
    fs::create_dir_all(&source).unwrap();
    let user_hooks = br#"{"version":1,"hooks":{"sessionStart":[{"command":"user-hook"}]}}"#;
    fs::write(source.join("hooks.json"), user_hooks).unwrap();
    fs::write(source.join("state.db"), b"cursor-user-state").unwrap();

    let output = tool
        .command("cursor")
        .args(["--model", "cursor-fast"])
        .env("ZENTTY_INSTANCE_SOCKET", runtime.join("instance.sock"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(source.join("hooks.json")).unwrap(), user_hooks);
    let receipt = fs::read_to_string(&tool.receipt).unwrap();
    assert!(receipt.starts_with("--model\ncursor-fast\n"), "{receipt}");
    assert!(receipt.contains("AGENT=cursor"), "{receipt}");
    assert!(receipt.contains("CURSOR_MODE=600"), "{receipt}");
    let overlay = receipt
        .lines()
        .find_map(|line| line.strip_prefix("CURSOR_CONFIG="))
        .unwrap();
    assert!(std::path::Path::new(overlay).starts_with(runtime));
    assert_eq!(
        fs::read_link(std::path::Path::new(overlay).join("state.db")).unwrap(),
        source.join("state.db")
    );
    let hooks: serde_json::Value =
        serde_json::from_slice(&fs::read(tool.receipt.with_extension("cursor")).unwrap()).unwrap();
    assert_eq!(hooks["hooks"].as_object().unwrap().len(), 10);
    assert_eq!(hooks["hooks"]["preToolUse"][0]["matcher"], "TodoWrite");
}

#[test]
fn persistent_agent_passthrough_does_not_install_or_emit_status_environment() {
    let tool = FakeTool::new("kimi");
    let output = tool.command("kimi").arg("login").output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt = fs::read_to_string(&tool.receipt).unwrap();
    assert!(receipt.starts_with("login\n"));
    assert!(receipt.contains("AGENT=\n"));
    assert!(receipt.contains("KIMI_PID= "));
    assert!(!tool.directory.join(".kimi-code/config.toml").exists());
}

#[test]
fn cursor_private_overlay_ignores_but_preserves_malformed_user_hooks() {
    let tool = FakeTool::new("cursor-agent");
    fs::create_dir_all(tool.directory.join(".cursor")).unwrap();
    fs::write(tool.directory.join(".cursor/hooks.json"), "not-json").unwrap();
    let output = tool
        .command("cursor")
        .arg("chat")
        .env("ZENTTY_CLI_DEBUG", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!String::from_utf8_lossy(&output.stderr).contains("launching directly"));
    let receipt = fs::read_to_string(&tool.receipt).unwrap();
    assert!(receipt.starts_with("chat\n"));
    assert!(receipt.contains("AGENT=cursor\n"));
    assert!(!receipt.contains("CURSOR_PID= "));
    assert!(receipt.contains("CURSOR_MODE=600"));
    assert_eq!(
        fs::read(tool.directory.join(".cursor/hooks.json")).unwrap(),
        b"not-json"
    );
}

#[test]
fn persistent_agent_launch_outside_a_pane_is_a_direct_non_mutating_exec() {
    let tool = FakeTool::new("droid");
    let output = tool
        .command("droid")
        .arg("chat")
        .env_remove("ZENTTY_PANE_TOKEN")
        .env_remove("ZENTTY_INSTANCE_SOCKET")
        .env_remove("ZENTTY_WORKLANE_ID")
        .env_remove("ZENTTY_PANE_ID")
        .output()
        .unwrap();
    assert!(output.status.success());
    let receipt = fs::read_to_string(&tool.receipt).unwrap();
    assert!(receipt.contains("AGENT=\n"));
    assert!(receipt.contains("DROID_PID= "));
    assert!(!tool.directory.join(".factory/settings.local.json").exists());
}

#[test]
fn copilot_launch_uses_a_private_merged_home_and_preserves_source_arguments() {
    let tool = FakeTool::new("copilot");
    let runtime = tool.directory.join("runtime");
    let source = tool.directory.join("copilot-source");
    fs::create_dir_all(&runtime).unwrap();
    fs::create_dir_all(&source).unwrap();
    fs::write(
        source.join("config.json"),
        b"{// retained user config\n\"theme\":\"dark\",}\n",
    )
    .unwrap();
    fs::write(source.join("state.db"), b"source-state").unwrap();
    let original = fs::read(source.join("config.json")).unwrap();

    let output = tool
        .command("copilot")
        .args([
            "--config-dir",
            source.to_str().unwrap(),
            "--prompt",
            "hello; still one argument",
        ])
        .env("ZENTTY_INSTANCE_SOCKET", runtime.join("instance.sock"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(source.join("config.json")).unwrap(), original);
    let receipt = fs::read_to_string(&tool.receipt).unwrap();
    assert!(!receipt.contains("--config-dir"), "{receipt}");
    assert!(
        receipt.starts_with("--prompt\nhello; still one argument\n"),
        "{receipt}"
    );
    assert!(receipt.contains("AGENT=copilot"), "{receipt}");
    assert!(receipt.contains("COPILOT_MODE=600"), "{receipt}");
    let config: serde_json::Value =
        serde_json::from_slice(&fs::read(tool.receipt.with_extension("copilot")).unwrap()).unwrap();
    assert_eq!(config["theme"], "dark");
    assert_eq!(config["hooks"].as_object().unwrap().len(), 6);
    let overlay = receipt
        .lines()
        .find_map(|line| line.strip_prefix("COPILOT_HOME="))
        .unwrap();
    assert!(std::path::Path::new(overlay).starts_with(runtime));
    assert_eq!(
        fs::read_link(std::path::Path::new(overlay).join("state.db")).unwrap(),
        source.join("state.db")
    );
}

#[test]
fn copilot_malformed_source_config_is_preserved_without_blocking_exec() {
    let tool = FakeTool::new("copilot");
    let source = tool.directory.join("copilot-malformed");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("config.json"), b"not-json-user-content").unwrap();
    let output = tool
        .command("copilot")
        .args(["--config-dir", source.to_str().unwrap(), "chat"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(tool.receipt.with_extension("copilot")).unwrap(),
        b"not-json-user-content"
    );
    assert_eq!(
        fs::read(source.join("config.json")).unwrap(),
        b"not-json-user-content"
    );
}

#[test]
fn opencode_launch_copies_user_config_and_the_staged_plugin_into_a_private_overlay() {
    let tool = FakeTool::new("opencode");
    let runtime = tool.directory.join("runtime");
    let source = tool.directory.join("opencode-source");
    fs::create_dir_all(&runtime).unwrap();
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("opencode.json"), b"{\"model\":\"user-choice\"}").unwrap();
    let synced_theme = tool.directory.join("zentty-synced.json");
    fs::write(
        &synced_theme,
        b"{\"$schema\":\"https://opencode.ai/theme.json\",\"theme\":{}}",
    )
    .unwrap();
    tool.resource(
        "opencode/plugins/zentty-opencode-zentty.js",
        "export const source = 'zentty';\n",
    );

    let output = tool
        .command("opencode")
        .arg("run")
        .env("ZENTTY_INSTANCE_SOCKET", runtime.join("instance.sock"))
        .env("ZENTTY_PANE_TOKEN", "a".repeat(64))
        .env("ZENTTY_OPENCODE_SYNC_THEME_FILE", &synced_theme)
        .env("OPENCODE_CONFIG_DIR", &source)
        .env("ZENTTY_RESOURCE_ROOT", tool.directory.join("resources"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(source.join("opencode.json")).unwrap(),
        b"{\"model\":\"user-choice\"}"
    );
    let receipt = fs::read_to_string(&tool.receipt).unwrap();
    assert!(receipt.starts_with("run\n"), "{receipt}");
    assert!(receipt.contains("AGENT=opencode"), "{receipt}");
    assert!(receipt.contains("OPENCODE_MODE=600"), "{receipt}");
    assert_eq!(
        fs::read(tool.receipt.with_extension("opencode")).unwrap(),
        b"export const source = 'zentty';\n"
    );
    let overlay = receipt
        .lines()
        .find_map(|line| line.strip_prefix("OPENCODE_CONFIG="))
        .unwrap();
    assert!(std::path::Path::new(overlay).starts_with(runtime));
    assert_eq!(
        fs::read(std::path::Path::new(overlay).join("opencode.json")).unwrap(),
        b"{\"model\":\"user-choice\"}"
    );
    let tui: serde_json::Value =
        serde_json::from_slice(&fs::read(std::path::Path::new(overlay).join("tui.json")).unwrap())
            .unwrap();
    assert_eq!(tui["theme"], "zentty-synced");
    assert_eq!(
        fs::read(
            std::path::Path::new(overlay)
                .join("themes")
                .join("zentty-synced.json")
        )
        .unwrap(),
        fs::read(synced_theme).unwrap()
    );
}

#[test]
fn opencode_rejects_source_symlinks_before_exec_and_selects_its_real_sibling() {
    let tool = FakeTool::new("opencode");
    let source = tool.directory.join("opencode-source");
    fs::create_dir_all(&source).unwrap();
    fs::write(tool.directory.join("outside.json"), b"private").unwrap();
    symlink(
        tool.directory.join("outside.json"),
        source.join("linked.json"),
    )
    .unwrap();
    tool.resource(
        "opencode/plugins/zentty-opencode-zentty.js",
        "export default {};\n",
    );
    let rejected = tool
        .command("opencode")
        .env("OPENCODE_CONFIG_DIR", &source)
        .env("ZENTTY_RESOURCE_ROOT", tool.directory.join("resources"))
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("contains a symlink"),
        "{}",
        String::from_utf8_lossy(&rejected.stderr)
    );
    assert!(!tool.receipt.exists());

    fs::remove_file(source.join("linked.json")).unwrap();
    let sibling = tool.directory.join(".opencode");
    fs::write(
        &sibling,
        "#!/bin/sh\nprintf 'SIBLING=%s\\n' \"$*\" >\"$ZENTTY_TEST_RECEIPT\"\n",
    )
    .unwrap();
    fs::set_permissions(&sibling, fs::Permissions::from_mode(0o700)).unwrap();
    let selected = tool
        .command("opencode")
        .args(["run", "hostile path;$HOME"])
        .env("OPENCODE_CONFIG_DIR", &source)
        .env("ZENTTY_RESOURCE_ROOT", tool.directory.join("resources"))
        .output()
        .unwrap();
    assert!(selected.status.success());
    assert_eq!(
        fs::read_to_string(&tool.receipt).unwrap(),
        "SIBLING=run hostile path;$HOME\n"
    );
}

#[test]
fn opencode_prelaunch_event_crosses_the_authenticated_socket_before_exec() {
    let tool = FakeTool::new("opencode");
    tool.resource(
        "opencode/plugins/zentty-opencode-zentty.js",
        "export default {};\n",
    );
    let mut registry = PaneTokenRegistry::default();
    registry
        .register(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            AgentTarget::new("window", "test-lane", "test-pane"),
        )
        .unwrap();
    let (sender, receiver) = mpsc::channel();
    let server = AgentIpcServer::start(
        tool.directory.join("instance.sock"),
        Arc::new(Mutex::new(registry)),
        sender,
    )
    .unwrap();
    let output = tool
        .command("opencode")
        .arg("run")
        .env("ZENTTY_RESOURCE_ROOT", tool.directory.join("resources"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let event = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(
        event.target,
        AgentTarget::new("window", "test-lane", "test-pane")
    );
    assert_eq!(
        serde_json::to_value(event.event).unwrap()["event"],
        "session.start"
    );
    server.shutdown().unwrap();
}

#[test]
fn prelaunch_delivery_failure_is_best_effort_and_only_debug_logged_on_request() {
    for debug in [false, true] {
        let tool = FakeTool::new("pi");
        tool.resource("pi/extensions/zentty-pi-zentty.js", "// extension\n");
        let mut command = tool.command("pi");
        command.env("ZENTTY_RESOURCE_ROOT", tool.directory.join("resources"));
        if debug {
            command.env("ZENTTY_CLI_DEBUG", "1");
        }
        let output = command.output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stderr)
                .contains("pre-launch agent event was not delivered"),
            debug
        );
    }
}

#[test]
fn pi_family_launches_inject_only_the_matching_staged_extension() {
    for (tool_name, relative, canonical) in [
        ("pi", "pi/extensions/zentty-pi-zentty.js", "Pi"),
        ("omp", "omp/extensions/zentty-omp-zentty.js", "OMP"),
    ] {
        let tool = FakeTool::new(tool_name);
        let extension = tool.resource(relative, "// staged extension\n");
        let output = tool
            .command(tool_name)
            .args(["--model", "local"])
            .env("ZENTTY_RESOURCE_ROOT", tool.directory.join("resources"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "tool={tool_name} stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let receipt = fs::read_to_string(&tool.receipt).unwrap();
        assert!(
            receipt.starts_with(&format!("-e\n{}\n--model\nlocal\n", extension.display())),
            "tool={tool_name} receipt={receipt}"
        );
        assert!(receipt.contains(&format!("AGENT={tool_name}")), "{receipt}");
        assert!(
            receipt.contains(&format!("CANONICAL={canonical}")),
            "{receipt}"
        );
    }
}

#[test]
fn management_commands_do_not_create_pi_family_overlays_or_status_environment() {
    for (tool_name, command) in [("pi", "install"), ("omp", "models")] {
        let tool = FakeTool::new(tool_name);
        tool.resource(
            &format!("{tool_name}/extensions/zentty-{tool_name}-zentty.js"),
            "// must not be selected\n",
        );
        let output = tool
            .command(tool_name)
            .arg(command)
            .env("ZENTTY_RESOURCE_ROOT", tool.directory.join("resources"))
            .output()
            .unwrap();
        assert!(output.status.success());
        let receipt = fs::read_to_string(&tool.receipt).unwrap();
        assert!(receipt.starts_with(&format!("{command}\n")), "{receipt}");
        assert!(receipt.contains("AGENT=\n"), "{receipt}");
        assert!(!receipt.starts_with("-e\n"), "{receipt}");
        assert!(!tool.directory.join("agent-overlays").exists());
    }
}

#[test]
fn small_harness_launch_uses_a_complete_private_ephemeral_hook_file() {
    let tool = FakeTool::new("small-harness");
    let runtime = tool.directory.join("runtime");
    fs::create_dir_all(&runtime).unwrap();
    let output = tool
        .command("small-harness")
        .arg("chat")
        .env("ZENTTY_INSTANCE_SOCKET", runtime.join("instance.sock"))
        .env("SMALL_HARNESS_MANAGED_HOOKS_JSON", "stale")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt = fs::read_to_string(&tool.receipt).unwrap();
    assert!(receipt.starts_with("chat\n"), "{receipt}");
    assert!(receipt.contains("AGENT=small-harness"), "{receipt}");
    assert!(receipt.contains("SMALL_MODE=600"), "{receipt}");
    let hooks: serde_json::Value =
        serde_json::from_slice(&fs::read(tool.receipt.with_extension("small")).unwrap()).unwrap();
    assert_eq!(hooks["source"], "zentty");
    assert_eq!(hooks["hooks"].as_object().unwrap().len(), 12);
    assert!(
        hooks["hooks"]["SessionStart"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("--adapter=small-harness")
    );
}

#[test]
fn managed_exec_preserves_cwd_streams_exit_status_and_signal_identity() {
    let exiting = FakeTool::new("small-harness");
    fs::write(
        &exiting.binary,
        "#!/bin/sh\nprintf 'stdout:<%s> cwd:<%s>\\n' \"$1\" \"$PWD\"\nprintf 'stderr:<%s>\\n' \"$2\" >&2\nexit 37\n",
    )
    .unwrap();
    let output = exiting
        .command("small-harness")
        .args(["hostile arg;$HOME", "error arg;`literal`"])
        .current_dir(&exiting.directory)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(37));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!(
            "stdout:<hostile arg;$HOME> cwd:<{}>\n",
            exiting.directory.display()
        )
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "stderr:<error arg;`literal`>\n"
    );

    let signalled = FakeTool::new("small-harness");
    fs::write(
        &signalled.binary,
        "#!/bin/sh\nprintf started >\"$ZENTTY_TEST_RECEIPT\"\nexec sleep 30\n",
    )
    .unwrap();
    let mut child = signalled.command("small-harness").spawn().unwrap();
    for _ in 0..200 {
        if signalled.receipt.is_file() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(signalled.receipt.is_file());
    child.kill().unwrap();
    assert_eq!(child.wait().unwrap().signal(), Some(9));
}
