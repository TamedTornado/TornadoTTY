use std::error::Error;
use std::fmt;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{FirstRunSpec, StableIdSource, Workspace, WorkspaceError, WorkspaceLoad};

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

    /// Loads an existing workspace or atomically creates the documented
    /// first-run topology only when the primary is absent.
    ///
    /// A malformed, unsupported, unreadable, or non-regular primary never
    /// falls back to first-run creation.
    ///
    /// # Errors
    ///
    /// Returns the existing load error, a first-run construction error, or an
    /// atomic-save error. No primary is published when construction fails.
    pub fn load_or_create(
        &self,
        source: &mut impl StableIdSource,
        spec: &FirstRunSpec,
    ) -> Result<WorkspaceLoad, WorkspaceStoreError> {
        if let Some(workspace) = self.load()? {
            return Ok(WorkspaceLoad::Existing(workspace));
        }
        let workspace = Workspace::first_run(source, spec).map_err(|source| {
            WorkspaceStoreError::InvalidState {
                path: self.path.clone(),
                source,
            }
        })?;
        self.save(&workspace)?;
        Ok(WorkspaceLoad::Created(workspace))
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
        self.save_bytes_with_operations(&bytes, &mut RealAtomicOperations)
    }

    fn save_bytes_with_operations(
        &self,
        bytes: &[u8],
        operations: &mut impl AtomicOperations,
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
            atomic_replace(&self.backup_path(), &previous, operations)?;
        }
        atomic_replace(&self.path, bytes, operations)
    }
}

trait AtomicOperations {
    fn write_all(&mut self, file: &mut File, bytes: &[u8]) -> io::Result<()>;
    fn sync_file(&mut self, file: &File) -> io::Result<()>;
    fn rename(&mut self, source: &Path, destination: &Path) -> io::Result<()>;
    fn sync_directory(&mut self, path: &Path) -> io::Result<()>;
}

struct RealAtomicOperations;

impl AtomicOperations for RealAtomicOperations {
    fn write_all(&mut self, file: &mut File, bytes: &[u8]) -> io::Result<()> {
        file.write_all(bytes)
    }

    fn sync_file(&mut self, file: &File) -> io::Result<()> {
        file.sync_all()
    }

    fn rename(&mut self, source: &Path, destination: &Path) -> io::Result<()> {
        fs::rename(source, destination)
    }

    fn sync_directory(&mut self, path: &Path) -> io::Result<()> {
        File::open(path)?.sync_all()
    }
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
    operations: &mut impl AtomicOperations,
) -> Result<(), WorkspaceStoreError> {
    let parent = path
        .parent()
        .filter(|candidate| !candidate.as_os_str().is_empty())
        .ok_or_else(|| io_error("resolve parent", path, io::ErrorKind::InvalidInput))?;
    reject_non_regular_existing(path, "replace workspace file")?;
    let (mut file, temp_path) = create_temp(path)?;
    let mut cleanup = TempCleanup::new(temp_path.clone());
    operations
        .write_all(&mut file, bytes)
        .map_err(|source| io_source("write workspace temp", &temp_path, source))?;
    operations
        .sync_file(&file)
        .map_err(|source| io_source("sync workspace temp", &temp_path, source))?;
    drop(file);
    operations
        .rename(&temp_path, path)
        .map_err(|source| io_source("rename workspace temp", path, source))?;
    cleanup.disarm();
    operations
        .sync_directory(parent)
        .map_err(|source| io_source("sync workspace directory", parent, source))?;
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

    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    enum FaultOperation {
        Write,
        FileSync,
        Rename,
        DirectorySync,
    }

    struct FailingAtomicOperations {
        target: FaultOperation,
        fail_on: usize,
        matching_calls: usize,
        real: RealAtomicOperations,
    }

    impl FailingAtomicOperations {
        fn new(target: FaultOperation, fail_on: usize) -> Self {
            Self {
                target,
                fail_on,
                matching_calls: 0,
                real: RealAtomicOperations,
            }
        }

        fn fail_now(&mut self, operation: FaultOperation) -> io::Result<()> {
            if self.target != operation {
                return Ok(());
            }
            self.matching_calls += 1;
            if self.matching_calls == self.fail_on {
                Err(io::Error::other(format!(
                    "injected {operation:?} failure {}",
                    self.fail_on
                )))
            } else {
                Ok(())
            }
        }
    }

    impl AtomicOperations for FailingAtomicOperations {
        fn write_all(&mut self, file: &mut File, bytes: &[u8]) -> io::Result<()> {
            self.fail_now(FaultOperation::Write)?;
            self.real.write_all(file, bytes)
        }

        fn sync_file(&mut self, file: &File) -> io::Result<()> {
            self.fail_now(FaultOperation::FileSync)?;
            self.real.sync_file(file)
        }

        fn rename(&mut self, source: &Path, destination: &Path) -> io::Result<()> {
            self.fail_now(FaultOperation::Rename)?;
            self.real.rename(source, destination)
        }

        fn sync_directory(&mut self, path: &Path) -> io::Result<()> {
            self.fail_now(FaultOperation::DirectorySync)?;
            self.real.sync_directory(path)
        }
    }

    fn assert_no_temp_files(directory: &Path) {
        assert!(fs::read_dir(directory).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".tmp.")
        }));
    }

    #[test]
    fn exact_atomic_operation_failures_preserve_a_complete_recoverable_state() {
        for operation in [
            FaultOperation::Write,
            FaultOperation::FileSync,
            FaultOperation::Rename,
            FaultOperation::DirectorySync,
        ] {
            for fail_on in 1..=2 {
                let directory = test_directory(&format!("{operation:?}-{fail_on}"));
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
                let result = store.save_bytes_with_operations(
                    &updated,
                    &mut FailingAtomicOperations::new(operation, fail_on),
                );
                assert!(
                    matches!(result, Err(WorkspaceStoreError::Io { .. })),
                    "{operation:?} call {fail_on} unexpectedly succeeded"
                );

                let primary = fs::read(store.path()).unwrap();
                if operation == FaultOperation::DirectorySync && fail_on == 2 {
                    assert_eq!(primary, updated, "renamed primary must be complete");
                } else {
                    assert_eq!(primary, original, "old primary must remain complete");
                }
                if store.backup_path().exists() {
                    assert_eq!(fs::read(store.backup_path()).unwrap(), original);
                }
                assert_no_temp_files(&directory);
                fs::remove_dir_all(directory).unwrap();
            }
        }
    }
}
