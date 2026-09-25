use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Runtime launch contract, not persisted agent identity or an approval grant.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum NotificationCapability {
    CodexTuiAttentionV1,
    #[default]
    #[serde(other)]
    Unknown,
}

/// Conservatively recognizes explicit CLI configuration for the supported
/// Codex TUI attention channel. Conflicting writes fail closed regardless of
/// their ordering; profiles/file defaults cannot override explicit CLI values.
#[must_use]
pub fn codex_notification_capability(arguments: &[String]) -> NotificationCapability {
    let mut method = false;
    let mut notifications = false;
    let mut condition = false;
    let mut args = arguments.iter();
    while let Some(arg) = args.next() {
        if arg == "--" {
            break;
        }
        let raw = if matches!(arg.as_str(), "-c" | "--config") {
            let Some(value) = args.next() else {
                return NotificationCapability::Unknown;
            };
            value.as_str()
        } else if let Some(value) = arg
            .strip_prefix("--config=")
            .or_else(|| arg.strip_prefix("-c").filter(|s| !s.is_empty()))
        {
            value.trim_start_matches('=')
        } else {
            continue;
        };
        let Some((key, raw_value)) = raw.split_once('=') else {
            return NotificationCapability::Unknown;
        };
        let key = key.trim();
        if key == "tui" {
            return NotificationCapability::Unknown;
        }
        if !matches!(
            key,
            "tui.notification_method" | "tui.notifications" | "tui.notification_condition"
        ) {
            continue;
        }
        // Codex permits bare string values in -c overrides as well as TOML.
        let value = toml::from_str::<toml::Table>(&format!("value={raw_value}"))
            .ok()
            .and_then(|parsed| parsed.get("value").cloned())
            .unwrap_or_else(|| toml::Value::String(raw_value.trim().to_owned()));
        match key {
            "tui.notification_method" if value.as_str() == Some("osc9") => method = true,
            "tui.notification_condition" if value.as_str() == Some("always") => condition = true,
            "tui.notifications"
                if value.as_array().is_some_and(|items| {
                    items.len() == 2
                        && items
                            .iter()
                            .any(|v| v.as_str() == Some("approval-requested"))
                        && items
                            .iter()
                            .any(|v| v.as_str() == Some("agent-turn-complete"))
                }) =>
            {
                notifications = true;
            }
            _ => return NotificationCapability::Unknown,
        }
    }
    if method && notifications && condition {
        NotificationCapability::CodexTuiAttentionV1
    } else {
        NotificationCapability::Unknown
    }
}

/// Binds a launcher declaration to the arguments of a live producer process.
#[must_use]
pub fn notification_launch_digest(arguments: &[String]) -> String {
    let mut hash = Sha256::new();
    for argument in arguments {
        hash.update(argument.as_bytes());
        hash.update([0]);
    }
    format!("{:x}", hash.finalize())
}

#[cfg(test)]
mod tests {
    #[test]
    fn restored_sessions_require_fresh_notification_proof() {
        let tasks = std::collections::BTreeMap::new();
        for authoritative in [false, true] {
            let mut store = crate::AgentStatusStore::default();
            store.seed_restored_starting(
                "pane",
                "restored",
                "Codex",
                None,
                crate::agent_status::RestoredTaskState {
                    tasks: &tasks,
                    authoritative,
                    progress: None,
                },
                1,
            );
            assert!(!store.terminal_notifications_use_agent_policy("pane"));
            assert!(!store.apply_terminal_notification("pane", None, Some("Work finished"), 2));
            assert!(!store.status_for_pane("pane").unwrap().requires_attention());
        }
    }
}
