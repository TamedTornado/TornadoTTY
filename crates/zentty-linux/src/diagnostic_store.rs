use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use zentty_core::{DIAGNOSTIC_SCHEMA_VERSION, DiagnosticReport, DiagnosticState};

const MAX_REPORT_BYTES: u64 = 64 * 1024;
const MAX_REPORT_COUNT: usize = 5;
const MAX_REPORT_AGE_SECONDS: u64 = 30 * 24 * 60 * 60;

#[derive(Clone, Debug)]
pub(crate) struct DiagnosticStore {
    root: PathBuf,
}

impl DiagnosticStore {
    pub(crate) fn new(root: impl Into<PathBuf>) -> Result<Self, String> {
        let root = root.into();
        fs::create_dir_all(&root)
            .map_err(|error| format!("could not create {}: {error}", root.display()))?;
        let metadata = fs::symlink_metadata(&root)
            .map_err(|error| format!("could not inspect {}: {error}", root.display()))?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err("diagnostic store must be a real private directory".to_owned());
        }
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("could not protect {}: {error}", root.display()))?;
        Ok(Self { root })
    }

    pub(crate) fn save(&self, report: &DiagnosticReport) -> Result<PathBuf, String> {
        if report.schema_version != DIAGNOSTIC_SCHEMA_VERSION {
            return Err("unsupported diagnostic schema version".to_owned());
        }
        if report.state == DiagnosticState::Cleared {
            return Err("cleared diagnostic reports cannot be persisted".to_owned());
        }
        let bytes = serde_json::to_vec_pretty(report)
            .map_err(|error| format!("could not encode diagnostic report: {error}"))?;
        if !report_size_allowed(bytes.len() as u64) {
            return Err(format!(
                "diagnostic report exceeds {MAX_REPORT_BYTES} bytes"
            ));
        }
        let path = self.report_path(&report.report_id)?;
        atomic_private_replace(&path, &bytes)?;
        Ok(path)
    }

    pub(crate) fn list(&self) -> Result<Vec<DiagnosticReport>, String> {
        let mut reports = Vec::new();
        for path in self.report_paths()? {
            match read_report(&path) {
                Ok(report) => reports.push(report),
                Err(error) => eprintln!(
                    "zentty-linux: diagnostic-store ignored={} detail={error}",
                    path.display()
                ),
            }
        }
        reports.sort_by(|left, right| {
            right
                .created_at_epoch
                .cmp(&left.created_at_epoch)
                .then_with(|| right.report_id.cmp(&left.report_id))
        });
        Ok(reports)
    }

    pub(crate) fn transition(
        &self,
        report_id: &str,
        state: DiagnosticState,
    ) -> Result<DiagnosticReport, String> {
        let path = self.report_path(report_id)?;
        let mut report = read_report(&path)?;
        report.transition(state).map_err(str::to_owned)?;
        if state == DiagnosticState::Cleared {
            remove_if_exists(&path)?;
        } else {
            self.save(&report)?;
        }
        Ok(report)
    }

    pub(crate) fn clear(&self) -> Result<usize, String> {
        let paths = self.report_paths()?;
        for path in &paths {
            remove_if_exists(path)?;
        }
        self.remove_temporary_files()?;
        Ok(paths.len())
    }

    pub(crate) fn prune(&self, now_epoch: u64) -> Result<usize, String> {
        self.remove_temporary_files()?;
        let mut reports = Vec::new();
        let mut removed = 0;
        for path in self.report_paths()? {
            match read_report(&path) {
                Ok(report)
                    if now_epoch.saturating_sub(report.created_at_epoch)
                        <= MAX_REPORT_AGE_SECONDS =>
                {
                    reports.push((path, report));
                }
                Ok(_) | Err(_) => {
                    remove_if_exists(&path)?;
                    removed += 1;
                }
            }
        }
        reports.sort_by(|(_, left), (_, right)| {
            right
                .created_at_epoch
                .cmp(&left.created_at_epoch)
                .then_with(|| right.report_id.cmp(&left.report_id))
        });
        for (path, _) in reports.into_iter().skip(MAX_REPORT_COUNT) {
            remove_if_exists(&path)?;
            removed += 1;
        }
        Ok(removed)
    }

    fn report_path(&self, report_id: &str) -> Result<PathBuf, String> {
        if report_id.is_empty() {
            return Err("invalid diagnostic report ID".to_owned());
        }
        if !report_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        {
            return Err("invalid diagnostic report ID".to_owned());
        }
        Ok(self.root.join(format!("diagnostic-{report_id}.json")))
    }

    fn report_paths(&self) -> Result<Vec<PathBuf>, String> {
        let entries = fs::read_dir(&self.root)
            .map_err(|error| format!("could not read {}: {error}", self.root.display()))?;
        Ok(entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("diagnostic-"))
            })
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
            })
            .collect())
    }

    fn remove_temporary_files(&self) -> Result<(), String> {
        for entry in fs::read_dir(&self.root)
            .map_err(|error| format!("could not read {}: {error}", self.root.display()))?
        {
            let path = entry
                .map_err(|error| format!("could not inspect diagnostic store: {error}"))?
                .path();
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(".diagnostic-") && name.contains(".tmp-"))
            {
                remove_if_exists(&path)?;
            }
        }
        Ok(())
    }
}

fn read_report(path: &Path) -> Result<DiagnosticReport, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
    if !metadata.is_file() || !report_size_allowed(metadata.len()) {
        return Err("diagnostic report is not a bounded regular file".to_owned());
    }
    let bytes =
        fs::read(path).map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let report: DiagnosticReport = serde_json::from_slice(&bytes)
        .map_err(|error| format!("malformed diagnostic report: {error}"))?;
    if report.schema_version != DIAGNOSTIC_SCHEMA_VERSION {
        return Err("unsupported diagnostic schema version".to_owned());
    }
    Ok(report)
}

fn report_size_allowed(size: u64) -> bool {
    size <= MAX_REPORT_BYTES
}

fn atomic_private_replace(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "diagnostic report has no parent directory".to_owned())?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "diagnostic report path is not UTF-8".to_owned())?;
    let temporary = parent.join(format!(".{name}.tmp-{}", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|error| format!("could not create {}: {error}", temporary.display()))?;
    let result = (|| {
        file.write_all(bytes)
            .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;
        file.sync_all()
            .map_err(|error| format!("could not sync {}: {error}", temporary.display()))?;
        fs::rename(&temporary, path)
            .map_err(|error| format!("could not replace {}: {error}", path.display()))?;
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("could not sync {}: {error}", parent.display()))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn remove_if_exists(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("could not remove {}: {error}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread;

    use zentty_core::{DiagnosticDraft, DiagnosticReason};

    use super::*;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

    fn temporary_directory() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "zentty-diagnostic-store-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        path
    }

    fn report(id: &str, created_at_epoch: u64) -> DiagnosticReport {
        DiagnosticReport::from_draft(&DiagnosticDraft {
            report_id: id,
            created_at_epoch,
            reason: DiagnosticReason::ManualSupport,
            application_version: "1.0.0",
            build_commit: "abc123",
            platform: "linux-x86_64",
            detail: "render failed",
            context: &BTreeMap::new(),
            home_directory: None,
        })
    }

    #[test]
    fn store_is_private_atomic_and_requires_review_before_sent() {
        let root = temporary_directory();
        let store = DiagnosticStore::new(&root).unwrap();
        let path = store.save(&report("one", 10)).unwrap();
        assert_eq!(
            fs::metadata(&root).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(store.transition("one", DiagnosticState::Sent).is_err());
        store
            .transition("one", DiagnosticState::PendingReview)
            .unwrap();
        store.transition("one", DiagnosticState::Sent).unwrap();
        assert_eq!(store.list().unwrap()[0].state, DiagnosticState::Sent);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn retention_removes_old_excess_malformed_and_interrupted_files() {
        let root = temporary_directory();
        let store = DiagnosticStore::new(&root).unwrap();
        for index in 0..8 {
            store
                .save(&report(&format!("report-{index}"), 1_000 + index))
                .unwrap();
        }
        fs::write(root.join("diagnostic-malformed.json"), b"{").unwrap();
        fs::write(root.join(".diagnostic-interrupted.json.tmp-1"), b"partial").unwrap();
        assert_eq!(store.prune(1_008).unwrap(), 4);
        assert_eq!(store.list().unwrap().len(), MAX_REPORT_COUNT);
        assert!(!root.join(".diagnostic-interrupted.json.tmp-1").exists());
        fs::remove_dir_all(root).unwrap();

        let age_root = temporary_directory();
        let age_store = DiagnosticStore::new(&age_root).unwrap();
        age_store.save(&report("old", 1)).unwrap();
        assert_eq!(age_store.prune(MAX_REPORT_AGE_SECONDS + 2).unwrap(), 1);
        assert!(age_store.list().unwrap().is_empty());
        fs::remove_dir_all(age_root).unwrap();
    }

    #[test]
    fn malformed_and_oversized_state_does_not_hide_valid_reports() {
        let root = temporary_directory();
        let store = DiagnosticStore::new(&root).unwrap();
        store.save(&report("valid", 10)).unwrap();
        fs::write(root.join("diagnostic-bad.json"), b"not json").unwrap();
        fs::write(root.join("other.json"), b"not a report").unwrap();
        fs::write(root.join("diagnostic-note.txt"), b"not a report").unwrap();
        fs::write(
            root.join("diagnostic-large.json"),
            vec![b'x'; usize::try_from(MAX_REPORT_BYTES).unwrap() + 1],
        )
        .unwrap();
        let reports = store.list().unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].report_id, "valid");
        assert_eq!(store.clear().unwrap(), 3);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn concurrent_writers_publish_complete_reports_without_loss() {
        let root = temporary_directory();
        let store = DiagnosticStore::new(&root).unwrap();
        let writers: Vec<_> = (0..MAX_REPORT_COUNT)
            .map(|index| {
                let store = store.clone();
                thread::spawn(move || {
                    store
                        .save(&report(&format!("concurrent-{index}"), 100 + index as u64))
                        .unwrap();
                })
            })
            .collect();
        for writer in writers {
            writer.join().unwrap();
        }
        let reports = store.list().unwrap();
        assert_eq!(reports.len(), MAX_REPORT_COUNT);
        assert!(
            reports
                .iter()
                .all(|report| report.detail == "render failed")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn store_and_reports_reject_symlinks() {
        use std::os::unix::fs::symlink;

        let root = temporary_directory();
        let target = root.with_extension("target");
        fs::create_dir_all(&target).unwrap();
        symlink(&target, &root).unwrap();
        assert!(DiagnosticStore::new(&root).is_err());
        fs::remove_file(&root).unwrap();

        let store = DiagnosticStore::new(&root).unwrap();
        let outside = root.with_extension("outside");
        fs::write(
            &outside,
            serde_json::to_vec(&report("outside", 10)).unwrap(),
        )
        .unwrap();
        symlink(&outside, root.join("diagnostic-linked.json")).unwrap();
        assert!(store.list().unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(target).unwrap();
        fs::remove_file(outside).unwrap();
    }

    #[test]
    fn size_and_identifier_boundaries_are_exact() {
        assert!(report_size_allowed(MAX_REPORT_BYTES));
        assert!(!report_size_allowed(MAX_REPORT_BYTES + 1));
        let root = temporary_directory();
        let store = DiagnosticStore::new(&root).unwrap();
        assert!(store.report_path("").is_err());
        assert!(store.report_path("bad/id").is_err());
        assert!(store.report_path("good-id_1").is_ok());
        fs::remove_dir_all(root).unwrap();
    }
}
