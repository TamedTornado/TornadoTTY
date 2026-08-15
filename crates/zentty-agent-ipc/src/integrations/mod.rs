use serde_json::{Map, Value, json};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use zentty_core::{AtomicFileAction, AtomicFileStore};

mod additional;

const MAX_CONFIG_BYTES: usize = 1024 * 1024;
const CURSOR_EVENTS: &[&str] = &[
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
];
const DROID_EVENTS: &[&str] = &[
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "Notification",
    "Stop",
    "SubagentStop",
];
const VIBE_BEGIN: &str = "# [Zentty Managed Hooks - Begin]";
const VIBE_END: &str = "# [Zentty Managed Hooks - End]";

/// Installs one source-defined user integration using the invoking CLI path.
///
/// # Errors
///
/// Returns an error when the target is unknown, user paths are unavailable,
/// existing data is malformed, or a bounded atomic filesystem operation fails.
pub fn install_integration(target: &str) -> Result<String, String> {
    let home = user_home()?;
    let cli = invoking_cli()?;
    match target {
        "amp-hooks" => install_amp(&home),
        "cursor-hooks" => install_cursor(&home, &cli),
        "droid-hooks" => install_droid(&home, &cli),
        "vibe-hooks" => install_vibe(&home, &cli),
        "kimi-hooks" | "grok-hooks" | "agy-hooks" | "hermes-hooks" => {
            additional::install(target, &home, &cli)
        }
        _ => Err(format!("unknown integration target {target:?}")),
    }
}

/// Removes only Zentty-owned entries for one source-defined integration.
///
/// # Errors
///
/// Returns an error under the same bounded filesystem conditions as
/// [`install_integration`].
pub fn uninstall_integration(target: &str) -> Result<String, String> {
    let home = user_home()?;
    match target {
        "amp-hooks" => uninstall_amp(&home),
        "cursor-hooks" => uninstall_cursor(&home),
        "droid-hooks" => uninstall_droid(&home),
        "vibe-hooks" => uninstall_vibe(&home),
        "kimi-hooks" | "grok-hooks" | "agy-hooks" | "hermes-hooks" => {
            additional::uninstall(target, &home)
        }
        _ => Err(format!("unknown integration target {target:?}")),
    }
}

fn user_home() -> Result<PathBuf, String> {
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is missing".to_owned())
}

fn invoking_cli() -> Result<PathBuf, String> {
    env::current_exe()
        .and_then(fs::canonicalize)
        .map_err(|error| format!("could not resolve invoking Zentty CLI: {error}"))
}

fn cursor_path(home: &Path) -> PathBuf {
    home.join(".cursor/hooks.json")
}

fn droid_path(home: &Path) -> PathBuf {
    home.join(".factory/settings.local.json")
}

fn droid_output_path(home: &Path) -> PathBuf {
    home.join(".factory/hooks/hooks.json")
}

fn vibe_path(home: &Path) -> PathBuf {
    home.join(".vibe/hooks.toml")
}

fn install_cursor(home: &Path, cli: &Path) -> Result<String, String> {
    let path = cursor_path(home);
    let command = hook_command(cli, "cursor")?;
    update_json(&path, |root| {
        root.insert("version".to_owned(), json!(1));
        let hooks = object_entry(root, "hooks")?;
        for event in CURSOR_EVENTS {
            let entries = array_entry(hooks, event)?;
            entries.retain(|entry| !contains_command(entry, "ipc agent-event --adapter=cursor"));
            let mut managed = Map::new();
            if matches!(*event, "preToolUse" | "postToolUse") {
                managed.insert("matcher".to_owned(), json!("TodoWrite"));
            }
            managed.insert("command".to_owned(), json!(command));
            entries.push(Value::Object(managed));
        }
        Ok(JsonUpdate::Replace)
    })?;
    Ok(format!(
        "Installed Zentty cursor hooks at {}.",
        path.display()
    ))
}

fn uninstall_cursor(home: &Path) -> Result<String, String> {
    let path = cursor_path(home);
    remove_json_hooks(
        &path,
        CURSOR_EVENTS,
        "ipc agent-event --adapter=cursor",
        false,
        true,
    )?;
    Ok(format!(
        "Removed Zentty cursor hook entries from {}.",
        path.display()
    ))
}

fn install_droid(home: &Path, cli: &Path) -> Result<String, String> {
    let path = droid_path(home);
    let command = hook_command(cli, "droid")?;
    update_json(&path, |root| {
        let hooks = object_entry(root, "hooks")?;
        for event in DROID_EVENTS {
            let entries = array_entry(hooks, event)?;
            entries.retain(|entry| !contains_command(entry, "ipc agent-event --adapter=droid"));
            entries.push(json!({
                "hooks": [{"type": "command", "command": command, "timeout": 10}]
            }));
        }
        Ok(JsonUpdate::Replace)
    })?;
    let output_path = droid_output_path(home);
    update_json(&output_path, |root| {
        root.entry("showHookOutput".to_owned())
            .or_insert_with(|| json!(false));
        Ok(JsonUpdate::Replace)
    })?;
    Ok(format!(
        "Installed Zentty Droid hooks at {}.",
        path.display()
    ))
}

fn uninstall_droid(home: &Path) -> Result<String, String> {
    let path = droid_path(home);
    remove_json_hooks(
        &path,
        DROID_EVENTS,
        "ipc agent-event --adapter=droid",
        true,
        false,
    )?;
    Ok(format!(
        "Removed Zentty Droid hook entries from {}.",
        path.display()
    ))
}

fn remove_json_hooks(
    path: &Path,
    events: &[&str],
    marker: &str,
    nested: bool,
    version_only_is_empty: bool,
) -> Result<(), String> {
    update_json(path, |root| {
        let Some(Value::Object(hooks)) = root.get_mut("hooks") else {
            return Ok(JsonUpdate::ReadOnly);
        };
        let mut removed = false;
        for event in events {
            let Some(Value::Array(entries)) = hooks.get_mut(*event) else {
                continue;
            };
            let before = entries.len();
            entries.retain(|entry| {
                let managed = if nested {
                    entry
                        .get("hooks")
                        .is_some_and(|hooks| contains_command(hooks, marker))
                } else {
                    contains_command(entry, marker)
                };
                !managed
            });
            removed |= before != entries.len();
            if entries.is_empty() {
                hooks.remove(*event);
            }
        }
        if !removed {
            return Ok(JsonUpdate::ReadOnly);
        }
        if hooks.is_empty() {
            root.remove("hooks");
        }
        let removable =
            root.is_empty() || (version_only_is_empty && root.keys().all(|key| key == "version"));
        Ok(if removable {
            JsonUpdate::Remove
        } else {
            JsonUpdate::Replace
        })
    })
}

fn install_vibe(home: &Path, cli: &Path) -> Result<String, String> {
    let path = vibe_path(home);
    let cli = cli
        .to_str()
        .ok_or_else(|| "invoking CLI path is not UTF-8".to_owned())?;
    let block = format!(
        "{VIBE_BEGIN}\n# DO NOT EDIT: These hooks are managed by Zentty.\n# Marker: ipc agent-event --adapter=vibe\n\
[[hooks]]\nname = \"zentty-post-agent-turn\"\ntype = \"post_agent_turn\"\ncommand = \"{cli} ipc agent-event --adapter=vibe\"\ntimeout = 15.0\ndescription = \"Zentty: Track Mistral Vibe session state\"\n\n\
[[hooks]]\nname = \"zentty-before-tool\"\ntype = \"before_tool\"\nmatch = \"*\"\ncommand = \"{cli} ipc agent-event --adapter=vibe\"\ntimeout = 60.0\ndescription = \"Zentty: Track Mistral Vibe tool calls\"\n\n\
[[hooks]]\nname = \"zentty-after-tool\"\ntype = \"after_tool\"\nmatch = \"*\"\ncommand = \"{cli} ipc agent-event --adapter=vibe\"\ntimeout = 60.0\ndescription = \"Zentty: Track Mistral Vibe tool completion\"\n{VIBE_END}\n"
    );
    let store = AtomicFileStore::new(&path, MAX_CONFIG_BYTES);
    store
        .transaction(|bytes| {
            let existing = String::from_utf8(bytes.unwrap_or_default().to_vec())
                .map_err(|_| "Vibe hooks file is not UTF-8".to_owned())?;
            let user = remove_managed_block(&existing, VIBE_BEGIN, VIBE_END)?;
            let prefix = user.trim_end();
            let content = if prefix.is_empty() {
                block.clone()
            } else {
                format!("{prefix}\n\n{block}")
            };
            Ok(if content == existing {
                AtomicFileAction::ReadOnly(())
            } else {
                AtomicFileAction::Replace {
                    bytes: content.into_bytes(),
                    value: (),
                }
            })
        })
        .map_err(|error| error.to_string())?;
    Ok(format!(
        "Installed Zentty Mistral Vibe hooks at {}.",
        path.display()
    ))
}

fn uninstall_vibe(home: &Path) -> Result<String, String> {
    let path = vibe_path(home);
    let store = AtomicFileStore::new(&path, MAX_CONFIG_BYTES);
    store
        .transaction(|bytes| {
            let Some(bytes) = bytes else {
                return Ok(AtomicFileAction::ReadOnly(()));
            };
            let existing = String::from_utf8(bytes.to_vec())
                .map_err(|_| "Vibe hooks file is not UTF-8".to_owned())?;
            let content = remove_managed_block(&existing, VIBE_BEGIN, VIBE_END)?;
            if content == existing {
                return Ok(AtomicFileAction::ReadOnly(()));
            }
            if content.trim().is_empty() {
                Ok(AtomicFileAction::Remove(()))
            } else {
                Ok(AtomicFileAction::Replace {
                    bytes: (content.trim_end().to_owned() + "\n").into_bytes(),
                    value: (),
                })
            }
        })
        .map_err(|error| error.to_string())?;
    Ok(format!(
        "Removed Zentty Mistral Vibe hook entries from {}.",
        path.display()
    ))
}

fn remove_managed_block(source: &str, begin: &str, end: &str) -> Result<String, String> {
    let start = source.find(begin);
    let finish = source.find(end);
    match (start, finish) {
        (None, None) => Ok(source.to_owned()),
        (Some(start), Some(finish)) if start <= finish => {
            let end_index = finish + end.len();
            Ok(format!("{}{}", &source[..start], &source[end_index..]))
        }
        _ => Err("managed hook block markers are malformed; refusing to overwrite".to_owned()),
    }
}

fn install_amp(home: &Path) -> Result<String, String> {
    let config = env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map_or_else(|| home.join(".config"), PathBuf::from);
    let destination = config.join("amp/plugins/zentty-amp-zentty.ts");
    let source = integration_resource("amp/plugins/zentty-amp-zentty.ts")?;
    let bytes = fs::read(&source)
        .map_err(|error| format!("could not read Amp plugin {}: {error}", source.display()))?;
    if let Ok(existing) = fs::read(&destination)
        && !String::from_utf8_lossy(&existing).contains("zentty-amp-plugin-v1")
    {
        return Err(format!(
            "refusing to overwrite unmarked Amp plugin at {}",
            destination.display()
        ));
    }
    AtomicFileStore::new(&destination, MAX_CONFIG_BYTES)
        .replace_bytes(&bytes)
        .map_err(|error| error.to_string())?;
    Ok(format!(
        "Installed Zentty Amp plugin under {}.",
        config.join("amp/plugins").display()
    ))
}

fn uninstall_amp(home: &Path) -> Result<String, String> {
    let config = env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map_or_else(|| home.join(".config"), PathBuf::from);
    let destination = config.join("amp/plugins/zentty-amp-zentty.ts");
    let store = AtomicFileStore::new(&destination, MAX_CONFIG_BYTES);
    store
        .transaction(|bytes| match bytes {
            Some(bytes) if String::from_utf8_lossy(bytes).contains("zentty-amp-plugin-v1") => {
                Ok(AtomicFileAction::Remove(()))
            }
            Some(_) => Err("refusing to remove an unmarked Amp plugin".to_owned()),
            None => Ok(AtomicFileAction::ReadOnly(())),
        })
        .map_err(|error| error.to_string())?;
    Ok(format!(
        "Removed Zentty Amp plugin from {}.",
        config.join("amp/plugins").display()
    ))
}

fn integration_resource(relative: &str) -> Result<PathBuf, String> {
    let executable = invoking_cli()?;
    let root = executable
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| "could not resolve staged resource root".to_owned())?;
    let path = root.join("share/zentty").join(relative);
    path.is_file()
        .then_some(path)
        .ok_or_else(|| format!("staged integration resource {relative:?} is missing"))
}

fn hook_command(cli: &Path, adapter: &str) -> Result<String, String> {
    let cli = cli
        .to_str()
        .ok_or_else(|| "invoking CLI path is not UTF-8".to_owned())?;
    let escaped = cli
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('`', "\\`");
    Ok(format!("\"{escaped}\" ipc agent-event --adapter={adapter}"))
}

enum JsonUpdate {
    ReadOnly,
    Replace,
    Remove,
}

fn update_json(
    path: &Path,
    update: impl FnOnce(&mut Map<String, Value>) -> Result<JsonUpdate, String>,
) -> Result<(), String> {
    AtomicFileStore::new(path, MAX_CONFIG_BYTES)
        .transaction(|bytes| {
            let mut root = match bytes {
                Some(bytes) if !bytes.is_empty() => parse_jsonc_object(bytes).map_err(|error| {
                    format!("{} is not valid JSON/JSONC: {error}", path.display())
                })?,
                _ => Map::new(),
            };
            match update(&mut root)? {
                JsonUpdate::ReadOnly => Ok(AtomicFileAction::ReadOnly(())),
                JsonUpdate::Remove => Ok(AtomicFileAction::Remove(())),
                JsonUpdate::Replace => {
                    let mut bytes = serde_json::to_vec_pretty(&root)
                        .map_err(|error| format!("could not encode {}: {error}", path.display()))?;
                    bytes.push(b'\n');
                    Ok(AtomicFileAction::Replace { bytes, value: () })
                }
            }
        })
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn parse_jsonc_object(bytes: &[u8]) -> Result<Map<String, Value>, serde_json::Error> {
    if let Ok(object) = serde_json::from_slice(bytes) {
        return Ok(object);
    }
    serde_json::from_slice(&strip_jsonc(bytes))
}

fn strip_jsonc(bytes: &[u8]) -> Vec<u8> {
    let mut stripped = bytes.to_vec();
    let mut index = 0;
    let mut in_string = false;
    let mut escaped = false;
    while index < stripped.len() {
        let byte = stripped[index];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if byte == b'"' {
            in_string = true;
            index += 1;
            continue;
        }
        if byte == b'/' && stripped.get(index + 1) == Some(&b'/') {
            while index < stripped.len() && stripped[index] != b'\n' {
                stripped[index] = b' ';
                index += 1;
            }
            continue;
        }
        if byte == b'/' && stripped.get(index + 1) == Some(&b'*') {
            stripped[index] = b' ';
            stripped[index + 1] = b' ';
            index += 2;
            while index + 1 < stripped.len()
                && !(stripped[index] == b'*' && stripped[index + 1] == b'/')
            {
                if stripped[index] != b'\n' {
                    stripped[index] = b' ';
                }
                index += 1;
            }
            if index + 1 < stripped.len() {
                stripped[index] = b' ';
                stripped[index + 1] = b' ';
                index += 2;
            }
            continue;
        }
        index += 1;
    }
    let mut output = Vec::with_capacity(stripped.len());
    let mut cursor = 0;
    while cursor < stripped.len() {
        if stripped[cursor] == b',' {
            let mut next = cursor + 1;
            while stripped.get(next).is_some_and(u8::is_ascii_whitespace) {
                next += 1;
            }
            if stripped
                .get(next)
                .is_some_and(|byte| matches!(byte, b'}' | b']'))
            {
                cursor += 1;
                continue;
            }
        }
        output.push(stripped[cursor]);
        cursor += 1;
    }
    output
}

fn object_entry<'a>(
    root: &'a mut Map<String, Value>,
    key: &str,
) -> Result<&'a mut Map<String, Value>, String> {
    root.entry(key.to_owned())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| format!("{key:?} must be an object"))
}

fn array_entry<'a>(
    root: &'a mut Map<String, Value>,
    key: &str,
) -> Result<&'a mut Vec<Value>, String> {
    root.entry(key.to_owned())
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| format!("hook event {key:?} must be an array"))
}

fn contains_command(value: &Value, marker: &str) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, value)| {
            (key == "command" && value.as_str().is_some_and(|value| value.contains(marker)))
                || contains_command(value, marker)
        }),
        Value::Array(values) => values.iter().any(|value| contains_command(value, marker)),
        _ => false,
    }
}
