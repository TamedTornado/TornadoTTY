use serde_json::{Map, Value, json};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use zentty_core::{AtomicFileAction, AtomicFileStore};

const MAX_BYTES: usize = 1024 * 1024;
const KIMI_BEGIN: &str = "### BEGIN ZENTTY KIMI HOOKS";
const KIMI_END: &str = "### END ZENTTY KIMI HOOKS";
const HERMES_BEGIN: &str = "# zentty hermes hooks begin";
const HERMES_END: &str = "# zentty hermes hooks end";
const AGY_MARKER: &str = "zentty-agy-hook-v1";
const GROK_MARKER: &str = "ipc agent-event --adapter=grok";

pub(super) fn install(target: &str, home: &Path, cli: &Path) -> Result<String, String> {
    match target {
        "kimi-hooks" => install_kimi(home, cli),
        "grok-hooks" => install_grok(home, cli),
        "agy-hooks" => install_agy(home, cli),
        "hermes-hooks" => install_hermes(home, cli),
        _ => Err(format!("unknown integration target {target:?}")),
    }
}

pub(super) fn uninstall(target: &str, home: &Path) -> Result<String, String> {
    match target {
        "kimi-hooks" => uninstall_kimi(home),
        "grok-hooks" => uninstall_grok(home),
        "agy-hooks" => uninstall_agy(home),
        "hermes-hooks" => uninstall_hermes(home),
        _ => Err(format!("unknown integration target {target:?}")),
    }
}

fn install_kimi(home: &Path, cli: &Path) -> Result<String, String> {
    let command = shell_command(cli, "ipc agent-event --adapter=kimi")?;
    let entries = [
        "SessionStart",
        "SessionEnd",
        "UserPromptSubmit",
        "Stop",
        "Notification",
        "PreToolUse",
        "PostToolUse",
    ]
    .iter()
    .map(|event| {
        format!(
            "[[hooks]]\nevent = \"{event}\"\ncommand = \"{}\"",
            toml_escape(&command)
        )
    })
    .collect::<Vec<_>>();
    let block = format!(
        "{KIMI_BEGIN}\n# zentty-managed-style = arrayTables\n{}\n{KIMI_END}\n",
        entries.join("\n\n")
    );
    for path in [home.join(".kimi/config.toml"), kimi_modern_path(home)] {
        install_kimi_file(&path, &command, &block)?;
    }
    Ok("Installed Zentty Kimi hooks for legacy and modern Kimi.".to_owned())
}

fn uninstall_kimi(home: &Path) -> Result<String, String> {
    for path in [home.join(".kimi/config.toml"), kimi_modern_path(home)] {
        uninstall_kimi_file(&path)?;
    }
    Ok("Removed Zentty Kimi hook entries.".to_owned())
}

fn install_kimi_file(path: &Path, command: &str, array_table_block: &str) -> Result<(), String> {
    let existing = read_text(path)?;
    let clean = uninstall_kimi_text(&existing)?;
    if let Some(open) = inline_hooks_open(&clean) {
        let has_elements = clean[open + 1..].contains('{');
        let comma = if has_elements { "," } else { "" };
        let inline = [
            "SessionStart",
            "SessionEnd",
            "UserPromptSubmit",
            "Stop",
            "Notification",
            "PreToolUse",
            "PostToolUse",
        ]
        .iter()
        .map(|event| {
            format!(
                "  {{ event = \"{event}\", command = \"{}\" }},",
                toml_escape(command)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
        let inline = format!(
            "\n{KIMI_BEGIN}\n# zentty-managed-style = inlineArray\n{}{comma}\n{KIMI_END}\n",
            inline.trim_end_matches(',')
        );
        let mut next = clean;
        next.insert_str(open + 1, &inline);
        replace_text(path, &next)
    } else {
        update_managed_text(path, KIMI_BEGIN, KIMI_END, array_table_block)
    }
}

fn uninstall_kimi_file(path: &Path) -> Result<(), String> {
    let existing = read_text(path)?;
    if existing.is_empty() {
        return Ok(());
    }
    let next = uninstall_kimi_text(&existing)?;
    let store = AtomicFileStore::new(path, MAX_BYTES);
    store
        .transaction(|_| {
            if next.trim().is_empty() {
                Ok(AtomicFileAction::Remove(()))
            } else {
                Ok(AtomicFileAction::Replace {
                    bytes: format!("{}\n", next.trim_end()).into_bytes(),
                    value: (),
                })
            }
        })
        .map(|(value, _)| value)
        .map_err(|error| error.to_string())
}

fn uninstall_kimi_text(source: &str) -> Result<String, String> {
    let Some(start) = source.find(KIMI_BEGIN) else {
        return if source.contains(KIMI_END) {
            Err("managed hook block markers are malformed; refusing to overwrite".to_owned())
        } else {
            Ok(source.to_owned())
        };
    };
    let finish = source.find(KIMI_END).ok_or_else(|| {
        "managed hook block markers are malformed; refusing to overwrite".to_owned()
    })?;
    if start > finish {
        return Err("managed hook block markers are malformed; refusing to overwrite".to_owned());
    }
    let mut end = finish + KIMI_END.len();
    if source.as_bytes().get(end) == Some(&b'\n') {
        end += 1;
    }
    Ok(format!("{}{}", &source[..start], &source[end..]))
}

fn inline_hooks_open(source: &str) -> Option<usize> {
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if let Some(after_key) = trimmed.strip_prefix("hooks") {
            let after_key = after_key.trim_start();
            if let Some(value) = after_key.strip_prefix('=')
                && value.trim_start().starts_with('[')
            {
                return line.find('[').map(|index| offset + index);
            }
        }
        offset += line.len();
    }
    None
}

fn kimi_modern_path(home: &Path) -> PathBuf {
    env::var_os("KIMI_CODE_HOME")
        .filter(|value| !value.is_empty())
        .map_or_else(|| home.join(".kimi-code"), PathBuf::from)
        .join("config.toml")
}

fn install_grok(home: &Path, cli: &Path) -> Result<String, String> {
    let root = home.join(".grok/hooks");
    let script = root.join("zentty-status/01-zentty-status.sh");
    let cli = shell_single_quote(path_text(cli)?);
    let script_bytes = format!(
        "#!/usr/bin/env bash\n# Zentty-managed hook for Grok Build status reporting.\n# Marker: {GROK_MARKER}\nZENTTY_BIN='{cli}'\nif [[ ! -x \"$ZENTTY_BIN\" ]]; then ZENTTY_BIN=\"$(command -v zentty 2>/dev/null || true)\"; fi\n[[ -n \"$ZENTTY_BIN\" ]] || exit 0\nexec \"$ZENTTY_BIN\" ipc agent-event --adapter=grok\n"
    );
    AtomicFileStore::new(&script, MAX_BYTES)
        .replace_bytes(script_bytes.as_bytes())
        .map_err(|error| error.to_string())?;
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("could not make {} executable: {error}", script.display()))?;

    let lifecycle = [
        "SessionStart",
        "SessionEnd",
        "UserPromptSubmit",
        "Stop",
        "Notification",
        "BeforeAgent",
        "AfterAgent",
    ];
    let tools = ["PreToolUse", "PostToolUse"];
    let mut hooks = Map::new();
    for event in lifecycle {
        hooks.insert(
            event.to_owned(),
            json!([{"hooks":[{"type":"command","command":script,"timeout":15}]}]),
        );
    }
    for event in tools {
        hooks.insert(
            event.to_owned(),
            json!([{"matcher":".*","hooks":[{"type":"command","command":script,"timeout":15}]}]),
        );
    }
    write_json(
        &root.join("zentty-status.json"),
        &Value::Object(Map::from_iter([("hooks".to_owned(), Value::Object(hooks))])),
    )?;
    Ok(format!(
        "Installed Zentty Grok hooks at {}.",
        root.display()
    ))
}

fn uninstall_grok(home: &Path) -> Result<String, String> {
    let root = home.join(".grok/hooks");
    remove_owned_file(&root.join("zentty-status.json"), |bytes| {
        String::from_utf8_lossy(bytes).contains("zentty-status")
    })?;
    remove_owned_file(&root.join("zentty-status/01-zentty-status.sh"), |bytes| {
        String::from_utf8_lossy(bytes).contains(GROK_MARKER)
    })?;
    Ok(format!(
        "Removed Zentty Grok hooks from {}.",
        root.display()
    ))
}

fn install_agy(home: &Path, cli: &Path) -> Result<String, String> {
    let path = home.join(".gemini/config/hooks.json");
    let events = [
        ("SessionStart", "session-start", 15, false),
        ("PreInvocation", "prompt-submit", 15, false),
        ("Stop", "stop", 15, false),
        ("turn-completion", "turn-completion", 15, false),
        ("Notification", "notification", 15, false),
        ("SessionEnd", "session-end", 15, false),
        ("PreToolUse", "pre-tool-use", 120, true),
        ("PostToolUse", "post-tool-use", 120, true),
    ];
    update_json(&path, |root| {
        if root
            .get("zentty")
            .is_some_and(|group| !contains_text(group, AGY_MARKER))
        {
            return Err(
                "refusing to replace an unowned Antigravity `zentty` hook group".to_owned(),
            );
        }
        let mut group = Map::new();
        for (agent_event, cli_event, timeout, wrapped) in events {
            let command = format!(
                ": {AGY_MARKER}; {} agy-hook {cli_event} 2>/dev/null || echo '{{}}'",
                shell_command(cli, "")?.trim_end()
            );
            let entry = json!({"type":"command","command":command,"timeout":timeout});
            group.insert(
                agent_event.to_owned(),
                if wrapped {
                    json!([{"matcher":"*","hooks":[entry]}])
                } else {
                    json!([entry])
                },
            );
        }
        root.insert("zentty".to_owned(), Value::Object(group));
        Ok(true)
    })?;
    Ok(format!(
        "Installed Zentty Antigravity hooks at {}.",
        path.display()
    ))
}

fn uninstall_agy(home: &Path) -> Result<String, String> {
    let path = home.join(".gemini/config/hooks.json");
    update_json(&path, |root| {
        let Some(group) = root.get("zentty") else {
            return Ok(false);
        };
        if !contains_text(group, AGY_MARKER) {
            return Err("refusing to remove an unowned Antigravity `zentty` hook group".to_owned());
        }
        root.remove("zentty");
        Ok(true)
    })?;
    Ok(format!(
        "Removed Zentty Antigravity hooks from {}.",
        path.display()
    ))
}

fn install_hermes(home: &Path, cli: &Path) -> Result<String, String> {
    let root = hermes_home(home);
    let events = [
        ("on_session_start", "on-session-start", 5),
        ("on_session_reset", "on-session-reset", 5),
        ("pre_llm_call", "pre-llm-call", 5),
        ("post_llm_call", "post-llm-call", 5),
        ("on_session_end", "on-session-end", 5),
        ("on_session_finalize", "on-session-finalize", 5),
        ("pre_tool_call", "pre-tool-call", 5),
        ("post_tool_call", "post-tool-call", 5),
        ("pre_approval_request", "pre-approval-request", 30),
        ("post_approval_response", "post-approval-response", 5),
    ];
    let scripts = root.join("hooks/zentty-status");
    let mut yaml = format!("  {HERMES_BEGIN}\n");
    let mut approvals = Vec::new();
    for (name, cli_event, timeout) in events {
        let script = scripts.join(format!("{cli_event}.sh"));
        let command = path_text(&script)?;
        let contents = format!(
            "#!/usr/bin/env bash\n# zentty hermes hook script v1\nexec {} hermes-hook {cli_event}\n",
            shell_command(cli, "")?.trim_end()
        );
        AtomicFileStore::new(&script, MAX_BYTES)
            .replace_bytes(contents.as_bytes())
            .map_err(|error| error.to_string())?;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755))
            .map_err(|error| error.to_string())?;
        write!(
            yaml,
            "  {name}:\n    - command: \"{}\"\n      timeout: {timeout}\n",
            yaml_escape(command)
        )
        .expect("writing to a String cannot fail");
        approvals.push(json!({
            "command": command,
            "event": name,
            "approved_at": "1970-01-01T00:00:00Z"
        }));
    }
    writeln!(yaml, "  {HERMES_END}").expect("writing to a String cannot fail");
    update_hermes_yaml(&root.join("config.yaml"), &yaml)?;
    let allowlist = root.join("shell-hooks-allowlist.json");
    update_json(&allowlist, |root| {
        let items = root
            .entry("approvals".to_owned())
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .ok_or_else(|| "Hermes allowlist `approvals` must be an array".to_owned())?;
        items.retain(|item| !is_hermes_approval(item));
        items.extend(approvals);
        Ok(true)
    })?;
    Ok(format!(
        "Installed Zentty Hermes hooks at {}.",
        root.display()
    ))
}

fn uninstall_hermes(home: &Path) -> Result<String, String> {
    let root = hermes_home(home);
    remove_managed_text(&root.join("config.yaml"), HERMES_BEGIN, HERMES_END)?;
    update_json(&root.join("shell-hooks-allowlist.json"), |allowlist| {
        let Some(items) = allowlist.get_mut("approvals").and_then(Value::as_array_mut) else {
            return Ok(false);
        };
        let before = items.len();
        items.retain(|item| !is_hermes_approval(item));
        Ok(before != items.len())
    })?;
    let scripts = root.join("hooks/zentty-status");
    if scripts.exists() {
        for entry in fs::read_dir(&scripts).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "lock")
            {
                continue;
            }
            remove_owned_file(&path, |bytes| {
                String::from_utf8_lossy(bytes).contains("zentty hermes hook script v1")
            })?;
        }
    }
    Ok(format!(
        "Removed Zentty Hermes hooks from {}.",
        root.display()
    ))
}

fn is_hermes_approval(value: &Value) -> bool {
    value
        .get("command")
        .and_then(Value::as_str)
        .is_some_and(|command| {
            command.contains("/hooks/zentty-status/") || command.contains("zentty hermes-hook")
        })
}

fn hermes_home(home: &Path) -> PathBuf {
    match env::var_os("HERMES_HOME").filter(|value| !value.is_empty()) {
        Some(value) if value == "~" => home.to_owned(),
        Some(value) if value.to_string_lossy().starts_with("~/") => {
            home.join(value.to_string_lossy().trim_start_matches("~/"))
        }
        Some(value) => PathBuf::from(value),
        None => home.join(".hermes"),
    }
}

fn update_hermes_yaml(path: &Path, block: &str) -> Result<(), String> {
    let existing = read_text(path)?;
    let clean = strip_block(&existing, HERMES_BEGIN, HERMES_END)?;
    let mut next = clean.trim_end().to_owned();
    if next.is_empty() {
        next.push_str("hooks:");
    }
    if !next.lines().any(|line| line.trim() == "hooks:") {
        next.push_str("\n\nhooks:");
    }
    let insertion = next.find("hooks:").unwrap() + "hooks:".len();
    next.insert_str(insertion, &format!("\n{block}"));
    next.push('\n');
    replace_text(path, &next)
}

fn update_managed_text(path: &Path, begin: &str, end: &str, block: &str) -> Result<(), String> {
    let existing = read_text(path)?;
    let clean = strip_block(&existing, begin, end)?;
    let prefix = clean.trim_end();
    let next = if prefix.is_empty() {
        block.to_owned()
    } else {
        format!("{prefix}\n\n{block}")
    };
    replace_text(path, &next)
}

fn remove_managed_text(path: &Path, begin: &str, end: &str) -> Result<(), String> {
    let existing = read_text(path)?;
    if existing.is_empty() {
        return Ok(());
    }
    let next = strip_block(&existing, begin, end)?;
    let store = AtomicFileStore::new(path, MAX_BYTES);
    store
        .transaction(|_| {
            if next.trim().is_empty() {
                Ok(AtomicFileAction::Remove(()))
            } else {
                Ok(AtomicFileAction::Replace {
                    bytes: format!("{}\n", next.trim_end()).into_bytes(),
                    value: (),
                })
            }
        })
        .map(|(value, _)| value)
        .map_err(|error| error.to_string())
}

fn strip_block(source: &str, begin: &str, end: &str) -> Result<String, String> {
    match (source.find(begin), source.find(end)) {
        (None, None) => Ok(source.to_owned()),
        (Some(start), Some(finish)) if start <= finish => {
            let mut end_index = finish + end.len();
            if source.as_bytes().get(end_index) == Some(&b'\n') {
                end_index += 1;
            }
            Ok(format!("{}{}", &source[..start], &source[end_index..]))
        }
        _ => Err("managed hook block markers are malformed; refusing to overwrite".to_owned()),
    }
}

fn update_json(
    path: &Path,
    update: impl FnOnce(&mut Map<String, Value>) -> Result<bool, String>,
) -> Result<(), String> {
    let existing = read_bytes(path)?;
    let mut root = if existing.is_empty() {
        Map::new()
    } else {
        serde_json::from_slice(&existing)
            .map_err(|error| format!("{} is not valid JSON: {error}", path.display()))?
    };
    if !update(&mut root)? {
        return Ok(());
    }
    if root.is_empty() {
        return remove_path(path);
    }
    write_json(path, &Value::Object(root))
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    AtomicFileStore::new(path, MAX_BYTES)
        .replace_bytes(&bytes)
        .map_err(|error| error.to_string())
}
fn replace_text(path: &Path, value: &str) -> Result<(), String> {
    AtomicFileStore::new(path, MAX_BYTES)
        .replace_bytes(value.as_bytes())
        .map_err(|error| error.to_string())
}
fn read_bytes(path: &Path) -> Result<Vec<u8>, String> {
    AtomicFileStore::new(path, MAX_BYTES)
        .transaction(|bytes| {
            Ok(AtomicFileAction::ReadOnly(
                bytes.unwrap_or_default().to_vec(),
            ))
        })
        .map(|(value, _)| value)
        .map_err(|error| error.to_string())
}
fn read_text(path: &Path) -> Result<String, String> {
    String::from_utf8(read_bytes(path)?).map_err(|_| format!("{} is not UTF-8", path.display()))
}
fn remove_path(path: &Path) -> Result<(), String> {
    AtomicFileStore::new(path, MAX_BYTES)
        .transaction(|_| Ok(AtomicFileAction::Remove(())))
        .map(|(value, _)| value)
        .map_err(|error| error.to_string())
}
fn remove_owned_file(path: &Path, owned: impl FnOnce(&[u8]) -> bool) -> Result<(), String> {
    let store = AtomicFileStore::new(path, MAX_BYTES);
    store
        .transaction(|bytes| match bytes {
            None => Ok(AtomicFileAction::ReadOnly(())),
            Some(bytes) if owned(bytes) => Ok(AtomicFileAction::Remove(())),
            Some(_) => Err(format!(
                "refusing to remove unowned file {}",
                path.display()
            )),
        })
        .map(|(value, _)| value)
        .map_err(|error| error.to_string())
}
fn contains_text(value: &Value, needle: &str) -> bool {
    value.to_string().contains(needle)
}
fn path_text(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| "integration path is not UTF-8".to_owned())
}
fn shell_command(cli: &Path, suffix: &str) -> Result<String, String> {
    Ok(format!(
        "'{}' {suffix}",
        shell_single_quote(path_text(cli)?)
    ))
}
fn shell_single_quote(value: &str) -> String {
    value.replace('\'', "'\\''")
}
fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
fn yaml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
