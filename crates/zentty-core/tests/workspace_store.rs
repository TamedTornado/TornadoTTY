use std::fs::{self, File, OpenOptions};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use zentty_core::{Pane, StableId, Workspace, WorkspaceStore, WorkspaceStoreError};

static DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let sequence = DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "zentty-store-integration-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn id(value: u64) -> StableId {
    StableId::parse(format!("00000000-0000-4000-8000-{value:012x}")).unwrap()
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

#[test]
fn real_filesystem_save_load_backup_and_corruption_are_fail_closed() {
    let directory = TestDirectory::new("round-trip");
    let store = WorkspaceStore::new(directory.0.join("workspace.json"));
    assert_eq!(store.load().unwrap(), None);

    let first = workspace();
    store.save(&first).unwrap();
    assert_eq!(store.load().unwrap(), Some(first.clone()));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(store.path()).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    let mut second = first.clone();
    second
        .rename_worklane(&id(2), &id(3), Some("second".into()))
        .unwrap();
    store.save(&second).unwrap();
    assert_eq!(store.load().unwrap(), Some(second));
    assert_eq!(store.load_backup().unwrap(), Some(first));

    let corrupt = b"{not-json\n";
    fs::write(store.path(), corrupt).unwrap();
    assert!(matches!(
        store.load(),
        Err(WorkspaceStoreError::InvalidState { .. })
    ));
    assert_eq!(fs::read(store.path()).unwrap(), corrupt);
}

#[test]
fn real_advisory_lock_contention_does_not_modify_primary() {
    let directory = TestDirectory::new("lock");
    let store = WorkspaceStore::new(directory.0.join("workspace.json"));
    let workspace = workspace();
    store.save(&workspace).unwrap();
    let original = fs::read(store.path()).unwrap();

    let lock_path = PathBuf::from(format!("{}{}", store.path().display(), ".lock"));
    let lock: File = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)
        .unwrap();
    lock.try_lock().unwrap();

    assert!(matches!(
        store.save(&workspace),
        Err(WorkspaceStoreError::Locked(_))
    ));
    assert_eq!(fs::read(store.path()).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn symlink_primary_is_rejected_for_load_and_save() {
    use std::os::unix::fs::symlink;

    let directory = TestDirectory::new("symlink");
    let target = directory.0.join("target.json");
    let primary = directory.0.join("workspace.json");
    let original = workspace().to_json().unwrap();
    fs::write(&target, &original).unwrap();
    symlink(&target, &primary).unwrap();
    let store = WorkspaceStore::new(primary);

    assert!(matches!(store.load(), Err(WorkspaceStoreError::Io { .. })));
    assert!(matches!(
        store.save(&workspace()),
        Err(WorkspaceStoreError::Io { .. })
    ));
    assert_eq!(fs::read(target).unwrap(), original);
}
