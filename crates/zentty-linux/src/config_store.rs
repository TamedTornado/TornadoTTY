use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use zentty_core::{AppConfig, ShortcutBinding};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ConfigSnapshot {
    pub(crate) config: AppConfig,
    pub(crate) path: PathBuf,
    pub(crate) warning: Option<String>,
}

pub(crate) struct ConfigStore;

const MAX_CONFIG_BYTES: u64 = 1_048_576;
static TEMP_NONCE: AtomicU64 = AtomicU64::new(0);

impl ConfigStore {
    pub(crate) fn load_default() -> Result<ConfigSnapshot, String> {
        Self::load(default_config_file_from(
            std::env::var_os("XDG_CONFIG_HOME"),
            std::env::var_os("HOME"),
        )?)
    }

    fn load(path: PathBuf) -> Result<ConfigSnapshot, String> {
        let mut file = match File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Ok(ConfigSnapshot {
                    config: AppConfig::default(),
                    path,
                    warning: None,
                });
            }
            Err(error) => {
                return Err(format!(
                    "could not read Zentty configuration {}: {error}",
                    path.display()
                ));
            }
        };
        let metadata = file.metadata().map_err(|error| {
            format!(
                "could not inspect Zentty configuration {}: {error}",
                path.display()
            )
        })?;
        if !metadata.is_file() {
            return Err(format!(
                "Zentty configuration is not a regular file: {}",
                path.display()
            ));
        }
        let bytes = match read_bounded(&mut file, metadata.len()) {
            Ok(bytes) => bytes,
            Err(BoundedReadError::TooLarge) => return Ok(invalid_snapshot(path)),
            Err(BoundedReadError::Io(error)) => {
                return Err(format!(
                    "could not read Zentty configuration {}: {error}",
                    path.display()
                ));
            }
        };
        let Ok(source) = String::from_utf8(bytes) else {
            return Ok(invalid_snapshot(path));
        };
        match AppConfig::parse_toml(&source) {
            Ok(config) => Ok(ConfigSnapshot {
                config,
                path,
                warning: None,
            }),
            Err(_) => Ok(invalid_snapshot(path)),
        }
    }

    pub(crate) fn update_default_ignored_port_rules(rules: &[String]) -> Result<PathBuf, String> {
        let path = default_config_file_from(
            std::env::var_os("XDG_CONFIG_HOME"),
            std::env::var_os("HOME"),
        )?;
        Self::update_ignored_port_rules(&path, rules)?;
        Ok(path)
    }

    pub(crate) fn update_default_preferred_browser(browser_id: &str) -> Result<PathBuf, String> {
        let path = default_config_file_from(
            std::env::var_os("XDG_CONFIG_HOME"),
            std::env::var_os("HOME"),
        )?;
        Self::update_preferred_browser(&path, browser_id)?;
        Ok(path)
    }

    pub(crate) fn update_default_shortcuts(
        bindings: &[ShortcutBinding],
    ) -> Result<PathBuf, String> {
        let path = default_config_file_from(
            std::env::var_os("XDG_CONFIG_HOME"),
            std::env::var_os("HOME"),
        )?;
        Self::update_shortcuts(&path, bindings)?;
        Ok(path)
    }

    fn update_ignored_port_rules(path: &Path, rules: &[String]) -> Result<(), String> {
        let target = resolve_config_target(path)?;
        let source = match fs::read_to_string(&target) {
            Ok(source) => source,
            Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
            Err(error) => return Err(format!("could not read {}: {error}", target.display())),
        };
        if source.len() as u64 > MAX_CONFIG_BYTES {
            return Err(format!("configuration exceeds {MAX_CONFIG_BYTES} bytes"));
        }
        let mut document = source
            .parse::<toml_edit::DocumentMut>()
            .map_err(|error| format!("could not edit invalid configuration: {error}"))?;
        let mut array = toml_edit::Array::new();
        for rule in rules {
            array.push(rule.as_str());
        }
        document["server_detection"]["ignored_port_rules"] = toml_edit::value(array);
        atomic_replace(&target, document.to_string().as_bytes())
    }

    fn update_preferred_browser(path: &Path, browser_id: &str) -> Result<(), String> {
        let target = resolve_config_target(path)?;
        let source = match fs::read_to_string(&target) {
            Ok(source) => source,
            Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
            Err(error) => return Err(format!("could not read {}: {error}", target.display())),
        };
        if source.len() as u64 > MAX_CONFIG_BYTES {
            return Err(format!("configuration exceeds {MAX_CONFIG_BYTES} bytes"));
        }
        let mut document = source
            .parse::<toml_edit::DocumentMut>()
            .map_err(|error| format!("could not edit invalid configuration: {error}"))?;
        document["server_detection"]["preferred_browser_id"] = toml_edit::value(browser_id);
        atomic_replace(&target, document.to_string().as_bytes())
    }

    fn update_shortcuts(path: &Path, bindings: &[ShortcutBinding]) -> Result<(), String> {
        let target = resolve_config_target(path)?;
        let source = match fs::read_to_string(&target) {
            Ok(source) => source,
            Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
            Err(error) => return Err(format!("could not read {}: {error}", target.display())),
        };
        if source.len() as u64 > MAX_CONFIG_BYTES {
            return Err(format!("configuration exceeds {MAX_CONFIG_BYTES} bytes"));
        }
        let mut document = source
            .parse::<toml_edit::DocumentMut>()
            .map_err(|error| format!("could not edit invalid configuration: {error}"))?;
        let mut tables = toml_edit::ArrayOfTables::new();
        for binding in bindings {
            let mut table = toml_edit::Table::new();
            table["command_id"] = toml_edit::value(&binding.command_id);
            table["shortcut"] = toml_edit::value(
                binding
                    .shortcut
                    .as_ref()
                    .map_or_else(String::new, zentty_core::KeyboardShortcut::storage_string),
            );
            tables.push(table);
        }
        let shortcuts = document
            .entry("shortcuts")
            .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()))
            .as_table_mut()
            .ok_or_else(|| "shortcuts configuration is not a table".to_owned())?;
        shortcuts["bindings"] = toml_edit::Item::ArrayOfTables(tables);
        atomic_replace(&target, document.to_string().as_bytes())
    }
}

fn resolve_config_target(path: &Path) -> Result<PathBuf, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let target = fs::read_link(path)
                .map_err(|error| format!("could not resolve {}: {error}", path.display()))?;
            Ok(if target.is_absolute() {
                target
            } else {
                path.parent().unwrap_or_else(|| Path::new(".")).join(target)
            })
        }
        Ok(metadata) if metadata.is_file() => Ok(path.to_owned()),
        Ok(_) => Err(format!(
            "configuration is not a regular file: {}",
            path.display()
        )),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(path.to_owned()),
        Err(error) => Err(format!("could not inspect {}: {error}", path.display())),
    }
}

fn atomic_replace(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err(format!("configuration exceeds {MAX_CONFIG_BYTES} bytes"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("configuration has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    let nonce = TEMP_NONCE.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(".zentty-config-{}-{nonce}.tmp", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|error| format!("could not create {}: {error}", temporary.display()))?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;
        fs::rename(&temporary, path)
            .map_err(|error| format!("could not replace {}: {error}", path.display()))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[derive(Debug)]
enum BoundedReadError {
    TooLarge,
    Io(std::io::Error),
}

fn read_bounded(reader: &mut impl Read, declared_len: u64) -> Result<Vec<u8>, BoundedReadError> {
    let capacity = usize::try_from(declared_len.min(MAX_CONFIG_BYTES))
        .map_err(|_| BoundedReadError::TooLarge)?;
    let mut bytes = Vec::with_capacity(capacity);
    reader
        .take(MAX_CONFIG_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(BoundedReadError::Io)?;
    if bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err(BoundedReadError::TooLarge);
    }
    Ok(bytes)
}

fn invalid_snapshot(path: PathBuf) -> ConfigSnapshot {
    ConfigSnapshot {
        config: AppConfig::default(),
        warning: Some(format!(
            "ignored invalid Zentty configuration {}: size, encoding, parse, or known-value validation failed",
            path.display()
        )),
        path,
    }
}

fn default_config_file_from(
    xdg_config_home: Option<OsString>,
    home: Option<OsString>,
) -> Result<PathBuf, String> {
    if let Some(path) = nonempty_path(xdg_config_home) {
        return Ok(path.join("zentty/config.toml"));
    }
    nonempty_path(home)
        .map(|home| home.join(".config/zentty/config.toml"))
        .ok_or_else(|| "could not resolve Zentty config: XDG_CONFIG_HOME and HOME are unset".into())
}

fn nonempty_path(value: Option<OsString>) -> Option<PathBuf> {
    value.filter(|value| !value.is_empty()).map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::{
        BoundedReadError, ConfigStore, MAX_CONFIG_BYTES, default_config_file_from, read_bounded,
    };
    use std::ffi::OsString;
    use std::fs;
    use std::io::Read;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};
    use zentty_core::AppConfig;

    fn private_root(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "zentty-config-{name}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn remove(root: &Path) {
        fs::remove_dir_all(root).unwrap_or_else(|error| {
            assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
        });
    }

    #[test]
    fn missing_file_returns_source_defaults_without_creating_state() {
        let root = private_root("missing");
        let path = root.join("zentty/config.toml");
        let snapshot = ConfigStore::load(path.clone()).unwrap();
        assert_eq!(snapshot.path, path);
        assert!(!snapshot.config.clipboard.always_clean_copies);
        assert_eq!(snapshot.warning, None);
        assert!(!root.exists());
    }

    #[test]
    fn valid_file_loads_one_clipboard_snapshot() {
        let root = private_root("valid");
        let path = root.join("zentty/config.toml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "[clipboard]\nalways_clean_copies = true\n").unwrap();
        let snapshot = ConfigStore::load(path).unwrap();
        assert!(snapshot.config.clipboard.always_clean_copies);
        assert_eq!(snapshot.warning, None);
        remove(&root);
    }

    #[test]
    fn ignored_port_update_preserves_comments_unknown_keys_and_a_config_symlink() {
        let root = private_root("ignored-ports");
        let target = root.join("shared/settings.toml");
        let path = root.join("zentty/config.toml");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &target,
            "# keep me\nunknown = \"value\"\n\n[server_detection]\npreferred_browser_id = \"system-default\"\n",
        )
        .unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();

        ConfigStore::update_ignored_port_rules(&path, &["3000-3002".into(), "5173".into()])
            .unwrap();

        assert_eq!(fs::read_link(&path).unwrap(), target);
        let source = fs::read_to_string(&path).unwrap();
        assert!(source.contains("# keep me"));
        assert!(source.contains("unknown = \"value\""));
        assert!(source.contains("preferred_browser_id = \"system-default\""));
        assert!(source.contains("ignored_port_rules = [\"3000-3002\", \"5173\"]"));
        assert_eq!(
            ConfigStore::load(path)
                .unwrap()
                .config
                .server_detection
                .ignored_port_rules,
            ["3000-3002", "5173"]
        );
        remove(&root);
    }

    #[test]
    fn preferred_browser_update_preserves_unrelated_server_configuration() {
        let root = private_root("preferred-browser");
        let path = root.join("zentty/config.toml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            "# keep me\n[server_detection]\nignored_port_rules = [\"5173\"]\n",
        )
        .unwrap();

        ConfigStore::update_preferred_browser(&path, "firefox").unwrap();

        let source = fs::read_to_string(&path).unwrap();
        assert!(source.contains("# keep me"));
        assert!(source.contains("ignored_port_rules = [\"5173\"]"));
        assert!(source.contains("preferred_browser_id = \"firefox\""));
        assert_eq!(
            ConfigStore::load(path)
                .unwrap()
                .config
                .server_detection
                .preferred_browser_id,
            "firefox"
        );
        remove(&root);
    }

    #[test]
    fn shortcut_update_preserves_comments_unknown_keys_and_symlink() {
        let root = private_root("shortcuts");
        let target = root.join("shared/settings.toml");
        let path = root.join("zentty/config.toml");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&target, "# keep me\nunknown = \"value\"\n").unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();

        ConfigStore::update_shortcuts(
            &path,
            &[
                zentty_core::ShortcutBinding {
                    command_id: "sidebar.toggle".into(),
                    shortcut: zentty_core::KeyboardShortcut::parse("command+option+s"),
                },
                zentty_core::ShortcutBinding {
                    command_id: "pane.close_focused".into(),
                    shortcut: None,
                },
            ],
        )
        .unwrap();

        assert_eq!(fs::read_link(&path).unwrap(), target);
        let source = fs::read_to_string(&path).unwrap();
        assert!(source.contains("# keep me"));
        assert!(source.contains("unknown = \"value\""));
        let loaded = ConfigStore::load(path).unwrap().config.shortcuts;
        assert_eq!(loaded.len(), 2);
        assert_eq!(
            loaded[0].shortcut.as_ref().unwrap().storage_string(),
            "command+option+s"
        );
        assert!(loaded[1].shortcut.is_none());
        remove(&root);
    }

    #[test]
    fn invalid_file_keeps_defaults_and_reports_no_user_contents() {
        let root = private_root("invalid");
        let path = root.join("zentty/config.toml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let secret = "do-not-log-this-secret";
        fs::write(
            &path,
            format!("[clipboard]\nalways_clean_copies = {secret}\n"),
        )
        .unwrap();
        let snapshot = ConfigStore::load(path).unwrap();
        assert!(!snapshot.config.clipboard.always_clean_copies);
        let warning = snapshot.warning.unwrap();
        assert!(warning.contains("ignored invalid Zentty configuration"));
        assert!(!warning.contains(secret));
        remove(&root);
    }

    #[test]
    fn oversized_file_is_bounded_and_keeps_defaults() {
        let root = private_root("oversized");
        let path = root.join("zentty/config.toml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let file = fs::File::create(&path).unwrap();
        file.set_len(1_048_577).unwrap();
        let snapshot = ConfigStore::load(path).unwrap();
        assert_eq!(snapshot.config, AppConfig::default());
        assert!(snapshot.warning.is_some());
        remove(&root);
    }

    #[test]
    fn invalid_utf8_is_malformed_without_echoing_bytes() {
        let root = private_root("invalid-utf8");
        let path = root.join("zentty/config.toml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"[clipboard]\nalways_clean_copies = \xff\n").unwrap();
        let snapshot = ConfigStore::load(path).unwrap();
        assert_eq!(snapshot.config, AppConfig::default());
        assert!(snapshot.warning.is_some());
        remove(&root);
    }

    #[test]
    fn directory_at_config_path_is_rejected() {
        let root = private_root("directory");
        let path = root.join("zentty/config.toml");
        fs::create_dir_all(&path).unwrap();
        let error = ConfigStore::load(path).unwrap_err();
        assert!(error.contains("not a regular file"));
        remove(&root);
    }

    #[test]
    fn exact_size_limit_is_accepted_and_growth_past_metadata_is_rejected() {
        let mut exact = std::io::repeat(b' ').take(MAX_CONFIG_BYTES);
        assert_eq!(
            read_bounded(&mut exact, MAX_CONFIG_BYTES).unwrap().len(),
            1_048_576
        );
        let mut grown = std::io::repeat(b' ').take(1_048_577);
        assert!(matches!(
            read_bounded(&mut grown, 1),
            Err(BoundedReadError::TooLarge)
        ));
    }

    #[test]
    fn config_path_precedence_and_empty_values_match_xdg() {
        assert_eq!(
            default_config_file_from(Some(OsString::from("/xdg")), Some(OsString::from("/home")))
                .unwrap(),
            Path::new("/xdg/zentty/config.toml")
        );
        assert_eq!(
            default_config_file_from(Some(OsString::new()), Some(OsString::from("/home"))).unwrap(),
            Path::new("/home/.config/zentty/config.toml")
        );
        assert!(default_config_file_from(None, None).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_file_is_not_treated_as_missing() {
        use std::os::unix::fs::PermissionsExt;

        let root = private_root("unreadable");
        let path = root.join("zentty/config.toml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "[clipboard]\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
        let result = ConfigStore::load(path.clone());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(result.unwrap_err().contains("could not read"));
        remove(&root);
    }
}
