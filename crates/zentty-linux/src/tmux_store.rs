use std::path::PathBuf;

use zentty_core::{AtomicFileAction, AtomicFileStore};
use zentty_tmux_compat::{StoreError, TeamStore};

#[derive(Clone, Debug)]
pub(crate) struct TmuxStoreFile {
    file: AtomicFileStore,
}

impl TmuxStoreFile {
    pub(crate) fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            file: AtomicFileStore::new(path, TeamStore::MAX_STORE_BYTES),
        }
    }

    pub(crate) fn default_path() -> Result<PathBuf, String> {
        default_path_from(
            std::env::var_os("XDG_CONFIG_HOME").as_deref(),
            std::env::var_os("HOME").as_deref(),
        )
    }

    #[cfg(test)]
    pub(crate) fn path(&self) -> &std::path::Path {
        self.file.path()
    }

    pub(crate) fn load(&self) -> Result<TeamStore, String> {
        let ((store, diagnostic), quarantine) = self
            .file
            .transaction(|bytes| match decode_store(bytes)? {
                DecodedStore::Valid(store) => Ok(AtomicFileAction::ReadOnly((store, None))),
                DecodedStore::Recovered { store, diagnostic } => {
                    let encoded = store.to_json().map_err(|error| error.to_string())?;
                    Ok(AtomicFileAction::QuarantineAndReplace {
                        bytes: encoded,
                        value: (store, Some(diagnostic)),
                    })
                }
            })
            .map_err(|error| error.to_string())?;
        if let Some(path) = quarantine {
            eprintln!(
                "zentty-linux: tmux-store-recovered quarantine={}",
                path.display()
            );
        }
        if let Some(diagnostic) = diagnostic {
            eprintln!("zentty-linux: tmux-store-recovery reason={diagnostic}");
        }
        Ok(store)
    }

    pub(crate) fn mutate<T>(
        &self,
        mutation: impl FnOnce(&mut TeamStore) -> Result<T, StoreError>,
    ) -> Result<(TeamStore, T), String> {
        let ((store, value, diagnostic), quarantine) = self
            .file
            .transaction(|bytes| {
                let (mut store, diagnostic) = match decode_store(bytes)? {
                    DecodedStore::Valid(store) => (store, None),
                    DecodedStore::Recovered { store, diagnostic } => (store, Some(diagnostic)),
                };
                let value = mutation(&mut store).map_err(|error| error.to_string())?;
                let encoded = store.to_json().map_err(|error| error.to_string())?;
                if diagnostic.is_some() {
                    Ok(AtomicFileAction::QuarantineAndReplace {
                        bytes: encoded,
                        value: (store, value, diagnostic),
                    })
                } else {
                    Ok(AtomicFileAction::Replace {
                        bytes: encoded,
                        value: (store, value, None),
                    })
                }
            })
            .map_err(|error| error.to_string())?;
        if let Some(path) = quarantine {
            eprintln!(
                "zentty-linux: tmux-store-recovered quarantine={}",
                path.display()
            );
        }
        if let Some(diagnostic) = diagnostic {
            eprintln!("zentty-linux: tmux-store-recovery reason={diagnostic}");
        }
        Ok((store, value))
    }
}

fn default_path_from(
    xdg_config_home: Option<&std::ffi::OsStr>,
    home: Option<&std::ffi::OsStr>,
) -> Result<PathBuf, String> {
    if let Some(path) = xdg_config_home {
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err("XDG_CONFIG_HOME must be absolute".to_owned());
        }
        return Ok(path.join("zentty/tmux-compat-store.json"));
    }
    home.map(|home| PathBuf::from(home).join(".config/zentty/tmux-compat-store.json"))
        .ok_or_else(|| "neither XDG_CONFIG_HOME nor HOME is set".to_owned())
}

enum DecodedStore {
    Valid(TeamStore),
    Recovered {
        store: TeamStore,
        diagnostic: String,
    },
}

fn decode_store(bytes: Option<&[u8]>) -> Result<DecodedStore, String> {
    match bytes {
        None => Ok(DecodedStore::Valid(TeamStore::default())),
        Some(bytes) => match TeamStore::from_json(bytes) {
            Ok(store) => Ok(DecodedStore::Valid(store)),
            Err(StoreError::InvalidJson(diagnostic)) => Ok(DecodedStore::Recovered {
                store: TeamStore::default(),
                diagnostic,
            }),
            Err(error) => Err(error.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{TmuxStoreFile, default_path_from};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use zentty_tmux_compat::{StoreError, TeamStore};

    static DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let sequence = DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "zentty-tmux-store-{label}-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn file(&self) -> TmuxStoreFile {
            TmuxStoreFile::new(self.0.join("tmux-compat-store.json"))
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            if self.0.exists() {
                fs::remove_dir_all(&self.0).unwrap();
            }
        }
    }

    #[test]
    fn xdg_path_is_absolute_and_preferred_with_home_fallback() {
        assert_eq!(
            default_path_from(Some("/config".as_ref()), Some("/home/user".as_ref())).unwrap(),
            PathBuf::from("/config/zentty/tmux-compat-store.json")
        );
        assert_eq!(
            default_path_from(None, Some("/home/user".as_ref())).unwrap(),
            PathBuf::from("/home/user/.config/zentty/tmux-compat-store.json")
        );
        assert!(default_path_from(Some("relative".as_ref()), None).is_err());
        assert!(default_path_from(None, None).is_err());
    }

    #[test]
    fn missing_state_is_empty_and_mutation_survives_a_fresh_reader() {
        let directory = TestDirectory::new("restart");
        let file = directory.file();
        assert_eq!(file.load().unwrap(), TeamStore::default());
        file.mutate(|store| store.set_buffer("agent", "durable"))
            .unwrap();
        assert_eq!(
            directory.file().load().unwrap().buffer(Some("agent")),
            "durable"
        );
    }

    #[test]
    fn malformed_state_is_preserved_but_future_schema_is_not_downgraded() {
        let directory = TestDirectory::new("recovery");
        let file = directory.file();
        fs::write(file.path(), b"not json").unwrap();
        assert_eq!(file.load().unwrap(), TeamStore::default());
        let quarantines = fs::read_dir(&directory.0)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".corrupt."))
            .collect::<Vec<_>>();
        assert_eq!(quarantines.len(), 1);
        assert_eq!(fs::read(quarantines[0].path()).unwrap(), b"not json");

        let future = br#"{"version":2}"#;
        fs::write(file.path(), future).unwrap();
        let error = file.load().unwrap_err();
        assert!(error.contains("unsupported tmux compatibility store version: 2"));
        assert_eq!(fs::read(file.path()).unwrap(), future);
    }

    #[test]
    fn serialized_schema_has_no_runtime_capability_or_secret_fields() {
        let directory = TestDirectory::new("schema");
        let file = directory.file();
        file.mutate(|store| {
            let _ = store.record_split("lane", "leader", "worker", false, Some(640));
            store.set_buffer("default", "visible terminal text")
        })
        .unwrap();
        let bytes = fs::read(file.path()).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("activePaneIDs"));
        for forbidden in ["token", "socket", "credential", "prompt", "transcript"] {
            assert!(!text.to_ascii_lowercase().contains(forbidden));
        }
    }

    #[test]
    fn rejected_mutation_preserves_the_previous_valid_file() {
        let directory = TestDirectory::new("transaction");
        let file = directory.file();
        file.mutate(|store| store.set_buffer("default", "prior"))
            .unwrap();
        let prior = fs::read(file.path()).unwrap();
        let error = file
            .mutate::<()>(|_| Err(StoreError::LimitExceeded))
            .unwrap_err();
        assert!(error.contains("limit exceeded"));
        assert_eq!(fs::read(file.path()).unwrap(), prior);
    }

    #[test]
    fn concurrent_writers_serialize_reload_modify_replace_without_lost_updates() {
        let directory = TestDirectory::new("concurrent");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
        let mut writers = Vec::new();
        for (name, value) in [("first", "one"), ("second", "two")] {
            let file = directory.file();
            let barrier = std::sync::Arc::clone(&barrier);
            writers.push(std::thread::spawn(move || {
                barrier.wait();
                file.mutate(|store| store.set_buffer(name, value)).unwrap();
            }));
        }
        barrier.wait();
        for writer in writers {
            writer.join().unwrap();
        }
        let store = directory.file().load().unwrap();
        assert_eq!(store.buffer(Some("first")), "one");
        assert_eq!(store.buffer(Some("second")), "two");
    }
}
