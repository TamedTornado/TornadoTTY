use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use zentty_linux::platform::{UserDirectory, resolve_user_path};

const MAX_AGE: Duration = Duration::from_hours(24);

pub(crate) fn default_directory() -> Result<PathBuf, String> {
    resolve_user_path(
        UserDirectory::Cache,
        std::env::var_os("XDG_CACHE_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
        Path::new("zentty/restore-output"),
    )
}

pub(crate) fn write(
    directory: &Path,
    pane_id: &str,
    text: &str,
) -> Result<Option<PathBuf>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let filename = archive_filename(pane_id)?;
    fs::create_dir_all(directory)
        .map_err(|error| format!("failed to create closed-pane archive directory: {error}"))?;
    purge_stale(directory, SystemTime::now())?;
    let destination = directory.join(filename);
    let temporary = directory.join(format!(".{}.tmp-{}", pane_id, std::process::id()));
    fs::write(&temporary, text)
        .map_err(|error| format!("failed to write closed-pane archive: {error}"))?;
    fs::rename(&temporary, &destination).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("failed to publish closed-pane archive: {error}")
    })?;
    Ok(Some(destination))
}

pub(crate) fn compose_prefill(archive: Option<&Path>, command: Option<&str>) -> Option<String> {
    let mut lines = Vec::new();
    if let Some(archive) = archive {
        lines.push(format!(
            "printf '\\nPrevious output: {}\\n\\n'",
            file_uri(archive)
        ));
    }
    if let Some(command) = command.map(str::trim).filter(|command| !command.is_empty()) {
        lines.push(command.to_owned());
    }
    (!lines.is_empty()).then(|| {
        lines.into_iter().fold(String::new(), |mut prefill, line| {
            writeln!(prefill, "{line}").expect("writing to a String cannot fail");
            prefill
        })
    })
}

fn archive_filename(pane_id: &str) -> Result<String, String> {
    if pane_id.is_empty()
        || !pane_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("closed-pane archive identity is invalid".to_owned());
    }
    Ok(format!("{pane_id}.txt"))
}

fn purge_stale(directory: &Path, now: SystemTime) -> Result<(), String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("failed to inspect closed-pane archives: {error}"))?;
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("failed to inspect closed-pane archive: {error}"))?;
        let metadata = entry
            .metadata()
            .map_err(|error| format!("failed to inspect closed-pane archive metadata: {error}"))?;
        let stale = metadata
            .modified()
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_none_or(|age| age > MAX_AGE);
        if metadata.is_file() && stale {
            fs::remove_file(entry.path())
                .map_err(|error| format!("failed to purge stale closed-pane archive: {error}"))?;
        }
    }
    Ok(())
}

fn file_uri(path: &Path) -> String {
    let bytes = path.as_os_str().as_encoded_bytes();
    let mut uri = String::from("file://");
    for byte in bytes {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~') {
            uri.push(char::from(*byte));
        } else {
            write!(uri, "%{byte:02X}").expect("writing to a String cannot fail");
        }
    }
    uri
}

#[cfg(test)]
mod tests {
    use super::{compose_prefill, purge_stale, write};
    use std::fs::{FileTimes, OpenOptions};
    use std::path::Path;
    use std::time::{Duration, SystemTime};

    #[test]
    fn archive_and_command_prefill_matches_source_order_without_shell_quote_injection() {
        let root = std::env::temp_dir().join(format!(
            "zentty-closed-pane-archive-{}-quote' space",
            std::process::id()
        ));
        let archive = write(&root, "pane-restored", "first\nsecond")
            .unwrap()
            .unwrap();
        assert_eq!(std::fs::read_to_string(&archive).unwrap(), "first\nsecond");
        let expected = format!(
            "printf '\\nPrevious output: file://{}\\n\\n'\ncodex resume safe-session\n",
            archive
                .to_string_lossy()
                .replace(' ', "%20")
                .replace('\'', "%27")
        );
        assert_eq!(
            compose_prefill(Some(&archive), Some("codex resume safe-session")).as_deref(),
            Some(expected.as_str())
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn empty_output_and_invalid_identity_never_publish_an_archive() {
        let root = Path::new("/tmp/not-created-by-empty-closed-pane");
        assert_eq!(write(root, "pane-safe", " \n").unwrap(), None);
        assert!(write(root, "../escape", "text").is_err());
    }

    #[test]
    fn purge_removes_only_regular_files_older_than_the_archive_lifetime() {
        let root =
            std::env::temp_dir().join(format!("zentty-closed-pane-purge-{}", std::process::id()));
        std::fs::create_dir_all(root.join("retained-directory")).unwrap();
        let recent = root.join("recent.txt");
        let stale = root.join("stale.txt");
        std::fs::write(&recent, "recent").unwrap();
        std::fs::write(&stale, "stale").unwrap();
        let now = SystemTime::now();
        OpenOptions::new()
            .write(true)
            .open(&stale)
            .unwrap()
            .set_times(FileTimes::new().set_modified(now - Duration::from_hours(25)))
            .unwrap();

        purge_stale(&root, now).unwrap();
        assert!(recent.is_file());
        assert!(!stale.exists());
        assert!(root.join("retained-directory").is_dir());
        std::fs::remove_dir_all(root).unwrap();
    }
}
