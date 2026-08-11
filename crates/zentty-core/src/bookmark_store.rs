use std::{error::Error, fmt, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::{
    AtomicFileAction, AtomicFileStore, AtomicFileStoreError, WorkspaceTemplate,
    WorkspaceTemplateBundle,
};

#[derive(Clone, Debug, PartialEq)]
pub struct BookmarkStoreSnapshot {
    pub templates: Vec<WorkspaceTemplate>,
    pub quarantined_path: Option<PathBuf>,
}

impl BookmarkStoreSnapshot {
    #[must_use]
    pub fn template(&self, id: &str) -> Option<&WorkspaceTemplate> {
        self.templates.iter().find(|template| template.id == id)
    }

    #[must_use]
    pub fn sorted_templates(&self) -> Vec<&WorkspaceTemplate> {
        let mut templates = self.templates.iter().collect::<Vec<_>>();
        templates.sort_by(|left, right| {
            right
                .pinned
                .cmp(&left.pinned)
                .then_with(|| right.last_used_at.cmp(&left.last_used_at))
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
                .then_with(|| left.id.cmp(&right.id))
        });
        templates
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BookmarkStore {
    file: AtomicFileStore,
}

impl BookmarkStore {
    pub const MAX_FILE_BYTES: usize = 1024 * 1024;

    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            file: AtomicFileStore::new(path, Self::MAX_FILE_BYTES),
        }
    }

    /// Resolves an existing final symlink once, then applies the normal atomic
    /// store policy to its canonical regular-file target. The link itself is
    /// never replaced, which supports managed dotfile layouts without making
    /// symlinks generally writable storage boundaries.
    ///
    /// # Errors
    ///
    /// Returns an error when an existing final symlink is dangling or does
    /// not resolve to a regular file.
    pub fn new_resolving_final_symlink(
        path: impl Into<PathBuf>,
    ) -> Result<Self, BookmarkStoreError> {
        let path = path.into();
        let resolved = match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let target = std::fs::canonicalize(&path).map_err(|error| {
                    BookmarkStoreError::UnsafePath(format!(
                        "bookmark symlink {} cannot be resolved: {error}",
                        path.display()
                    ))
                })?;
                let target_metadata = std::fs::metadata(&target).map_err(|error| {
                    BookmarkStoreError::UnsafePath(format!(
                        "bookmark symlink target {} cannot be inspected: {error}",
                        target.display()
                    ))
                })?;
                if !target_metadata.is_file() {
                    return Err(BookmarkStoreError::UnsafePath(format!(
                        "bookmark symlink target is not a regular file: {}",
                        target.display()
                    )));
                }
                target
            }
            Ok(_) => path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => path,
            Err(error) => {
                return Err(BookmarkStoreError::UnsafePath(format!(
                    "bookmark path {} cannot be inspected: {error}",
                    path.display()
                )));
            }
        };
        Ok(Self::new(resolved))
    }

    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        self.file.path()
    }

    /// Loads the latest bundle while holding the shared persistent-file lock.
    /// Malformed data is preserved and removed from the active path.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe storage, I/O failure, size limits, or a
    /// schema newer than this build supports.
    pub fn load(&self) -> Result<BookmarkStoreSnapshot, BookmarkStoreError> {
        let (decoded, quarantine) = self
            .file
            .transaction(|bytes| {
                let action = match decode_bundle(bytes) {
                    DecodedBundle::Missing => AtomicFileAction::ReadOnly(Ok(Vec::new())),
                    DecodedBundle::Valid(bundle) => {
                        AtomicFileAction::ReadOnly(Ok(bundle.templates))
                    }
                    DecodedBundle::Future(found) => {
                        AtomicFileAction::ReadOnly(Err(BookmarkStoreError::FutureSchema {
                            found,
                            supported: WorkspaceTemplateBundle::CURRENT_SCHEMA_VERSION,
                        }))
                    }
                    DecodedBundle::Corrupt => AtomicFileAction::Quarantine(Ok(Vec::new())),
                };
                Ok(action)
            })
            .map_err(map_storage_error)?;
        let templates = decoded?;
        Ok(BookmarkStoreSnapshot {
            templates,
            quarantined_path: quarantine,
        })
    }

    /// Inserts or replaces one template against the latest on-disk bundle.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid templates or persistence failures.
    pub fn upsert(
        &self,
        mut template: WorkspaceTemplate,
        now: &str,
    ) -> Result<(), BookmarkStoreError> {
        validate_template(&template)?;
        let name = std::mem::take(&mut template.name);
        name.trim().clone_into(&mut template.name);
        now.clone_into(&mut template.updated_at);
        self.mutate(now, move |templates| {
            if let Some(existing) = templates.iter_mut().find(|item| item.id == template.id) {
                *existing = template;
            } else {
                templates.push(template);
            }
            Ok(())
        })
        .map(|((), _)| ())
    }

    /// Renames an existing template; blank names and missing IDs are no-ops.
    ///
    /// # Errors
    ///
    /// Returns an error when the latest store cannot be read or persisted.
    pub fn rename(&self, id: &str, name: &str, now: &str) -> Result<bool, BookmarkStoreError> {
        let name = name.trim();
        if name.is_empty() {
            return Ok(false);
        }
        self.mutate(now, |templates| {
            let Some(template) = templates.iter_mut().find(|template| template.id == id) else {
                return Ok(false);
            };
            name.clone_into(&mut template.name);
            now.clone_into(&mut template.updated_at);
            Ok(true)
        })
        .map(|(changed, _)| changed)
    }

    /// Changes source-compatible pin ordering for an existing template.
    ///
    /// # Errors
    ///
    /// Returns an error when the latest store cannot be read or persisted.
    pub fn set_pinned(
        &self,
        id: &str,
        pinned: bool,
        now: &str,
    ) -> Result<bool, BookmarkStoreError> {
        self.mutate(now, |templates| {
            let Some(template) = templates
                .iter_mut()
                .find(|template| template.id == id && template.pinned != pinned)
            else {
                return Ok(false);
            };
            template.pinned = pinned;
            now.clone_into(&mut template.updated_at);
            Ok(true)
        })
        .map(|(changed, _)| changed)
    }

    /// Records activation recency for an existing template.
    ///
    /// # Errors
    ///
    /// Returns an error when the latest store cannot be read or persisted.
    pub fn record_use(&self, id: &str, now: &str) -> Result<bool, BookmarkStoreError> {
        self.mutate(now, |templates| {
            let Some(template) = templates.iter_mut().find(|template| template.id == id) else {
                return Ok(false);
            };
            template.last_used_at = Some(now.to_owned());
            now.clone_into(&mut template.updated_at);
            Ok(true)
        })
        .map(|(changed, _)| changed)
    }

    /// Deletes an existing template; a missing ID is a no-op.
    ///
    /// # Errors
    ///
    /// Returns an error when the latest store cannot be read or persisted.
    pub fn delete(&self, id: &str, now: &str) -> Result<bool, BookmarkStoreError> {
        self.mutate(now, |templates| {
            let original_len = templates.len();
            templates.retain(|template| template.id != id);
            Ok(templates.len() != original_len)
        })
        .map(|(changed, _)| changed)
    }

    /// Duplicates a template under a caller-provided fresh stable identity.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identity or persistence failure.
    pub fn duplicate(
        &self,
        id: &str,
        new_id: &str,
        now: &str,
    ) -> Result<Option<WorkspaceTemplate>, BookmarkStoreError> {
        if new_id.trim().is_empty() {
            return Err(BookmarkStoreError::InvalidTemplate(
                "template ID must not be blank".into(),
            ));
        }
        self.mutate(now, |templates| {
            if templates.iter().any(|template| template.id == new_id) {
                return Err(BookmarkStoreError::InvalidTemplate(
                    "duplicate target ID already exists".into(),
                ));
            }
            let Some(original) = templates.iter().find(|template| template.id == id) else {
                return Ok(None);
            };
            let mut copy = original.clone();
            new_id.clone_into(&mut copy.id);
            copy.name = duplicate_name(&copy.name, templates);
            copy.pinned = false;
            now.clone_into(&mut copy.created_at);
            now.clone_into(&mut copy.updated_at);
            copy.last_used_at = None;
            templates.push(copy.clone());
            Ok(Some(copy))
        })
        .map(|(copy, _)| copy)
    }

    fn mutate<T>(
        &self,
        now: &str,
        mutation: impl FnOnce(&mut Vec<WorkspaceTemplate>) -> Result<T, BookmarkStoreError>,
    ) -> Result<(T, BookmarkStoreSnapshot), BookmarkStoreError> {
        let (result, quarantine) = self
            .file
            .transaction(|bytes| {
                let (mut templates, corrupt) = match decode_bundle(bytes) {
                    DecodedBundle::Missing => (Vec::new(), false),
                    DecodedBundle::Valid(bundle) => (bundle.templates, false),
                    DecodedBundle::Future(found) => {
                        return Ok(AtomicFileAction::ReadOnly(Err(
                            BookmarkStoreError::FutureSchema {
                                found,
                                supported: WorkspaceTemplateBundle::CURRENT_SCHEMA_VERSION,
                            },
                        )));
                    }
                    DecodedBundle::Corrupt => (Vec::new(), true),
                };
                let original_templates = templates.clone();
                let value = match mutation(&mut templates) {
                    Ok(value) => value,
                    Err(error) => return Ok(AtomicFileAction::ReadOnly(Err(error))),
                };
                for template in &templates {
                    if let Err(error) = validate_template(template) {
                        return Ok(AtomicFileAction::ReadOnly(Err(error)));
                    }
                }
                if !corrupt && templates == original_templates {
                    return Ok(AtomicFileAction::ReadOnly(Ok((value, templates))));
                }
                let bytes = match encode_bundle(&WorkspaceTemplateBundle {
                    schema_version: WorkspaceTemplateBundle::CURRENT_SCHEMA_VERSION,
                    saved_at: now.to_owned(),
                    templates: templates.clone(),
                }) {
                    Ok(bytes) => bytes,
                    Err(error) => return Ok(AtomicFileAction::ReadOnly(Err(error))),
                };
                let value = Ok((value, templates));
                Ok(if corrupt {
                    AtomicFileAction::QuarantineAndReplace { bytes, value }
                } else {
                    AtomicFileAction::Replace { bytes, value }
                })
            })
            .map_err(map_storage_error)?;
        let (value, templates) = result?;
        Ok((
            value,
            BookmarkStoreSnapshot {
                templates,
                quarantined_path: quarantine,
            },
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTemplateExportEnvelope {
    pub schema_version: i64,
    pub exported_at: String,
    pub template: WorkspaceTemplate,
}

impl WorkspaceTemplateExportEnvelope {
    pub const CURRENT_SCHEMA_VERSION: i64 = 1;

    /// Encodes one portable preset envelope.
    ///
    /// # Errors
    ///
    /// Returns an error when the template is invalid or encoding exceeds the
    /// shared bookmark-store limit.
    pub fn export(template: WorkspaceTemplate, now: &str) -> Result<Vec<u8>, BookmarkStoreError> {
        validate_template(&template)?;
        let envelope = Self {
            schema_version: Self::CURRENT_SCHEMA_VERSION,
            exported_at: now.to_owned(),
            template: template.into_portable_preset(now),
        };
        let bytes = serde_json::to_vec_pretty(&envelope)
            .map_err(|error| BookmarkStoreError::Encode(error.to_string()))?;
        enforce_import_limit(&bytes)?;
        Ok(bytes)
    }

    /// Decodes a portable preset while assigning new local identity and time.
    ///
    /// # Errors
    ///
    /// Returns an error for oversize, malformed, future-schema, or invalid
    /// input.
    pub fn import(
        bytes: &[u8],
        new_id: &str,
        now: &str,
    ) -> Result<WorkspaceTemplate, BookmarkStoreError> {
        enforce_import_limit(bytes)?;
        let envelope = serde_json::from_slice::<Self>(bytes)
            .map_err(|error| BookmarkStoreError::Decode(error.to_string()))?;
        if envelope.schema_version > Self::CURRENT_SCHEMA_VERSION {
            return Err(BookmarkStoreError::FutureExportSchema {
                found: envelope.schema_version,
                supported: Self::CURRENT_SCHEMA_VERSION,
            });
        }
        let mut template = envelope.template.into_portable_preset(now);
        new_id.clone_into(&mut template.id);
        now.clone_into(&mut template.created_at);
        now.clone_into(&mut template.updated_at);
        template.pinned = false;
        template.last_used_at = None;
        validate_template(&template)?;
        Ok(template)
    }
}

#[derive(Debug)]
pub enum BookmarkStoreError {
    UnsafePath(String),
    Storage(AtomicFileStoreError),
    FutureSchema { found: i64, supported: i64 },
    FutureTemplateSchema { found: i64, supported: i64 },
    FutureExportSchema { found: i64, supported: i64 },
    ImportTooLarge { size: usize, maximum: usize },
    InvalidTemplate(String),
    Decode(String),
    Encode(String),
}

impl fmt::Display for BookmarkStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsafePath(path) => write!(formatter, "unsafe bookmark path: {path}"),
            Self::Storage(error) => error.fmt(formatter),
            Self::FutureSchema { found, supported } => write!(
                formatter,
                "bookmark schema {found} is newer than supported schema {supported}"
            ),
            Self::FutureTemplateSchema { found, supported } => write!(
                formatter,
                "template schema {found} is newer than supported schema {supported}"
            ),
            Self::FutureExportSchema { found, supported } => write!(
                formatter,
                "preset export schema {found} is newer than supported schema {supported}"
            ),
            Self::ImportTooLarge { size, maximum } => {
                write!(formatter, "preset is {size} bytes; maximum is {maximum}")
            }
            Self::InvalidTemplate(message) => write!(formatter, "invalid template: {message}"),
            Self::Decode(message) => write!(formatter, "could not decode bookmarks: {message}"),
            Self::Encode(message) => write!(formatter, "could not encode bookmarks: {message}"),
        }
    }
}

impl Error for BookmarkStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error),
            _ => None,
        }
    }
}

enum DecodedBundle {
    Missing,
    Valid(WorkspaceTemplateBundle),
    Future(i64),
    Corrupt,
}

fn decode_bundle(bytes: Option<&[u8]>) -> DecodedBundle {
    let Some(bytes) = bytes else {
        return DecodedBundle::Missing;
    };
    let Ok(bundle) = serde_json::from_slice::<WorkspaceTemplateBundle>(bytes) else {
        return DecodedBundle::Corrupt;
    };
    if bundle.schema_version > WorkspaceTemplateBundle::CURRENT_SCHEMA_VERSION {
        return DecodedBundle::Future(bundle.schema_version);
    }
    if bundle
        .templates
        .iter()
        .any(|template| template.schema_version > WorkspaceTemplate::CURRENT_SCHEMA_VERSION)
    {
        return DecodedBundle::Future(
            bundle
                .templates
                .iter()
                .map(|template| template.schema_version)
                .max()
                .unwrap_or(WorkspaceTemplate::CURRENT_SCHEMA_VERSION),
        );
    }
    DecodedBundle::Valid(bundle)
}

fn encode_bundle(bundle: &WorkspaceTemplateBundle) -> Result<Vec<u8>, BookmarkStoreError> {
    serde_json::to_vec_pretty(bundle).map_err(|error| BookmarkStoreError::Encode(error.to_string()))
}

fn validate_template(template: &WorkspaceTemplate) -> Result<(), BookmarkStoreError> {
    if template.schema_version > WorkspaceTemplate::CURRENT_SCHEMA_VERSION {
        return Err(BookmarkStoreError::FutureTemplateSchema {
            found: template.schema_version,
            supported: WorkspaceTemplate::CURRENT_SCHEMA_VERSION,
        });
    }
    if template.id.trim().is_empty() {
        return Err(BookmarkStoreError::InvalidTemplate(
            "template ID must not be blank".into(),
        ));
    }
    if template.name.trim().is_empty() {
        return Err(BookmarkStoreError::InvalidTemplate(
            "template name must not be blank".into(),
        ));
    }
    Ok(())
}

fn duplicate_name(source: &str, templates: &[WorkspaceTemplate]) -> String {
    let base = source.trim();
    let candidate = if base.is_empty() {
        "Copy".to_owned()
    } else {
        format!("{base} copy")
    };
    if !templates.iter().any(|template| template.name == candidate) {
        return candidate;
    }
    let maximum_suffix = templates
        .len()
        .checked_add(2)
        .expect("template count is bounded by the bookmark file size");
    for suffix in 2..=maximum_suffix {
        let attempt = format!("{candidate} {suffix}");
        if !templates.iter().any(|template| template.name == attempt) {
            return attempt;
        }
    }
    unreachable!("a finite template set must leave a duplicate suffix available")
}

fn enforce_import_limit(bytes: &[u8]) -> Result<(), BookmarkStoreError> {
    if bytes.len() > BookmarkStore::MAX_FILE_BYTES {
        return Err(BookmarkStoreError::ImportTooLarge {
            size: bytes.len(),
            maximum: BookmarkStore::MAX_FILE_BYTES,
        });
    }
    Ok(())
}

fn map_storage_error(error: AtomicFileStoreError) -> BookmarkStoreError {
    match error {
        AtomicFileStoreError::Symlink { ref path } => {
            BookmarkStoreError::UnsafePath(path.display().to_string())
        }
        error => BookmarkStoreError::Storage(error),
    }
}
