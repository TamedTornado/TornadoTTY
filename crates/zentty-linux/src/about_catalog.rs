use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

const MAX_CATALOG_BYTES: u64 = 2 * 1024 * 1024;
const MAX_NOTICE_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_TOTAL_NOTICE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AboutMetadata {
    pub(crate) version: String,
    pub(crate) build: String,
    pub(crate) commit: String,
}

impl AboutMetadata {
    pub(crate) fn compiled() -> Self {
        Self::from_values(
            env!("CARGO_PKG_VERSION"),
            option_env!("ZENTTY_BUILD_PROFILE").unwrap_or("unknown"),
            option_env!("ZENTTY_BUILD_COMMIT").unwrap_or("unknown"),
            option_env!("ZENTTY_BUILD_TREE").unwrap_or("unknown"),
        )
    }

    fn from_values(version: &str, profile: &str, commit: &str, tree: &str) -> Self {
        let version = nonempty_or(version, "Unknown");
        let profile = nonempty_or(profile, "unknown");
        let tree = match tree.trim() {
            "clean" => "clean",
            "dirty" => "dirty",
            _ => "unknown",
        };
        let commit = if is_lower_hex_commit(commit) {
            commit.to_owned()
        } else {
            "unknown".to_owned()
        };
        Self {
            version,
            build: format!("{profile} ({tree})"),
            commit,
        }
    }
}

fn nonempty_or(value: &str, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.to_owned()
    } else {
        value.to_owned()
    }
}

fn is_lower_hex_commit(value: &str) -> bool {
    value.len() == 12
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn compiled_catalog_revision() -> &'static str {
    option_env!("ZENTTY_BUILD_REVISION")
        .filter(|revision| is_lower_hex_revision(revision))
        .unwrap_or("unknown")
}

fn is_lower_hex_revision(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LicenseEntry {
    pub(crate) id: String,
    pub(crate) ecosystem: String,
    pub(crate) display_name: String,
    pub(crate) version: String,
    pub(crate) license: String,
    pub(crate) source_url: String,
    pub(crate) homepage_url: Option<String>,
    pub(crate) full_text: String,
}

impl LicenseEntry {
    pub(crate) fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty()
            || [
                self.display_name.as_str(),
                self.version.as_str(),
                self.license.as_str(),
                self.ecosystem.as_str(),
            ]
            .iter()
            .any(|candidate| candidate.to_lowercase().contains(&query))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LicenseCatalog {
    pub(crate) entries: Vec<LicenseEntry>,
    pub(crate) zentty_revision: String,
    pub(crate) ghostty_revision: String,
}

impl LicenseCatalog {
    pub(crate) fn load(root: &Path) -> Result<Self, String> {
        let catalog_path = root.join("catalog-v1.json");
        let metadata = regular_file_metadata(&catalog_path)?;
        if metadata.len() == 0 || metadata.len() > MAX_CATALOG_BYTES {
            return Err("license catalog size is outside the reviewed bound".to_owned());
        }
        let bytes = fs::read(&catalog_path)
            .map_err(|error| format!("could not read {}: {error}", catalog_path.display()))?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|error| format!("license catalog is malformed: {error}"))?;
        let object = exact_object(&value, &["entries", "generated_from", "schema_version"])?;
        if object.get("schema_version").and_then(Value::as_u64) != Some(1) {
            return Err("license catalog schema version is unsupported".to_owned());
        }
        let generated = exact_object(required(object, "generated_from")?, &["ghostty", "zentty"])?;
        let (zentty_repository, zentty_revision) = source_identity(generated, "zentty")?;
        let (ghostty_repository, ghostty_revision) = source_identity(generated, "ghostty")?;
        if !url_is_reviewed(&zentty_repository) || !url_is_reviewed(&ghostty_repository) {
            return Err("catalog source repository is not reviewed HTTPS".to_owned());
        }
        let raw_entries = required(object, "entries")?
            .as_array()
            .ok_or_else(|| "license catalog entries must be an array".to_owned())?;
        if raw_entries.is_empty() {
            return Err("license catalog contains no entries".to_owned());
        }
        let mut ids = BTreeSet::new();
        let mut total_notice_bytes = 0usize;
        let mut entries = Vec::with_capacity(raw_entries.len());
        for raw in raw_entries {
            let entry = parse_entry(root, raw, &mut total_notice_bytes)?;
            if !ids.insert(entry.id.clone()) {
                return Err(format!("duplicate license entry id: {}", entry.id));
            }
            entries.push(entry);
        }
        entries.sort_by(|left, right| {
            left.display_name
                .to_lowercase()
                .cmp(&right.display_name.to_lowercase())
                .then_with(|| left.version.cmp(&right.version))
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(Self {
            entries,
            zentty_revision,
            ghostty_revision,
        })
    }
}

pub(crate) fn default_notice_roots(executable: &Path) -> Result<Vec<PathBuf>, String> {
    let prefix = executable.parent().and_then(Path::parent).ok_or_else(|| {
        format!(
            "Zentty executable has no install prefix: {}",
            executable.display()
        )
    })?;
    Ok(vec![
        prefix.join("share/zentty/package-notices"),
        PathBuf::from("/usr/share/doc/zentty/third-party"),
    ])
}

pub(crate) fn default_icon_paths(executable: &Path) -> Result<Vec<PathBuf>, String> {
    let prefix = executable.parent().and_then(Path::parent).ok_or_else(|| {
        format!(
            "Zentty executable has no install prefix: {}",
            executable.display()
        )
    })?;
    let relative = Path::new("share/icons/hicolor/256x256/apps/com.zentty.zentty.png");
    Ok(vec![
        prefix.join(relative),
        Path::new("/usr").join(relative),
    ])
}

pub(crate) fn load_default_catalog() -> Result<LicenseCatalog, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("could not locate Zentty executable: {error}"))?;
    let roots = default_notice_roots(&executable)?;
    load_catalog_from_roots(&roots, compiled_catalog_revision())
}

fn load_catalog_from_roots(
    roots: &[PathBuf],
    expected_zentty_revision: &str,
) -> Result<LicenseCatalog, String> {
    let mut failures = Vec::new();
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        match LicenseCatalog::load(root) {
            Ok(catalog)
                if expected_zentty_revision == "unknown"
                    || catalog.zentty_revision == expected_zentty_revision =>
            {
                return Ok(catalog);
            }
            Ok(catalog) => failures.push(format!(
                "{}: catalog Zentty revision {} does not match executable revision {}",
                root.display(),
                catalog.zentty_revision,
                expected_zentty_revision
            )),
            Err(error) => failures.push(format!("{}: {error}", root.display())),
        }
    }
    if failures.is_empty() {
        Err("third-party license resources are not installed".to_owned())
    } else {
        Err(format!(
            "third-party license resources are invalid: {}",
            failures.join("; ")
        ))
    }
}

fn parse_entry(
    root: &Path,
    raw: &Value,
    total_notice_bytes: &mut usize,
) -> Result<LicenseEntry, String> {
    let object = exact_object(
        raw,
        &[
            "display_name",
            "ecosystem",
            "homepage_url",
            "id",
            "license",
            "notice",
            "source_url",
            "version",
        ],
    )?;
    let id = required_string(object, "id")?;
    if !safe_identifier(&id) {
        return Err(format!("license entry id is unsafe: {id}"));
    }
    let source_url = required_string(object, "source_url")?;
    if !url_is_reviewed(&source_url) {
        return Err(format!("license source URL is not reviewed HTTPS: {id}"));
    }
    let homepage_url = match required(object, "homepage_url")? {
        Value::Null => None,
        Value::String(value) if url_is_reviewed(value) => Some(value.clone()),
        _ => return Err(format!("license homepage URL is invalid: {id}")),
    };
    let notice = exact_object(required(object, "notice")?, &["kind", "path"])?;
    let kind = required_string(notice, "kind")?;
    let relative = PathBuf::from(required_string(notice, "path")?);
    validate_relative_path(&relative)?;
    let full_text = read_notice(root, &relative, &kind, total_notice_bytes)?;
    Ok(LicenseEntry {
        id,
        ecosystem: required_string(object, "ecosystem")?,
        display_name: required_string(object, "display_name")?,
        version: required_string(object, "version")?,
        license: required_string(object, "license")?,
        source_url,
        homepage_url,
        full_text,
    })
}

fn read_notice(
    root: &Path,
    relative: &Path,
    kind: &str,
    total_notice_bytes: &mut usize,
) -> Result<String, String> {
    let path = root.join(relative);
    let mut files = match kind {
        "file" => vec![path],
        "directory" => {
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
            if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
                return Err(format!(
                    "notice directory is not a real directory: {}",
                    path.display()
                ));
            }
            let mut files = fs::read_dir(&path)
                .map_err(|error| format!("could not read {}: {error}", path.display()))?
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("could not enumerate {}: {error}", path.display()))?;
            files.sort();
            files
        }
        _ => return Err(format!("unsupported notice kind: {kind}")),
    };
    if files.is_empty() {
        return Err(format!("notice resource is empty: {}", relative.display()));
    }
    let mut chunks = Vec::with_capacity(files.len());
    for file in files.drain(..) {
        let metadata = regular_file_metadata(&file)?;
        if metadata.len() == 0 || metadata.len() > MAX_NOTICE_FILE_BYTES {
            return Err(format!(
                "notice file size is outside the reviewed bound: {}",
                file.display()
            ));
        }
        let size = usize::try_from(metadata.len())
            .map_err(|_| "notice file size cannot be represented".to_owned())?;
        add_notice_bytes(total_notice_bytes, size, MAX_TOTAL_NOTICE_BYTES)?;
        let text = fs::read_to_string(&file).map_err(|error| {
            format!(
                "notice text is not valid UTF-8 at {}: {error}",
                file.display()
            )
        })?;
        let name = file
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("notice filename is not UTF-8: {}", file.display()))?;
        chunks.push(format!("===== {name} =====\n\n{text}"));
    }
    Ok(chunks.join("\n\n"))
}

fn add_notice_bytes(total: &mut usize, size: usize, maximum: usize) -> Result<(), String> {
    *total = total
        .checked_add(size)
        .ok_or_else(|| "notice catalog size overflowed".to_owned())?;
    if *total > maximum {
        return Err("notice catalog exceeds the reviewed total size".to_owned());
    }
    Ok(())
}

fn regular_file_metadata(path: &Path) -> Result<fs::Metadata, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(format!(
            "resource is not a real regular file: {}",
            path.display()
        ));
    }
    Ok(metadata)
}

fn exact_object<'a>(value: &'a Value, expected: &[&str]) -> Result<&'a Map<String, Value>, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "license catalog value must be an object".to_owned())?;
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err("license catalog contains missing or unknown fields".to_owned());
    }
    Ok(object)
}

fn required<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a Value, String> {
    object
        .get(key)
        .ok_or_else(|| format!("license catalog field is missing: {key}"))
}

fn required_string(object: &Map<String, Value>, key: &str) -> Result<String, String> {
    let value = required(object, key)?
        .as_str()
        .ok_or_else(|| format!("license catalog field must be a string: {key}"))?
        .trim();
    if value.is_empty() || value.len() > 4096 || value.chars().any(char::is_control) {
        return Err(format!("license catalog field is invalid: {key}"));
    }
    Ok(value.to_owned())
}

fn source_identity(generated: &Map<String, Value>, name: &str) -> Result<(String, String), String> {
    let source = exact_object(required(generated, name)?, &["repository", "revision"])?;
    let repository = required_string(source, "repository")?;
    let revision = required_string(source, "revision")?;
    if !is_lower_hex_revision(&revision) {
        return Err(format!("catalog {name} revision is invalid"));
    }
    Ok((repository, revision))
}

fn safe_identifier(value: &str) -> bool {
    value.len() <= 512
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-' | b'/')
        })
}

fn url_is_reviewed(value: &str) -> bool {
    value.len() <= 2048
        && value.starts_with("https://")
        && !value.chars().any(char::is_whitespace)
        && !value.chars().any(char::is_control)
}

fn validate_relative_path(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("license notice path is unsafe".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "zentty-about-catalog-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("cargo/sample-1.2.3")).unwrap();
        fs::write(root.join("cargo/sample-1.2.3/LICENSE"), "Sample license").unwrap();
        root
    }

    fn catalog(entry: &str) -> String {
        format!(
            r#"{{"schema_version":1,"generated_from":{{"zentty":{{"repository":"https://github.com/TamedTornado/zentty","revision":"{revision}"}},"ghostty":{{"repository":"https://github.com/TamedTornado/ghostty","revision":"{revision}"}}}},"entries":[{entry}]}}"#,
            revision = "a".repeat(40)
        )
    }

    fn valid_entry() -> &'static str {
        r#"{"id":"cargo/sample/1.2.3","ecosystem":"cargo","display_name":"Sample","version":"1.2.3","license":"MIT","source_url":"https://crates.io/crates/sample/1.2.3","homepage_url":null,"notice":{"kind":"directory","path":"cargo/sample-1.2.3"}}"#
    }

    #[test]
    fn metadata_is_truthful_and_rejects_non_commit_values() {
        let metadata =
            AboutMetadata::from_values(" 0.1.0 ", "release-safe", "abcdef123456", "dirty");
        assert_eq!(metadata.version, "0.1.0");
        assert_eq!(metadata.build, "release-safe (dirty)");
        assert_eq!(metadata.commit, "abcdef123456");
        assert_eq!(
            AboutMetadata::from_values("", "", "ABC", "surprising"),
            AboutMetadata {
                version: "Unknown".to_owned(),
                build: "unknown (unknown)".to_owned(),
                commit: "unknown".to_owned(),
            }
        );
        assert_eq!(
            AboutMetadata::from_values("1.2.3", "release-safe", &"c".repeat(40), "clean").build,
            "release-safe (clean)"
        );
    }

    #[test]
    fn display_commits_and_catalog_revisions_have_distinct_provenance_contracts() {
        assert!(is_lower_hex_commit("abcdef123456"));
        assert!(!is_lower_hex_commit("abcdef12345"));
        assert!(!is_lower_hex_commit(&"a".repeat(40)));
        assert!(!is_lower_hex_commit("ABCDEF123456"));

        assert!(is_lower_hex_revision(&"b".repeat(40)));
        assert!(!is_lower_hex_revision("abcdef123456"));
        assert!(!is_lower_hex_revision(&"g".repeat(40)));

        // Ordinary developer test builds deliberately have no staged-product
        // provenance environment. Staged builds exercise the full value in
        // the real About integration journey.
        assert_eq!(compiled_catalog_revision(), "unknown");
    }

    #[test]
    fn catalog_loads_real_text_and_searches_metadata() {
        let root = root("valid");
        fs::write(root.join("catalog-v1.json"), catalog(valid_entry())).unwrap();
        let loaded = LicenseCatalog::load(&root).unwrap();
        assert_eq!(loaded.entries.len(), 1);
        assert!(loaded.entries[0].full_text.contains("Sample license"));
        assert!(loaded.entries[0].matches("mit"));
        assert!(loaded.entries[0].matches("1.2"));
        assert!(!loaded.entries[0].matches("apache"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn catalog_rejects_unknown_fields_duplicates_traversal_links_and_missing_text() {
        let root = root("negative");
        let duplicate = format!("{},{}", valid_entry(), valid_entry());
        fs::write(root.join("catalog-v1.json"), catalog(&duplicate)).unwrap();
        assert!(
            LicenseCatalog::load(&root)
                .unwrap_err()
                .contains("duplicate")
        );

        let unsafe_entry = valid_entry().replace("cargo/sample-1.2.3\"}", "../sample\"}");
        fs::write(root.join("catalog-v1.json"), catalog(&unsafe_entry)).unwrap();
        assert!(LicenseCatalog::load(&root).unwrap_err().contains("unsafe"));

        let unsafe_url = valid_entry().replace("https://crates.io", "file:///tmp");
        fs::write(root.join("catalog-v1.json"), catalog(&unsafe_url)).unwrap();
        assert!(LicenseCatalog::load(&root).unwrap_err().contains("HTTPS"));

        fs::write(
            root.join("catalog-v1.json"),
            catalog(&valid_entry().replace(
                "\"homepage_url\":null",
                "\"homepage_url\":null,\"unknown\":true",
            )),
        )
        .unwrap();
        assert!(LicenseCatalog::load(&root).unwrap_err().contains("unknown"));

        fs::write(root.join("catalog-v1.json"), catalog(valid_entry())).unwrap();
        fs::remove_file(root.join("cargo/sample-1.2.3/LICENSE")).unwrap();
        assert!(LicenseCatalog::load(&root).unwrap_err().contains("empty"));

        let mut oversized = fs::File::create(root.join("cargo/sample-1.2.3/LICENSE")).unwrap();
        oversized.set_len(MAX_NOTICE_FILE_BYTES + 1).unwrap();
        oversized.flush().unwrap();
        assert!(LicenseCatalog::load(&root).unwrap_err().contains("size"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resource_discovery_models_staged_then_installed_layouts() {
        assert_eq!(
            default_notice_roots(Path::new("/opt/zentty/bin/zentty-linux")).unwrap(),
            vec![
                PathBuf::from("/opt/zentty/share/zentty/package-notices"),
                PathBuf::from("/usr/share/doc/zentty/third-party"),
            ]
        );
        assert_eq!(
            default_icon_paths(Path::new("/opt/zentty/bin/zentty-linux")).unwrap(),
            vec![
                PathBuf::from("/opt/zentty/share/icons/hicolor/256x256/apps/com.zentty.zentty.png"),
                PathBuf::from("/usr/share/icons/hicolor/256x256/apps/com.zentty.zentty.png"),
            ]
        );
    }

    #[test]
    fn executable_identity_rejects_stale_catalog_and_missing_resources() {
        let root = root("identity");
        fs::write(root.join("catalog-v1.json"), catalog(valid_entry())).unwrap();
        let revision = "a".repeat(40);
        assert!(load_catalog_from_roots(std::slice::from_ref(&root), &revision).is_ok());
        let error =
            load_catalog_from_roots(std::slice::from_ref(&root), &"b".repeat(40)).unwrap_err();
        assert!(error.contains("does not match executable revision"));
        fs::remove_dir_all(&root).unwrap();
        assert!(
            load_catalog_from_roots(&[root], &revision)
                .unwrap_err()
                .contains("not installed")
        );
    }

    #[test]
    fn catalog_rejects_empty_malformed_and_truncated_json() {
        let root = root("json-shape");
        fs::write(root.join("catalog-v1.json"), "").unwrap();
        assert!(
            LicenseCatalog::load(&root)
                .unwrap_err()
                .contains("size is outside")
        );
        for invalid in ["not json", r#"{"schema_version":1"#] {
            fs::write(root.join("catalog-v1.json"), invalid).unwrap();
            assert!(
                LicenseCatalog::load(&root)
                    .unwrap_err()
                    .contains("malformed")
            );
        }
        let oversized = fs::File::create(root.join("catalog-v1.json")).unwrap();
        oversized.set_len(MAX_CATALOG_BYTES + 1).unwrap();
        assert!(
            LicenseCatalog::load(&root)
                .unwrap_err()
                .contains("size is outside")
        );
        oversized.set_len(MAX_CATALOG_BYTES).unwrap();
        assert!(
            LicenseCatalog::load(&root)
                .unwrap_err()
                .contains("malformed")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn catalog_validates_each_repository_and_optional_homepage_independently() {
        let root = root("links");
        let valid = catalog(valid_entry());
        for invalid in [
            valid.replacen(
                "https://github.com/TamedTornado/zentty",
                "http://example.invalid/zentty",
                1,
            ),
            valid.replacen(
                "https://github.com/TamedTornado/ghostty",
                "http://example.invalid/ghostty",
                1,
            ),
        ] {
            fs::write(root.join("catalog-v1.json"), invalid).unwrap();
            assert!(
                LicenseCatalog::load(&root)
                    .unwrap_err()
                    .contains("source repository")
            );
        }
        let with_homepage = valid_entry().replace(
            "\"homepage_url\":null",
            "\"homepage_url\":\"https://example.com/sample\"",
        );
        fs::write(root.join("catalog-v1.json"), catalog(&with_homepage)).unwrap();
        assert_eq!(
            LicenseCatalog::load(&root).unwrap().entries[0].homepage_url,
            Some("https://example.com/sample".to_owned())
        );
        let unsafe_homepage = with_homepage.replace("https://example.com/sample", "file:///tmp");
        fs::write(root.join("catalog-v1.json"), catalog(&unsafe_homepage)).unwrap();
        assert!(
            LicenseCatalog::load(&root)
                .unwrap_err()
                .contains("homepage URL")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn notice_file_kind_and_directory_file_shape_are_exact() {
        let root = root("notice-shape");
        let file_entry = valid_entry()
            .replace("\"kind\":\"directory\"", "\"kind\":\"file\"")
            .replace("cargo/sample-1.2.3\"}", "cargo/sample-1.2.3/LICENSE\"}");
        fs::write(root.join("catalog-v1.json"), catalog(&file_entry)).unwrap();
        assert!(
            LicenseCatalog::load(&root).unwrap().entries[0]
                .full_text
                .contains("Sample license")
        );

        let notice = root.join("cargo/sample-1.2.3/LICENSE");
        fs::write(&notice, "").unwrap();
        assert!(
            LicenseCatalog::load(&root)
                .unwrap_err()
                .contains("size is outside")
        );
        let oversized = fs::File::create(&notice).unwrap();
        oversized.set_len(MAX_NOTICE_FILE_BYTES + 1).unwrap();
        assert!(
            LicenseCatalog::load(&root)
                .unwrap_err()
                .contains("size is outside")
        );
        oversized.set_len(MAX_NOTICE_FILE_BYTES).unwrap();
        assert!(LicenseCatalog::load(&root).is_ok());

        fs::write(&notice, "Sample license").unwrap();
        fs::write(root.join("catalog-v1.json"), catalog(valid_entry())).unwrap();
        fs::remove_dir_all(root.join("cargo/sample-1.2.3")).unwrap();
        fs::write(root.join("cargo/sample-1.2.3"), "not a directory").unwrap();
        assert!(
            LicenseCatalog::load(&root)
                .unwrap_err()
                .contains("not a real directory")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn total_notice_bound_accepts_exact_limit_and_rejects_excess_and_overflow() {
        let mut total = 7;
        assert!(add_notice_bytes(&mut total, 3, 10).is_ok());
        assert_eq!(total, 10);
        assert!(
            add_notice_bytes(&mut total, 1, 10)
                .unwrap_err()
                .contains("exceeds")
        );
        let mut overflow = usize::MAX;
        assert!(
            add_notice_bytes(&mut overflow, 1, usize::MAX)
                .unwrap_err()
                .contains("overflowed")
        );
    }
}
