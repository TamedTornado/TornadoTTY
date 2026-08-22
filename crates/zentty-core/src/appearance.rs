use std::fmt;

pub const FALLBACK_DARK_THEME: &str = "GitHub-Dark-Personal";
pub const FALLBACK_LIGHT_THEME: &str = "GitHub Light Default";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ThemeMode {
    Automatic,
    #[default]
    Dark,
    Light,
}

/// Source-defined strength of the active worklane's identity-color projection.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SidebarSelectionEmphasis {
    #[default]
    Subtle,
    Vivid,
}

impl SidebarSelectionEmphasis {
    #[must_use]
    pub const fn config_value(self) -> &'static str {
        match self {
            Self::Subtle => "subtle",
            Self::Vivid => "vivid",
        }
    }

    pub(crate) fn parse_config_value(value: &str) -> Option<Self> {
        match value {
            "subtle" => Some(Self::Subtle),
            "vivid" => Some(Self::Vivid),
            _ => None,
        }
    }
}

#[cfg(test)]
mod sidebar_selection_emphasis_tests {
    use super::SidebarSelectionEmphasis;

    #[test]
    fn persisted_values_round_trip_exactly() {
        for (emphasis, persisted) in [
            (SidebarSelectionEmphasis::Subtle, "subtle"),
            (SidebarSelectionEmphasis::Vivid, "vivid"),
        ] {
            assert_eq!(emphasis.config_value(), persisted);
            assert_eq!(
                SidebarSelectionEmphasis::parse_config_value(persisted),
                Some(emphasis)
            );
        }
    }

    #[test]
    fn unknown_persisted_value_is_rejected() {
        assert_eq!(
            SidebarSelectionEmphasis::parse_config_value("fluorescent"),
            None
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeModeCommand {
    Toggle,
    Dark,
    Light,
    Automatic,
}

impl ThemeModeCommand {
    #[must_use]
    pub const fn resolve(self, current: ThemeMode, desktop_is_dark: bool) -> ThemeMode {
        match self {
            Self::Dark => ThemeMode::Dark,
            Self::Light => ThemeMode::Light,
            Self::Automatic => ThemeMode::Automatic,
            Self::Toggle => {
                let effective_is_dark = match current {
                    ThemeMode::Dark => true,
                    ThemeMode::Light => false,
                    ThemeMode::Automatic => desktop_is_dark,
                };
                if effective_is_dark {
                    ThemeMode::Light
                } else {
                    ThemeMode::Dark
                }
            }
        }
    }
}

impl ThemeMode {
    #[must_use]
    pub fn parse_config_value(value: &str) -> Option<Self> {
        match value {
            "automatic" | "followMacOS" => Some(Self::Automatic),
            "dark" | "alwaysDark" => Some(Self::Dark),
            "light" | "alwaysLight" => Some(Self::Light),
            _ => None,
        }
    }

    #[must_use]
    pub const fn config_value(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    /// The public CLI deliberately follows the source application's shorter
    /// vocabulary rather than exposing the persisted TOML spelling.
    #[must_use]
    pub const fn cli_token(self) -> &'static str {
        match self {
            Self::Automatic => "auto",
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemeSpec {
    pub mode: ThemeMode,
    pub dark_theme_name: Option<String>,
    pub light_theme_name: Option<String>,
}

impl ThemeSpec {
    #[must_use]
    pub fn new(
        mode: ThemeMode,
        dark_theme_name: Option<&str>,
        light_theme_name: Option<&str>,
    ) -> Self {
        Self {
            mode,
            dark_theme_name: dark_theme_name.and_then(sanitize_theme_name),
            light_theme_name: light_theme_name.and_then(sanitize_theme_name),
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        let mut dark = None;
        let mut light = None;
        let mut unqualified = None;
        for component in trim_quotes(raw).split(',').map(str::trim) {
            if let Some(name) = component.strip_prefix("dark:") {
                dark = sanitize_theme_name(name);
            } else if let Some(name) = component.strip_prefix("light:") {
                light = sanitize_theme_name(name);
            } else if unqualified.is_none() {
                unqualified = sanitize_theme_name(component);
            }
        }
        if dark.is_some() || light.is_some() {
            let mode = match (dark.is_some(), light.is_some()) {
                (true, true) => ThemeMode::Automatic,
                (true, false) => ThemeMode::Dark,
                (false, true) => ThemeMode::Light,
                (false, false) => unreachable!(),
            };
            Some(Self {
                mode,
                dark_theme_name: dark,
                light_theme_name: light,
            })
        } else {
            unqualified.map(|name| Self {
                mode: ThemeMode::Dark,
                dark_theme_name: Some(name),
                light_theme_name: None,
            })
        }
    }

    #[must_use]
    pub fn resolved_dark_theme_name(&self) -> &str {
        self.dark_theme_name
            .as_deref()
            .unwrap_or(FALLBACK_DARK_THEME)
    }

    #[must_use]
    pub fn resolved_light_theme_name(&self) -> &str {
        self.light_theme_name
            .as_deref()
            .unwrap_or(FALLBACK_LIGHT_THEME)
    }

    #[must_use]
    pub fn active_theme_name(&self, desktop_is_dark: bool) -> &str {
        match self.mode {
            ThemeMode::Automatic => {
                if desktop_is_dark {
                    self.resolved_dark_theme_name()
                } else {
                    self.resolved_light_theme_name()
                }
            }
            ThemeMode::Dark => self.resolved_dark_theme_name(),
            ThemeMode::Light => self.resolved_light_theme_name(),
        }
    }
}

impl fmt::Display for ThemeSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.mode {
            ThemeMode::Automatic => write!(
                formatter,
                "dark:{},light:{}",
                self.resolved_dark_theme_name(),
                self.resolved_light_theme_name()
            ),
            ThemeMode::Dark => formatter.write_str(self.resolved_dark_theme_name()),
            ThemeMode::Light => formatter.write_str(self.resolved_light_theme_name()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackgroundOpacity(u8);

impl BackgroundOpacity {
    #[must_use]
    pub fn from_fraction(value: f64) -> Option<Self> {
        if !value.is_finite() {
            return None;
        }
        let percent = format!("{:.0}", value.clamp(0.0, 1.0) * 100.0)
            .parse::<u8>()
            .ok()?;
        Some(Self(percent))
    }

    #[must_use]
    pub const fn percent(self) -> u8 {
        self.0
    }
}

impl fmt::Display for BackgroundOpacity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:.2}", f64::from(self.0) / 100.0)
    }
}

#[must_use]
pub fn update_ghostty_value(content: Option<&str>, key: &str, value: &str) -> Option<String> {
    if !matches!(key, "theme" | "background-opacity") || value.contains(['\r', '\n']) {
        return None;
    }
    let new_line = format!("{key} = {value}");
    let mut lines = content.unwrap_or_default().split('\n').collect::<Vec<_>>();
    let matching = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            active_key(line)
                .filter(|candidate| *candidate == key)
                .map(|_| index)
        })
        .collect::<Vec<_>>();
    if matching.is_empty() {
        lines.insert(0, &new_line);
    } else {
        // Ghostty uses the final active assignment. Replace it and remove earlier
        // duplicates so an external edit cannot silently defeat the UI value.
        let last = *matching.last()?;
        lines[last] = &new_line;
        for index in matching.into_iter().rev().skip(1) {
            lines.remove(index);
        }
    }
    while lines.last() == Some(&"") {
        lines.pop();
    }
    Some(format!("{}\n", lines.join("\n")))
}

fn active_key(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
        return None;
    }
    trimmed.split_once('=').map(|(key, _)| key.trim())
}

fn sanitize_theme_name(raw: &str) -> Option<String> {
    let name = raw
        .chars()
        .filter(|character| *character != '"' && *character != '\r' && *character != '\n')
        .collect::<String>();
    let name = trim_quotes(name.trim()).trim();
    (!name.is_empty()).then(|| name.to_owned())
}

fn trim_quotes(value: &str) -> &str {
    value.trim_matches(['"', '\''])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_spec_round_trips_both_independent_source_slots() {
        let spec = ThemeSpec::new(
            ThemeMode::Automatic,
            Some("Catppuccin Frappe"),
            Some("Catppuccin Latte"),
        );
        assert_eq!(
            spec.to_string(),
            "dark:Catppuccin Frappe,light:Catppuccin Latte"
        );
        assert_eq!(ThemeSpec::parse(&spec.to_string()), Some(spec));
    }

    #[test]
    fn source_single_and_qualified_forms_resolve_exact_modes_and_fallbacks() {
        let dark = ThemeSpec::parse("TokyoNight").unwrap();
        assert_eq!(dark.mode, ThemeMode::Dark);
        assert_eq!(dark.active_theme_name(false), "TokyoNight");
        let light = ThemeSpec::parse("light:GitHub Light Default").unwrap();
        assert_eq!(light.mode, ThemeMode::Light);
        assert_eq!(light.resolved_dark_theme_name(), FALLBACK_DARK_THEME);
        let automatic = ThemeSpec::parse("light:Light,dark:Dark").unwrap();
        assert_eq!(automatic.active_theme_name(true), "Dark");
        assert_eq!(automatic.active_theme_name(false), "Light");
    }

    #[test]
    fn source_mode_commands_preserve_toggle_semantics_in_automatic_mode() {
        assert_eq!(
            ThemeModeCommand::Toggle.resolve(ThemeMode::Dark, false),
            ThemeMode::Light
        );
        assert_eq!(
            ThemeModeCommand::Toggle.resolve(ThemeMode::Light, true),
            ThemeMode::Dark
        );
        assert_eq!(
            ThemeModeCommand::Toggle.resolve(ThemeMode::Automatic, true),
            ThemeMode::Light
        );
        assert_eq!(
            ThemeModeCommand::Toggle.resolve(ThemeMode::Automatic, false),
            ThemeMode::Dark
        );
        assert_eq!(
            ThemeModeCommand::Automatic.resolve(ThemeMode::Dark, true),
            ThemeMode::Automatic
        );
        assert_eq!(ThemeMode::Automatic.config_value(), "automatic");
        assert_eq!(ThemeMode::Automatic.cli_token(), "auto");
        assert_eq!(ThemeMode::Dark.config_value(), "dark");
        assert_eq!(ThemeMode::Light.config_value(), "light");
    }

    #[test]
    fn theme_names_cannot_inject_configuration_lines() {
        let spec = ThemeSpec::new(ThemeMode::Dark, Some("\"Safe\nconfig-file = bad"), None);
        assert_eq!(
            spec.dark_theme_name.as_deref(),
            Some("Safeconfig-file = bad")
        );
        assert_eq!(ThemeSpec::parse("'  '"), None);
    }

    #[test]
    fn opacity_rejects_nonfinite_clamps_and_uses_source_precision() {
        assert_eq!(BackgroundOpacity::from_fraction(f64::NAN), None);
        assert_eq!(
            BackgroundOpacity::from_fraction(-2.0).unwrap().to_string(),
            "0.00"
        );
        assert_eq!(
            BackgroundOpacity::from_fraction(0.876).unwrap().to_string(),
            "0.88"
        );
        assert_eq!(
            BackgroundOpacity::from_fraction(0.876).unwrap().percent(),
            88
        );
        assert_eq!(
            BackgroundOpacity::from_fraction(2.0).unwrap().to_string(),
            "1.00"
        );
    }

    #[test]
    fn config_update_preserves_comments_unknowns_and_reconciles_duplicates() {
        let source =
            "# theme = Old\nfont-size = 13\ntheme = First\n// theme = Other\ntheme = Last\n";
        assert_eq!(
            update_ghostty_value(Some(source), "theme", "New").as_deref(),
            Some("# theme = Old\nfont-size = 13\n// theme = Other\ntheme = New\n")
        );
    }

    #[test]
    fn config_update_inserts_new_owned_key_and_rejects_injection_or_unknown_keys() {
        assert_eq!(
            update_ghostty_value(Some("font-size = 13\n"), "background-opacity", "0.75").as_deref(),
            Some("background-opacity = 0.75\nfont-size = 13\n")
        );
        assert_eq!(update_ghostty_value(None, "font-size", "14"), None);
        assert_eq!(
            update_ghostty_value(None, "theme", "Safe\nfont-size = 1"),
            None
        );
    }

    #[test]
    fn active_key_rejects_each_comment_and_empty_line_shape() {
        assert_eq!(active_key(""), None);
        assert_eq!(active_key("   "), None);
        assert_eq!(active_key("# theme = Ignored"), None);
        assert_eq!(active_key("// theme = Ignored"), None);
        assert_eq!(active_key(" theme = Active "), Some("theme"));
    }
}
