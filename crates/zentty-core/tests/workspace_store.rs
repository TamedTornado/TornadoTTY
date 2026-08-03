use std::fs::{self, File, OpenOptions};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use zentty_core::{
    FirstRunSpec, Pane, StableId, StableIdSource, Workspace, WorkspaceError, WorkspaceLoad,
    WorkspaceStore, WorkspaceStoreError,
};

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

struct SequentialIds {
    next: u64,
    calls: usize,
}

impl SequentialIds {
    fn new(next: u64) -> Self {
        Self { next, calls: 0 }
    }
}

impl StableIdSource for SequentialIds {
    fn next_id(&mut self) -> Result<StableId, WorkspaceError> {
        let generated = id(self.next);
        self.next += 1;
        self.calls += 1;
        Ok(generated)
    }
}

struct CollidingIds;

impl StableIdSource for CollidingIds {
    fn next_id(&mut self) -> Result<StableId, WorkspaceError> {
        Ok(id(300))
    }
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

#[test]
fn missing_state_creates_one_documented_topology_exactly_once() {
    let directory = TestDirectory::new("first-run");
    let store = WorkspaceStore::new(directory.0.join("workspace.json"));
    let spec = FirstRunSpec::new("/tmp", "default-shell");
    let mut source = SequentialIds::new(100);

    let created = store.load_or_create(&mut source, &spec).unwrap();
    assert!(created.was_created());
    assert_eq!(source.calls, 4);
    let workspace = created.workspace();
    assert_eq!(workspace.revision(), 0);
    assert_eq!(workspace.windows().len(), 1);
    assert_eq!(workspace.windows()[0].worklanes().len(), 1);
    assert_eq!(workspace.windows()[0].worklanes()[0].panes().len(), 1);
    assert_eq!(
        workspace.windows()[0].worklanes()[0].panes()[0].cwd(),
        std::path::Path::new("/tmp")
    );
    assert_eq!(
        workspace.windows()[0].worklanes()[0].panes()[0].launch_profile_id(),
        "default-shell"
    );
    assert_eq!(store.load().unwrap(), Some(workspace.clone()));

    let existing = store.load_or_create(&mut source, &spec).unwrap();
    assert!(matches!(existing, WorkspaceLoad::Existing(_)));
    assert_eq!(source.calls, 4, "existing state consumed new identities");
    assert_eq!(existing.into_workspace(), workspace.clone());
}

#[test]
fn invalid_or_colliding_first_run_never_publishes_a_primary() {
    let directory = TestDirectory::new("first-run-invalid");
    let store = WorkspaceStore::new(directory.0.join("workspace.json"));
    let mut source = SequentialIds::new(200);
    let invalid = FirstRunSpec::new("relative", "default-shell");
    assert!(matches!(
        store.load_or_create(&mut source, &invalid),
        Err(WorkspaceStoreError::InvalidState { .. })
    ));
    assert!(!store.path().exists());

    assert!(matches!(
        store.load_or_create(
            &mut CollidingIds,
            &FirstRunSpec::new("/tmp", "default-shell")
        ),
        Err(WorkspaceStoreError::InvalidState { .. })
    ));
    assert!(!store.path().exists());
}

#[test]
fn corrupt_primary_never_falls_back_to_first_run() {
    let directory = TestDirectory::new("first-run-corrupt");
    let store = WorkspaceStore::new(directory.0.join("workspace.json"));
    let corrupt = b"{corrupt";
    fs::write(store.path(), corrupt).unwrap();
    let mut source = SequentialIds::new(400);

    assert!(matches!(
        store.load_or_create(&mut source, &FirstRunSpec::new("/tmp", "default-shell")),
        Err(WorkspaceStoreError::InvalidState { .. })
    ));
    assert_eq!(source.calls, 0);
    assert_eq!(fs::read(store.path()).unwrap(), corrupt);
}
