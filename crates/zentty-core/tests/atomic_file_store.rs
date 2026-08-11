use std::error::Error;
use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use zentty_core::{AtomicFileAction, AtomicFileStore, AtomicFileStoreError};

static DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let sequence = DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "zentty-atomic-file-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn store(&self, max_bytes: usize) -> AtomicFileStore {
        AtomicFileStore::new(self.0.join("state.json"), max_bytes)
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
fn missing_read_and_private_atomic_replace_share_one_boundary() {
    let directory = TestDirectory::new("replace");
    let store = directory.store(64);
    let (missing, quarantine) = store
        .transaction(|bytes| Ok(AtomicFileAction::ReadOnly(bytes.is_none())))
        .unwrap();
    assert!(missing);
    assert!(quarantine.is_none());

    store
        .transaction(|_| {
            Ok(AtomicFileAction::Replace {
                bytes: b"first".to_vec(),
                value: (),
            })
        })
        .unwrap();
    let (bytes, _) = store
        .transaction(|bytes| Ok(AtomicFileAction::ReadOnly(bytes.unwrap().to_vec())))
        .unwrap();
    assert_eq!(bytes, b"first");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(store.path()).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(store.lock_path())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(&directory.0).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    assert!(fs::read_dir(&directory.0).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".tmp.")
    }));
}

#[test]
fn quarantine_without_replacement_preserves_invalid_bytes_and_leaves_source_absent() {
    let directory = TestDirectory::new("quarantine-only");
    let store = directory.store(64);
    fs::write(store.path(), b"invalid").unwrap();
    let (value, quarantine) = store
        .transaction(|bytes| {
            assert_eq!(bytes, Some(b"invalid".as_slice()));
            Ok(AtomicFileAction::Quarantine("preserved"))
        })
        .unwrap();

    assert_eq!(value, "preserved");
    assert!(!store.path().exists());
    assert_eq!(fs::read(quarantine.unwrap()).unwrap(), b"invalid");
}

#[test]
fn bounds_reject_existing_and_replacement_bytes_without_mutation() {
    let directory = TestDirectory::new("bounds");
    let store = directory.store(4);
    fs::write(store.path(), b"12345").unwrap();
    assert!(matches!(
        store.transaction(|_| Ok(AtomicFileAction::ReadOnly(()))),
        Err(AtomicFileStoreError::LimitExceeded { max_bytes: 4, .. })
    ));
    fs::write(store.path(), b"old").unwrap();
    assert!(matches!(
        store.transaction(|_| Ok(AtomicFileAction::Replace {
            bytes: b"12345".to_vec(),
            value: (),
        })),
        Err(AtomicFileStoreError::LimitExceeded { max_bytes: 4, .. })
    ));
    assert_eq!(fs::read(store.path()).unwrap(), b"old");

    store
        .transaction(|_| {
            Ok(AtomicFileAction::Replace {
                bytes: b"1234".to_vec(),
                value: (),
            })
        })
        .unwrap();
    let (exact, _) = store
        .transaction(|bytes| Ok(AtomicFileAction::ReadOnly(bytes.unwrap().to_vec())))
        .unwrap();
    assert_eq!(exact, b"1234");
}

#[test]
fn quarantine_preserves_the_exact_prior_bytes_before_replacement() {
    let directory = TestDirectory::new("quarantine");
    let store = directory.store(64);
    fs::write(store.path(), b"not json").unwrap();
    let ((), quarantine) = store
        .transaction(|_| {
            Ok(AtomicFileAction::QuarantineAndReplace {
                bytes: b"{}".to_vec(),
                value: (),
            })
        })
        .unwrap();
    let quarantine = quarantine.unwrap();
    assert_eq!(fs::read(quarantine).unwrap(), b"not json");
    assert_eq!(fs::read(store.path()).unwrap(), b"{}");
}

#[test]
fn held_lock_fails_on_the_bounded_deadline_without_running_the_callback() {
    let directory = TestDirectory::new("lock");
    let store = directory.store(64);
    store
        .transaction(|_| Ok(AtomicFileAction::ReadOnly(())))
        .unwrap();
    let lock = File::options()
        .read(true)
        .write(true)
        .open(store.lock_path())
        .unwrap();
    lock.lock().unwrap();
    let started = std::time::Instant::now();
    let error = store
        .transaction::<()>(|_| panic!("contended transaction callback ran"))
        .unwrap_err();
    let elapsed = started.elapsed();
    assert!(matches!(error, AtomicFileStoreError::LockTimeout { .. }));
    assert!(elapsed >= std::time::Duration::from_millis(225));
    assert!(elapsed < std::time::Duration::from_secs(2));
    lock.unlock().unwrap();
}

#[test]
fn errors_retain_actionable_display_and_sources() {
    let directory = TestDirectory::new("errors");
    let bounded = directory.store(1);
    let bounded_error = bounded
        .transaction(|_| {
            Ok(AtomicFileAction::Replace {
                bytes: b"too large".to_vec(),
                value: (),
            })
        })
        .unwrap_err();
    assert!(bounded_error.to_string().contains("1-byte limit"));
    assert!(bounded_error.source().is_none());

    fs::create_dir(bounded.path()).unwrap();
    let io_error = bounded
        .transaction(|_| Ok(AtomicFileAction::ReadOnly(())))
        .unwrap_err();
    assert!(io_error.to_string().contains("validate regular file"));
    assert!(io_error.source().is_some());
}

#[cfg(unix)]
#[test]
fn static_parent_data_and_lock_symlinks_are_rejected() {
    use std::os::unix::fs::symlink;

    let directory = TestDirectory::new("symlink");
    let outside = directory.0.join("outside");
    fs::create_dir(&outside).unwrap();

    let parent_link = directory.0.join("parent-link");
    symlink(&outside, &parent_link).unwrap();
    let parent_store = AtomicFileStore::new(parent_link.join("state.json"), 64);
    assert!(matches!(
        parent_store.transaction(|_| Ok(AtomicFileAction::ReadOnly(()))),
        Err(AtomicFileStoreError::Symlink { .. })
    ));

    let store = directory.store(64);
    let outside_file = outside.join("outside.json");
    fs::write(&outside_file, b"outside").unwrap();
    symlink(&outside_file, store.path()).unwrap();
    assert!(matches!(
        store.transaction(|_| Ok(AtomicFileAction::ReadOnly(()))),
        Err(AtomicFileStoreError::Symlink { .. })
    ));
    fs::remove_file(store.path()).unwrap();

    symlink(&outside_file, store.lock_path()).unwrap();
    assert!(matches!(
        store.transaction(|_| Ok(AtomicFileAction::ReadOnly(()))),
        Err(AtomicFileStoreError::Symlink { .. })
    ));
    assert_eq!(fs::read(outside_file).unwrap(), b"outside");
}

#[cfg(unix)]
#[test]
fn symlinked_existing_ancestor_above_the_immediate_parent_is_rejected() {
    use std::os::unix::fs::symlink;

    let directory = TestDirectory::new("ancestor-symlink");
    let outside = directory.0.join("outside");
    fs::create_dir(&outside).unwrap();
    let root_link = directory.0.join("config-link");
    symlink(&outside, &root_link).unwrap();
    let store = AtomicFileStore::new(root_link.join("zentty/state.json"), 64);
    assert!(matches!(
        store.transaction(|_| Ok(AtomicFileAction::ReadOnly(()))),
        Err(AtomicFileStoreError::Symlink { path }) if path == root_link
    ));
}
