use serde::Deserialize;

use crate::{CleanCopyOptions, CommandFlattenAggressiveness};

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
    pub clipboard: ClipboardConfig,
    pub server_detection: ServerDetectionConfig,
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
            clipboard: document.clipboard.into_config(),
            server_detection: document.server_detection.into_config(),
        })
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Document {
    clipboard: ClipboardDocument,
    server_detection: ServerDetectionDocument,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerDetectionConfig {
    pub passive_detection_enabled: bool,
    pub preferred_browser_id: String,
    pub enabled_browser_target_ids: Vec<String>,
    pub ignored_port_rules: Vec<String>,
}

impl Default for ServerDetectionConfig {
    fn default() -> Self {
        Self {
            passive_detection_enabled: true,
            preferred_browser_id: "system-default".into(),
            enabled_browser_target_ids: Vec::new(),
            ignored_port_rules: Vec::new(),
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ServerDetectionDocument {
    passive_detection_enabled: Option<bool>,
    preferred_browser_id: Option<String>,
    enabled_browser_target_ids: Option<Vec<String>>,
    ignored_port_rules: Option<Vec<String>>,
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
