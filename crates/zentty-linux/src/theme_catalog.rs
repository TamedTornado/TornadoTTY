use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use zentty_linux::platform::{UserDirectory, resolve_user_directory};

const MAX_THEME_BYTES: u64 = 64 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ThemeColor {
    hex: String,
    red: u8,
    green: u8,
    blue: u8,
}

impl ThemeColor {
    fn parse(value: &str) -> Option<Self> {
        let value = value.trim().trim_matches(['\'', '"']);
        let hex = value.strip_prefix('#')?;
        let (red, green, blue) = match hex.len() {
            3 => (
                repeated_hex(hex.get(0..1)?)?,
                repeated_hex(hex.get(1..2)?)?,
                repeated_hex(hex.get(2..3)?)?,
            ),
            6 | 8 => (
                u8::from_str_radix(hex.get(0..2)?, 16).ok()?,
                u8::from_str_radix(hex.get(2..4)?, 16).ok()?,
                u8::from_str_radix(hex.get(4..6)?, 16).ok()?,
            ),
            _ => return None,
        };
        Some(Self {
            hex: format!("#{red:02X}{green:02X}{blue:02X}"),
            red,
            green,
            blue,
        })
    }

    pub(crate) fn hex(&self) -> &str {
        &self.hex
    }

    pub(crate) fn is_dark(&self) -> bool {
        // ITU-R BT.709 relative luminance, scaled to avoid floating point.
        u32::from(self.red) * 2126 + u32::from(self.green) * 7152 + u32::from(self.blue) * 722
            < 128 * 10_000
    }

    pub(crate) fn rgb(&self) -> (f64, f64, f64) {
        (
            f64::from(self.red) / 255.0,
            f64::from(self.green) / 255.0,
            f64::from(self.blue) / 255.0,
        )
    }
}

fn repeated_hex(value: &str) -> Option<u8> {
    u8::from_str_radix(&format!("{value}{value}"), 16).ok()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ThemePreview {
    pub(crate) name: String,
    pub(crate) background: ThemeColor,
    pub(crate) foreground: ThemeColor,
    pub(crate) palette: Vec<ThemeColor>,
    pub(crate) cursor: Option<ThemeColor>,
    pub(crate) cursor_text: Option<ThemeColor>,
    pub(crate) selection_background: Option<ThemeColor>,
    pub(crate) selection_foreground: Option<ThemeColor>,
    pub(crate) user_owned: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ThemeFilter {
    Dark,
    Light,
    All,
}

impl ThemePreview {
    pub(crate) fn matches(&self, query: &str, filter: ThemeFilter) -> bool {
        let matches_filter = match filter {
            ThemeFilter::Dark => self.background.is_dark(),
            ThemeFilter::Light => !self.background.is_dark(),
            ThemeFilter::All => true,
        };
        matches_filter
            && self
                .name
                .to_lowercase()
                .contains(&query.trim().to_lowercase())
    }
}

pub(crate) fn discover_themes(
    bundled_directory: &Path,
    user_directory: &Path,
) -> Vec<ThemePreview> {
    let mut themes = BTreeMap::<String, ThemePreview>::new();
    load_directory(bundled_directory, false, &mut themes);
    load_directory(user_directory, true, &mut themes);
    let mut themes = themes.into_values().collect::<Vec<_>>();
    themes.sort_by_cached_key(|theme| theme.name.to_lowercase());
    themes
}

fn load_directory(directory: &Path, user_owned: bool, themes: &mut BTreeMap<String, ThemePreview>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = fs::metadata(&path) else {
            continue;
        };
        if !is_eligible_theme_file(metadata.is_file(), metadata.len()) {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Ok(contents) = fs::read_to_string(&path) else {
            continue;
        };
        if let Some(theme) = parse_theme(&name, &contents, user_owned) {
            themes.insert(name, theme);
        }
    }
}

fn is_eligible_theme_file(is_file: bool, bytes: u64) -> bool {
    is_file && bytes > 0 && bytes <= MAX_THEME_BYTES
}

fn parse_theme(name: &str, contents: &str, user_owned: bool) -> Option<ThemePreview> {
    let mut background = None;
    let mut foreground = None;
    let mut palette = BTreeMap::<u8, ThemeColor>::new();
    let mut cursor = None;
    let mut cursor_text = None;
    let mut selection_background = None;
    let mut selection_foreground = None;
    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if is_ignored_theme_line(line) {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "background" => background = ThemeColor::parse(value),
            "foreground" => foreground = ThemeColor::parse(value),
            "cursor-color" => cursor = ThemeColor::parse(value),
            "cursor-text" => cursor_text = ThemeColor::parse(value),
            "selection-background" => selection_background = ThemeColor::parse(value),
            "selection-foreground" => selection_foreground = ThemeColor::parse(value),
            "palette" => {
                if let Some((index, color)) = value.split_once('=')
                    && let Ok(index) = index.trim().parse::<u8>()
                    && index < 16
                    && let Some(color) = ThemeColor::parse(color)
                {
                    palette.insert(index, color);
                }
            }
            _ => {}
        }
    }
    Some(ThemePreview {
        name: name.to_owned(),
        background: background?,
        foreground: foreground?,
        palette: palette.into_values().collect(),
        cursor,
        cursor_text,
        selection_background,
        selection_foreground,
        user_owned,
    })
}

fn is_ignored_theme_line(line: &str) -> bool {
    line.is_empty() || line.starts_with('#') || line.starts_with("//")
}

pub(crate) fn default_theme_directories(
    executable: &Path,
    xdg_config_home: Option<&OsStr>,
    home: Option<&OsStr>,
) -> Result<(PathBuf, PathBuf), String> {
    let prefix = executable.parent().and_then(Path::parent).ok_or_else(|| {
        format!(
            "Zentty executable has no install prefix: {}",
            executable.display()
        )
    })?;
    let bundled = prefix.join("share/zentty/ghostty/themes");
    let user_root = resolve_user_directory(UserDirectory::Config, xdg_config_home, home)
        .map_err(|error| format!("could not resolve Ghostty themes: {error}"))?;
    Ok((bundled, user_root.join("ghostty/themes")))
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_THEME_BYTES, ThemeColor, ThemeFilter, default_theme_directories, discover_themes,
        is_eligible_theme_file, is_ignored_theme_line, parse_theme,
    };
    use std::ffi::OsStr;
    use std::fs;
    use std::path::Path;

    fn root(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "zentty-theme-catalog-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn parses_preview_colors_palette_and_classification() {
        let theme = parse_theme(
            "A Theme",
            "# comment\nbackground = #123\nforeground = '#fefefe'\ncursor-color = #abcdef\ncursor-text = #010203\nselection-background = #112233\nselection-foreground = #ddeeff\npalette = 2=#00ff00\npalette = 15=#0000ff\npalette = 16=#ff0000\npalette = 17=#ffffff\n",
            false,
        )
        .unwrap();
        assert_eq!(theme.background.hex(), "#112233");
        assert_eq!(theme.foreground.hex(), "#FEFEFE");
        assert_eq!(theme.cursor.as_ref().unwrap().hex(), "#ABCDEF");
        assert_eq!(theme.cursor_text.as_ref().unwrap().hex(), "#010203");
        assert_eq!(
            theme.selection_background.as_ref().unwrap().hex(),
            "#112233"
        );
        assert_eq!(
            theme.selection_foreground.as_ref().unwrap().hex(),
            "#DDEEFF"
        );
        assert_eq!(
            theme
                .palette
                .iter()
                .map(ThemeColor::hex)
                .collect::<Vec<_>>(),
            ["#00FF00", "#0000FF"]
        );
        assert!(theme.background.is_dark());
        assert!(theme.matches(" theme ", ThemeFilter::Dark));
        assert!(!theme.matches("theme", ThemeFilter::Light));
        assert!(theme.matches("", ThemeFilter::All));
        assert!(parse_theme("invalid", "background = #000000", false).is_none());
        assert!(ThemeColor::parse("red").is_none());

        let mid_gray = ThemeColor::parse("#808080").unwrap();
        assert!(!mid_gray.is_dark(), "the luminance boundary is light");
        assert_eq!(
            mid_gray.rgb(),
            (128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0)
        );

        // Each channel contributes independently to the BT.709 classification.
        // These near-boundary colors keep mutation coverage from accepting a
        // accidentally changed coefficient or arithmetic operator.
        assert!(!ThemeColor::parse("#FF6702").unwrap().is_dark());
        assert!(!ThemeColor::parse("#00B20A").unwrap().is_dark());
        assert!(is_ignored_theme_line(""));
        assert!(is_ignored_theme_line("# comment"));
        assert!(is_ignored_theme_line("// comment"));
        assert!(!is_ignored_theme_line("background=#000000"));
        assert!(!is_eligible_theme_file(false, 1));
        assert!(!is_eligible_theme_file(true, 0));
        assert!(is_eligible_theme_file(true, MAX_THEME_BYTES));
        assert!(!is_eligible_theme_file(true, MAX_THEME_BYTES + 1));
    }

    #[test]
    fn user_theme_overrides_bundled_and_invalid_entries_are_not_catalogued() {
        let root = root("precedence");
        let bundled = root.join("bundled");
        let user = root.join("user");
        fs::create_dir_all(&bundled).unwrap();
        fs::create_dir_all(&user).unwrap();
        fs::write(
            bundled.join("Same"),
            "background=#000000\nforeground=#ffffff\n",
        )
        .unwrap();
        fs::write(
            user.join("Same"),
            "background=#eeeeee\nforeground=#111111\n",
        )
        .unwrap();
        fs::write(user.join("Broken"), "background=#000000\n").unwrap();
        fs::create_dir(user.join("Directory")).unwrap();
        let mut padded_theme = "background=#010101\nforeground=#fefefe\n#".to_owned();
        padded_theme.push_str(&" padding".repeat(256));
        fs::write(bundled.join("Padded"), padded_theme).unwrap();
        let themes = discover_themes(&bundled, &user);
        assert_eq!(themes.len(), 2);
        let same = themes.iter().find(|theme| theme.name == "Same").unwrap();
        assert_eq!(same.background.hex(), "#EEEEEE");
        assert!(same.user_owned);
        assert!(themes.iter().any(|theme| theme.name == "Padded"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolves_installed_and_xdg_theme_directories() {
        let (bundled, user) = default_theme_directories(
            Path::new("/opt/zentty/bin/zentty-linux"),
            Some(OsStr::new("/xdg")),
            Some(OsStr::new("/home/user")),
        )
        .unwrap();
        assert_eq!(
            bundled,
            Path::new("/opt/zentty/share/zentty/ghostty/themes")
        );
        assert_eq!(user, Path::new("/xdg/ghostty/themes"));
        let (_, user) = default_theme_directories(
            Path::new("/opt/zentty/bin/zentty-linux"),
            None,
            Some(OsStr::new("/home/user")),
        )
        .unwrap();
        assert_eq!(user, Path::new("/home/user/.config/ghostty/themes"));
        assert!(default_theme_directories(Path::new("zentty"), None, None).is_err());
    }
}
