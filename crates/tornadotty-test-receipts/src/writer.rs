use std::env;
use std::fs::{File, OpenOptions, symlink_metadata};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path};
use std::sync::{Mutex, OnceLock};

use crate::parser::ReceiptRecord;
use crate::{
    LifecycleState, MAX_FILE_BYTES, MAX_RECORD_BYTES, MAX_RECORDS, ReceiptErrorKind, ReceiptEvent,
    Result, SCHEMA_VERSION, error,
};

pub const RECEIPT_ENVIRONMENT_VARIABLE: &str = "TORNADOTTY_TEST_RECEIPT_FILE";

static GLOBAL_WRITER: OnceLock<Mutex<Option<ReceiptWriter>>> = OnceLock::new();

#[derive(Debug)]
pub struct ReceiptWriter {
    file: File,
    next_sequence: u64,
    bytes_written: usize,
}

impl ReceiptWriter {
    /// Creates a new owner-only receipt file in a canonical owner-only directory.
    ///
    /// # Errors
    ///
    /// Returns an error when the path is unsafe, already exists, or cannot be
    /// opened as a new mode-0600 regular file.
    pub fn create(path: &Path) -> Result<Self> {
        validate_new_receipt_path(path)?;
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(path)
            .map_err(|cause| {
                error(
                    ReceiptErrorKind::Io,
                    format!("could not create receipt: {cause}"),
                )
            })?;
        let metadata = file.metadata().map_err(|cause| {
            error(
                ReceiptErrorKind::Io,
                format!("could not inspect receipt: {cause}"),
            )
        })?;
        if metadata.uid() != rustix::process::getuid().as_raw()
            || metadata.permissions().mode() & 0o777 != 0o600
        {
            return Err(error(
                ReceiptErrorKind::UnsafePath,
                "created receipt is not an owner-only regular file",
            ));
        }
        Ok(Self {
            file,
            next_sequence: 1,
            bytes_written: 0,
        })
    }

    /// Appends one bounded, validated event and flushes the record boundary.
    ///
    /// # Errors
    ///
    /// Returns an error when the event is invalid or oversized, serialization
    /// fails, or the record cannot be written and flushed.
    pub fn write(&mut self, event: ReceiptEvent) -> Result<()> {
        if self.next_sequence
            > u64::try_from(MAX_RECORDS).map_err(|_| {
                error(
                    ReceiptErrorKind::Oversized,
                    "receipt record limit cannot be represented",
                )
            })?
        {
            return Err(error(
                ReceiptErrorKind::Oversized,
                "receipt stream exceeds 8192 records",
            ));
        }
        event.validate()?;
        let record = ReceiptRecord {
            schema_version: SCHEMA_VERSION,
            sequence: self.next_sequence,
            event,
        };
        let mut encoded = serde_json::to_vec(&record).map_err(|cause| {
            error(
                ReceiptErrorKind::InvalidEvent,
                format!("could not encode receipt event: {cause}"),
            )
        })?;
        if encoded.len() > MAX_RECORD_BYTES {
            return Err(error(
                ReceiptErrorKind::Oversized,
                "encoded receipt event exceeds 8 KiB",
            ));
        }
        encoded.push(b'\n');
        let next_bytes_written = self
            .bytes_written
            .checked_add(encoded.len())
            .filter(|total| *total <= MAX_FILE_BYTES)
            .ok_or_else(|| error(ReceiptErrorKind::Oversized, "receipt stream exceeds 8 MiB"))?;
        self.file.write_all(&encoded).map_err(|cause| {
            error(
                ReceiptErrorKind::Io,
                format!("could not append receipt event: {cause}"),
            )
        })?;
        self.file.flush().map_err(|cause| {
            error(
                ReceiptErrorKind::Io,
                format!("could not flush receipt event: {cause}"),
            )
        })?;
        self.next_sequence += 1;
        self.bytes_written = next_bytes_written;
        Ok(())
    }
}

/// Initializes the process-wide optional writer from
/// [`RECEIPT_ENVIRONMENT_VARIABLE`] and emits `process_started` when active.
///
/// # Errors
///
/// Returns an error for an unsafe receipt path, I/O failure, or repeated
/// initialization in the same process.
pub fn initialize_from_environment() -> Result<bool> {
    let writer = match env::var_os(RECEIPT_ENVIRONMENT_VARIABLE) {
        None => None,
        Some(path) if path.is_empty() => {
            return Err(error(
                ReceiptErrorKind::UnsafePath,
                "receipt environment path is empty",
            ));
        }
        Some(path) => Some(ReceiptWriter::create(Path::new(&path))?),
    };
    GLOBAL_WRITER.set(Mutex::new(writer)).map_err(|_| {
        error(
            ReceiptErrorKind::AlreadyInitialized,
            "global receipt writer",
        )
    })?;
    let global_writer = GLOBAL_WRITER.get().ok_or_else(|| {
        error(
            ReceiptErrorKind::AlreadyInitialized,
            "global receipt writer was not retained",
        )
    })?;
    let active = global_writer
        .lock()
        .map_err(|_| error(ReceiptErrorKind::Io, "receipt writer lock was poisoned"))?
        .is_some();
    if active {
        emit(ReceiptEvent::Lifecycle {
            state: LifecycleState::ProcessStarted,
            pane_id: None,
        })?;
    }
    Ok(active)
}

/// Emits one event when the optional process-wide writer is active.
///
/// # Errors
///
/// Returns an error when the global writer is poisoned or cannot append the
/// validated event.
pub fn emit(event: ReceiptEvent) -> Result<bool> {
    let Some(writer) = GLOBAL_WRITER.get() else {
        return Ok(false);
    };
    let mut writer = writer
        .lock()
        .map_err(|_| error(ReceiptErrorKind::Io, "receipt writer lock was poisoned"))?;
    let Some(writer) = writer.as_mut() else {
        return Ok(false);
    };
    writer.write(event)?;
    Ok(true)
}

/// Emits the terminal `process_stopped` lifecycle record when active.
///
/// # Errors
///
/// Returns an error when the global writer cannot append the record.
pub fn finish() -> Result<bool> {
    emit(ReceiptEvent::Lifecycle {
        state: LifecycleState::ProcessStopped,
        pane_id: None,
    })
}

fn validate_new_receipt_path(path: &Path) -> Result<()> {
    if !path.is_absolute()
        || path.file_name().is_none()
        || path
            .components()
            .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
    {
        return Err(error(
            ReceiptErrorKind::UnsafePath,
            "receipt path must be absolute, normalized, and name one file",
        ));
    }
    let parent = path.parent().ok_or_else(|| {
        error(
            ReceiptErrorKind::UnsafePath,
            "receipt path has no parent directory",
        )
    })?;
    let metadata = symlink_metadata(parent).map_err(|cause| {
        error(
            ReceiptErrorKind::UnsafePath,
            format!("could not inspect receipt directory: {cause}"),
        )
    })?;
    let canonical_parent = parent.canonicalize().map_err(|cause| {
        error(
            ReceiptErrorKind::UnsafePath,
            format!("could not resolve receipt directory: {cause}"),
        )
    })?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || canonical_parent != parent
        || metadata.uid() != rustix::process::getuid().as_raw()
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(error(
            ReceiptErrorKind::UnsafePath,
            "receipt directory must be canonical, owner-controlled, and mode 0700",
        ));
    }
    if symlink_metadata(path).is_ok() {
        return Err(error(
            ReceiptErrorKind::UnsafePath,
            "receipt target must not already exist",
        ));
    }
    Ok(())
}
