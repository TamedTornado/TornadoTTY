use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::atomic_file_store::{AtomicFileStore, AtomicFileStoreError};
use crate::{SaveReason, SessionRestoreDraftWindow, SessionRestoreEnvelope, WorkspaceRecipe};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionRestoreStore {
    snapshot_path: PathBuf,
    lifecycle_path: PathBuf,
}

impl SessionRestoreStore {
    #[must_use]
    pub fn new(snapshot_path: impl Into<PathBuf>, lifecycle_path: impl Into<PathBuf>) -> Self {
        Self {
            snapshot_path: snapshot_path.into(),
            lifecycle_path: lifecycle_path.into(),
        }
    }

    #[must_use]
    pub fn snapshot_path(&self) -> &Path {
        &self.snapshot_path
    }

    #[must_use]
    pub fn lifecycle_path(&self) -> &Path {
        &self.lifecycle_path
    }

    /// Selects the same launch behavior as `SessionRestoreStore` in `ZenTTY`.
    /// An unclean prior lifecycle restores even when normal restoration is
    /// disabled; a clean or absent lifecycle respects the preference.
    ///
    /// # Errors
    ///
    /// Returns an I/O or decoding error without changing either file.
    pub fn prepare_for_launch(
        &self,
        restore_preference_enabled: bool,
    ) -> Result<Option<LaunchDecision>, SessionRestoreStoreError> {
        let Some(envelope) = load_json(&self.snapshot_path)? else {
            return Ok(None);
        };
        let lifecycle: Option<LifecycleState> = load_json(&self.lifecycle_path)?;
        if lifecycle.is_some_and(|state| !state.clean_exit) {
            return Ok(Some(LaunchDecision {
                reason: LaunchReason::CrashRecovery,
                envelope,
            }));
        }
        if restore_preference_enabled {
            Ok(Some(LaunchDecision {
                reason: LaunchReason::NormalRestore,
                envelope,
            }))
        } else {
            Ok(None)
        }
    }

    /// Atomically marks the current launch as unclean until orderly shutdown.
    ///
    /// # Errors
    ///
    /// Returns an I/O or encoding error from lifecycle persistence.
    pub fn mark_launch_started(&self, updated_at: f64) -> Result<(), SessionRestoreStoreError> {
        self.save_lifecycle(false, updated_at)
    }

    /// Atomically marks the current launch as cleanly terminated.
    ///
    /// # Errors
    ///
    /// Returns an I/O or encoding error from lifecycle persistence.
    pub fn mark_clean_exit(&self, updated_at: f64) -> Result<(), SessionRestoreStoreError> {
        self.save_lifecycle(true, updated_at)
    }

    /// Atomically persists a session snapshot. A clean-exit snapshot retains
    /// missing live restore drafts only for panes still present in the new
    /// workspace, matching `ZenTTY`'s merge behavior.
    ///
    /// # Errors
    ///
    /// Returns an I/O or JSON error. A failure before rename leaves the prior
    /// snapshot untouched.
    pub fn save_snapshot(
        &self,
        envelope: &SessionRestoreEnvelope,
    ) -> Result<(), SessionRestoreStoreError> {
        let envelope = if envelope.reason == SaveReason::CleanExit {
            let previous = load_json(&self.snapshot_path)?;
            merge_missing_restore_drafts(envelope.clone(), previous.as_ref())
        } else {
            envelope.clone()
        };
        atomic_write_json(&self.snapshot_path, &envelope)
    }

    /// Removes a consumed or disabled snapshot when present.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if an existing snapshot cannot be removed.
    pub fn consume_snapshot(&self) -> Result<(), SessionRestoreStoreError> {
        match fs::remove_file(&self.snapshot_path) {
            Ok(()) => Ok(()),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(io_error("remove snapshot", &self.snapshot_path, source)),
        }
    }

    fn save_lifecycle(
        &self,
        clean_exit: bool,
        updated_at: f64,
    ) -> Result<(), SessionRestoreStoreError> {
        atomic_write_json(
            &self.lifecycle_path,
            &LifecycleState {
                clean_exit,
                updated_at,
            },
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LaunchDecision {
    pub reason: LaunchReason,
    pub envelope: SessionRestoreEnvelope,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchReason {
    NormalRestore,
    CrashRecovery,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PersistenceRequest {
    None,
    SaveSnapshot(SessionRestoreEnvelope),
    DeleteSnapshot,
}

#[derive(Clone, Debug)]
pub struct SnapshotPersistence {
    store: SessionRestoreStore,
    latest_accepted_generation: u64,
}

impl SnapshotPersistence {
    #[must_use]
    pub const fn new(store: SessionRestoreStore) -> Self {
        Self {
            store,
            latest_accepted_generation: 0,
        }
    }

    #[must_use]
    pub const fn store(&self) -> &SessionRestoreStore {
        &self.store
    }

    /// Applies a request only when it is not older than the newest accepted
    /// generation. The Linux UI queue owns asynchronous scheduling; this core
    /// type owns the deterministic stale-generation rule.
    ///
    /// # Errors
    ///
    /// Returns the underlying snapshot-store error.
    pub fn persist(
        &mut self,
        request: PersistenceRequest,
        generation: u64,
    ) -> Result<bool, SessionRestoreStoreError> {
        if generation < self.latest_accepted_generation {
            return Ok(false);
        }
        self.latest_accepted_generation = generation;
        match request {
            PersistenceRequest::None => {}
            PersistenceRequest::SaveSnapshot(envelope) => self.store.save_snapshot(&envelope)?,
            PersistenceRequest::DeleteSnapshot => self.store.consume_snapshot()?,
        }
        Ok(true)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LifecycleState {
    clean_exit: bool,
    updated_at: f64,
}

#[derive(Debug)]
pub enum SessionRestoreStoreError {
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    Json {
        operation: &'static str,
        path: PathBuf,
        source: serde_json::Error,
    },
}

impl fmt::Display for SessionRestoreStoreError {
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
            Self::Json {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "{operation} failed for {}: {source}",
                path.display()
            ),
        }
    }
}

impl Error for SessionRestoreStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json { source, .. } => Some(source),
        }
    }
}

fn load_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, SessionRestoreStoreError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(io_error("read JSON", path, source)),
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|source| json_error("decode JSON", path, source))
}

fn atomic_write_json(path: &Path, value: &impl Serialize) -> Result<(), SessionRestoreStoreError> {
    let bytes =
        serde_json::to_vec(value).map_err(|source| json_error("encode JSON", path, source))?;
    AtomicFileStore::new(path, usize::MAX)
        .replace_bytes(&bytes)
        .map_err(|error| atomic_store_error(path, error))
}

fn atomic_store_error(path: &Path, error: AtomicFileStoreError) -> SessionRestoreStoreError {
    match error {
        AtomicFileStoreError::Io {
            operation,
            path: error_path,
            source,
        } => io_error(
            if operation == "replace file" {
                "replace JSON"
            } else {
                "persist JSON"
            },
            &error_path,
            source,
        ),
        error => io_error("persist JSON", path, io::Error::other(error)),
    }
}

fn merge_missing_restore_drafts(
    mut current: SessionRestoreEnvelope,
    previous: Option<&SessionRestoreEnvelope>,
) -> SessionRestoreEnvelope {
    let Some(previous) = previous else {
        return current;
    };
    let valid_panes = pane_ids_by_window(&current.workspace);
    let mut existing: BTreeMap<String, BTreeSet<String>> = current
        .restore_draft_windows
        .iter()
        .map(|window| {
            (
                window.window_id.clone(),
                window
                    .pane_drafts
                    .iter()
                    .map(|draft| draft.pane_id.clone())
                    .collect(),
            )
        })
        .collect();

    for previous_window in &previous.restore_draft_windows {
        let Some(valid_pane_ids) = valid_panes.get(&previous_window.window_id) else {
            continue;
        };
        let existing_pane_ids = existing
            .entry(previous_window.window_id.clone())
            .or_default();
        let missing: Vec<_> = previous_window
            .pane_drafts
            .iter()
            .filter(|draft| {
                valid_pane_ids.contains(&draft.pane_id)
                    && !existing_pane_ids.contains(&draft.pane_id)
            })
            .cloned()
            .collect();
        if missing.is_empty() {
            continue;
        }
        existing_pane_ids.extend(missing.iter().map(|draft| draft.pane_id.clone()));
        if let Some(window) = current
            .restore_draft_windows
            .iter_mut()
            .find(|window| window.window_id == previous_window.window_id)
        {
            window.pane_drafts.extend(missing);
        } else {
            current
                .restore_draft_windows
                .push(SessionRestoreDraftWindow {
                    window_id: previous_window.window_id.clone(),
                    pane_drafts: missing,
                });
        }
    }
    current
}

fn pane_ids_by_window(workspace: &WorkspaceRecipe) -> BTreeMap<String, BTreeSet<String>> {
    workspace
        .windows
        .iter()
        .map(|window| {
            let panes = window
                .worklanes
                .iter()
                .flat_map(|worklane| &worklane.columns)
                .flat_map(|column| &column.panes)
                .map(|pane| pane.id.clone())
                .collect();
            (window.id.clone(), panes)
        })
        .collect()
}

fn io_error(operation: &'static str, path: &Path, source: io::Error) -> SessionRestoreStoreError {
    SessionRestoreStoreError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

fn json_error(
    operation: &'static str,
    path: &Path,
    source: serde_json::Error,
) -> SessionRestoreStoreError {
    SessionRestoreStoreError::Json {
        operation,
        path: path.to_path_buf(),
        source,
    }
}
