use std::collections::BTreeMap;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Map, json};
use zentty_core::{AppearanceConfig, FALLBACK_DARK_THEME, FALLBACK_LIGHT_THEME};

use crate::theme_catalog::{ThemeColor, ThemePreview, default_theme_directories, discover_themes};

const TUI_THEME_SCHEMA: &str = "https://opencode.ai/theme.json";
pub(crate) const SYNCED_THEME_FILE: &str = "zentty-synced.json";
const MAX_SYNCED_THEME_BYTES: usize = 64 * 1024;
const MAX_PROCESS_ENVIRONMENT_BYTES: u64 = 1024 * 1024;
static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) fn synced_theme_data(
    dark_theme: &ThemePreview,
    light_theme: &ThemePreview,
) -> Result<Vec<u8>, String> {
    let dark = tui_tokens(dark_theme);
    let light = tui_tokens(light_theme);
    let mut keys = dark.keys().chain(light.keys()).cloned().collect::<Vec<_>>();
    keys.sort();
    keys.dedup();
    let mut theme = Map::new();
    for key in keys {
        let (Some(dark), Some(light)) = (dark.get(&key), light.get(&key)) else {
            continue;
        };
        theme.insert(key, json!({"dark": dark, "light": light}));
    }
    theme.insert("thinkingOpacity".to_owned(), json!(0.6));
    serde_json::to_vec_pretty(&json!({
        "$schema": TUI_THEME_SCHEMA,
        "theme": theme,
    }))
    .map_err(|error| format!("could not encode synchronized OpenCode theme: {error}"))
}

pub(crate) fn publish_theme_source(path: &Path, data: &[u8]) -> Result<bool, String> {
    if data.is_empty() || data.len() > MAX_SYNCED_THEME_BYTES {
        return Err("synchronized OpenCode theme is outside the size boundary".to_owned());
    }
    if fs::read(path).ok().as_deref() == Some(data) {
        return Ok(false);
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("OpenCode theme source has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create OpenCode theme source directory: {error}"))?;
    let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(".{}-{sequence}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|error| format!("could not create OpenCode theme source: {error}"))?;
    file.write_all(data)
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("could not write OpenCode theme source: {error}"))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("could not publish OpenCode theme source: {error}"))?;
    Ok(true)
}

pub(crate) fn refresh_running_overlay(
    overlay_config: &Path,
    data: &[u8],
    pid: i32,
    mut signal: impl FnMut(i32) -> Result<(), String>,
) -> Result<bool, String> {
    if pid <= 0 {
        return Err("OpenCode process ID must be positive".to_owned());
    }
    let tui = fs::read(overlay_config.join("tui.json"))
        .map_err(|error| format!("could not read managed OpenCode tui.json: {error}"))?;
    let tui: serde_json::Value = serde_json::from_slice(&tui)
        .map_err(|_| "managed OpenCode tui.json is malformed".to_owned())?;
    if tui.get("theme").and_then(serde_json::Value::as_str) != Some("zentty-synced") {
        return Ok(false);
    }
    let path = overlay_config.join("themes").join(SYNCED_THEME_FILE);
    let metadata = fs::symlink_metadata(&path)
        .map_err(|error| format!("could not inspect managed OpenCode theme: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("managed OpenCode theme is not a regular file".to_owned());
    }
    if !publish_theme_source(&path, data)? {
        return Ok(false);
    }
    signal(pid)?;
    Ok(true)
}

pub(crate) fn process_owns_overlay(pid: i32, pane_id: &str, overlay_config: &Path) -> bool {
    if pid <= 0 {
        return false;
    }
    let path = Path::new("/proc").join(pid.to_string()).join("environ");
    let Ok(metadata) = fs::metadata(&path) else {
        return false;
    };
    if !metadata.is_file() || metadata.len() > MAX_PROCESS_ENVIRONMENT_BYTES {
        return false;
    }
    let Ok(environment) = fs::read(path) else {
        return false;
    };
    let expected_pane = format!("ZENTTY_PANE_ID={pane_id}");
    let expected_overlay = format!("OPENCODE_CONFIG_DIR={}", overlay_config.display());
    let mut pane_matches = false;
    let mut overlay_matches = false;
    for entry in environment.split(|byte| *byte == 0) {
        pane_matches |= entry == expected_pane.as_bytes();
        overlay_matches |= entry == expected_overlay.as_bytes();
    }
    pane_matches && overlay_matches
}

fn tui_tokens(theme: &ThemePreview) -> BTreeMap<String, String> {
    let background = &theme.background;
    let foreground = &theme.foreground;
    let is_dark = background.is_dark();
    let primary = palette(
        theme,
        &[12, 4],
        &[theme.cursor.as_ref(), Some(foreground)],
        &[],
    );
    let secondary = palette(theme, &[13, 5], &[], &[&primary]);
    let accent = palette(theme, &[11, 3, 13, 5], &[], &[&secondary]);
    let success = palette(theme, &[10, 2], &[], &[&primary]);
    let warning = palette(theme, &[11, 3], &[], &[&accent]);
    let error = palette(theme, &[9, 1], &[], &[&accent]);
    let info = palette(theme, &[14, 6, 12, 4], &[], &[&primary]);
    let text = foreground.hex().to_owned();
    let text_muted = theme.indexed_palette.get(&8).map_or_else(
        || foreground.mixed_hex(background, if is_dark { 0.38 } else { 0.34 }),
        |color| color.hex().to_owned(),
    );
    let panel = background.mixed_hex(foreground, if is_dark { 0.05 } else { 0.03 });
    let element = background.mixed_hex(foreground, if is_dark { 0.09 } else { 0.06 });
    let border = background.mixed_hex(foreground, if is_dark { 0.24 } else { 0.18 });
    let border_active = background.mixed_hex(foreground, if is_dark { 0.34 } else { 0.26 });
    let border_subtle = background.mixed_hex(foreground, if is_dark { 0.14 } else { 0.10 });
    let success_color = ThemeColor::parse(&success).unwrap_or_else(|| foreground.clone());
    let error_color = ThemeColor::parse(&error).unwrap_or_else(|| foreground.clone());
    let added_bg = background.mixed_hex(&success_color, if is_dark { 0.18 } else { 0.14 });
    let removed_bg = background.mixed_hex(&error_color, if is_dark { 0.18 } else { 0.14 });
    let added_line = background.mixed_hex(&success_color, if is_dark { 0.10 } else { 0.08 });
    let removed_line = background.mixed_hex(&error_color, if is_dark { 0.10 } else { 0.08 });

    let values = [
        ("primary", primary.as_str()),
        ("secondary", secondary.as_str()),
        ("accent", accent.as_str()),
        ("success", success.as_str()),
        ("warning", warning.as_str()),
        ("error", error.as_str()),
        ("info", info.as_str()),
        ("text", text.as_str()),
        ("textMuted", text_muted.as_str()),
        ("background", background.hex()),
        ("backgroundPanel", panel.as_str()),
        ("backgroundElement", element.as_str()),
        ("backgroundMenu", element.as_str()),
        ("border", border.as_str()),
        ("borderActive", border_active.as_str()),
        ("borderSubtle", border_subtle.as_str()),
        ("diffAdded", success.as_str()),
        ("diffRemoved", error.as_str()),
        ("diffContext", text_muted.as_str()),
        ("diffHunkHeader", info.as_str()),
        ("diffHighlightAdded", success.as_str()),
        ("diffHighlightRemoved", error.as_str()),
        ("diffAddedBg", added_bg.as_str()),
        ("diffRemovedBg", removed_bg.as_str()),
        ("diffContextBg", panel.as_str()),
        ("diffLineNumber", text_muted.as_str()),
        ("diffAddedLineNumberBg", added_line.as_str()),
        ("diffRemovedLineNumberBg", removed_line.as_str()),
        ("markdownText", text.as_str()),
        ("markdownHeading", secondary.as_str()),
        ("markdownLink", primary.as_str()),
        ("markdownLinkText", info.as_str()),
        ("markdownCode", success.as_str()),
        ("markdownBlockQuote", warning.as_str()),
        ("markdownEmph", warning.as_str()),
        ("markdownStrong", accent.as_str()),
        ("markdownHorizontalRule", text_muted.as_str()),
        ("markdownListItem", primary.as_str()),
        ("markdownListEnumeration", info.as_str()),
        ("markdownImage", primary.as_str()),
        ("markdownImageText", info.as_str()),
        ("markdownCodeBlock", text.as_str()),
        ("syntaxComment", text_muted.as_str()),
        ("syntaxKeyword", secondary.as_str()),
        ("syntaxFunction", primary.as_str()),
        ("syntaxVariable", error.as_str()),
        ("syntaxString", success.as_str()),
        ("syntaxNumber", accent.as_str()),
        ("syntaxType", warning.as_str()),
        ("syntaxOperator", info.as_str()),
        ("syntaxPunctuation", text.as_str()),
    ];
    values
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect()
}

fn palette(
    theme: &ThemePreview,
    indexes: &[u8],
    fallback_colors: &[Option<&ThemeColor>],
    fallback_hexes: &[&str],
) -> String {
    indexes
        .iter()
        .find_map(|index| theme.indexed_palette.get(index))
        .or_else(|| fallback_colors.iter().flatten().copied().next())
        .map_or_else(
            || {
                fallback_hexes
                    .first()
                    .copied()
                    .unwrap_or("#000000")
                    .to_owned()
            },
            |color| color.hex().to_owned(),
        )
}

pub(crate) fn data_for_appearance(appearance: &AppearanceConfig) -> Result<Vec<u8>, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("could not locate themes for OpenCode sync: {error}"))?;
    let (bundled, user) = default_theme_directories(
        &executable,
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )?;
    let themes = discover_themes(&bundled, &user);
    data_for_catalog(appearance, &themes)
}

fn data_for_catalog(
    appearance: &AppearanceConfig,
    themes: &[ThemePreview],
) -> Result<Vec<u8>, String> {
    let dark_name = appearance
        .preferred_dark_theme_name
        .as_deref()
        .unwrap_or(FALLBACK_DARK_THEME);
    let light_name = appearance
        .preferred_light_theme_name
        .as_deref()
        .unwrap_or(FALLBACK_LIGHT_THEME);
    let find = |name: &str| themes.iter().find(|theme| theme.name == name);
    let dark = find(dark_name)
        .ok_or_else(|| format!("OpenCode sync could not resolve dark theme {dark_name:?}"))?;
    // The source falls back to the resolved dark theme when no light
    // counterpart exists. Preserve that launch behavior for custom catalogs.
    let light = find(light_name).unwrap_or(dark);
    synced_theme_data(dark, light)
}

#[cfg(test)]
mod tests {
    use super::{
        data_for_catalog, publish_theme_source, refresh_running_overlay, synced_theme_data,
    };
    use crate::theme_catalog::parse_theme;
    use serde_json::Value;
    use std::fs;
    use zentty_core::AppearanceConfig;

    #[test]
    fn source_theme_pair_generates_stable_opencode_variants() {
        let dark = parse_theme(
            "Night",
            "background=#101820\nforeground=#E6EDF3\npalette=4=#58A6FF\npalette=10=#3FB950\npalette=12=#79C0FF\npalette=13=#D2A8FF\n",
            false,
        )
        .unwrap();
        let light = parse_theme(
            "Day",
            "background=#FFF8E7\nforeground=#3C3836\npalette=4=#005F87\npalette=10=#008700\npalette=12=#1F78B4\npalette=13=#875F87\n",
            false,
        )
        .unwrap();
        let first = synced_theme_data(&dark, &light).unwrap();
        assert_eq!(first, synced_theme_data(&dark, &light).unwrap());
        let json: Value = serde_json::from_slice(&first).unwrap();
        assert_eq!(json["$schema"], "https://opencode.ai/theme.json");
        assert_eq!(json["theme"]["background"]["dark"], "#101820");
        assert_eq!(json["theme"]["background"]["light"], "#FFF8E7");
        assert_eq!(json["theme"]["primary"]["dark"], "#79C0FF");
        assert_eq!(json["theme"]["thinkingOpacity"], 0.6);
    }

    #[test]
    fn publication_is_bounded_atomic_and_idempotent() {
        let root = std::env::temp_dir().join(format!(
            "zentty-opencode-theme-source-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let path = root.join("source.json");
        assert!(publish_theme_source(&path, b"{\"theme\":{}}\n").unwrap());
        assert!(!publish_theme_source(&path, b"{\"theme\":{}}\n").unwrap());
        assert!(publish_theme_source(&path, b"").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"{\"theme\":{}}\n");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn appearance_names_select_independent_catalog_slots_with_dark_fallback() {
        let dark = parse_theme("Night", "background=#101820\nforeground=#E6EDF3\n", false).unwrap();
        let light = parse_theme("Day", "background=#FFF8E7\nforeground=#3C3836\n", false).unwrap();
        let mut appearance = AppearanceConfig {
            preferred_dark_theme_name: Some("Night".to_owned()),
            preferred_light_theme_name: Some("Day".to_owned()),
            ..AppearanceConfig::default()
        };
        let json: Value =
            serde_json::from_slice(&data_for_catalog(&appearance, &[dark.clone(), light]).unwrap())
                .unwrap();
        assert_eq!(json["theme"]["background"]["light"], "#FFF8E7");
        appearance.preferred_light_theme_name = Some("Missing".to_owned());
        let json: Value =
            serde_json::from_slice(&data_for_catalog(&appearance, &[dark]).unwrap()).unwrap();
        assert_eq!(json["theme"]["background"]["light"], "#101820");
        appearance.preferred_dark_theme_name = Some("Missing".to_owned());
        assert!(data_for_catalog(&appearance, &[]).is_err());
    }

    #[test]
    fn live_refresh_requires_managed_selection_and_signals_only_after_change() {
        let root =
            std::env::temp_dir().join(format!("zentty-opencode-live-theme-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("themes")).unwrap();
        fs::write(root.join("tui.json"), r#"{"theme":"zentty-synced"}"#).unwrap();
        fs::write(root.join("themes/zentty-synced.json"), b"old").unwrap();
        let mut signaled = Vec::new();
        assert!(
            refresh_running_overlay(&root, b"new", 42, |pid| {
                signaled.push(pid);
                Ok(())
            })
            .unwrap()
        );
        assert_eq!(signaled, [42]);
        assert!(!refresh_running_overlay(&root, b"new", 42, |_| Ok(())).unwrap());
        fs::write(root.join("tui.json"), r#"{"theme":"user-choice"}"#).unwrap();
        assert!(!refresh_running_overlay(&root, b"other", 42, |_| Ok(())).unwrap());
        assert!(refresh_running_overlay(&root, b"other", 0, |_| Ok(())).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
