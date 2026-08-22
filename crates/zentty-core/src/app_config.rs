use std::collections::BTreeMap;

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::shortcut::ShortcutDocument;
use crate::{
    AgentIntegrationState, BackgroundOpacity, CleanCopyOptions, CommandFlattenAggressiveness,
    LINUX_OPEN_WITH_BUILTIN_IDS, ShortcutBinding, SidebarSelectionEmphasis, SidebarWidthPreference,
    ThemeMode, ThemeSpec,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfirmationsConfig {
    pub confirm_before_closing_pane: bool,
    pub confirm_before_closing_window: bool,
    pub confirm_before_quitting: bool,
}

impl Default for ConfirmationsConfig {
    fn default() -> Self {
        Self {
            confirm_before_closing_pane: true,
            confirm_before_closing_window: true,
            confirm_before_quitting: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RestoreConfig {
    pub restore_workspace_on_launch: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NotificationsConfig {
    /// Empty selects the desktop environment's default notification sound.
    pub sound_name: String,
    /// Source-compatible metadata for a user-installed custom sound.
    pub custom_sound_display_name: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateChannel {
    Stable,
    Beta,
}

impl UpdateChannel {
    #[must_use]
    pub const fn config_value(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Beta => "beta",
        }
    }

    fn parse_config_value(value: &str) -> Option<Self> {
        match value {
            "stable" => Some(Self::Stable),
            "beta" => Some(Self::Beta),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UpdatesConfig {
    pub channel: UpdateChannel,
}

impl Default for UpdatesConfig {
    fn default() -> Self {
        Self {
            channel: UpdateChannel::Stable,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ErrorReportingConfig {
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SidebarVisibilityMode {
    #[default]
    PinnedOpen,
    Hidden,
}

impl SidebarVisibilityMode {
    #[must_use]
    pub const fn config_value(self) -> &'static str {
        match self {
            Self::PinnedOpen => "pinnedOpen",
            Self::Hidden => "hidden",
        }
    }

    fn parse_config_value(value: &str) -> Option<Self> {
        match value {
            "pinnedOpen" => Some(Self::PinnedOpen),
            "hidden" => Some(Self::Hidden),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SidebarConfig {
    pub width: i32,
    pub visibility: SidebarVisibilityMode,
}

impl Default for SidebarConfig {
    fn default() -> Self {
        Self {
            width: SidebarWidthPreference::DEFAULT,
            visibility: SidebarVisibilityMode::PinnedOpen,
        }
    }
}

impl Default for RestoreConfig {
    fn default() -> Self {
        Self {
            restore_workspace_on_launch: true,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AppConfig {
    pub sidebar: SidebarConfig,
    pub appearance: AppearanceConfig,
    pub confirmations: ConfirmationsConfig,
    pub restore: RestoreConfig,
    pub clipboard: ClipboardConfig,
    pub notifications: NotificationsConfig,
    pub updates: UpdatesConfig,
    pub error_reporting: ErrorReportingConfig,
    pub open_with: OpenWithConfig,
    pub server_detection: ServerDetectionConfig,
    pub worklanes: WorklaneConfig,
    pub pane_layout: PaneLayoutConfig,
    pub panes: PaneConfig,
    pub agent_teams: AgentTeamsConfig,
    pub agent_caffeination: AgentCaffeinationConfig,
    pub menu_bar: MenuBarConfig,
    pub agent_integrations: AgentIntegrationsConfig,
    pub shortcuts: Vec<ShortcutBinding>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PartialAppConfig {
    pub config: AppConfig,
    pub retained_sections: Vec<&'static str>,
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
            sidebar: document.sidebar.into_config()?,
            appearance: document.appearance.into_config()?,
            confirmations: document.confirmations.into_config(),
            restore: document.restore.into_config(),
            clipboard: document.clipboard.into_config(),
            notifications: document.notifications.into_config(),
            updates: document.updates.into_config()?,
            error_reporting: document.error_reporting.into_config(),
            open_with: document.open_with.into_config().normalized(),
            server_detection: document.server_detection.into_config().normalized(),
            worklanes: document.worklanes.into_config()?,
            pane_layout: document.pane_layout.into_config()?,
            panes: document.panes.into_config()?,
            agent_teams: document.agent_teams.into_config(),
            agent_caffeination: document.agent_caffeination.into_config(),
            menu_bar: document.menu_bar.into_config(),
            agent_integrations: document.agent_integrations.into_config(),
            shortcuts: document.shortcuts.into_bindings()?,
        })
    }

    /// Parses independently valid top-level sections for a live reload.
    ///
    /// TOML syntax remains an all-or-nothing boundary. After syntax succeeds,
    /// an invalid known section retains only its value from `last_good`; absent
    /// sections are valid and therefore use source defaults.
    ///
    /// # Errors
    ///
    /// Returns a content-safe error when the document is not valid TOML.
    pub fn parse_toml_partial(source: &str, last_good: &Self) -> Result<PartialAppConfig, String> {
        let table = toml::from_str::<toml::Table>(source)
            .map_err(|_| "invalid Zentty configuration syntax".to_owned())?;
        let mut retained_sections = Vec::new();

        macro_rules! section {
            ($field:ident, $document:ty, $convert:expr) => {{
                let name = stringify!($field);
                match parse_partial_section::<$document, _>(&table, name, $convert) {
                    Ok(value) => value,
                    Err(()) => {
                        retained_sections.push(name);
                        last_good.$field.clone()
                    }
                }
            }};
        }

        let config = Self {
            sidebar: section!(sidebar, SidebarDocument, SidebarDocument::into_config),
            appearance: section!(
                appearance,
                AppearanceDocument,
                AppearanceDocument::into_config
            ),
            confirmations: section!(
                confirmations,
                ConfirmationsDocument,
                |document: ConfirmationsDocument| Ok(document.into_config())
            ),
            restore: section!(restore, RestoreDocument, |document: RestoreDocument| Ok(
                document.into_config()
            )),
            clipboard: section!(
                clipboard,
                ClipboardDocument,
                |document: ClipboardDocument| Ok(document.into_config())
            ),
            notifications: section!(
                notifications,
                NotificationsDocument,
                |document: NotificationsDocument| Ok(document.into_config())
            ),
            updates: section!(updates, UpdatesDocument, UpdatesDocument::into_config),
            error_reporting: section!(
                error_reporting,
                ErrorReportingDocument,
                |document: ErrorReportingDocument| Ok(document.into_config())
            ),
            open_with: section!(
                open_with,
                OpenWithDocument,
                |document: OpenWithDocument| Ok(document.into_config().normalized())
            ),
            server_detection: section!(
                server_detection,
                ServerDetectionDocument,
                |document: ServerDetectionDocument| Ok(document.into_config().normalized())
            ),
            worklanes: section!(worklanes, WorklaneDocument, WorklaneDocument::into_config),
            pane_layout: section!(
                pane_layout,
                PaneLayoutDocument,
                PaneLayoutDocument::into_config
            ),
            panes: section!(panes, PaneDocument, PaneDocument::into_config),
            agent_teams: section!(
                agent_teams,
                AgentTeamsDocument,
                |document: AgentTeamsDocument| Ok(document.into_config())
            ),
            agent_caffeination: section!(
                agent_caffeination,
                AgentCaffeinationDocument,
                |document: AgentCaffeinationDocument| Ok(document.into_config())
            ),
            menu_bar: section!(menu_bar, MenuBarDocument, |document: MenuBarDocument| Ok(
                document.into_config()
            )),
            agent_integrations: section!(
                agent_integrations,
                AgentIntegrationsDocument,
                |document: AgentIntegrationsDocument| Ok(document.into_config())
            ),
            shortcuts: section!(shortcuts, ShortcutDocument, ShortcutDocument::into_bindings),
        };
        Ok(PartialAppConfig {
            config,
            retained_sections,
        })
    }
}

fn parse_partial_section<T, U>(
    table: &toml::Table,
    name: &str,
    convert: impl FnOnce(T) -> Result<U, String>,
) -> Result<U, ()>
where
    T: Default + DeserializeOwned,
    U: Default,
{
    let document = match table.get(name) {
        Some(value) => value.clone().try_into::<T>().map_err(|_| ())?,
        None => T::default(),
    };
    convert(document).map_err(|_| ())
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Document {
    sidebar: SidebarDocument,
    appearance: AppearanceDocument,
    confirmations: ConfirmationsDocument,
    restore: RestoreDocument,
    clipboard: ClipboardDocument,
    notifications: NotificationsDocument,
    updates: UpdatesDocument,
    error_reporting: ErrorReportingDocument,
    open_with: OpenWithDocument,
    server_detection: ServerDetectionDocument,
    worklanes: WorklaneDocument,
    pane_layout: PaneLayoutDocument,
    panes: PaneDocument,
    agent_teams: AgentTeamsDocument,
    agent_caffeination: AgentCaffeinationDocument,
    menu_bar: MenuBarDocument,
    agent_integrations: AgentIntegrationsDocument,
    shortcuts: ShortcutDocument,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SidebarDocument {
    width: Option<i32>,
    visibility: Option<String>,
}

impl SidebarDocument {
    fn into_config(self) -> Result<SidebarConfig, String> {
        let defaults = SidebarConfig::default();
        let width = self.width.unwrap_or(defaults.width);
        if !(SidebarWidthPreference::MINIMUM..=SidebarWidthPreference::MAXIMUM).contains(&width) {
            return Err(format!("invalid sidebar.width: {width}"));
        }
        let visibility = self.visibility.map_or(Ok(defaults.visibility), |value| {
            SidebarVisibilityMode::parse_config_value(&value)
                .ok_or_else(|| format!("invalid sidebar.visibility: {value}"))
        })?;
        Ok(SidebarConfig { width, visibility })
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct NotificationsDocument {
    sound_name: String,
    custom_sound_display_name: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct UpdatesDocument {
    channel: Option<String>,
}

impl UpdatesDocument {
    fn into_config(self) -> Result<UpdatesConfig, String> {
        let channel = self.channel.map_or(Ok(UpdateChannel::Stable), |value| {
            UpdateChannel::parse_config_value(&value)
                .ok_or_else(|| format!("invalid updates.channel: {value}"))
        })?;
        Ok(UpdatesConfig { channel })
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ErrorReportingDocument {
    enabled: Option<bool>,
}

impl ErrorReportingDocument {
    fn into_config(self) -> ErrorReportingConfig {
        ErrorReportingConfig {
            enabled: self.enabled.unwrap_or(false),
        }
    }
}

impl NotificationsDocument {
    fn into_config(self) -> NotificationsConfig {
        NotificationsConfig {
            sound_name: self.sound_name,
            custom_sound_display_name: self.custom_sound_display_name,
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ConfirmationsDocument {
    #[serde(rename = "confirm_before_closing_pane")]
    closing_pane: Option<bool>,
    #[serde(rename = "confirm_before_closing_window")]
    closing_window: Option<bool>,
    #[serde(rename = "confirm_before_quitting")]
    quitting: Option<bool>,
}

impl ConfirmationsDocument {
    fn into_config(self) -> ConfirmationsConfig {
        let defaults = ConfirmationsConfig::default();
        ConfirmationsConfig {
            confirm_before_closing_pane: self
                .closing_pane
                .unwrap_or(defaults.confirm_before_closing_pane),
            confirm_before_closing_window: self
                .closing_window
                .unwrap_or(defaults.confirm_before_closing_window),
            confirm_before_quitting: self.quitting.unwrap_or(defaults.confirm_before_quitting),
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RestoreDocument {
    restore_workspace_on_launch: Option<bool>,
}

impl RestoreDocument {
    fn into_config(self) -> RestoreConfig {
        RestoreConfig {
            restore_workspace_on_launch: self
                .restore_workspace_on_launch
                .unwrap_or(RestoreConfig::default().restore_workspace_on_launch),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppearanceConfig {
    pub theme_mode: ThemeMode,
    pub preferred_dark_theme_name: Option<String>,
    pub preferred_light_theme_name: Option<String>,
    pub background_opacity: Option<BackgroundOpacity>,
    pub sync_opencode_theme_with_terminal: bool,
    pub sidebar_selection_emphasis: SidebarSelectionEmphasis,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::Dark,
            preferred_dark_theme_name: None,
            preferred_light_theme_name: None,
            background_opacity: None,
            sync_opencode_theme_with_terminal: true,
            sidebar_selection_emphasis: SidebarSelectionEmphasis::Subtle,
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
    sidebar_selection_emphasis: Option<String>,
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
            sidebar_selection_emphasis: self.sidebar_selection_emphasis.map_or(
                Ok(defaults.sidebar_selection_emphasis),
                |value| {
                    SidebarSelectionEmphasis::parse_config_value(&value).ok_or_else(|| {
                        format!("invalid appearance.sidebar_selection_emphasis: {value}")
                    })
                },
            )?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NewWorklanePlacement {
    Top,
    AfterCurrent,
    End,
}

impl NewWorklanePlacement {
    #[must_use]
    pub const fn config_value(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::AfterCurrent => "after_current",
            Self::End => "end",
        }
    }

    fn parse_config_value(value: &str) -> Option<Self> {
        match value {
            "top" => Some(Self::Top),
            "after_current" => Some(Self::AfterCurrent),
            "end" => Some(Self::End),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorklaneConfig {
    pub new_worklane_placement: NewWorklanePlacement,
}

impl Default for WorklaneConfig {
    fn default() -> Self {
        Self {
            new_worklane_placement: NewWorklanePlacement::AfterCurrent,
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct WorklaneDocument {
    new_worklane_placement: Option<String>,
}

impl WorklaneDocument {
    fn into_config(self) -> Result<WorklaneConfig, String> {
        let placement = self.new_worklane_placement.map_or(
            Ok(NewWorklanePlacement::AfterCurrent),
            |value| {
                NewWorklanePlacement::parse_config_value(&value)
                    .ok_or_else(|| format!("invalid worklanes.new_worklane_placement: {value}"))
            },
        )?;
        Ok(WorklaneConfig {
            new_worklane_placement: placement,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaneRightBehaviorMode {
    Adaptive,
    AlwaysSplit,
    AlwaysAdd,
}

impl PaneRightBehaviorMode {
    #[must_use]
    pub const fn config_value(self) -> &'static str {
        match self {
            Self::Adaptive => "adaptive",
            Self::AlwaysSplit => "alwaysSplit",
            Self::AlwaysAdd => "alwaysAdd",
        }
    }

    fn parse_config_value(value: &str) -> Option<Self> {
        match value {
            "adaptive" => Some(Self::Adaptive),
            "alwaysSplit" => Some(Self::AlwaysSplit),
            "alwaysAdd" => Some(Self::AlwaysAdd),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PaneLayoutConfig {
    pub right_split_behavior: PaneRightBehaviorMode,
    pub visible_split_window_width: u16,
}

impl Default for PaneLayoutConfig {
    fn default() -> Self {
        Self {
            right_split_behavior: PaneRightBehaviorMode::Adaptive,
            visible_split_window_width: 1920,
        }
    }
}

impl PaneLayoutConfig {
    #[must_use]
    pub fn right_insertion_behavior(
        self,
        viewport_width: i32,
    ) -> crate::PaneRightInsertionBehavior {
        match self.right_split_behavior {
            PaneRightBehaviorMode::AlwaysSplit => crate::PaneRightInsertionBehavior::VisibleSplit,
            PaneRightBehaviorMode::AlwaysAdd => crate::PaneRightInsertionBehavior::WorklaneAdd,
            PaneRightBehaviorMode::Adaptive => {
                if viewport_width >= i32::from(self.visible_split_window_width) {
                    crate::PaneRightInsertionBehavior::VisibleSplit
                } else {
                    crate::PaneRightInsertionBehavior::WorklaneAdd
                }
            }
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PaneLayoutDocument {
    right_split_behavior: Option<String>,
    visible_split_window_width: Option<u16>,
}

impl PaneLayoutDocument {
    fn into_config(self) -> Result<PaneLayoutConfig, String> {
        let defaults = PaneLayoutConfig::default();
        let right_split_behavior =
            self.right_split_behavior
                .map_or(Ok(defaults.right_split_behavior), |value| {
                    PaneRightBehaviorMode::parse_config_value(&value)
                        .ok_or_else(|| format!("invalid pane_layout.right_split_behavior: {value}"))
                })?;
        let visible_split_window_width = self
            .visible_split_window_width
            .unwrap_or(defaults.visible_split_window_width);
        if !matches!(visible_split_window_width, 1200 | 1440 | 1680 | 1920 | 2560) {
            return Err(format!(
                "invalid pane_layout.visible_split_window_width: {visible_split_window_width}"
            ));
        }
        Ok(PaneLayoutConfig {
            right_split_behavior,
            visible_split_window_width,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FocusFollowsMouseDelay {
    Immediate,
    Short,
}

impl FocusFollowsMouseDelay {
    #[must_use]
    pub const fn config_value(self) -> &'static str {
        match self {
            Self::Immediate => "immediate",
            Self::Short => "short",
        }
    }

    fn parse_config_value(value: &str) -> Option<Self> {
        match value {
            "immediate" => Some(Self::Immediate),
            "short" => Some(Self::Short),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // Mirrors the source's independent pane preference keys.
pub struct PaneConfig {
    pub show_labels: bool,
    pub show_borders: bool,
    pub inactive_opacity_percent: u8,
    pub show_project_icons: bool,
    pub smooth_scroll_enabled: bool,
    pub focus_follows_mouse: bool,
    pub focus_follows_mouse_delay: FocusFollowsMouseDelay,
}

impl Default for PaneConfig {
    fn default() -> Self {
        Self {
            show_labels: true,
            show_borders: true,
            inactive_opacity_percent: 70,
            show_project_icons: true,
            smooth_scroll_enabled: false,
            focus_follows_mouse: false,
            focus_follows_mouse_delay: FocusFollowsMouseDelay::Short,
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PaneDocument {
    show_labels: Option<bool>,
    show_borders: Option<bool>,
    inactive_opacity: Option<f64>,
    show_project_icons: Option<bool>,
    smooth_scroll_enabled: Option<bool>,
    focus_follows_mouse: Option<bool>,
    focus_follows_mouse_delay: Option<String>,
}

impl PaneDocument {
    fn into_config(self) -> Result<PaneConfig, String> {
        let defaults = PaneConfig::default();
        let opacity = self.inactive_opacity.unwrap_or(0.7);
        if !opacity.is_finite() || !(0.6..=1.0).contains(&opacity) {
            return Err(format!("invalid panes.inactive_opacity: {opacity}"));
        }
        let focus_follows_mouse_delay = self.focus_follows_mouse_delay.map_or(
            Ok(defaults.focus_follows_mouse_delay),
            |value| {
                FocusFollowsMouseDelay::parse_config_value(&value)
                    .ok_or_else(|| format!("invalid panes.focus_follows_mouse_delay: {value}"))
            },
        )?;
        Ok(PaneConfig {
            show_labels: self.show_labels.unwrap_or(defaults.show_labels),
            show_borders: self.show_borders.unwrap_or(defaults.show_borders),
            // The finite 0.6..=1.0 check above proves this rounded value is 60..=100.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            inactive_opacity_percent: (opacity * 100.0).round() as u8,
            show_project_icons: self
                .show_project_icons
                .unwrap_or(defaults.show_project_icons),
            smooth_scroll_enabled: self
                .smooth_scroll_enabled
                .unwrap_or(defaults.smooth_scroll_enabled),
            focus_follows_mouse: self
                .focus_follows_mouse
                .unwrap_or(defaults.focus_follows_mouse),
            focus_follows_mouse_delay,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AgentTeamsConfig {
    pub enabled: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct AgentTeamsDocument {
    enabled: Option<bool>,
}

impl AgentTeamsDocument {
    fn into_config(self) -> AgentTeamsConfig {
        AgentTeamsConfig {
            enabled: self.enabled.unwrap_or(false),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentCaffeinationConfig {
    pub enabled: bool,
}

impl Default for AgentCaffeinationConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct AgentCaffeinationDocument {
    enabled: Option<bool>,
}

impl AgentCaffeinationDocument {
    fn into_config(self) -> AgentCaffeinationConfig {
        AgentCaffeinationConfig {
            enabled: self.enabled.unwrap_or(true),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MenuBarConfig {
    pub show_status_item: bool,
}

impl Default for MenuBarConfig {
    fn default() -> Self {
        Self {
            show_status_item: true,
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct MenuBarDocument {
    show_status_item: Option<bool>,
}

impl MenuBarDocument {
    fn into_config(self) -> MenuBarConfig {
        MenuBarConfig {
            show_status_item: self.show_status_item.unwrap_or(true),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AgentIntegrationsConfig {
    pub states: BTreeMap<String, AgentIntegrationState>,
    pub grandfathered_v1: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct AgentIntegrationsDocument {
    grandfathered_v1: Option<bool>,
    states: BTreeMap<String, String>,
}

impl AgentIntegrationsDocument {
    fn into_config(self) -> AgentIntegrationsConfig {
        AgentIntegrationsConfig {
            states: self
                .states
                .into_iter()
                .filter_map(|(tool, value)| {
                    AgentIntegrationState::parse_config_value(&value).map(|state| (tool, state))
                })
                .collect(),
            grandfathered_v1: self.grandfathered_v1.unwrap_or(false),
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

    /// Reconciles persisted preferences with the targets the platform can
    /// actually resolve at presentation time. This mirrors Zentty's source
    /// settings behavior: stale custom applications are removed, unavailable
    /// targets cannot remain enabled, and primary selection falls back in the
    /// surviving enabled order.
    #[must_use]
    pub fn reconciled_available(self, available_target_ids: &[String]) -> Self {
        use std::collections::HashSet;

        let available = available_target_ids.iter().collect::<HashSet<_>>();
        let requested_primary = self.primary_target_id.clone();
        let custom_apps = self
            .custom_apps
            .into_iter()
            .filter(|app| available.contains(&app.id))
            .collect::<Vec<_>>();
        let enabled_target_ids = self
            .enabled_target_ids
            .into_iter()
            .filter(|id| available.contains(id))
            .collect::<Vec<_>>();
        let primary_target_id = if enabled_target_ids.contains(&requested_primary) {
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
        .normalized()
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

    /// Reconciles persisted browser preferences with the applications the
    /// platform can resolve when the Dev Servers page is presented.
    #[must_use]
    pub fn reconciled_available(self, available_target_ids: &[String]) -> Self {
        use std::collections::HashSet;

        let available = available_target_ids.iter().collect::<HashSet<_>>();
        let custom_browsers = self
            .custom_browsers
            .into_iter()
            .filter(|browser| available.contains(&browser.id))
            .collect::<Vec<_>>();
        let enabled_browser_target_ids = self
            .enabled_browser_target_ids
            .into_iter()
            .filter(|id| available.contains(id))
            .collect::<Vec<_>>();
        let preferred_browser_id = if available.contains(&self.preferred_browser_id)
            && (enabled_browser_target_ids.is_empty()
                || enabled_browser_target_ids.contains(&self.preferred_browser_id))
        {
            self.preferred_browser_id
        } else {
            "system-default".into()
        };

        Self {
            passive_detection_enabled: self.passive_detection_enabled,
            preferred_browser_id,
            enabled_browser_target_ids,
            custom_browsers,
            ignored_port_rules: self.ignored_port_rules,
        }
        .normalized()
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
    use super::{
        AppConfig, NewWorklanePlacement, PaneLayoutConfig, PaneRightBehaviorMode, SidebarConfig,
        SidebarVisibilityMode,
    };
    use crate::{PaneRightInsertionBehavior, SidebarWidthPreference};

    #[test]
    fn sidebar_defaults_and_source_values_parse_without_persisting_hover_peek() {
        assert_eq!(
            AppConfig::parse_toml("").unwrap().sidebar,
            SidebarConfig::default()
        );
        let config = AppConfig::parse_toml(
            r#"
                [sidebar]
                width = 420
                visibility = "hidden"
            "#,
        )
        .unwrap();
        assert_eq!(config.sidebar.width, SidebarWidthPreference::MAXIMUM);
        assert_eq!(config.sidebar.visibility, SidebarVisibilityMode::Hidden);
        assert_eq!(config.sidebar.visibility.config_value(), "hidden");
        let pinned =
            AppConfig::parse_toml("[sidebar]\nwidth = 180\nvisibility = \"pinnedOpen\"\n").unwrap();
        assert_eq!(pinned.sidebar.width, SidebarWidthPreference::MINIMUM);
        assert_eq!(pinned.sidebar.visibility, SidebarVisibilityMode::PinnedOpen);
        assert_eq!(pinned.sidebar.visibility.config_value(), "pinnedOpen");

        for invalid in [
            "[sidebar]\nwidth = 179\n",
            "[sidebar]\nwidth = 421\n",
            "[sidebar]\nvisibility = \"hoverPeek\"\n",
            "[sidebar]\nvisibility = \"floating\"\n",
        ] {
            assert!(
                AppConfig::parse_toml(invalid).is_err(),
                "accepted {invalid}"
            );
        }
    }

    #[test]
    fn invalid_sidebar_partial_reload_retains_only_last_good_sidebar() {
        let last_good = AppConfig::parse_toml(
            "[sidebar]\nwidth = 319\nvisibility = \"hidden\"\n[restore]\nrestore_workspace_on_launch = false\n",
        )
        .unwrap();
        let partial = AppConfig::parse_toml_partial(
            "[sidebar]\nwidth = 900\n[restore]\nrestore_workspace_on_launch = true\n",
            &last_good,
        )
        .unwrap();
        assert_eq!(partial.retained_sections, ["sidebar"]);
        assert_eq!(partial.config.sidebar, last_good.sidebar);
        assert!(partial.config.restore.restore_workspace_on_launch);
    }

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

    #[test]
    fn adaptive_right_insertion_changes_exactly_at_each_supported_threshold() {
        for threshold in [1200, 1440, 1680, 1920, 2560] {
            let config = PaneLayoutConfig {
                right_split_behavior: PaneRightBehaviorMode::Adaptive,
                visible_split_window_width: threshold,
            };
            assert_eq!(
                config.right_insertion_behavior(i32::from(threshold) - 1),
                PaneRightInsertionBehavior::WorklaneAdd
            );
            assert_eq!(
                config.right_insertion_behavior(i32::from(threshold)),
                PaneRightInsertionBehavior::VisibleSplit
            );
        }
    }

    #[test]
    fn explicit_right_insertion_modes_ignore_viewport_width() {
        for width in [0, 1199, 1920, i32::MAX] {
            assert_eq!(
                PaneLayoutConfig {
                    right_split_behavior: PaneRightBehaviorMode::AlwaysSplit,
                    visible_split_window_width: 1920,
                }
                .right_insertion_behavior(width),
                PaneRightInsertionBehavior::VisibleSplit
            );
            assert_eq!(
                PaneLayoutConfig {
                    right_split_behavior: PaneRightBehaviorMode::AlwaysAdd,
                    visible_split_window_width: 1920,
                }
                .right_insertion_behavior(width),
                PaneRightInsertionBehavior::WorklaneAdd
            );
        }
    }

    #[test]
    fn workspace_settings_parse_placement_and_round_opacity_without_clamping() {
        let config = AppConfig::parse_toml(
            r#"
                [worklanes]
                new_worklane_placement = "end"
                [panes]
                inactive_opacity = 0.816
            "#,
        )
        .unwrap();
        assert_eq!(
            config.worklanes.new_worklane_placement,
            NewWorklanePlacement::End
        );
        assert_eq!(config.panes.inactive_opacity_percent, 82);

        for invalid in ["0.599", "1.001", "nan"] {
            assert!(
                AppConfig::parse_toml(&format!("[panes]\ninactive_opacity = {invalid}\n")).is_err()
            );
        }
    }
}
