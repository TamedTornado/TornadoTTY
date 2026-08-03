use std::error::Error;
use std::fmt;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{Workspace, WorkspaceError};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceStore {
    path: PathBuf,
}

impl WorkspaceStore {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn backup_path(&self) -> PathBuf {
        sibling_with_suffix(&self.path, ".bak")
    }

    /// Loads the primary workspace without modifying it on any failure.
    ///
    /// # Errors
    ///
    /// Returns an I/O error or the strict codec error associated with the
    /// primary path. A missing primary is returned as `Ok(None)`.
    pub fn load(&self) -> Result<Option<Workspace>, WorkspaceStoreError> {
        load_path(&self.path)
    }

    /// Loads the explicit backup without silently replacing the primary.
    ///
    /// # Errors
    ///
    /// Returns an I/O or strict codec error associated with the backup path.
    /// A missing backup is returned as `Ok(None)`.
    pub fn load_backup(&self) -> Result<Option<Workspace>, WorkspaceStoreError> {
        load_path(&self.backup_path())
    }

    /// Atomically replaces the primary and retains the previous complete
    /// primary as a separately atomic backup.
    ///
    /// # Errors
    ///
    /// Returns an error if encoding, lock acquisition, temp creation, write,
    /// file sync, backup, rename, or directory sync fails. A failure before
    /// rename leaves the previous primary untouched.
    pub fn save(&self, workspace: &Workspace) -> Result<(), WorkspaceStoreError> {
        let bytes = workspace
            .to_json()
            .map_err(|source| WorkspaceStoreError::InvalidState {
                path: self.path.clone(),
                source,
            })?;
        self.save_bytes_with_observer(&bytes, |_| Ok(()))
    }

    fn save_bytes_with_observer(
        &self,
        bytes: &[u8],
        mut observe: impl FnMut(WriteStage) -> io::Result<()>,
    ) -> Result<(), WorkspaceStoreError> {
        let parent = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .ok_or_else(|| io_error("resolve parent", &self.path, io::ErrorKind::InvalidInput))?;
        fs::create_dir_all(parent)
            .map_err(|source| io_source("create state directory", parent, source))?;

        let lock_path = sibling_with_suffix(&self.path, ".lock");
        reject_non_regular_existing(&lock_path, "open workspace lock")?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .map_err(|source| io_source("open workspace lock", &lock_path, source))?;
        lock.try_lock().map_err(|source| match source {
            TryLockError::WouldBlock => WorkspaceStoreError::Locked(lock_path.clone()),
            TryLockError::Error(source) => io_source("lock workspace", &lock_path, source),
        })?;

        if self.path.exists() {
            reject_non_regular_existing(&self.path, "read previous workspace")?;
            let previous = fs::read(&self.path)
                .map_err(|source| io_source("read previous workspace", &self.path, source))?;
            atomic_replace(&self.backup_path(), &previous, &mut observe)?;
        }
        atomic_replace(&self.path, bytes, &mut observe)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum WriteStage {
    TempCreated,
    DataWritten,
    DataSynced,
    Renamed,
    DirectorySynced,
}

#[derive(Debug)]
pub enum WorkspaceStoreError {
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    Locked(PathBuf),
    InvalidState {
        path: PathBuf,
        source: WorkspaceError,
    },
}

impl fmt::Display for WorkspaceStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "{operation} failed for {}: {source}",
                path.display()
            ),
            Self::Locked(path) => {
                write!(formatter, "workspace is locked: {}", path.display())
            }
            Self::InvalidState { path, source } => write!(
                formatter,
                "workspace state at {} is invalid: {source}",
                path.display()
            ),
        }
    }
}

impl Error for WorkspaceStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::InvalidState { source, .. } => Some(source),
            Self::Locked(_) => None,
        }
    }
}

fn load_path(path: &Path) -> Result<Option<Workspace>, WorkspaceStoreError> {
    reject_non_regular_existing(path, "read workspace")?;
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(io_source("read workspace", path, source)),
    };
    Workspace::from_json(&bytes)
        .map(Some)
        .map_err(|source| WorkspaceStoreError::InvalidState {
            path: path.to_path_buf(),
            source,
        })
}

fn atomic_replace(
    path: &Path,
    bytes: &[u8],
    observe: &mut impl FnMut(WriteStage) -> io::Result<()>,
) -> Result<(), WorkspaceStoreError> {
    let parent = path
        .parent()
        .filter(|candidate| !candidate.as_os_str().is_empty())
        .ok_or_else(|| io_error("resolve parent", path, io::ErrorKind::InvalidInput))?;
    reject_non_regular_existing(path, "replace workspace file")?;
    let (mut file, temp_path) = create_temp(path)?;
    let mut cleanup = TempCleanup::new(temp_path.clone());
    observe(WriteStage::TempCreated)
        .map_err(|source| io_source("observe temp creation", &temp_path, source))?;
    file.write_all(bytes)
        .map_err(|source| io_source("write workspace temp", &temp_path, source))?;
    observe(WriteStage::DataWritten)
        .map_err(|source| io_source("observe temp write", &temp_path, source))?;
    file.sync_all()
        .map_err(|source| io_source("sync workspace temp", &temp_path, source))?;
    observe(WriteStage::DataSynced)
        .map_err(|source| io_source("observe temp sync", &temp_path, source))?;
    drop(file);
    fs::rename(&temp_path, path)
        .map_err(|source| io_source("rename workspace temp", path, source))?;
    cleanup.disarm();
    observe(WriteStage::Renamed)
        .map_err(|source| io_source("observe workspace rename", path, source))?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| io_source("sync workspace directory", parent, source))?;
    observe(WriteStage::DirectorySynced)
        .map_err(|source| io_source("observe directory sync", parent, source))?;
    Ok(())
}

fn create_temp(path: &Path) -> Result<(File, PathBuf), WorkspaceStoreError> {
    let parent = path
        .parent()
        .filter(|candidate| !candidate.as_os_str().is_empty())
        .ok_or_else(|| io_error("resolve parent", path, io::ErrorKind::InvalidInput))?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| io_error("resolve filename", path, io::ErrorKind::InvalidInput))?;
    for _ in 0..100 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temp_path = parent.join(format!(".{name}.tmp.{}.{sequence}", std::process::id()));
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&temp_path) {
            Ok(file) => return Ok((file, temp_path)),
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {}
            Err(source) => {
                return Err(io_source("create workspace temp", &temp_path, source));
            }
        }
    }
    Err(io_error(
        "create workspace temp",
        path,
        io::ErrorKind::AlreadyExists,
    ))
}

fn reject_non_regular_existing(
    path: &Path,
    operation: &'static str,
) -> Result<(), WorkspaceStoreError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => return Err(io_source(operation, path, source)),
    };
    if metadata.file_type().is_file() {
        Ok(())
    } else {
        Err(io_error(operation, path, io::ErrorKind::InvalidInput))
    }
}

fn sibling_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_owned();
    value.push(suffix);
    PathBuf::from(value)
}

fn io_source(operation: &'static str, path: &Path, source: io::Error) -> WorkspaceStoreError {
    WorkspaceStoreError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

fn io_error(operation: &'static str, path: &Path, kind: io::ErrorKind) -> WorkspaceStoreError {
    io_source(operation, path, io::Error::from(kind))
}

struct TempCleanup {
    path: Option<PathBuf>,
}

impl TempCleanup {
    fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    fn disarm(&mut self) {
        self.path = None;
    }
}

impl Drop for TempCleanup {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Pane, StableId};

    fn id(value: u64) -> StableId {
        StableId::parse(format!("00000000-0000-4000-8000-{value:012x}"))
            .expect("test ID must be valid")
    }

    fn workspace() -> Workspace {
        Workspace::new(
            id(1),
            id(2),
            id(3),
            Pane::new(id(4), "/tmp", "default").unwrap(),
        )
        .unwrap()
    }

    fn test_directory(label: &str) -> PathBuf {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "zentty-store-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn failure_before_rename_preserves_primary_and_removes_temp() {
        let directory = test_directory("before-rename");
        let store = WorkspaceStore::new(directory.join("workspace.json"));
        let original = workspace().to_json().unwrap();
        fs::write(store.path(), &original).unwrap();
        let updated = {
            let mut value = workspace();
            value
                .rename_worklane(&id(2), &id(3), Some("updated".into()))
                .unwrap();
            value.to_json().unwrap()
        };

        let mut synced_files = 0;
        let result = store.save_bytes_with_observer(&updated, |stage| {
            if stage == WriteStage::DataSynced {
                synced_files += 1;
            }
            if synced_files == 2 && stage == WriteStage::DataSynced {
                Err(io::Error::other("injected interruption"))
            } else {
                Ok(())
            }
        });
        assert!(result.is_err());
        assert_eq!(fs::read(store.path()).unwrap(), original);
        assert_eq!(fs::read(store.backup_path()).unwrap(), original);
        assert!(fs::read_dir(&directory).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".tmp.")
        }));
        fs::remove_dir_all(directory).unwrap();
    }
}
