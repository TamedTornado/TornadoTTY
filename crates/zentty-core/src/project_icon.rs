use std::{
    collections::HashMap,
    fs,
    path::{Component, Path, PathBuf},
};

const DEFAULT_NEGATIVE_TTL_SECONDS: u64 = 5 * 60;
const MAX_SOURCE_BYTES: u64 = 256 * 1024;
const MAX_ICON_BYTES: u64 = 8 * 1024 * 1024;

const ICON_CANDIDATES: &[&str] = &[
    "favicon.svg",
    "favicon.ico",
    "favicon.png",
    "public/favicon.svg",
    "public/favicon.ico",
    "public/favicon.png",
    "public/apple-touch-icon.png",
    "apple-touch-icon.png",
    "images/favicon/favicon.svg",
    "images/favicon/apple-touch-icon.png",
    "images/favicon/favicon-32x32.png",
    "images/favicon/favicon.ico",
    "images/favicon.svg",
    "images/favicon.ico",
    "images/favicon.png",
    "images/logo_color.svg",
    "images/logo.svg",
    "images/logo_color.png",
    "images/logo.png",
    "app/favicon.ico",
    "app/favicon.png",
    "app/icon.svg",
    "app/icon.png",
    "app/icon.ico",
    "src/favicon.ico",
    "src/favicon.svg",
    "src/app/favicon.ico",
    "src/app/icon.svg",
    "src/app/icon.png",
    "assets/icon.svg",
    "assets/icon.png",
    "assets/logo.svg",
    "assets/logo.png",
    ".idea/icon.svg",
];

const ICON_SOURCE_FILES: &[&str] = &[
    "index.html",
    "public/index.html",
    "app/routes/__root.tsx",
    "src/routes/__root.tsx",
    "app/root.tsx",
    "src/root.tsx",
    "src/index.html",
];

const APP_ICON_MANIFESTS: &[&str] = &[
    "Assets.xcassets/AppIcon.appiconset/Contents.json",
    "Resources/Assets.xcassets/AppIcon.appiconset/Contents.json",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectIconLookup {
    Hit(PathBuf),
    Miss,
}

#[derive(Clone, Debug)]
enum CachedResolution {
    Hit(PathBuf),
    Miss { checked_at: u64 },
}

#[derive(Clone, Debug)]
pub struct ProjectIconCache {
    negative_ttl_seconds: u64,
    entries: HashMap<PathBuf, CachedResolution>,
}

impl Default for ProjectIconCache {
    fn default() -> Self {
        Self::new(DEFAULT_NEGATIVE_TTL_SECONDS)
    }
}

impl ProjectIconCache {
    #[must_use]
    pub fn new(negative_ttl_seconds: u64) -> Self {
        Self {
            negative_ttl_seconds,
            entries: HashMap::new(),
        }
    }

    /// Resolves one project icon using the source candidate order.
    ///
    /// # Errors
    ///
    /// Returns an error when `project_root` is absent, not a directory, or
    /// cannot be canonicalized safely.
    pub fn resolve_at(
        &mut self,
        project_root: &Path,
        now_seconds: u64,
    ) -> Result<ProjectIconLookup, String> {
        let root = fs::canonicalize(project_root)
            .map_err(|error| format!("could not resolve project root: {error}"))?;
        if !root.is_dir() {
            return Err("project root is not a directory".to_owned());
        }
        if let Some(cached) = self.entries.get(&root) {
            match cached {
                CachedResolution::Hit(path) => return Ok(ProjectIconLookup::Hit(path.clone())),
                CachedResolution::Miss { checked_at }
                    if now_seconds.saturating_sub(*checked_at) < self.negative_ttl_seconds =>
                {
                    return Ok(ProjectIconLookup::Miss);
                }
                CachedResolution::Miss { .. } => {}
            }
        }
        let resolved = scan(&root).map_or(ProjectIconLookup::Miss, ProjectIconLookup::Hit);
        let cached = match &resolved {
            ProjectIconLookup::Hit(path) => CachedResolution::Hit(path.clone()),
            ProjectIconLookup::Miss => CachedResolution::Miss {
                checked_at: now_seconds,
            },
        };
        self.entries.insert(root, cached);
        Ok(resolved)
    }

    pub fn invalidate(&mut self, project_root: &Path) -> bool {
        fs::canonicalize(project_root)
            .ok()
            .is_some_and(|root| self.entries.remove(&root).is_some())
    }

    pub fn invalidate_all(&mut self) {
        self.entries.clear();
    }
}

fn scan(root: &Path) -> Option<PathBuf> {
    for relative in ICON_CANDIDATES {
        if let Some(path) = secure_icon(root, &root.join(relative)) {
            return Some(path);
        }
    }
    for manifest in APP_ICON_MANIFESTS {
        let manifest = root.join(manifest);
        let Some(source) = secure_source(root, &manifest) else {
            continue;
        };
        let Some(bytes) = read_bounded(&source, MAX_SOURCE_BYTES) else {
            continue;
        };
        let Some(value) = serde_json::from_slice::<serde_json::Value>(&bytes).ok() else {
            continue;
        };
        let Some(images) = value.get("images").and_then(serde_json::Value::as_array) else {
            continue;
        };
        let Some(filename) = images
            .iter()
            .filter_map(|image| {
                let filename = image.get("filename")?.as_str()?.trim();
                if filename.is_empty() {
                    return None;
                }
                let size = parse_dimension(image.get("size").and_then(|value| value.as_str()), 0.0);
                let scale =
                    parse_dimension(image.get("scale").and_then(|value| value.as_str()), 1.0);
                Some((size * scale, filename))
            })
            .max_by(|left, right| left.0.total_cmp(&right.0))
            .map(|(_, filename)| filename)
        else {
            continue;
        };
        let Some(parent) = manifest.parent() else {
            continue;
        };
        if let Some(path) = secure_icon(root, &parent.join(filename)) {
            return Some(path);
        }
    }
    for source_file in ICON_SOURCE_FILES {
        let Some(source_path) = secure_source(root, &root.join(source_file)) else {
            continue;
        };
        let Some(bytes) = read_bounded(&source_path, MAX_SOURCE_BYTES) else {
            continue;
        };
        let Some(source) = std::str::from_utf8(&bytes).ok() else {
            continue;
        };
        let Some(href) = extract_icon_href(source) else {
            continue;
        };
        let Some(relative) = safe_href_path(&href) else {
            continue;
        };
        for candidate in [root.join(&relative), root.join("public").join(&relative)] {
            if let Some(path) = secure_icon(root, &candidate) {
                return Some(path);
            }
        }
    }
    None
}

fn secure_source(root: &Path, candidate: &Path) -> Option<PathBuf> {
    let path = fs::canonicalize(candidate).ok()?;
    path.starts_with(root)
        .then_some(path)
        .filter(|path| path.is_file())
}

fn secure_icon(root: &Path, candidate: &Path) -> Option<PathBuf> {
    let path = secure_source(root, candidate)?;
    let metadata = fs::metadata(&path).ok()?;
    (metadata.len() <= MAX_ICON_BYTES && valid_icon_payload(&path)).then_some(path)
}

fn valid_icon_payload(path: &Path) -> bool {
    let Some(bytes) = read_bounded(path, MAX_ICON_BYTES) else {
        return false;
    };
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        Some("ico") => bytes.starts_with(&[0, 0, 1, 0]),
        Some("svg") => std::str::from_utf8(&bytes)
            .ok()
            .is_some_and(|source| source.to_ascii_lowercase().contains("<svg")),
        _ => false,
    }
}

fn read_bounded(path: &Path, maximum: u64) -> Option<Vec<u8>> {
    (fs::metadata(path).ok()?.len() <= maximum)
        .then(|| fs::read(path).ok())
        .flatten()
}

fn parse_dimension(value: Option<&str>, fallback: f64) -> f64 {
    value
        .and_then(|value| {
            value
                .trim()
                .trim_end_matches(['x', 'X'])
                .split(['x', 'X'])
                .next()
        })
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or(fallback)
}

fn extract_icon_href(source: &str) -> Option<String> {
    let lower = source.to_ascii_lowercase();
    for (start, end_marker) in [("<link", '>'), ("{", '}')] {
        let mut offset = 0;
        while let Some(relative_start) = lower[offset..].find(start) {
            let Some(block_start) = offset.checked_add(relative_start) else {
                break;
            };
            let Some(relative_end) = lower[block_start..].find(end_marker) else {
                break;
            };
            let Some(block_end) = relative_end
                .checked_add(block_start)
                .and_then(|end| end.checked_add(1))
            else {
                break;
            };
            let block = &source[block_start..block_end];
            if let Some(rel) = quoted_field(block, "rel") {
                let rel = rel.to_ascii_lowercase();
                if matches!(rel.trim(), "icon" | "shortcut icon")
                    && let Some(href) = quoted_field(block, "href")
                {
                    return Some(href);
                }
            }
            offset = block_end;
        }
    }
    None
}

fn quoted_field(block: &str, field: &str) -> Option<String> {
    let lower = block.to_ascii_lowercase();
    let mut offset = 0;
    while let Some(relative) = lower[offset..].find(field) {
        let start = offset.checked_add(relative)?;
        let before_ok = start == 0 || !is_field_name_byte(lower.as_bytes()[start - 1]);
        let after = start.checked_add(field.len())?;
        if before_ok {
            let tail = block[after..].trim_start();
            if let Some(tail) = tail
                .strip_prefix('=')
                .or_else(|| tail.strip_prefix(':'))
                .map(str::trim_start)
                && let Some(quote) = tail.as_bytes().first().copied()
                && matches!(quote, b'\'' | b'"')
            {
                let value = &tail[1..];
                if let Some(end) = value.as_bytes().iter().position(|byte| *byte == quote) {
                    return Some(value[..end].to_owned());
                }
            }
        }
        offset = after;
    }
    None
}

fn is_field_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b':')
}

fn safe_href_path(href: &str) -> Option<PathBuf> {
    let href = href.split(['?', '#']).next()?.trim();
    if href.is_empty() || href.contains("://") || href.starts_with("data:") {
        return None;
    }
    let path = Path::new(href.trim_start_matches('/'));
    path.components()
        .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
        .then(|| path.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{extract_icon_href, parse_dimension, quoted_field, safe_href_path};

    #[test]
    fn markup_parser_skips_lookalike_fields_and_malformed_blocks() {
        let source = r#"
            <link data-rel="icon" href="wrong.svg">
            <link relish="icon" href="also-wrong.svg">
            <link rel="stylesheet" href="styles.css">
            <link href="right.svg" rel="icon">
        "#;
        assert_eq!(extract_icon_href(source).as_deref(), Some("right.svg"));
        assert_eq!(quoted_field("rel='icon'", "rel").as_deref(), Some("icon"));
        assert_eq!(quoted_field("xrel='icon'", "rel"), None);
        assert_eq!(quoted_field("relx='icon'", "rel"), None);
        assert_eq!(quoted_field("rel=icon", "rel"), None);
    }

    #[test]
    fn dimension_parser_multiplies_valid_size_and_scale_components() {
        for (value, fallback, expected) in [
            (Some("16x16"), 0.0, 16.0),
            (Some("2X"), 0.0, 2.0),
            (Some("NaNxNaN"), 7.0, 7.0),
            (Some("-1x-1"), 8.0, 8.0),
            (None, 9.0, 9.0),
        ] {
            assert!((parse_dimension(value, fallback) - expected).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn href_policy_accepts_only_nonempty_relative_local_paths() {
        assert_eq!(
            safe_href_path("icons/a.svg?x=1#y"),
            Some("icons/a.svg".into())
        );
        assert_eq!(safe_href_path("/icons/a.svg"), Some("icons/a.svg".into()));
        assert_eq!(safe_href_path(""), None);
        assert_eq!(safe_href_path("data:image/svg+xml,x"), None);
        assert_eq!(safe_href_path("https://example.test/a.svg"), None);
        assert_eq!(safe_href_path("../outside.svg"), None);
    }
}
