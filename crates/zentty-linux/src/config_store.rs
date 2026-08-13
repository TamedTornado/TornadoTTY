use std::ffi::OsString;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use zentty_core::{
    AppConfig, AppearanceConfig, ClipboardConfig, ConfirmationsConfig, FALLBACK_DARK_THEME,
    NotificationsConfig, RestoreConfig, ShortcutBinding, ThemeMode, ThemeSpec, UpdatesConfig,
    update_ghostty_value,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ConfigSnapshot {
    pub(crate) config: AppConfig,
    pub(crate) path: PathBuf,
    pub(crate) warning: Option<String>,
}

pub(crate) struct ConfigStore;

const MAX_CONFIG_BYTES: u64 = 1_048_576;
const MAX_THEME_BYTES: u64 = 64 * 1024;
const LOCK_DEADLINE: Duration = Duration::from_millis(250);
const LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(5);
static TEMP_NONCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ThemeInstallOutcome {
    NotReferenced,
    AlreadyPresent,
    Installed,
}

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

    pub(crate) fn update_default_appearance(
        appearance: &AppearanceConfig,
    ) -> Result<PathBuf, String> {
        let path = default_config_file_from(
            std::env::var_os("XDG_CONFIG_HOME"),
            std::env::var_os("HOME"),
        )?;
        Self::update_appearance(&path, appearance)?;
        Ok(path)
    }

    pub(crate) fn update_default_general(
        confirmations: ConfirmationsConfig,
        restore: RestoreConfig,
        clipboard: ClipboardConfig,
    ) -> Result<PathBuf, String> {
        let path = default_config_file_from(
            std::env::var_os("XDG_CONFIG_HOME"),
            std::env::var_os("HOME"),
        )?;
        Self::update_general(&path, confirmations, restore, clipboard)?;
        Ok(path)
    }

    pub(crate) fn update_default_notifications(
        notifications: &NotificationsConfig,
    ) -> Result<PathBuf, String> {
        let path = default_config_file_from(
            std::env::var_os("XDG_CONFIG_HOME"),
            std::env::var_os("HOME"),
        )?;
        Self::update_notifications(&path, notifications)?;
        Ok(path)
    }

    pub(crate) fn update_default_updates(updates: UpdatesConfig) -> Result<PathBuf, String> {
        let path = default_config_file_from(
            std::env::var_os("XDG_CONFIG_HOME"),
            std::env::var_os("HOME"),
        )?;
        Self::update_updates(&path, updates)?;
        Ok(path)
    }

    fn update_updates(path: &Path, updates: UpdatesConfig) -> Result<(), String> {
        let target = resolve_config_target(path)?;
        with_config_lock(&target, || {
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
            document
                .entry("updates")
                .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()));
            if !document["updates"].is_table() {
                return Err("updates configuration is not a table".to_owned());
            }
            document["updates"]["channel"] = toml_edit::value(updates.channel.config_value());
            atomic_replace(&target, document.to_string().as_bytes())
        })
    }

    fn update_notifications(
        path: &Path,
        notifications: &NotificationsConfig,
    ) -> Result<(), String> {
        let target = resolve_config_target(path)?;
        with_config_lock(&target, || {
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
            document
                .entry("notifications")
                .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()));
            if !document["notifications"].is_table() {
                return Err("notifications configuration is not a table".to_owned());
            }
            document["notifications"]["sound_name"] = toml_edit::value(&notifications.sound_name);
            if let Some(display_name) = &notifications.custom_sound_display_name {
                document["notifications"]["custom_sound_display_name"] =
                    toml_edit::value(display_name);
            } else if let Some(table) = document["notifications"].as_table_mut() {
                table.remove("custom_sound_display_name");
            }
            atomic_replace(&target, document.to_string().as_bytes())
        })
    }

    fn update_general(
        path: &Path,
        confirmations: ConfirmationsConfig,
        restore: RestoreConfig,
        clipboard: ClipboardConfig,
    ) -> Result<(), String> {
        let target = resolve_config_target(path)?;
        with_config_lock(&target, || {
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
            for section in ["confirmations", "restore", "clipboard"] {
                document
                    .entry(section)
                    .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()));
                if !document[section].is_table() {
                    return Err(format!("{section} configuration is not a table"));
                }
            }
            document["confirmations"]["confirm_before_closing_pane"] =
                toml_edit::value(confirmations.confirm_before_closing_pane);
            document["confirmations"]["confirm_before_closing_window"] =
                toml_edit::value(confirmations.confirm_before_closing_window);
            document["confirmations"]["confirm_before_quitting"] =
                toml_edit::value(confirmations.confirm_before_quitting);
            document["restore"]["restore_workspace_on_launch"] =
                toml_edit::value(restore.restore_workspace_on_launch);
            document["clipboard"]["always_clean_copies"] =
                toml_edit::value(clipboard.always_clean_copies);
            let options = clipboard.clean_options;
            document["clipboard"]["flatten_multi_line_commands"] =
                toml_edit::value(options.flatten_multi_line_commands);
            document["clipboard"]["command_flatten_aggressiveness"] =
                toml_edit::value(options.command_flatten_aggressiveness.config_value());
            document["clipboard"]["preserve_blank_lines_when_flattening"] =
                toml_edit::value(options.preserve_blank_lines_when_flattening);
            document["clipboard"]["remove_box_drawing"] =
                toml_edit::value(options.remove_box_drawing);
            document["clipboard"]["flatten_slash_command_selections"] =
                toml_edit::value(options.flatten_slash_command_selections);
            document["clipboard"]["strip_url_tracking_parameters"] =
                toml_edit::value(options.strip_url_tracking_parameters);
            document["clipboard"]["quote_paths_with_spaces"] =
                toml_edit::value(options.quote_paths_with_spaces);
            document["clipboard"]["show_copy_markdown_command"] =
                toml_edit::value(clipboard.show_copy_markdown_command);
            atomic_replace(&target, document.to_string().as_bytes())
        })
    }

    fn update_appearance(path: &Path, appearance: &AppearanceConfig) -> Result<(), String> {
        let target = resolve_config_target(path)?;
        with_config_lock(&target, || {
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
            document
                .entry("appearance")
                .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()));
            if !document["appearance"].is_table() {
                return Err("appearance configuration is not a table".to_owned());
            }
            document["appearance"]["theme_mode"] =
                toml_edit::value(appearance.theme_mode.config_value());
            set_optional_appearance_string(
                &mut document,
                "preferred_dark_theme_name",
                appearance.preferred_dark_theme_name.as_deref(),
            )?;
            set_optional_appearance_string(
                &mut document,
                "preferred_light_theme_name",
                appearance.preferred_light_theme_name.as_deref(),
            )?;
            if let Some(opacity) = appearance.background_opacity {
                document["appearance"]["local_background_opacity"] =
                    toml_edit::value(f64::from(opacity.percent()) / 100.0);
            } else if let Some(table) = document["appearance"].as_table_mut() {
                table.remove("local_background_opacity");
            }
            document["appearance"]["sync_opencode_theme_with_terminal"] =
                toml_edit::value(appearance.sync_opencode_theme_with_terminal);
            atomic_replace(&target, document.to_string().as_bytes())
        })
    }

    pub(crate) fn update_default_ghostty_theme(spec: &ThemeSpec) -> Result<PathBuf, String> {
        Self::update_default_ghostty_value("theme", &spec.to_string())
    }

    pub(crate) fn install_default_fallback_theme_if_referenced(
        spec: &ThemeSpec,
    ) -> Result<(), String> {
        let resource = default_theme_resource_path()?;
        let target = default_fallback_theme_path(
            std::env::var_os("XDG_CONFIG_HOME"),
            std::env::var_os("HOME"),
        )?;
        install_fallback_theme_if_referenced(spec, &resource, &target).map(|_| ())
    }

    pub(crate) fn update_default_ghostty_value(key: &str, value: &str) -> Result<PathBuf, String> {
        let path = default_ghostty_config_file_from(
            std::env::var_os("XDG_CONFIG_HOME"),
            std::env::var_os("HOME"),
        )?;
        Self::update_ghostty_value(&path, key, value)?;
        Ok(path)
    }

    fn update_ghostty_value(path: &Path, key: &str, value: &str) -> Result<(), String> {
        let target = resolve_config_target(path)?;
        with_config_lock(&target, || {
            let source = match fs::read_to_string(&target) {
                Ok(source) => source,
                Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
                Err(error) => return Err(format!("could not read {}: {error}", target.display())),
            };
            if source.len() as u64 > MAX_CONFIG_BYTES {
                return Err(format!("configuration exceeds {MAX_CONFIG_BYTES} bytes"));
            }
            let updated = update_ghostty_value(Some(&source), key, value)
                .ok_or_else(|| format!("refused unsupported or unsafe Ghostty key: {key}"))?;
            atomic_replace(&target, updated.as_bytes())
        })
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

fn install_fallback_theme_if_referenced(
    spec: &ThemeSpec,
    resource: &Path,
    target: &Path,
) -> Result<ThemeInstallOutcome, String> {
    let references_fallback = match spec.mode {
        ThemeMode::Dark => spec.resolved_dark_theme_name() == FALLBACK_DARK_THEME,
        ThemeMode::Light => spec.resolved_light_theme_name() == FALLBACK_DARK_THEME,
        ThemeMode::Automatic => {
            spec.resolved_dark_theme_name() == FALLBACK_DARK_THEME
                || spec.resolved_light_theme_name() == FALLBACK_DARK_THEME
        }
    };
    if !references_fallback {
        return Ok(ThemeInstallOutcome::NotReferenced);
    }
    match fs::symlink_metadata(target) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(format!(
                "refusing fallback theme symlink: {}",
                target.display()
            ));
        }
        Ok(metadata) if metadata.is_file() => return Ok(ThemeInstallOutcome::AlreadyPresent),
        Ok(_) => {
            return Err(format!(
                "fallback theme path is not a regular file: {}",
                target.display()
            ));
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(format!("could not inspect {}: {error}", target.display())),
    }

    let bytes = fs::read(resource).map_err(|error| {
        format!(
            "could not read bundled theme {}: {error}",
            resource.display()
        )
    })?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_THEME_BYTES {
        return Err(format!(
            "bundled fallback theme must contain 1..={MAX_THEME_BYTES} bytes"
        ));
    }
    let parent = target
        .parent()
        .ok_or_else(|| format!("fallback theme has no parent: {}", target.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    let nonce = TEMP_NONCE.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(
        ".zentty-fallback-theme.{}.{}",
        std::process::id(),
        nonce
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|error| format!("could not create {}: {error}", temporary.display()))?;
    let result = (|| {
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;
        fs::hard_link(&temporary, target)
            .map_err(|error| fallback_theme_publication_error(&error, target))?;
        Ok(ThemeInstallOutcome::Installed)
    })();
    drop(file);
    let _ = fs::remove_file(&temporary);
    result
}

fn fallback_theme_publication_error(error: &std::io::Error, target: &Path) -> String {
    if error.kind() == ErrorKind::AlreadyExists {
        format!("fallback theme appeared concurrently: {}", target.display())
    } else {
        format!("could not publish {}: {error}", target.display())
    }
}

fn default_theme_resource_path() -> Result<PathBuf, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("could not locate Zentty executable: {error}"))?;
    let prefix = executable.parent().and_then(Path::parent).ok_or_else(|| {
        format!(
            "Zentty executable has no install prefix: {}",
            executable.display()
        )
    })?;
    Ok(prefix
        .join("share/zentty/ghostty/themes")
        .join(FALLBACK_DARK_THEME))
}

fn default_fallback_theme_path(
    xdg_config_home: Option<OsString>,
    home: Option<OsString>,
) -> Result<PathBuf, String> {
    if let Some(path) = nonempty_path(xdg_config_home) {
        return Ok(path.join("ghostty/themes").join(FALLBACK_DARK_THEME));
    }
    nonempty_path(home)
        .map(|home| {
            home.join(".config/ghostty/themes")
                .join(FALLBACK_DARK_THEME)
        })
        .ok_or_else(|| {
            "could not resolve Ghostty themes: XDG_CONFIG_HOME and HOME are unset".to_owned()
        })
}

fn set_optional_appearance_string(
    document: &mut toml_edit::DocumentMut,
    key: &str,
    value: Option<&str>,
) -> Result<(), String> {
    if let Some(value) = value {
        document["appearance"][key] = toml_edit::value(value);
    } else {
        document["appearance"]
            .as_table_mut()
            .ok_or_else(|| "appearance configuration is not a table".to_owned())?
            .remove(key);
    }
    Ok(())
}

fn with_config_lock<T>(
    path: &Path,
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("configuration has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    let lock_path = parent.join(".zentty-appearance.lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .custom_flags(libc::O_NOFOLLOW)
        .mode(0o600)
        .open(&lock_path)
        .map_err(|error| format!("could not open {}: {error}", lock_path.display()))?;
    let deadline = Instant::now() + LOCK_DEADLINE;
    loop {
        match lock.try_lock() {
            Ok(()) => break,
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                thread::sleep(LOCK_RETRY_INTERVAL);
            }
            Err(TryLockError::WouldBlock) => {
                return Err(format!("timed out acquiring {}", lock_path.display()));
            }
            Err(TryLockError::Error(error)) => {
                return Err(format!("could not lock {}: {error}", lock_path.display()));
            }
        }
    }
    let result = operation();
    drop(lock);
    result
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

fn default_ghostty_config_file_from(
    xdg_config_home: Option<OsString>,
    home: Option<OsString>,
) -> Result<PathBuf, String> {
    if let Some(path) = nonempty_path(xdg_config_home) {
        return Ok(path.join("ghostty/config"));
    }
    nonempty_path(home)
        .map(|home| home.join(".config/ghostty/config"))
        .ok_or_else(|| {
            "could not resolve Ghostty config: XDG_CONFIG_HOME and HOME are unset".into()
        })
}

fn nonempty_path(value: Option<OsString>) -> Option<PathBuf> {
    value.filter(|value| !value.is_empty()).map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::{
        BoundedReadError, ConfigStore, MAX_CONFIG_BYTES, ThemeInstallOutcome,
        default_config_file_from, default_fallback_theme_path, default_ghostty_config_file_from,
        default_theme_resource_path, fallback_theme_publication_error,
        install_fallback_theme_if_referenced, read_bounded,
    };
    use std::ffi::OsString;
    use std::fs;
    use std::io::Read;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};
    use zentty_core::{
        AppConfig, BackgroundOpacity, ClipboardConfig, CommandFlattenAggressiveness,
        ConfirmationsConfig, NotificationsConfig, RestoreConfig, ThemeMode, ThemeSpec,
    };

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
    fn general_update_preserves_symlink_comments_unknowns_and_all_source_values() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let root = private_root("general-update");
        let target = root.join("shared/settings.toml");
        let path = root.join("zentty/config.toml");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &target,
            "# preserve\nfuture_root = 7\n[clipboard]\nfuture_clipboard = true\n[appearance]\ntheme_mode = \"light\"\n",
        )
        .unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        let symlink_inode = fs::symlink_metadata(&path).unwrap().ino();
        let confirmations = ConfirmationsConfig {
            confirm_before_closing_pane: false,
            confirm_before_closing_window: false,
            confirm_before_quitting: false,
        };
        let restore = RestoreConfig {
            restore_workspace_on_launch: false,
        };
        let mut clipboard = ClipboardConfig {
            always_clean_copies: true,
            ..ClipboardConfig::default()
        };
        clipboard.clean_options.flatten_multi_line_commands = false;
        clipboard.clean_options.command_flatten_aggressiveness = CommandFlattenAggressiveness::High;
        clipboard.clean_options.preserve_blank_lines_when_flattening = true;
        clipboard.clean_options.remove_box_drawing = false;
        clipboard.clean_options.flatten_slash_command_selections = false;
        clipboard.clean_options.strip_url_tracking_parameters = false;
        clipboard.clean_options.quote_paths_with_spaces = false;
        clipboard.show_copy_markdown_command = false;

        ConfigStore::update_general(&path, confirmations, restore, clipboard).unwrap();
        assert_eq!(fs::symlink_metadata(&path).unwrap().ino(), symlink_inode);
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let source = fs::read_to_string(&target).unwrap();
        assert!(source.contains("# preserve"));
        assert!(source.contains("future_root = 7"));
        assert!(source.contains("future_clipboard = true"));
        assert!(source.contains("[appearance]"));
        let parsed = AppConfig::parse_toml(&source).unwrap();
        assert_eq!(parsed.confirmations, confirmations);
        assert_eq!(parsed.restore, restore);
        assert_eq!(parsed.clipboard, clipboard);
        remove(&root);
    }

    #[test]
    fn notification_update_preserves_symlink_comments_unknowns_and_mode() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let root = private_root("notification-update");
        let target = root.join("shared/settings.toml");
        let path = root.join("zentty/config.toml");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &target,
            "# preserve\nfuture_root = 7\n[notifications]\nfuture = true\n[appearance]\ntheme_mode = \"light\"\n",
        )
        .unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        let symlink_inode = fs::symlink_metadata(&path).unwrap().ino();
        let notifications = NotificationsConfig {
            sound_name: "message-new-instant".into(),
            custom_sound_display_name: Some("Custom alert.ogg".into()),
        };

        ConfigStore::update_notifications(&path, &notifications).unwrap();
        assert_eq!(fs::symlink_metadata(&path).unwrap().ino(), symlink_inode);
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let source = fs::read_to_string(&target).unwrap();
        assert!(source.contains("# preserve"));
        assert!(source.contains("future_root = 7"));
        assert!(source.contains("future = true"));
        assert!(source.contains("[appearance]"));
        assert_eq!(
            AppConfig::parse_toml(&source).unwrap().notifications,
            notifications
        );

        let without_custom = NotificationsConfig {
            sound_name: String::new(),
            custom_sound_display_name: None,
        };
        ConfigStore::update_notifications(&path, &without_custom).unwrap();
        let source = fs::read_to_string(&target).unwrap();
        assert!(!source.contains("custom_sound_display_name"));
        assert_eq!(
            AppConfig::parse_toml(&source).unwrap().notifications,
            without_custom
        );
        remove(&root);
    }

    #[test]
    fn update_channel_preserves_symlink_comments_unknowns_privacy_and_mode() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        use zentty_core::{UpdateChannel, UpdatesConfig};

        let root = private_root("update-channel");
        let target = root.join("shared/settings.toml");
        let path = root.join("zentty/config.toml");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &target,
            "# preserve\nfuture_root = 7\n[updates]\nchannel = \"stable\"\nfuture = true\n[error_reporting]\nenabled = false\n",
        )
        .unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        let symlink_inode = fs::symlink_metadata(&path).unwrap().ino();

        ConfigStore::update_updates(
            &path,
            UpdatesConfig {
                channel: UpdateChannel::Beta,
            },
        )
        .unwrap();

        assert_eq!(fs::symlink_metadata(&path).unwrap().ino(), symlink_inode);
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let source = fs::read_to_string(&target).unwrap();
        assert!(source.contains("# preserve"));
        assert!(source.contains("future_root = 7"));
        assert!(source.contains("future = true"));
        let parsed = AppConfig::parse_toml(&source).unwrap();
        assert_eq!(parsed.updates.channel, UpdateChannel::Beta);
        assert!(!parsed.error_reporting.enabled);
        remove(&root);
    }

    #[test]
    fn general_update_rejects_invalid_or_non_table_source_without_replacement() {
        for (name, source) in [
            ("malformed", "[clipboard\n"),
            ("non-table", "clipboard = true\n"),
        ] {
            let root = private_root(name);
            let path = root.join("zentty/config.toml");
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, source).unwrap();
            assert!(
                ConfigStore::update_general(
                    &path,
                    ConfirmationsConfig::default(),
                    RestoreConfig::default(),
                    ClipboardConfig::default(),
                )
                .is_err()
            );
            assert_eq!(fs::read_to_string(&path).unwrap(), source);
            remove(&root);
        }
    }

    #[test]
    fn general_update_distinguishes_missing_files_from_other_read_failures() {
        use std::os::unix::fs::PermissionsExt;

        let root = private_root("general-read-boundaries");
        let missing = root.join("missing/config.toml");
        ConfigStore::update_general(
            &missing,
            ConfirmationsConfig::default(),
            RestoreConfig::default(),
            ClipboardConfig::default(),
        )
        .unwrap();
        assert!(
            fs::read_to_string(&missing)
                .unwrap()
                .contains("confirm_before_closing_pane = true")
        );

        let unreadable = root.join("unreadable-config.toml");
        fs::write(&unreadable, "# private").unwrap();
        fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000)).unwrap();
        let error = ConfigStore::update_general(
            &unreadable,
            ConfirmationsConfig::default(),
            RestoreConfig::default(),
            ClipboardConfig::default(),
        )
        .unwrap_err();
        assert!(error.contains("could not read"), "{error}");
        fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(fs::read_to_string(&unreadable).unwrap(), "# private");
        remove(&root);
    }

    #[test]
    fn general_update_accepts_the_exact_size_limit_and_rejects_one_byte_more() {
        let maximum = usize::try_from(MAX_CONFIG_BYTES).unwrap();
        for (name, length, should_succeed) in
            [("exact", maximum, true), ("over", maximum + 1, false)]
        {
            let root = private_root(name);
            let path = root.join("zentty/config.toml");
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            let base = concat!(
                "[confirmations]\n",
                "confirm_before_closing_pane = true\n",
                "confirm_before_closing_window = true\n",
                "confirm_before_quitting = true\n",
                "[restore]\n",
                "restore_workspace_on_launch = true\n",
                "[clipboard]\n",
                "always_clean_copies = false\n",
                "flatten_multi_line_commands = true\n",
                "command_flatten_aggressiveness = \"normal\"\n",
                "preserve_blank_lines_when_flattening = false\n",
                "remove_box_drawing = true\n",
                "flatten_slash_command_selections = true\n",
                "strip_url_tracking_parameters = true\n",
                "quote_paths_with_spaces = true\n",
                "show_copy_markdown_command = true\n",
            );
            let source = format!("#{}\n{base}", "x".repeat(length - base.len() - 2));
            assert_eq!(source.len(), length);
            fs::write(&path, &source).unwrap();
            let result = ConfigStore::update_general(
                &path,
                ConfirmationsConfig::default(),
                RestoreConfig::default(),
                ClipboardConfig::default(),
            );
            assert_eq!(result.is_ok(), should_succeed, "{result:?}");
            if !should_succeed {
                assert_eq!(fs::read_to_string(&path).unwrap(), source);
            }
            remove(&root);
        }
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
        assert_eq!(
            default_ghostty_config_file_from(
                Some(OsString::from("/xdg")),
                Some(OsString::from("/home"))
            )
            .unwrap(),
            Path::new("/xdg/ghostty/config")
        );
    }

    #[cfg(unix)]
    #[test]
    fn ghostty_appearance_update_preserves_symlink_comments_unknowns_and_permissions() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let root = private_root("ghostty-appearance-symlink");
        let dotfiles = root.join("dotfiles");
        let config_dir = root.join("xdg/ghostty");
        fs::create_dir_all(&dotfiles).unwrap();
        fs::create_dir_all(&config_dir).unwrap();
        let target = dotfiles.join("ghostty.conf");
        fs::write(
            &target,
            "# retained\nfont-size = 14\ntheme = Old\ntheme = Stale\n",
        )
        .unwrap();
        let link = config_dir.join("config");
        symlink("../../dotfiles/ghostty.conf", &link).unwrap();

        let spec = ThemeSpec::new(
            ThemeMode::Automatic,
            Some("Catppuccin Frappe"),
            Some("Catppuccin Latte"),
        );
        ConfigStore::update_ghostty_value(&link, "theme", &spec.to_string()).unwrap();

        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            "# retained\nfont-size = 14\ntheme = dark:Catppuccin Frappe,light:Catppuccin Latte\n"
        );
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o600
        );
        remove(&root);
    }

    #[test]
    fn concurrent_ghostty_updates_serialize_without_losing_distinct_owned_keys() {
        let root = private_root("ghostty-appearance-concurrent");
        let path = root.join("ghostty/config");
        let theme_path = path.clone();
        let opacity_path = path.clone();
        let theme = std::thread::spawn(move || {
            ConfigStore::update_ghostty_value(&theme_path, "theme", "TokyoNight")
        });
        let opacity = std::thread::spawn(move || {
            ConfigStore::update_ghostty_value(&opacity_path, "background-opacity", "0.75")
        });
        theme.join().unwrap().unwrap();
        opacity.join().unwrap().unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("theme = TokyoNight\n"));
        assert!(contents.contains("background-opacity = 0.75\n"));
        remove(&root);
    }

    #[test]
    fn ghostty_writer_rejects_unsupported_keys_and_normalizes_opacity() {
        let root = private_root("ghostty-appearance-validation");
        let path = root.join("ghostty/config");
        assert!(ConfigStore::update_ghostty_value(&path, "font-size", "1").is_err());
        let opacity = BackgroundOpacity::from_fraction(4.0).unwrap();
        ConfigStore::update_ghostty_value(&path, "background-opacity", &opacity.to_string())
            .unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "background-opacity = 1.00\n"
        );
        remove(&root);
    }

    #[test]
    fn appearance_update_round_trips_and_preserves_unrelated_zentty_configuration() {
        let root = private_root("zentty-appearance-update");
        let path = root.join("zentty/config.toml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "# retained\n[future]\nenabled = true\n").unwrap();
        let appearance = zentty_core::AppearanceConfig {
            theme_mode: ThemeMode::Automatic,
            preferred_dark_theme_name: Some("Catppuccin Frappe".into()),
            preferred_light_theme_name: Some("Catppuccin Latte".into()),
            background_opacity: BackgroundOpacity::from_fraction(0.87),
            sync_opencode_theme_with_terminal: false,
        };
        ConfigStore::update_appearance(&path, &appearance).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("# retained\n"));
        assert!(contents.contains("[future]\nenabled = true\n"));
        assert_eq!(
            ConfigStore::load(path).unwrap().config.appearance,
            appearance
        );
        remove(&root);
    }

    #[test]
    fn fallback_theme_installs_exact_private_bytes_once_and_only_when_referenced() {
        use std::os::unix::fs::PermissionsExt;

        let root = private_root("fallback-theme-install");
        let resource = root.join("resource");
        let target = root.join("xdg/ghostty/themes/GitHub-Dark-Personal");
        fs::create_dir_all(&root).unwrap();
        fs::write(&resource, b"background = #010203\n").unwrap();
        let unrelated = ThemeSpec::new(
            ThemeMode::Automatic,
            Some("Other Dark"),
            Some("Other Light"),
        );
        assert_eq!(
            install_fallback_theme_if_referenced(&unrelated, &root.join("missing"), &target)
                .unwrap(),
            ThemeInstallOutcome::NotReferenced
        );
        let light = ThemeSpec::new(ThemeMode::Light, None, None);
        assert_eq!(
            install_fallback_theme_if_referenced(&light, &root.join("missing"), &target).unwrap(),
            ThemeInstallOutcome::NotReferenced
        );
        for automatic in [
            ThemeSpec::new(ThemeMode::Automatic, None, Some("Other Light")),
            ThemeSpec::new(
                ThemeMode::Automatic,
                Some("Other Dark"),
                Some("GitHub-Dark-Personal"),
            ),
        ] {
            assert_eq!(
                install_fallback_theme_if_referenced(&automatic, &resource, &target).unwrap(),
                ThemeInstallOutcome::Installed
            );
            fs::remove_file(&target).unwrap();
        }
        let fallback = ThemeSpec::new(ThemeMode::Dark, None, None);
        assert_eq!(
            install_fallback_theme_if_referenced(&fallback, &resource, &target).unwrap(),
            ThemeInstallOutcome::Installed
        );
        assert_eq!(fs::read(&target).unwrap(), b"background = #010203\n");
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::write(&resource, b"changed\n").unwrap();
        assert_eq!(
            install_fallback_theme_if_referenced(&fallback, &resource, &target).unwrap(),
            ThemeInstallOutcome::AlreadyPresent
        );
        assert_eq!(fs::read(&target).unwrap(), b"background = #010203\n");
        remove(&root);
    }

    #[test]
    fn fallback_theme_refuses_symlink_and_invalid_resource() {
        use std::os::unix::fs::symlink;

        let root = private_root("fallback-theme-reject");
        let resource = root.join("resource");
        let target = root.join("themes/GitHub-Dark-Personal");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&resource, b"").unwrap();
        let fallback = ThemeSpec::new(ThemeMode::Dark, None, None);
        assert!(
            install_fallback_theme_if_referenced(&fallback, &resource, &target)
                .unwrap_err()
                .contains("must contain")
        );
        symlink(&resource, &target).unwrap();
        assert!(
            install_fallback_theme_if_referenced(&fallback, &resource, &target)
                .unwrap_err()
                .contains("symlink")
        );
        fs::remove_file(&target).unwrap();
        fs::create_dir(&target).unwrap();
        assert!(
            install_fallback_theme_if_referenced(&fallback, &resource, &target)
                .unwrap_err()
                .contains("not a regular file")
        );
        remove(&root);
    }

    #[test]
    fn fallback_theme_bounds_and_inspection_errors_are_distinct() {
        let root = private_root("fallback-theme-bounds");
        let resource = root.join("resource");
        let fallback = ThemeSpec::new(ThemeMode::Dark, None, None);
        let limit = usize::try_from(super::MAX_THEME_BYTES).unwrap();
        fs::create_dir_all(&root).unwrap();
        fs::write(&resource, vec![b'x'; limit]).unwrap();
        let exact = root.join("exact/GitHub-Dark-Personal");
        assert_eq!(
            install_fallback_theme_if_referenced(&fallback, &resource, &exact).unwrap(),
            ThemeInstallOutcome::Installed
        );
        fs::write(&resource, vec![b'x'; limit + 1]).unwrap();
        assert!(
            install_fallback_theme_if_referenced(
                &fallback,
                &resource,
                &root.join("oversize/GitHub-Dark-Personal")
            )
            .unwrap_err()
            .contains("must contain")
        );
        let non_directory = root.join("not-a-directory");
        fs::write(&non_directory, b"file").unwrap();
        assert!(
            install_fallback_theme_if_referenced(
                &fallback,
                &resource,
                &non_directory.join("GitHub-Dark-Personal")
            )
            .unwrap_err()
            .contains("could not inspect")
        );
        remove(&root);
    }

    #[test]
    fn fallback_theme_path_prefers_xdg_and_requires_a_configuration_root() {
        assert_eq!(
            default_fallback_theme_path(Some("/xdg".into()), Some("/home/user".into())).unwrap(),
            Path::new("/xdg/ghostty/themes/GitHub-Dark-Personal")
        );
        assert_eq!(
            default_fallback_theme_path(None, Some("/home/user".into())).unwrap(),
            Path::new("/home/user/.config/ghostty/themes/GitHub-Dark-Personal")
        );
        assert!(default_fallback_theme_path(None, None).is_err());
        assert!(
            default_theme_resource_path()
                .unwrap()
                .ends_with("share/zentty/ghostty/themes/GitHub-Dark-Personal")
        );
        assert!(
            fallback_theme_publication_error(
                &std::io::Error::from(std::io::ErrorKind::AlreadyExists),
                Path::new("/theme")
            )
            .contains("appeared concurrently")
        );
        assert!(
            fallback_theme_publication_error(
                &std::io::Error::from(std::io::ErrorKind::PermissionDenied),
                Path::new("/theme")
            )
            .contains("could not publish")
        );
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
