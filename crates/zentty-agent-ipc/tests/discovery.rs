use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::os::unix::net::UnixListener;
use zentty_agent_ipc::{discover_instances, publish_instance};

const INSTANCE: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const TOKEN: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

fn fixture(name: &str) -> (std::path::PathBuf, std::path::PathBuf, UnixListener) {
    let root = std::env::temp_dir().join(format!(
        "zentty-discovery-{}-{:?}-{name}",
        std::process::id(),
        std::thread::current().id()
    ));
    let directory = root.join("zentty/instance-test");
    fs::create_dir_all(&directory).unwrap();
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
    let socket = directory.join("instance.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
    (root, directory, listener)
}

#[test]
fn discovery_separates_metadata_from_the_redacted_owner_private_credential() {
    let (root, directory, listener) = fixture("valid");
    publish_instance(&directory, INSTANCE, TOKEN).unwrap();
    let metadata = fs::read_to_string(directory.join("instance.json")).unwrap();
    assert!(metadata.contains(INSTANCE));
    assert!(!metadata.contains(TOKEN));
    assert_eq!(
        fs::metadata(directory.join("instance.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(directory.join("automation.token"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let instances = discover_instances(&root).unwrap();
    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0].instance_id, INSTANCE);
    assert_eq!(instances[0].credential.expose(), TOKEN);
    assert_eq!(
        format!("{:?}", instances[0].credential),
        "InstanceCredential([REDACTED])"
    );
    drop(listener);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn discovery_ignores_non_private_symlinked_and_stale_candidates() {
    let (root, directory, listener) = fixture("hostile");
    publish_instance(&directory, INSTANCE, TOKEN).unwrap();
    fs::set_permissions(
        directory.join("automation.token"),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(discover_instances(&root).unwrap().is_empty());
    fs::set_permissions(
        directory.join("automation.token"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    fs::remove_file(directory.join("instance.json")).unwrap();
    symlink("automation.token", directory.join("instance.json")).unwrap();
    assert!(discover_instances(&root).unwrap().is_empty());
    drop(listener);
    fs::remove_dir_all(root).unwrap();
}
