use std::error::Error;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

const LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(5);
const LOCK_DEADLINE: Duration = Duration::from_millis(250);

/// Result requested by one transaction while the adjacent lock is held.
pub enum AtomicFileAction<T> {
    ReadOnly(T),
    Replace { bytes: Vec<u8>, value: T },
    Quarantine(T),
    QuarantineAndReplace { bytes: Vec<u8>, value: T },
}

/// One bounded, atomically replaced file protected by a stable adjacent lock.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtomicFileStore {
    path: PathBuf,
    lock_path: PathBuf,
    max_bytes: usize,
}

impl AtomicFileStore {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>, max_bytes: usize) -> Self {
        let path = path.into();
        let lock_path = path.with_extension("lock");
        Self {
            path,
            lock_path,
            max_bytes,
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }

    /// Reads, validates, and optionally replaces the file under one bounded
    /// cross-process lock acquisition.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe paths, lock contention, I/O failures,
    /// oversize input/output, or a rejected transaction callback.
    pub fn transaction<T>(
        &self,
        update: impl FnOnce(Option<&[u8]>) -> Result<AtomicFileAction<T>, String>,
    ) -> Result<(T, Option<PathBuf>), AtomicFileStoreError> {
        let parent = self.parent()?;
        prepare_private_directory(parent)?;
        reject_symlink(&self.path)?;
        reject_symlink(&self.lock_path)?;
        let lock = open_private_lock(&self.lock_path)?;
        acquire_lock(&lock, &self.lock_path)?;
        let bytes = read_bounded(&self.path, self.max_bytes)?;
        let action = update(bytes.as_deref()).map_err(AtomicFileStoreError::Transaction)?;
        let result = match action {
            AtomicFileAction::ReadOnly(value) => (value, None),
            AtomicFileAction::Replace { bytes, value } => {
                self.replace(&bytes)?;
                (value, None)
            }
            AtomicFileAction::Quarantine(value) => {
                let quarantine = self.quarantine()?;
                (value, quarantine)
            }
            AtomicFileAction::QuarantineAndReplace { bytes, value } => {
                let quarantine = self.quarantine()?;
                self.replace(&bytes)?;
                (value, quarantine)
            }
        };
        drop(lock);
        Ok(result)
    }

    /// Replaces the file without decoding or reading its prior contents.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe paths, lock contention, I/O failures, or
    /// oversize output.
    pub fn replace_bytes(&self, bytes: &[u8]) -> Result<(), AtomicFileStoreError> {
        let parent = self.parent()?;
        prepare_private_directory(parent)?;
        reject_symlink(&self.path)?;
        reject_symlink(&self.lock_path)?;
        let lock = open_private_lock(&self.lock_path)?;
        acquire_lock(&lock, &self.lock_path)?;
        self.replace(bytes)?;
        drop(lock);
        Ok(())
    }

    fn parent(&self) -> Result<&Path, AtomicFileStoreError> {
        self.path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| invalid_path("resolve parent", &self.path))
    }

    fn replace(&self, bytes: &[u8]) -> Result<(), AtomicFileStoreError> {
        if bytes.len() > self.max_bytes {
            return Err(AtomicFileStoreError::LimitExceeded {
                path: self.path.clone(),
                max_bytes: self.max_bytes,
            });
        }
        let parent = self.parent()?;
        let (mut file, temp_path) = create_private_temp(&self.path)?;
        let mut cleanup = TempCleanup(Some(temp_path.clone()));
        file.write_all(bytes)
            .map_err(|source| io_error("write temporary file", &temp_path, source))?;
        file.sync_all()
            .map_err(|source| io_error("sync temporary file", &temp_path, source))?;
        drop(file);
        fs::rename(&temp_path, &self.path)
            .map_err(|source| io_error("replace file", &self.path, source))?;
        cleanup.disarm();
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|source| io_error("sync parent directory", parent, source))
    }

    fn quarantine(&self) -> Result<Option<PathBuf>, AtomicFileStoreError> {
        if !self.path.exists() {
            return Ok(None);
        }
        let file_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| invalid_path("resolve filename", &self.path))?;
        let parent = self.parent()?;
        for _ in 0..100 {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let quarantine = parent.join(format!(
                "{file_name}.corrupt.{}.{sequence}",
                std::process::id()
            ));
            match fs::hard_link(&self.path, &quarantine) {
                Ok(()) => match fs::remove_file(&self.path) {
                    Ok(()) => return Ok(Some(quarantine)),
                    Err(source) => {
                        let _ = fs::remove_file(&quarantine);
                        return Err(io_error("remove quarantined file", &self.path, source));
                    }
                },
                Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {}
                Err(source) => {
                    return Err(io_error("quarantine existing file", &self.path, source));
                }
            }
        }
        Err(io_error(
            "allocate quarantine path",
            &self.path,
            io::Error::from(io::ErrorKind::AlreadyExists),
        ))
    }
}

#[derive(Debug)]
pub enum AtomicFileStoreError {
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    LimitExceeded {
        path: PathBuf,
        max_bytes: usize,
    },
    LockTimeout {
        path: PathBuf,
    },
    Symlink {
        path: PathBuf,
    },
    Transaction(String),
}

impl fmt::Display for AtomicFileStoreError {
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
            Self::LimitExceeded { path, max_bytes } => write!(
                formatter,
                "file exceeds {max_bytes}-byte limit: {}",
                path.display()
            ),
            Self::LockTimeout { path } => {
                write!(
                    formatter,
                    "timed out acquiring file lock: {}",
                    path.display()
                )
            }
            Self::Symlink { path } => {
                write!(
                    formatter,
                    "refusing symlinked file boundary: {}",
                    path.display()
                )
            }
            Self::Transaction(message) => formatter.write_str(message),
        }
    }
}

impl Error for AtomicFileStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::LimitExceeded { .. }
            | Self::LockTimeout { .. }
            | Self::Symlink { .. }
            | Self::Transaction(_) => None,
        }
    }
}

fn prepare_private_directory(path: &Path) -> Result<(), AtomicFileStoreError> {
    reject_existing_ancestor_symlinks(path)?;
    fs::create_dir_all(path).map_err(|source| io_error("create parent directory", path, source))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|source| io_error("set parent permissions", path, source))?;
    }
    Ok(())
}

fn reject_existing_ancestor_symlinks(path: &Path) -> Result<(), AtomicFileStoreError> {
    for ancestor in path.ancestors() {
        reject_symlink(ancestor)?;
    }
    Ok(())
}

fn reject_symlink(path: &Path) -> Result<(), AtomicFileStoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(AtomicFileStoreError::Symlink { path: path.into() })
        }
        Ok(_) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(io_error("inspect path", path, source)),
    }
}

fn open_private_lock(path: &Path) -> Result<File, AtomicFileStoreError> {
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(path)
        .map_err(|source| io_error("open lock", path, source))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|source| io_error("set lock permissions", path, source))?;
    }
    reject_symlink(path)?;
    Ok(file)
}

fn acquire_lock(file: &File, path: &Path) -> Result<(), AtomicFileStoreError> {
    let deadline = Instant::now() + LOCK_DEADLINE;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(()),
            Err(std::fs::TryLockError::WouldBlock) => {
                if Instant::now() >= deadline {
                    return Err(AtomicFileStoreError::LockTimeout { path: path.into() });
                }
                thread::sleep(LOCK_RETRY_INTERVAL);
            }
            Err(std::fs::TryLockError::Error(source)) => {
                return Err(io_error("lock", path, source));
            }
        }
    }
}

fn read_bounded(path: &Path, max_bytes: usize) -> Result<Option<Vec<u8>>, AtomicFileStoreError> {
    reject_symlink(path)?;
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(io_error("open file", path, source)),
    };
    reject_symlink(path)?;
    let metadata = file
        .metadata()
        .map_err(|source| io_error("inspect file", path, source))?;
    if !metadata.is_file() {
        return Err(invalid_path("validate regular file", path));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|source| io_error("set file permissions", path, source))?;
    }
    let max_bytes_u64 = u64::try_from(max_bytes).unwrap_or(u64::MAX);
    if metadata.len() > max_bytes_u64 {
        return Err(AtomicFileStoreError::LimitExceeded {
            path: path.into(),
            max_bytes,
        });
    }
    let capacity =
        usize::try_from(metadata.len()).map_err(|_| AtomicFileStoreError::LimitExceeded {
            path: path.into(),
            max_bytes,
        })?;
    let mut bytes = Vec::with_capacity(capacity);
    Read::by_ref(&mut file)
        .take(max_bytes_u64.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| io_error("read file", path, source))?;
    if bytes.len() > max_bytes {
        return Err(AtomicFileStoreError::LimitExceeded {
            path: path.into(),
            max_bytes,
        });
    }
    Ok(Some(bytes))
}

fn create_private_temp(path: &Path) -> Result<(File, PathBuf), AtomicFileStoreError> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid_path("resolve parent", path))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid_path("resolve filename", path))?;
    for _ in 0..100 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temp_path = parent.join(format!(
            ".{file_name}.tmp.{}.{sequence}",
            std::process::id()
        ));
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
            Err(source) => return Err(io_error("create temporary file", &temp_path, source)),
        }
    }
    Err(io_error(
        "create temporary file",
        path,
        io::Error::from(io::ErrorKind::AlreadyExists),
    ))
}

fn io_error(operation: &'static str, path: &Path, source: io::Error) -> AtomicFileStoreError {
    AtomicFileStoreError::Io {
        operation,
        path: path.into(),
        source,
    }
}

fn invalid_path(operation: &'static str, path: &Path) -> AtomicFileStoreError {
    io_error(
        operation,
        path,
        io::Error::from(io::ErrorKind::InvalidInput),
    )
}

struct TempCleanup(Option<PathBuf>);

impl TempCleanup {
    fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for TempCleanup {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = fs::remove_file(path);
        }
    }
}
