use serde::Deserialize;

use crate::shortcut::ShortcutDocument;
use crate::{
    BackgroundOpacity, CleanCopyOptions, CommandFlattenAggressiveness, LINUX_OPEN_WITH_BUILTIN_IDS,
    ShortcutBinding, ThemeMode, ThemeSpec,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClipboardConfig {
    pub always_clean_copies: bool,
    pub clean_options: CleanCopyOptions,
    pub show_copy_markdown_command: bool,
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            always_clean_copies: false,
            clean_options: CleanCopyOptions::default(),
            show_copy_markdown_command: true,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AppConfig {
    pub appearance: AppearanceConfig,
    pub clipboard: ClipboardConfig,
    pub open_with: OpenWithConfig,
    pub server_detection: ServerDetectionConfig,
    pub panes: PaneConfig,
    pub shortcuts: Vec<ShortcutBinding>,
}

impl AppConfig {
    /// Parses the source-compatible Zentty TOML subset owned by this build.
    ///
    /// # Errors
    ///
    /// Returns an error when TOML syntax or any known setting value is invalid.
    pub fn parse_toml(source: &str) -> Result<Self, String> {
        let document = toml::from_str::<Document>(source)
            .map_err(|error| format!("invalid Zentty configuration: {error}"))?;
        Ok(Self {
            appearance: document.appearance.into_config()?,
            clipboard: document.clipboard.into_config(),
            open_with: document.open_with.into_config().normalized(),
            server_detection: document.server_detection.into_config().normalized(),
            panes: document.panes.into_config(),
            shortcuts: document.shortcuts.into_bindings()?,
        })
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Document {
    appearance: AppearanceDocument,
    clipboard: ClipboardDocument,
    open_with: OpenWithDocument,
    server_detection: ServerDetectionDocument,
    panes: PaneDocument,
    shortcuts: ShortcutDocument,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppearanceConfig {
    pub theme_mode: ThemeMode,
    pub preferred_dark_theme_name: Option<String>,
    pub preferred_light_theme_name: Option<String>,
    pub background_opacity: Option<BackgroundOpacity>,
    pub sync_opencode_theme_with_terminal: bool,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::Dark,
            preferred_dark_theme_name: None,
            preferred_light_theme_name: None,
            background_opacity: None,
            sync_opencode_theme_with_terminal: true,
        }
    }
}

impl AppearanceConfig {
    #[must_use]
    pub fn theme_spec(&self) -> ThemeSpec {
        ThemeSpec::new(
            self.theme_mode,
            self.preferred_dark_theme_name.as_deref(),
            self.preferred_light_theme_name.as_deref(),
        )
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct AppearanceDocument {
    theme_mode: Option<String>,
    preferred_dark_theme_name: Option<String>,
    preferred_light_theme_name: Option<String>,
    #[serde(alias = "background_opacity")]
    local_background_opacity: Option<f64>,
    sync_opencode_theme_with_terminal: Option<bool>,
}

impl AppearanceDocument {
    fn into_config(self) -> Result<AppearanceConfig, String> {
        let defaults = AppearanceConfig::default();
        let theme_mode = self.theme_mode.map_or(Ok(defaults.theme_mode), |value| {
            ThemeMode::parse_config_value(&value)
                .ok_or_else(|| format!("invalid appearance.theme_mode: {value}"))
        })?;
        let background_opacity = match self.local_background_opacity {
            Some(value) => Some(
                BackgroundOpacity::from_fraction(value)
                    .ok_or_else(|| "appearance.background_opacity must be finite".to_owned())?,
            ),
            None => None,
        };
        Ok(AppearanceConfig {
            theme_mode,
            preferred_dark_theme_name: self.preferred_dark_theme_name,
            preferred_light_theme_name: self.preferred_light_theme_name,
            background_opacity,
            sync_opencode_theme_with_terminal: self
                .sync_opencode_theme_with_terminal
                .unwrap_or(defaults.sync_opencode_theme_with_terminal),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PaneConfig {
    pub show_project_icons: bool,
}

impl Default for PaneConfig {
    fn default() -> Self {
        Self {
            show_project_icons: true,
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PaneDocument {
    show_project_icons: Option<bool>,
}

impl PaneDocument {
    fn into_config(self) -> PaneConfig {
        PaneConfig {
            show_project_icons: self.show_project_icons.unwrap_or(true),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenWithCustomApp {
    pub id: String,
    pub name: String,
    pub path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenWithConfig {
    pub primary_target_id: String,
    pub enabled_target_ids: Vec<String>,
    pub custom_apps: Vec<OpenWithCustomApp>,
}

impl Default for OpenWithConfig {
    fn default() -> Self {
        Self {
            primary_target_id: "system-file-manager".into(),
            enabled_target_ids: vec![
                "system-file-manager".into(),
                "vscode".into(),
                "cursor".into(),
                "system-terminal".into(),
            ],
            custom_apps: Vec::new(),
        }
    }
}

impl OpenWithConfig {
    #[must_use]
    pub fn normalized(self) -> Self {
        use std::collections::{HashMap, HashSet};

        let built_in_ids = LINUX_OPEN_WITH_BUILTIN_IDS
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let mut custom_apps = Vec::new();
        let mut custom_ids = HashSet::new();
        let mut duplicate_ids = HashMap::new();
        for app in self.custom_apps {
            if app.id.is_empty() || app.name.is_empty() || app.path.is_empty() {
                continue;
            }
            if let Some(existing) = custom_apps
                .iter()
                .find(|existing: &&OpenWithCustomApp| existing.path == app.path)
            {
                duplicate_ids.insert(app.id, existing.id.clone());
                continue;
            }
            if built_in_ids.contains(app.id.as_str()) || !custom_ids.insert(app.id.clone()) {
                continue;
            }
            custom_apps.push(app);
        }

        let valid_custom_ids = custom_apps
            .iter()
            .map(|app| app.id.as_str())
            .collect::<HashSet<_>>();
        let mut seen_enabled = HashSet::new();
        let enabled_target_ids = self
            .enabled_target_ids
            .into_iter()
            .map(|id| duplicate_ids.get(&id).cloned().unwrap_or(id))
            .filter(|id| {
                (built_in_ids.contains(id.as_str()) || valid_custom_ids.contains(id.as_str()))
                    && seen_enabled.insert(id.clone())
            })
            .collect::<Vec<_>>();
        let requested_primary = duplicate_ids
            .get(&self.primary_target_id)
            .cloned()
            .unwrap_or(self.primary_target_id);
        let primary_target_id = if built_in_ids.contains(requested_primary.as_str())
            || valid_custom_ids.contains(requested_primary.as_str())
        {
            requested_primary
        } else {
            enabled_target_ids
                .first()
                .cloned()
                .unwrap_or_else(|| Self::default().primary_target_id)
        };

        Self {
            primary_target_id,
            enabled_target_ids,
            custom_apps,
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct OpenWithDocument {
    primary_target_id: Option<String>,
    enabled_target_ids: Option<Vec<String>>,
    custom_apps: Option<Vec<OpenWithCustomAppDocument>>,
}

#[derive(Deserialize)]
struct OpenWithCustomAppDocument {
    id: String,
    name: String,
    path: String,
}

impl OpenWithDocument {
    fn into_config(self) -> OpenWithConfig {
        let defaults = OpenWithConfig::default();
        OpenWithConfig {
            primary_target_id: self.primary_target_id.unwrap_or(defaults.primary_target_id),
            enabled_target_ids: self
                .enabled_target_ids
                .unwrap_or(defaults.enabled_target_ids),
            custom_apps: self
                .custom_apps
                .unwrap_or_default()
                .into_iter()
                .map(|app| OpenWithCustomApp {
                    id: app.id,
                    name: app.name,
                    path: app.path,
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerDetectionConfig {
    pub passive_detection_enabled: bool,
    pub preferred_browser_id: String,
    pub enabled_browser_target_ids: Vec<String>,
    pub custom_browsers: Vec<ServerBrowserCustomApp>,
    pub ignored_port_rules: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerBrowserCustomApp {
    pub id: String,
    pub name: String,
    pub path: String,
    pub bundle_identifier: Option<String>,
}

impl Default for ServerDetectionConfig {
    fn default() -> Self {
        Self {
            passive_detection_enabled: true,
            preferred_browser_id: "system-default".into(),
            enabled_browser_target_ids: Vec::new(),
            custom_browsers: Vec::new(),
            ignored_port_rules: Vec::new(),
        }
    }
}

impl ServerDetectionConfig {
    #[must_use]
    pub fn normalized(self) -> Self {
        use std::collections::{HashMap, HashSet};

        let mut custom_browsers = Vec::new();
        let mut custom_ids = HashSet::new();
        let mut duplicate_ids = HashMap::new();
        for browser in self.custom_browsers {
            if browser.id.is_empty()
                || browser.id == "system-default"
                || browser.name.is_empty()
                || browser.path.is_empty()
            {
                continue;
            }
            if let Some(existing) = custom_browsers
                .iter()
                .find(|existing: &&ServerBrowserCustomApp| existing.path == browser.path)
            {
                duplicate_ids.insert(browser.id, existing.id.clone());
                continue;
            }
            if !custom_ids.insert(browser.id.clone()) {
                continue;
            }
            custom_browsers.push(browser);
        }

        let mut seen_enabled = HashSet::new();
        let enabled_browser_target_ids = self
            .enabled_browser_target_ids
            .into_iter()
            .map(|id| duplicate_ids.get(&id).cloned().unwrap_or(id))
            .filter(|id| !id.is_empty() && seen_enabled.insert(id.clone()))
            .collect::<Vec<_>>();
        let requested_preferred = duplicate_ids
            .get(&self.preferred_browser_id)
            .cloned()
            .unwrap_or(self.preferred_browser_id);
        let valid_custom_ids = custom_browsers
            .iter()
            .map(|browser| browser.id.as_str())
            .collect::<HashSet<_>>();
        let preferred_browser_id = if requested_preferred.is_empty()
            || (requested_preferred.starts_with("custom:")
                && !valid_custom_ids.contains(requested_preferred.as_str()))
        {
            "system-default".into()
        } else {
            requested_preferred
        };

        Self {
            passive_detection_enabled: self.passive_detection_enabled,
            preferred_browser_id,
            enabled_browser_target_ids,
            custom_browsers,
            ignored_port_rules: self.ignored_port_rules,
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ServerDetectionDocument {
    passive_detection_enabled: Option<bool>,
    preferred_browser_id: Option<String>,
    enabled_browser_target_ids: Option<Vec<String>>,
    custom_browsers: Option<Vec<ServerBrowserCustomAppDocument>>,
    ignored_port_rules: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct ServerBrowserCustomAppDocument {
    id: String,
    name: String,
    path: String,
    bundle_identifier: Option<String>,
}

impl ServerDetectionDocument {
    fn into_config(self) -> ServerDetectionConfig {
        let defaults = ServerDetectionConfig::default();
        ServerDetectionConfig {
            passive_detection_enabled: self
                .passive_detection_enabled
                .unwrap_or(defaults.passive_detection_enabled),
            preferred_browser_id: self
                .preferred_browser_id
                .unwrap_or(defaults.preferred_browser_id),
            enabled_browser_target_ids: self
                .enabled_browser_target_ids
                .unwrap_or(defaults.enabled_browser_target_ids),
            custom_browsers: self
                .custom_browsers
                .unwrap_or_default()
                .into_iter()
                .filter(|browser| {
                    !browser.id.is_empty() && !browser.name.is_empty() && !browser.path.is_empty()
                })
                .map(|browser| ServerBrowserCustomApp {
                    id: browser.id,
                    name: browser.name,
                    path: browser.path,
                    bundle_identifier: browser.bundle_identifier,
                })
                .collect(),
            ignored_port_rules: self
                .ignored_port_rules
                .unwrap_or(defaults.ignored_port_rules),
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ClipboardDocument {
    always_clean_copies: Option<bool>,
    flatten_multi_line_commands: Option<bool>,
    command_flatten_aggressiveness: Option<Aggressiveness>,
    preserve_blank_lines_when_flattening: Option<bool>,
    remove_box_drawing: Option<bool>,
    flatten_slash_command_selections: Option<bool>,
    strip_url_tracking_parameters: Option<bool>,
    quote_paths_with_spaces: Option<bool>,
    show_copy_markdown_command: Option<bool>,
}

impl ClipboardDocument {
    fn into_config(self) -> ClipboardConfig {
        let defaults = ClipboardConfig::default();
        let options = defaults.clean_options;
        ClipboardConfig {
            always_clean_copies: self
                .always_clean_copies
                .unwrap_or(defaults.always_clean_copies),
            clean_options: CleanCopyOptions {
                flatten_multi_line_commands: self
                    .flatten_multi_line_commands
                    .unwrap_or(options.flatten_multi_line_commands),
                command_flatten_aggressiveness: self
                    .command_flatten_aggressiveness
                    .map_or(options.command_flatten_aggressiveness, Into::into),
                preserve_blank_lines_when_flattening: self
                    .preserve_blank_lines_when_flattening
                    .unwrap_or(options.preserve_blank_lines_when_flattening),
                remove_box_drawing: self
                    .remove_box_drawing
                    .unwrap_or(options.remove_box_drawing),
                flatten_slash_command_selections: self
                    .flatten_slash_command_selections
                    .unwrap_or(options.flatten_slash_command_selections),
                strip_url_tracking_parameters: self
                    .strip_url_tracking_parameters
                    .unwrap_or(options.strip_url_tracking_parameters),
                quote_paths_with_spaces: self
                    .quote_paths_with_spaces
                    .unwrap_or(options.quote_paths_with_spaces),
            },
            show_copy_markdown_command: self
                .show_copy_markdown_command
                .unwrap_or(defaults.show_copy_markdown_command),
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Aggressiveness {
    Low,
    Normal,
    High,
}

impl From<Aggressiveness> for CommandFlattenAggressiveness {
    fn from(value: Aggressiveness) -> Self {
        match value {
            Aggressiveness::Low => Self::Low,
            Aggressiveness::Normal => Self::Normal,
            Aggressiveness::High => Self::High,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn source_shortcut_toml_parses_bindings_and_explicit_unbinds() {
        let config = AppConfig::parse_toml(
            r#"
                [[shortcuts.bindings]]
                command_id = "sidebar.toggle"
                shortcut = "command+option+s"

                [[shortcuts.bindings]]
                command_id = "pane.close_focused"
                shortcut = ""
            "#,
        )
        .unwrap();
        assert_eq!(config.shortcuts.len(), 2);
        assert_eq!(
            config.shortcuts[0]
                .shortcut
                .as_ref()
                .unwrap()
                .storage_string(),
            "command+option+s"
        );
        assert!(config.shortcuts[1].shortcut.is_none());
    }

    #[test]
    fn malformed_shortcut_toml_is_not_silently_defaulted() {
        let error = AppConfig::parse_toml(
            r#"
                [[shortcuts.bindings]]
                command_id = "sidebar.toggle"
                shortcut = "hyper+s"
            "#,
        )
        .unwrap_err();
        assert!(error.contains("invalid shortcut for sidebar.toggle"));
    }
}
