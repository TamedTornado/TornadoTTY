use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use zentty_linux::platform::{UserDirectory, resolve_user_path};

use zentty_core::{
    AgentCaffeinationConfig, AgentIntegrationsConfig, AgentTeamsConfig, AppConfig,
    AppearanceConfig, ClipboardConfig, ConfirmationsConfig, FALLBACK_DARK_THEME, MenuBarConfig,
    NotificationsConfig, OpenWithConfig, PaneConfig, PaneLayoutConfig, RestoreConfig,
    ServerDetectionConfig, ShortcutBinding, SidebarConfig, ThemeMode, ThemeSpec, UpdatesConfig,
    WorklaneConfig, update_ghostty_value,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ConfigSnapshot {
    pub(crate) config: AppConfig,
    pub(crate) path: PathBuf,
    pub(crate) warning: Option<String>,
}

pub(crate) struct ConfigReloadSnapshot {
    pub(crate) config: AppConfig,
    pub(crate) retained_sections: Vec<&'static str>,
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
        Self::load(default_config_file()?)
    }

    pub(crate) fn load_path_for_reload(
        path: &Path,
        last_good: &AppConfig,
    ) -> Result<ConfigReloadSnapshot, String> {
        match read_config_contents(path)? {
            ConfigContents::Missing => Err(format!(
                "configuration target is unavailable: {}",
                path.display()
            )),
            ConfigContents::Invalid => Ok(ConfigReloadSnapshot {
                config: last_good.clone(),
                retained_sections: Vec::new(),
                warning: Some(content_safe_invalid_warning(path)),
            }),
            ConfigContents::Source(source) => {
                match AppConfig::parse_toml_partial(&source, last_good) {
                    Ok(partial) => Ok(ConfigReloadSnapshot {
                        config: partial.config,
                        retained_sections: partial.retained_sections,
                        warning: None,
                    }),
                    Err(_) => Ok(ConfigReloadSnapshot {
                        config: last_good.clone(),
                        retained_sections: Vec::new(),
                        warning: Some(content_safe_invalid_warning(path)),
                    }),
                }
            }
        }
    }

    fn load(path: PathBuf) -> Result<ConfigSnapshot, String> {
        match read_config_contents(&path)? {
            ConfigContents::Missing => Ok(ConfigSnapshot {
                config: AppConfig::default(),
                path,
                warning: None,
            }),
            ConfigContents::Invalid => Ok(invalid_snapshot(path)),
            ConfigContents::Source(source) => match AppConfig::parse_toml(&source) {
                Ok(config) => Ok(ConfigSnapshot {
                    config,
                    path,
                    warning: None,
                }),
                Err(_) => Ok(invalid_snapshot(path)),
            },
        }
    }

    pub(crate) fn update_default_ignored_port_rules(rules: &[String]) -> Result<PathBuf, String> {
        let path = default_config_file()?;
        Self::update_ignored_port_rules(&path, rules)?;
        Ok(path)
    }

    pub(crate) fn update_default_preferred_browser(browser_id: &str) -> Result<PathBuf, String> {
        let path = default_config_file()?;
        Self::update_preferred_browser(&path, browser_id)?;
        Ok(path)
    }

    pub(crate) fn update_default_shortcuts(
        bindings: &[ShortcutBinding],
    ) -> Result<PathBuf, String> {
        let path = default_config_file()?;
        Self::update_shortcuts(&path, bindings)?;
        Ok(path)
    }

    pub(crate) fn update_default_appearance(
        appearance: &AppearanceConfig,
    ) -> Result<PathBuf, String> {
        let path = default_config_file()?;
        Self::update_appearance(&path, appearance)?;
        Ok(path)
    }

    pub(crate) fn update_default_sidebar(sidebar: SidebarConfig) -> Result<PathBuf, String> {
        let path = default_config_file()?;
        Self::update_sidebar(&path, sidebar)?;
        Ok(path)
    }

    fn update_sidebar(path: &Path, sidebar: SidebarConfig) -> Result<(), String> {
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
                .entry("sidebar")
                .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()));
            let table = document["sidebar"]
                .as_table_mut()
                .ok_or_else(|| "sidebar configuration is not a table".to_owned())?;
            table["width"] = toml_edit::value(i64::from(sidebar.width));
            table["visibility"] = toml_edit::value(sidebar.visibility.config_value());
            atomic_replace(&target, document.to_string().as_bytes())
        })
    }

    pub(crate) fn update_default_general(
        confirmations: ConfirmationsConfig,
        restore: RestoreConfig,
        clipboard: ClipboardConfig,
    ) -> Result<PathBuf, String> {
        let path = default_config_file()?;
        Self::update_general(&path, confirmations, restore, clipboard)?;
        Ok(path)
    }

    pub(crate) fn update_default_notifications(
        notifications: &NotificationsConfig,
    ) -> Result<PathBuf, String> {
        let path = default_config_file()?;
        Self::update_notifications(&path, notifications)?;
        Ok(path)
    }

    pub(crate) fn update_default_updates(updates: UpdatesConfig) -> Result<PathBuf, String> {
        let path = default_config_file()?;
        Self::update_updates(&path, updates)?;
        Ok(path)
    }

    pub(crate) fn update_default_error_reporting(
        error_reporting: zentty_core::ErrorReportingConfig,
    ) -> Result<PathBuf, String> {
        let path = default_config_file()?;
        Self::update_error_reporting(&path, error_reporting)?;
        Ok(path)
    }

    pub(crate) fn update_default_workspace_panes(
        worklanes: WorklaneConfig,
        pane_layout: PaneLayoutConfig,
        panes: PaneConfig,
    ) -> Result<PathBuf, String> {
        let path = default_config_file()?;
        Self::update_workspace_panes(&path, worklanes, pane_layout, panes)?;
        Ok(path)
    }

    pub(crate) fn update_default_open_with(config: &OpenWithConfig) -> Result<PathBuf, String> {
        let path = default_config_file()?;
        Self::update_open_with(&path, config)?;
        Ok(path)
    }

    fn update_open_with(path: &Path, config: &OpenWithConfig) -> Result<(), String> {
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
                .entry("open_with")
                .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()));
            let table = document["open_with"]
                .as_table_mut()
                .ok_or_else(|| "open_with configuration is not a table".to_owned())?;
            table["primary_target_id"] = toml_edit::value(&config.primary_target_id);
            let mut enabled = toml_edit::Array::new();
            for id in &config.enabled_target_ids {
                enabled.push(id);
            }
            table["enabled_target_ids"] = toml_edit::value(enabled);
            let mut custom_apps = toml_edit::ArrayOfTables::new();
            for app in &config.custom_apps {
                let mut custom = toml_edit::Table::new();
                custom["id"] = toml_edit::value(&app.id);
                custom["name"] = toml_edit::value(&app.name);
                custom["path"] = toml_edit::value(&app.path);
                custom_apps.push(custom);
            }
            table["custom_apps"] = toml_edit::Item::ArrayOfTables(custom_apps);
            atomic_replace(&target, document.to_string().as_bytes())
        })
    }

    pub(crate) fn update_default_server_detection(
        config: &ServerDetectionConfig,
    ) -> Result<PathBuf, String> {
        let path = default_config_file()?;
        Self::update_server_detection(&path, config)?;
        Ok(path)
    }

    fn update_server_detection(path: &Path, config: &ServerDetectionConfig) -> Result<(), String> {
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
                .entry("server_detection")
                .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()));
            let table = document["server_detection"]
                .as_table_mut()
                .ok_or_else(|| "server_detection configuration is not a table".to_owned())?;
            table["passive_detection_enabled"] = toml_edit::value(config.passive_detection_enabled);
            table["preferred_browser_id"] = toml_edit::value(&config.preferred_browser_id);
            let mut enabled = toml_edit::Array::new();
            for id in &config.enabled_browser_target_ids {
                enabled.push(id);
            }
            table["enabled_browser_target_ids"] = toml_edit::value(enabled);
            let mut ignored = toml_edit::Array::new();
            for rule in &config.ignored_port_rules {
                ignored.push(rule);
            }
            table["ignored_port_rules"] = toml_edit::value(ignored);
            let mut browsers = toml_edit::ArrayOfTables::new();
            for browser in &config.custom_browsers {
                let mut custom = toml_edit::Table::new();
                custom["id"] = toml_edit::value(&browser.id);
                custom["name"] = toml_edit::value(&browser.name);
                custom["path"] = toml_edit::value(&browser.path);
                if let Some(bundle_identifier) = &browser.bundle_identifier {
                    custom["bundle_identifier"] = toml_edit::value(bundle_identifier);
                }
                browsers.push(custom);
            }
            table["custom_browsers"] = toml_edit::Item::ArrayOfTables(browsers);
            atomic_replace(&target, document.to_string().as_bytes())
        })
    }

    pub(crate) fn update_default_agents(
        teams: AgentTeamsConfig,
        caffeination: AgentCaffeinationConfig,
        menu_bar: MenuBarConfig,
        integrations: &AgentIntegrationsConfig,
    ) -> Result<PathBuf, String> {
        let path = default_config_file()?;
        Self::update_agents(&path, teams, caffeination, menu_bar, integrations)?;
        Ok(path)
    }

    fn update_agents(
        path: &Path,
        teams: AgentTeamsConfig,
        caffeination: AgentCaffeinationConfig,
        menu_bar: MenuBarConfig,
        integrations: &AgentIntegrationsConfig,
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
            for section in [
                "agent_teams",
                "agent_caffeination",
                "menu_bar",
                "agent_integrations",
            ] {
                document
                    .entry(section)
                    .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()));
                if !document[section].is_table() {
                    return Err(format!("{section} configuration is not a table"));
                }
            }
            document["agent_teams"]["enabled"] = toml_edit::value(teams.enabled);
            document["agent_caffeination"]["enabled"] = toml_edit::value(caffeination.enabled);
            document["menu_bar"]["show_status_item"] = toml_edit::value(menu_bar.show_status_item);
            document["agent_integrations"]["grandfathered_v1"] =
                toml_edit::value(integrations.grandfathered_v1);
            let mut states = toml_edit::Table::new();
            for (tool, state) in &integrations.states {
                states[tool] = toml_edit::value(state.config_value());
            }
            document["agent_integrations"]["states"] = toml_edit::Item::Table(states);
            atomic_replace(&target, document.to_string().as_bytes())
        })
    }

    fn update_workspace_panes(
        path: &Path,
        worklanes: WorklaneConfig,
        pane_layout: PaneLayoutConfig,
        panes: PaneConfig,
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
            for section in ["worklanes", "pane_layout", "panes"] {
                document
                    .entry(section)
                    .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()));
                if !document[section].is_table() {
                    return Err(format!("{section} configuration is not a table"));
                }
            }
            document["worklanes"]["new_worklane_placement"] =
                toml_edit::value(worklanes.new_worklane_placement.config_value());
            document["pane_layout"]["right_split_behavior"] =
                toml_edit::value(pane_layout.right_split_behavior.config_value());
            document["pane_layout"]["visible_split_window_width"] =
                toml_edit::value(i64::from(pane_layout.visible_split_window_width));
            document["panes"]["show_labels"] = toml_edit::value(panes.show_labels);
            document["panes"]["show_borders"] = toml_edit::value(panes.show_borders);
            document["panes"]["inactive_opacity"] =
                toml_edit::value(f64::from(panes.inactive_opacity_percent) / 100.0);
            document["panes"]["show_project_icons"] = toml_edit::value(panes.show_project_icons);
            document["panes"]["smooth_scroll_enabled"] =
                toml_edit::value(panes.smooth_scroll_enabled);
            document["panes"]["focus_follows_mouse"] = toml_edit::value(panes.focus_follows_mouse);
            document["panes"]["focus_follows_mouse_delay"] =
                toml_edit::value(panes.focus_follows_mouse_delay.config_value());
            atomic_replace(&target, document.to_string().as_bytes())
        })
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

    fn update_error_reporting(
        path: &Path,
        error_reporting: zentty_core::ErrorReportingConfig,
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
                .entry("error_reporting")
                .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()));
            if !document["error_reporting"].is_table() {
                return Err("error_reporting configuration is not a table".to_owned());
            }
            document["error_reporting"]["enabled"] = toml_edit::value(error_reporting.enabled);
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
            document["notifications"]["notify_when_pane_visible"] =
                toml_edit::value(notifications.notify_when_pane_visible);
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
            document["restore"]["start_restored_sessions_in_background"] =
                toml_edit::value(restore.start_restored_sessions_in_background);
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
            document["appearance"]["sidebar_selection_emphasis"] =
                toml_edit::value(appearance.sidebar_selection_emphasis.config_value());
            atomic_replace(&target, document.to_string().as_bytes())
        })
    }

    pub(crate) fn update_default_ghostty_theme(spec: &ThemeSpec) -> Result<PathBuf, String> {
        let executable = std::env::current_exe()
            .map_err(|error| format!("could not locate Zentty executable: {error}"))?;
        let (bundled, user) = crate::theme_catalog::default_theme_directories(
            &executable,
            std::env::var_os("XDG_CONFIG_HOME").as_deref(),
            std::env::var_os("HOME").as_deref(),
        )?;
        let runtime_spec = ghostty_theme_spec_for_runtime(spec, &bundled, &user)?;
        Self::update_default_ghostty_value("theme", &runtime_spec.to_string())
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
            std::env::var_os("XDG_CONFIG_HOME").as_deref(),
            std::env::var_os("HOME").as_deref(),
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
        with_config_lock(&target, || {
            let source = editable_config_source(&target)?;
            let mut document = source
                .parse::<toml_edit::DocumentMut>()
                .map_err(|error| format!("could not edit invalid configuration: {error}"))?;
            let mut array = toml_edit::Array::new();
            for rule in rules {
                array.push(rule.as_str());
            }
            document["server_detection"]["ignored_port_rules"] = toml_edit::value(array);
            atomic_replace(&target, document.to_string().as_bytes())
        })
    }

    fn update_preferred_browser(path: &Path, browser_id: &str) -> Result<(), String> {
        let target = resolve_config_target(path)?;
        with_config_lock(&target, || {
            let source = editable_config_source(&target)?;
            let mut document = source
                .parse::<toml_edit::DocumentMut>()
                .map_err(|error| format!("could not edit invalid configuration: {error}"))?;
            document["server_detection"]["preferred_browser_id"] = toml_edit::value(browser_id);
            atomic_replace(&target, document.to_string().as_bytes())
        })
    }

    fn update_shortcuts(path: &Path, bindings: &[ShortcutBinding]) -> Result<(), String> {
        let target = resolve_config_target(path)?;
        with_config_lock(&target, || {
            let source = editable_config_source(&target)?;
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
        })
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
            "{} executable has no install prefix: {}",
            zentty_core::PRODUCT_NAME,
            executable.display()
        )
    })?;
    Ok(prefix
        .join("share/zentty/ghostty/themes")
        .join(FALLBACK_DARK_THEME))
}

fn ghostty_theme_spec_for_runtime(
    spec: &ThemeSpec,
    bundled_directory: &Path,
    user_directory: &Path,
) -> Result<ThemeSpec, String> {
    let resolve =
        |name: &str| ghostty_theme_reference_for_runtime(name, bundled_directory, user_directory);
    match spec.mode {
        ThemeMode::Dark => Ok(ThemeSpec {
            mode: ThemeMode::Dark,
            dark_theme_name: Some(resolve(spec.resolved_dark_theme_name())?),
            light_theme_name: None,
        }),
        ThemeMode::Light => Ok(ThemeSpec {
            mode: ThemeMode::Light,
            dark_theme_name: None,
            light_theme_name: Some(resolve(spec.resolved_light_theme_name())?),
        }),
        ThemeMode::Automatic => Ok(ThemeSpec {
            mode: ThemeMode::Automatic,
            dark_theme_name: Some(resolve(spec.resolved_dark_theme_name())?),
            light_theme_name: Some(resolve(spec.resolved_light_theme_name())?),
        }),
    }
}

fn ghostty_theme_reference_for_runtime(
    name: &str,
    bundled_directory: &Path,
    user_directory: &Path,
) -> Result<String, String> {
    let requested = Path::new(name);
    if requested.is_absolute()
        || requested.components().count() != 1
        || !matches!(
            requested.components().next(),
            Some(std::path::Component::Normal(_))
        )
    {
        return Ok(name.to_owned());
    }
    if fs::metadata(user_directory.join(requested)).is_ok_and(|metadata| metadata.is_file()) {
        return Ok(name.to_owned());
    }
    let candidate = bundled_directory.join(requested);
    match fs::symlink_metadata(&candidate) {
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(name.to_owned()),
        Err(error) => {
            return Err(format!(
                "could not inspect bundled theme {}: {error}",
                candidate.display()
            ));
        }
        Ok(metadata) if !metadata.file_type().is_file() => {
            return Err(format!(
                "bundled theme is not a regular file: {}",
                candidate.display()
            ));
        }
        Ok(_) => {}
    }
    let canonical_root = fs::canonicalize(bundled_directory).map_err(|error| {
        format!(
            "could not resolve bundled theme directory {}: {error}",
            bundled_directory.display()
        )
    })?;
    let canonical = fs::canonicalize(&candidate).map_err(|error| {
        format!(
            "could not resolve bundled theme {}: {error}",
            candidate.display()
        )
    })?;
    if !canonical.starts_with(&canonical_root) {
        return Err(format!(
            "bundled theme escapes its resource directory: {}",
            candidate.display()
        ));
    }
    canonical.to_str().map(str::to_owned).ok_or_else(|| {
        format!(
            "bundled theme path is not valid UTF-8: {}",
            canonical.display()
        )
    })
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
    let lock_path = parent.join(".zentty-config.lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .custom_flags(libc::O_NOFOLLOW)
        .mode(0o600)
        .open(&lock_path)
        .map_err(|error| format!("could not open {}: {error}", lock_path.display()))?;
    lock.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("could not secure {}: {error}", lock_path.display()))?;
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

pub(crate) fn resolve_config_target(path: &Path) -> Result<PathBuf, String> {
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
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("could not sync {}: {error}", parent.display()))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn editable_config_source(path: &Path) -> Result<String, String> {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("could not read {}: {error}", path.display())),
    };
    if source.len() as u64 > MAX_CONFIG_BYTES {
        return Err(format!("configuration exceeds {MAX_CONFIG_BYTES} bytes"));
    }
    Ok(source)
}

enum ConfigContents {
    Missing,
    Invalid,
    Source(String),
}

fn read_config_contents(path: &Path) -> Result<ConfigContents, String> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(ConfigContents::Missing),
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
            "{} configuration is not a regular file: {}",
            zentty_core::PRODUCT_NAME,
            path.display()
        ));
    }
    let bytes = match read_bounded(&mut file, metadata.len()) {
        Ok(bytes) => bytes,
        Err(BoundedReadError::TooLarge) => return Ok(ConfigContents::Invalid),
        Err(BoundedReadError::Io(error)) => {
            return Err(format!(
                "could not read Zentty configuration {}: {error}",
                path.display()
            ));
        }
    };
    String::from_utf8(bytes)
        .map(ConfigContents::Source)
        .or(Ok(ConfigContents::Invalid))
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
        warning: Some(content_safe_invalid_warning(&path)),
        path,
    }
}

fn content_safe_invalid_warning(path: &Path) -> String {
    format!(
        "ignored invalid Zentty configuration {}: size, encoding, parse, or known-value validation failed",
        path.display()
    )
}

fn default_config_file() -> Result<PathBuf, String> {
    let path = default_config_file_from(
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )?;
    ensure_private_config_parent(&path)?;
    Ok(path)
}

fn ensure_private_config_parent(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("configuration has no parent: {}", path.display()))?;
    match fs::symlink_metadata(parent) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(format!(
                "refusing symlinked Zentty configuration directory: {}",
                parent.display()
            ));
        }
        Ok(metadata) if !metadata.is_dir() => {
            return Err(format!(
                "{} configuration parent is not a directory: {}",
                zentty_core::PRODUCT_NAME,
                parent.display()
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true).mode(0o700);
            builder
                .create(parent)
                .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
        }
        Err(error) => {
            return Err(format!("could not inspect {}: {error}", parent.display()));
        }
    }
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("could not secure {}: {error}", parent.display()))?;

    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => {
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))
                .map_err(|error| format!("could not secure {}: {error}", path.display()))?;
        }
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(format!("could not inspect {}: {error}", path.display())),
    }
    Ok(())
}

fn default_config_file_from(
    xdg_config_home: Option<&OsStr>,
    home: Option<&OsStr>,
) -> Result<PathBuf, String> {
    resolve_user_path(
        UserDirectory::Config,
        xdg_config_home,
        home,
        Path::new("zentty/config.toml"),
    )
    .map_err(|error| format!("could not resolve Zentty config: {error}"))
}

fn default_ghostty_config_file_from(
    xdg_config_home: Option<&OsStr>,
    home: Option<&OsStr>,
) -> Result<PathBuf, String> {
    resolve_user_path(
        UserDirectory::Config,
        xdg_config_home,
        home,
        Path::new("ghostty/config"),
    )
    .map_err(|error| format!("could not resolve Ghostty config: {error}"))
}

fn nonempty_path(value: Option<OsString>) -> Option<PathBuf> {
    value.filter(|value| !value.is_empty()).map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::{
        BoundedReadError, ConfigStore, MAX_CONFIG_BYTES, ThemeInstallOutcome, default_config_file,
        default_config_file_from, default_fallback_theme_path, default_ghostty_config_file_from,
        default_theme_resource_path, editable_config_source, ensure_private_config_parent,
        fallback_theme_publication_error, ghostty_theme_spec_for_runtime,
        install_fallback_theme_if_referenced, read_bounded, with_config_lock,
    };
    use std::ffi::OsStr;
    use std::fs;
    use std::io::Read;
    use std::path::Path;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use zentty_core::{
        AgentCaffeinationConfig, AgentIntegrationState, AgentIntegrationsConfig, AgentTeamsConfig,
        AppConfig, BackgroundOpacity, ClipboardConfig, CommandFlattenAggressiveness,
        ConfirmationsConfig, FocusFollowsMouseDelay, MenuBarConfig, NewWorklanePlacement,
        NotificationsConfig, OpenWithConfig, OpenWithCustomApp, PaneConfig, PaneLayoutConfig,
        PaneRightBehaviorMode, RestoreConfig, ServerBrowserCustomApp, ServerDetectionConfig,
        SidebarConfig, SidebarVisibilityMode, ThemeMode, ThemeSpec, WorklaneConfig,
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

    #[test]
    fn config_parent_and_lock_are_private_and_symlinked_parent_is_rejected() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};

        let root = private_root("private-parent");
        let path = root.join("xdg/zentty/config.toml");
        ensure_private_config_parent(&path).unwrap();
        assert_eq!(
            fs::metadata(path.parent().unwrap()).unwrap().mode() & 0o777,
            0o700
        );

        fs::set_permissions(path.parent().unwrap(), fs::Permissions::from_mode(0o777)).unwrap();
        fs::write(&path, b"").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
        ensure_private_config_parent(&path).unwrap();
        assert_eq!(
            fs::metadata(path.parent().unwrap()).unwrap().mode() & 0o777,
            0o700
        );
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);

        with_config_lock(&path, || Ok(())).unwrap();
        assert_eq!(
            fs::metadata(path.parent().unwrap().join(".zentty-config.lock"))
                .unwrap()
                .mode()
                & 0o777,
            0o600
        );

        let external = root.join("external");
        fs::create_dir_all(&external).unwrap();
        let linked_parent = root.join("linked-parent");
        symlink(&external, &linked_parent).unwrap();
        assert!(
            ensure_private_config_parent(&linked_parent.join("config.toml"))
                .unwrap_err()
                .contains("symlinked")
        );

        let file_parent = root.join("not-a-directory");
        fs::write(&file_parent, b"not a directory").unwrap();
        assert!(
            ensure_private_config_parent(&file_parent.join("config.toml"))
                .unwrap_err()
                .contains("not a directory")
        );

        let denied = root.join("denied");
        fs::create_dir(&denied).unwrap();
        fs::set_permissions(&denied, fs::Permissions::from_mode(0o000)).unwrap();
        let denied_error = ensure_private_config_parent(&denied.join("zentty/config.toml"))
            .expect_err("an inaccessible ancestor must not be mistaken for an absent directory");
        fs::set_permissions(&denied, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(denied_error.contains("could not inspect"));

        let linked_config_parent = root.join("linked-config-parent");
        fs::create_dir(&linked_config_parent).unwrap();
        let operator_target = root.join("operator-target.toml");
        fs::write(&operator_target, b"").unwrap();
        fs::set_permissions(&operator_target, fs::Permissions::from_mode(0o640)).unwrap();
        let linked_config = linked_config_parent.join("config.toml");
        symlink(&operator_target, &linked_config).unwrap();
        ensure_private_config_parent(&linked_config).unwrap();
        assert_eq!(
            fs::metadata(&operator_target).unwrap().mode() & 0o777,
            0o640,
            "securing the logical path must not chmod an operator-owned symlink target"
        );

        let overlong_config = path.parent().unwrap().join("x".repeat(300));
        assert!(
            ensure_private_config_parent(&overlong_config)
                .unwrap_err()
                .contains("could not inspect")
        );
        remove(&root);
    }

    #[test]
    fn default_config_file_resolves_and_secures_the_process_xdg_home() {
        const CHILD_MARKER: &str = "ZENTTY_CONFIG_STORE_XDG_CHILD";
        if let Some(expected) = std::env::var_os(CHILD_MARKER) {
            let path = default_config_file().unwrap();
            assert_eq!(path, Path::new(&expected).join("zentty/config.toml"));
            assert!(path.parent().unwrap().is_dir());
            return;
        }

        let root = private_root("default-xdg");
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("config_store::tests::default_config_file_resolves_and_secures_the_process_xdg_home")
            .arg("--nocapture")
            .env("XDG_CONFIG_HOME", &root)
            .env(CHILD_MARKER, &root)
            .status()
            .unwrap();
        assert!(status.success());
        assert!(root.join("zentty").is_dir());
        remove(&root);
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
            start_restored_sessions_in_background: true,
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
            notify_when_pane_visible: false,
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
            notify_when_pane_visible: true,
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

        ConfigStore::update_error_reporting(
            &path,
            zentty_core::ErrorReportingConfig { enabled: true },
        )
        .unwrap();
        assert_eq!(fs::symlink_metadata(&path).unwrap().ino(), symlink_inode);
        let source = fs::read_to_string(&target).unwrap();
        assert!(source.contains("# preserve"));
        assert!(source.contains("future = true"));
        let parsed = AppConfig::parse_toml(&source).unwrap();
        assert_eq!(parsed.updates.channel, UpdateChannel::Beta);
        assert!(parsed.error_reporting.enabled);
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
    fn open_with_update_preserves_comments_unknown_keys_and_custom_app_order() {
        let root = private_root("open-with-settings");
        let path = root.join("zentty/config.toml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            "# keep me\nunknown = \"value\"\n[open_with]\nfuture = true\n[appearance]\ntheme_mode = \"light\"\n",
        )
        .unwrap();
        let config = OpenWithConfig {
            primary_target_id: "custom:tool".into(),
            enabled_target_ids: vec!["custom:tool".into(), "system-file-manager".into()],
            custom_apps: vec![OpenWithCustomApp {
                id: "custom:tool".into(),
                name: "Tool".into(),
                path: "/opt/tool".into(),
            }],
        };

        ConfigStore::update_open_with(&path, &config).unwrap();

        let source = fs::read_to_string(&path).unwrap();
        assert!(source.contains("# keep me"));
        assert!(source.contains("unknown = \"value\""));
        assert!(source.contains("future = true"));
        let parsed = AppConfig::parse_toml(&source).unwrap();
        assert_eq!(parsed.open_with, config);
        assert_eq!(parsed.appearance.theme_mode, ThemeMode::Light);
        remove(&root);
    }

    #[test]
    fn server_detection_update_preserves_comments_unknowns_and_all_settings() {
        let root = private_root("server-settings");
        let path = root.join("zentty/config.toml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            "# keep me\nunknown = \"value\"\n[server_detection]\nfuture = true\n",
        )
        .unwrap();
        let config = ServerDetectionConfig {
            passive_detection_enabled: false,
            preferred_browser_id: "custom:browser".into(),
            enabled_browser_target_ids: vec!["custom:browser".into(), "firefox".into()],
            custom_browsers: vec![ServerBrowserCustomApp {
                id: "custom:browser".into(),
                name: "Browser".into(),
                path: "/opt/browser".into(),
                bundle_identifier: None,
            }],
            ignored_port_rules: vec!["3000-3002".into(), "5173".into()],
        };

        ConfigStore::update_server_detection(&path, &config).unwrap();

        let source = fs::read_to_string(&path).unwrap();
        assert!(source.contains("# keep me"));
        assert!(source.contains("unknown = \"value\""));
        assert!(source.contains("future = true"));
        assert_eq!(
            AppConfig::parse_toml(&source).unwrap().server_detection,
            config
        );
        remove(&root);
    }

    #[test]
    fn agent_update_preserves_comments_unknowns_and_source_sections() {
        let root = private_root("agent-settings");
        let path = root.join("zentty/config.toml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            "# keep me\nunknown = \"value\"\n[agent_teams]\nfuture = true\n",
        )
        .unwrap();
        let teams = AgentTeamsConfig { enabled: true };
        let caffeination = AgentCaffeinationConfig { enabled: false };
        let menu_bar = MenuBarConfig {
            show_status_item: false,
        };
        let integrations = AgentIntegrationsConfig {
            states: std::collections::BTreeMap::from([
                ("claude".into(), AgentIntegrationState::Off),
                ("codex".into(), AgentIntegrationState::On),
            ]),
            grandfathered_v1: true,
        };

        ConfigStore::update_agents(&path, teams, caffeination, menu_bar, &integrations).unwrap();

        let source = fs::read_to_string(&path).unwrap();
        assert!(source.contains("# keep me"));
        assert!(source.contains("unknown = \"value\""));
        assert!(source.contains("future = true"));
        let parsed = AppConfig::parse_toml(&source).unwrap();
        assert_eq!(parsed.agent_teams, teams);
        assert_eq!(parsed.agent_caffeination, caffeination);
        assert_eq!(parsed.menu_bar, menu_bar);
        assert_eq!(parsed.agent_integrations, integrations);
        remove(&root);
    }

    #[test]
    fn workspace_pane_update_preserves_comments_unknown_keys_and_symlink() {
        let root = private_root("workspace-pane-settings");
        let target = root.join("shared/settings.toml");
        let path = root.join("zentty/config.toml");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &target,
            "# keep me\nunknown = \"value\"\n[panes]\nfuture = true\n[appearance]\ntheme_mode = \"light\"\n",
        )
        .unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        let worklanes = WorklaneConfig {
            new_worklane_placement: NewWorklanePlacement::End,
        };
        let pane_layout = PaneLayoutConfig {
            right_split_behavior: PaneRightBehaviorMode::AlwaysAdd,
            visible_split_window_width: 1680,
        };
        let panes = PaneConfig {
            show_labels: false,
            show_borders: false,
            inactive_opacity_percent: 83,
            show_project_icons: false,
            smooth_scroll_enabled: true,
            focus_follows_mouse: true,
            focus_follows_mouse_delay: FocusFollowsMouseDelay::Immediate,
        };

        ConfigStore::update_workspace_panes(&path, worklanes, pane_layout, panes).unwrap();

        assert_eq!(fs::read_link(&path).unwrap(), target);
        let source = fs::read_to_string(&path).unwrap();
        assert!(source.contains("# keep me"));
        assert!(source.contains("unknown = \"value\""));
        assert!(source.contains("future = true"));
        let parsed = AppConfig::parse_toml(&source).unwrap();
        assert_eq!(parsed.worklanes, worklanes);
        assert_eq!(parsed.pane_layout, pane_layout);
        assert_eq!(parsed.panes, panes);
        assert_eq!(parsed.appearance.theme_mode, ThemeMode::Light);
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
            default_config_file_from(Some(OsStr::new("/xdg")), Some(OsStr::new("/home"))).unwrap(),
            Path::new("/xdg/zentty/config.toml")
        );
        assert_eq!(
            default_config_file_from(Some(OsStr::new("")), Some(OsStr::new("/home"))).unwrap(),
            Path::new("/home/.config/zentty/config.toml")
        );
        assert!(default_config_file_from(None, None).is_err());
        assert_eq!(
            default_ghostty_config_file_from(Some(OsStr::new("/xdg")), Some(OsStr::new("/home")))
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
            "# retained\nfont-size = 14\nbackground-image = /home/user/wall paper.png\ntheme = Old\ntheme = Stale\n",
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
            "# retained\nfont-size = 14\nbackground-image = /home/user/wall paper.png\ntheme = dark:Catppuccin Frappe,light:Catppuccin Latte\n"
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
    fn every_product_config_writer_contends_on_the_shared_target_lock() {
        use std::sync::mpsc;

        let root = private_root("all-writers-shared-lock");
        let path = root.join("zentty/config.toml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "[server_detection]\n").unwrap();

        let assert_contends = |operation: Box<dyn FnOnce() + Send>| {
            let (sender, receiver) = mpsc::channel();
            let worker = with_config_lock(&path, || {
                let worker = std::thread::spawn(move || {
                    operation();
                    sender.send(()).unwrap();
                });
                assert!(
                    receiver.recv_timeout(Duration::from_millis(50)).is_err(),
                    "writer bypassed the shared config lock"
                );
                Ok(worker)
            })
            .unwrap();
            receiver.recv_timeout(Duration::from_secs(1)).unwrap();
            worker.join().unwrap();
        };

        let ignored_path = path.clone();
        assert_contends(Box::new(move || {
            ConfigStore::update_ignored_port_rules(&ignored_path, &["3000".into()]).unwrap();
        }));
        let browser_path = path.clone();
        assert_contends(Box::new(move || {
            ConfigStore::update_preferred_browser(&browser_path, "system-default").unwrap();
        }));
        let shortcut_path = path.clone();
        assert_contends(Box::new(move || {
            ConfigStore::update_shortcuts(
                &shortcut_path,
                &[zentty_core::ShortcutBinding {
                    command_id: "pane.close".into(),
                    shortcut: None,
                }],
            )
            .unwrap();
        }));
        remove(&root);
    }

    #[test]
    fn shared_editable_source_distinguishes_missing_unreadable_and_size_boundaries() {
        use std::os::unix::fs::PermissionsExt;

        let root = private_root("shared-editable-source-boundaries");
        let path = root.join("zentty/config.toml");
        assert_eq!(editable_config_source(&path).unwrap(), "");

        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "private").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
        assert!(
            editable_config_source(&path)
                .unwrap_err()
                .contains("could not read")
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

        let maximum = usize::try_from(MAX_CONFIG_BYTES).unwrap();
        fs::write(&path, vec![b' '; maximum]).unwrap();
        assert_eq!(editable_config_source(&path).unwrap().len(), maximum);
        fs::write(&path, vec![b' '; maximum + 1]).unwrap();
        assert!(
            editable_config_source(&path)
                .unwrap_err()
                .contains("exceeds")
        );
        remove(&root);
    }

    #[test]
    fn concurrent_product_updates_preserve_distinct_owned_sections() {
        let root = private_root("zentty-config-concurrent-sections");
        let path = root.join("zentty/config.toml");
        let general_path = path.clone();
        let updates_path = path.clone();
        let general = std::thread::spawn(move || {
            let clipboard = ClipboardConfig {
                always_clean_copies: true,
                ..ClipboardConfig::default()
            };
            ConfigStore::update_general(
                &general_path,
                ConfirmationsConfig::default(),
                RestoreConfig::default(),
                clipboard,
            )
        });
        let updates = std::thread::spawn(move || {
            ConfigStore::update_updates(
                &updates_path,
                zentty_core::UpdatesConfig {
                    channel: zentty_core::UpdateChannel::Beta,
                },
            )
        });
        general.join().unwrap().unwrap();
        updates.join().unwrap().unwrap();
        let config = ConfigStore::load(path).unwrap().config;
        assert!(config.clipboard.always_clean_copies);
        assert_eq!(config.updates.channel, zentty_core::UpdateChannel::Beta);
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
            sidebar_selection_emphasis: zentty_core::SidebarSelectionEmphasis::Vivid,
        };
        ConfigStore::update_appearance(&path, &appearance).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("# retained\n"));
        assert!(contents.contains("[future]\nenabled = true\n"));
        assert!(contents.contains("sidebar_selection_emphasis = \"vivid\""));
        assert_eq!(
            ConfigStore::load(path).unwrap().config.appearance,
            appearance
        );
        remove(&root);
    }

    #[test]
    fn sidebar_update_round_trips_atomically_and_preserves_unrelated_configuration() {
        use std::os::unix::fs::PermissionsExt;

        let root = private_root("zentty-sidebar-update");
        let path = root.join("zentty/config.toml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "# retained\n[future]\nenabled = true\n").unwrap();
        let sidebar = SidebarConfig {
            width: 319,
            visibility: SidebarVisibilityMode::Hidden,
        };
        ConfigStore::update_sidebar(&path, sidebar).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("# retained"));
        assert!(contents.contains("[future]"));
        assert!(contents.contains("[sidebar]"));
        assert!(contents.contains("width = 319"));
        assert!(contents.contains("visibility = \"hidden\""));
        assert_eq!(ConfigStore::load(path).unwrap().config.sidebar, sidebar);

        let missing = root.join("missing/config.toml");
        ConfigStore::update_sidebar(&missing, SidebarConfig::default()).unwrap();
        assert_eq!(
            ConfigStore::load(missing).unwrap().config.sidebar,
            SidebarConfig::default()
        );

        let unreadable = root.join("unreadable.toml");
        fs::write(&unreadable, "[sidebar]\n").unwrap();
        fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000)).unwrap();
        let error = ConfigStore::update_sidebar(&unreadable, sidebar).unwrap_err();
        assert!(error.contains("could not read"), "{error}");
        fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o600)).unwrap();

        let bounded = root.join("bounded.toml");
        let prefix = "[sidebar]\nwidth = 319\nvisibility = \"hidden\"\n#";
        let mut exact = prefix.as_bytes().to_vec();
        exact.resize(usize::try_from(MAX_CONFIG_BYTES).unwrap(), b' ');
        fs::write(&bounded, &exact).unwrap();
        ConfigStore::update_sidebar(&bounded, sidebar).unwrap();
        exact.push(b' ');
        fs::write(&bounded, exact).unwrap();
        assert!(
            ConfigStore::update_sidebar(&bounded, sidebar)
                .unwrap_err()
                .contains("configuration exceeds")
        );
        remove(&root);
    }

    #[test]
    fn default_sidebar_update_uses_the_single_process_config_authority() {
        const CHILD_MARKER: &str = "ZENTTY_CONFIG_STORE_SIDEBAR_CHILD";
        if let Some(expected) = std::env::var_os(CHILD_MARKER) {
            let sidebar = SidebarConfig {
                width: 307,
                visibility: SidebarVisibilityMode::Hidden,
            };
            let path = ConfigStore::update_default_sidebar(sidebar).unwrap();
            assert_eq!(path, Path::new(&expected).join("zentty/config.toml"));
            assert_eq!(ConfigStore::load(path).unwrap().config.sidebar, sidebar);
            return;
        }

        let root = private_root("default-sidebar-xdg");
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("config_store::tests::default_sidebar_update_uses_the_single_process_config_authority")
            .arg("--nocapture")
            .env("XDG_CONFIG_HOME", &root)
            .env(CHILD_MARKER, &root)
            .status()
            .unwrap();
        assert!(status.success());
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
    fn runtime_theme_spec_uses_bundled_absolute_paths_without_overriding_user_themes() {
        let root = private_root("runtime-theme-reference");
        let bundled = root.join("prefix/share/zentty/ghostty/themes");
        let user = root.join("xdg/ghostty/themes");
        fs::create_dir_all(&bundled).unwrap();
        fs::create_dir_all(&user).unwrap();
        fs::write(bundled.join("Abernathy"), "background = #111416\n").unwrap();
        fs::write(bundled.join("User Wins"), "background = #000000\n").unwrap();
        fs::write(user.join("User Wins"), "background = #ffffff\n").unwrap();

        let bundled_only = ThemeSpec::new(ThemeMode::Dark, Some("Abernathy"), None);
        let resolved = ghostty_theme_spec_for_runtime(&bundled_only, &bundled, &user).unwrap();
        assert_eq!(
            resolved.to_string(),
            fs::canonicalize(bundled.join("Abernathy"))
                .unwrap()
                .to_str()
                .unwrap()
        );

        let automatic = ThemeSpec::new(ThemeMode::Automatic, Some("Abernathy"), Some("User Wins"));
        let resolved = ghostty_theme_spec_for_runtime(&automatic, &bundled, &user).unwrap();
        assert!(resolved.to_string().starts_with("dark:/"));
        assert!(resolved.to_string().ends_with(",light:User Wins"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn runtime_theme_spec_does_not_resolve_unsafe_or_unavailable_names() {
        let root = private_root("runtime-theme-unavailable");
        let bundled = root.join("bundled");
        let user = root.join("user");
        fs::create_dir_all(&bundled).unwrap();
        fs::create_dir_all(&user).unwrap();
        fs::create_dir_all(bundled.join("nested")).unwrap();
        fs::write(bundled.join("nested/theme"), "background = #111416\n").unwrap();

        for name in [
            "Missing",
            "nested/theme",
            "..",
            "../escape",
            "/operator/theme",
        ] {
            let spec = ThemeSpec::new(ThemeMode::Dark, Some(name), None);
            assert_eq!(
                ghostty_theme_spec_for_runtime(&spec, &bundled, &user)
                    .unwrap()
                    .to_string(),
                name
            );
        }

        fs::create_dir(bundled.join("Not A File")).unwrap();
        let invalid = ThemeSpec::new(ThemeMode::Dark, Some("Not A File"), None);
        assert!(
            ghostty_theme_spec_for_runtime(&invalid, &bundled, &user)
                .unwrap_err()
                .contains("not a regular file")
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn runtime_theme_spec_reports_bundled_lookup_errors_other_than_absence() {
        use std::os::unix::fs::symlink;

        let root = private_root("runtime-theme-lookup-error");
        let bundled = root.join("bundled-loop");
        let user = root.join("user");
        fs::create_dir_all(&user).unwrap();
        symlink("bundled-loop", &bundled).unwrap();

        let spec = ThemeSpec::new(ThemeMode::Dark, Some("Abernathy"), None);
        let error = ghostty_theme_spec_for_runtime(&spec, &bundled, &user).unwrap_err();
        assert!(error.contains("could not inspect bundled theme"));

        fs::remove_dir_all(root).unwrap();
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
