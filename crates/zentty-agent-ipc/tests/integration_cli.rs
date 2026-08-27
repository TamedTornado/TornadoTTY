use serde_json::Value;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestHome(PathBuf);

impl TestHome {
    fn new(label: &str) -> Self {
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "zentty integration {label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn run(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_zentty"))
            .args(arguments)
            .env("HOME", &self.0)
            .env("XDG_CONFIG_HOME", self.0.join("xdg-config"))
            .env_remove("KIMI_CODE_HOME")
            .env_remove("HERMES_HOME")
            .output()
            .unwrap()
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        if self.0.exists() {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn real_cursor_install_is_idempotent_and_uninstall_preserves_user_entries() {
    let home = TestHome::new("cursor");
    let path = home.path(".cursor/hooks.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        br#"{
          // Cursor permits JSON-with-comments.
          "version": 9,
          /* Preserve future user-owned data. */
          "future": {"keep": true,},
          "hooks": {"sessionStart": [{"command": "user-hook"}],},
        }"#,
    )
    .unwrap();

    assert_success(&home.run(&["install", "cursor-hooks"]));
    let first = fs::read(&path).unwrap();
    assert_success(&home.run(&["install", "cursor-hooks"]));
    assert_eq!(fs::read(&path).unwrap(), first);

    let installed = read_json(&path);
    assert_eq!(installed["version"], 1);
    assert_eq!(installed["future"]["keep"], true);
    assert_eq!(
        installed["hooks"]["sessionStart"][0]["command"],
        "user-hook"
    );
    for event in [
        "sessionStart",
        "sessionEnd",
        "beforeSubmitPrompt",
        "stop",
        "beforeShellExecution",
        "afterShellExecution",
        "preToolUse",
        "postToolUse",
        "subagentStart",
        "subagentStop",
    ] {
        let entries = installed["hooks"][event].as_array().unwrap();
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry["command"]
                    .as_str()
                    .is_some_and(|value| value.contains("--adapter=cursor")))
                .count(),
            1,
            "event={event}"
        );
    }

    assert_success(&home.run(&["uninstall", "cursor-hooks"]));
    let removed = read_json(&path);
    assert_eq!(removed["future"]["keep"], true);
    assert_eq!(removed["hooks"]["sessionStart"][0]["command"], "user-hook");
    assert!(
        !String::from_utf8(fs::read(path).unwrap())
            .unwrap()
            .contains("--adapter=cursor")
    );
}

#[test]
fn real_droid_install_preserves_explicit_output_policy_and_nested_user_hooks() {
    let home = TestHome::new("droid");
    let settings = home.path(".factory/settings.local.json");
    let output = home.path(".factory/hooks/hooks.json");
    fs::create_dir_all(settings.parent().unwrap()).unwrap();
    fs::create_dir_all(output.parent().unwrap()).unwrap();
    fs::write(
        &settings,
        br#"{"future":3,"hooks":{"Stop":[{"hooks":[{"type":"command","command":"user-stop"}]}]}}"#,
    )
    .unwrap();
    fs::write(&output, br#"{"showHookOutput":true,"future":4}"#).unwrap();

    assert_success(&home.run(&["install", "droid-hooks"]));
    assert_success(&home.run(&["install", "droid-hooks"]));
    let installed = read_json(&settings);
    assert_eq!(installed["future"], 3);
    assert_eq!(installed["hooks"]["Stop"].as_array().unwrap().len(), 2);
    assert_eq!(read_json(&output)["showHookOutput"], true);
    assert_success(&home.run(&["uninstall", "droid-hooks"]));
    let removed = read_json(&settings);
    assert_eq!(removed["hooks"]["Stop"].as_array().unwrap().len(), 1);
    assert_eq!(
        removed["hooks"]["Stop"][0]["hooks"][0]["command"],
        "user-stop"
    );
}

#[test]
fn real_vibe_install_replaces_one_owned_block_and_uninstall_restores_user_content() {
    let home = TestHome::new("vibe");
    let path = home.path(".vibe/hooks.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let user = "# user hook\n[[hooks]]\nname = \"mine\"\n";
    fs::write(&path, user).unwrap();

    assert_success(&home.run(&["install", "vibe-hooks"]));
    let first = fs::read_to_string(&path).unwrap();
    assert_eq!(first.matches("# [Zentty Managed Hooks - Begin]").count(), 1);
    assert_eq!(first.matches("ipc agent-event --adapter=vibe").count(), 4);
    assert!(first.starts_with(user));
    assert_success(&home.run(&["install", "vibe-hooks"]));
    assert_eq!(fs::read_to_string(&path).unwrap(), first);

    assert_success(&home.run(&["uninstall", "vibe-hooks"]));
    assert_eq!(fs::read_to_string(path).unwrap(), user);
}

#[test]
fn malformed_or_symlinked_user_config_fails_without_mutation() {
    let home = TestHome::new("refuse");
    let cursor = home.path(".cursor/hooks.json");
    fs::create_dir_all(cursor.parent().unwrap()).unwrap();
    fs::write(&cursor, b"not-json").unwrap();
    let output = home.run(&["install", "cursor-hooks"]);
    assert!(!output.status.success());
    assert_eq!(fs::read(&cursor).unwrap(), b"not-json");

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let target = home.path("outside.json");
        fs::write(&target, b"{}").unwrap();
        fs::remove_file(&cursor).unwrap();
        symlink(&target, &cursor).unwrap();
        let output = home.run(&["install", "cursor-hooks"]);
        assert!(!output.status.success());
        assert_eq!(fs::read(&target).unwrap(), b"{}");
    }
}

#[test]
fn real_kimi_install_manages_both_supported_config_roots() {
    let home = TestHome::new("kimi");
    for relative in [".kimi/config.toml", ".kimi-code/config.toml"] {
        let path = home.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let initial = if relative.contains("kimi-code") {
            "model = \"user-choice\"\nhooks = [{ event = \"Mine\", command = \"user\" }]\n"
        } else {
            "model = \"user-choice\"\n"
        };
        fs::write(&path, initial).unwrap();
    }
    assert_success(&home.run(&["install", "kimi-hooks"]));
    assert_success(&home.run(&["install", "kimi-hooks"]));
    let children = (0..2)
        .map(|_| {
            Command::new(env!("CARGO_BIN_EXE_zentty"))
                .args(["install", "kimi-hooks"])
                .env("HOME", &home.0)
                .env("XDG_CONFIG_HOME", home.0.join("xdg-config"))
                .env_remove("KIMI_CODE_HOME")
                .spawn()
                .unwrap()
        })
        .collect::<Vec<_>>();
    for child in children {
        assert!(child.wait_with_output().unwrap().status.success());
    }
    for relative in [".kimi/config.toml", ".kimi-code/config.toml"] {
        let text = fs::read_to_string(home.path(relative)).unwrap();
        assert_eq!(text.matches("### BEGIN ZENTTY KIMI HOOKS").count(), 1);
        assert_eq!(text.matches("--adapter=kimi").count(), 7);
        assert!(text.contains("model = \"user-choice\""));
        if relative.contains("kimi-code") {
            assert!(text.contains("event = \"Mine\""));
        }
    }
    assert_success(&home.run(&["uninstall", "kimi-hooks"]));
    for relative in [".kimi/config.toml", ".kimi-code/config.toml"] {
        let text = fs::read_to_string(home.path(relative)).unwrap();
        assert!(text.contains("model = \"user-choice\""));
        assert!(!text.contains("ZENTTY KIMI"));
        if relative.contains("kimi-code") {
            assert!(text.contains("event = \"Mine\""));
        }
    }
}

#[test]
fn real_grok_install_uses_one_owned_config_and_executable_forwarder() {
    let home = TestHome::new("grok");
    assert_success(&home.run(&["install", "grok-hooks"]));
    let config = home.path(".grok/hooks/zentty-status.json");
    let script = home.path(".grok/hooks/zentty-status/01-zentty-status.sh");
    let json = read_json(&config);
    assert_eq!(json["hooks"].as_object().unwrap().len(), 9);
    assert_eq!(json["hooks"]["PreToolUse"][0]["matcher"], ".*");
    assert_ne!(
        fs::metadata(&script).unwrap().permissions().mode() & 0o100,
        0
    );
    assert_success(&home.run(&["uninstall", "grok-hooks"]));
    assert!(!config.exists());
    assert!(!script.exists());
}

#[test]
fn real_agy_install_preserves_foreign_groups_and_owns_exactly_one_group() {
    let home = TestHome::new("agy");
    let path = home.path(".gemini/config/hooks.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, br#"{"foreign":{"keep":true}}"#).unwrap();
    assert_success(&home.run(&["install", "agy-hooks"]));
    assert_success(&home.run(&["install", "agy-hooks"]));
    let json = read_json(&path);
    assert_eq!(json["foreign"]["keep"], true);
    assert_eq!(json["zentty"].as_object().unwrap().len(), 8);
    assert!(json["zentty"].to_string().contains("zentty-agy-hook-v1"));
    assert_success(&home.run(&["uninstall", "agy-hooks"]));
    assert_eq!(
        read_json(&path),
        serde_json::json!({"foreign":{"keep":true}})
    );
}

#[test]
fn real_hermes_install_preserves_yaml_and_allowlist_entries() {
    let home = TestHome::new("hermes");
    let config = home.path(".hermes/config.yaml");
    let allowlist = home.path(".hermes/shell-hooks-allowlist.json");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(&config, "model: user\nhooks:\n").unwrap();
    fs::write(
        &allowlist,
        br#"{"future":{"keep":true},"approvals":[{"command":"user","event":"mine"}]}"#,
    )
    .unwrap();
    assert_success(&home.run(&["install", "hermes-hooks"]));
    assert_success(&home.run(&["install", "hermes-hooks"]));
    let yaml = fs::read_to_string(&config).unwrap();
    assert_eq!(yaml.matches("# zentty hermes hooks begin").count(), 1);
    assert_eq!(yaml.matches("hooks/zentty-status/").count(), 10);
    assert!(yaml.contains("model: user"));
    assert_eq!(
        read_json(&allowlist)["approvals"].as_array().unwrap().len(),
        11
    );
    assert_eq!(read_json(&allowlist)["future"]["keep"], true);
    assert_success(&home.run(&["uninstall", "hermes-hooks"]));
    assert!(fs::read_to_string(config).unwrap().contains("model: user"));
    assert_eq!(
        read_json(&allowlist)["approvals"].as_array().unwrap().len(),
        1
    );
    assert_eq!(read_json(&allowlist)["future"]["keep"], true);
}
