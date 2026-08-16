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
    let descriptor: serde_json::Value = serde_json::from_str(&metadata).unwrap();
    assert_eq!(
        descriptor["processStartTicks"].as_u64().unwrap(),
        current_process_start_ticks()
    );
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
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(discover_instances(&root).unwrap().is_empty());
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(
        directory.join("instance.json"),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(discover_instances(&root).unwrap().is_empty());
    fs::set_permissions(
        directory.join("instance.json"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    fs::remove_file(directory.join("instance.json")).unwrap();
    symlink("automation.token", directory.join("instance.json")).unwrap();
    assert!(discover_instances(&root).unwrap().is_empty());
    drop(listener);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn publication_rejects_each_malformed_identity_independently() {
    let (_root, directory, listener) = fixture("identity");
    for (instance, token) in [
        (&INSTANCE[..63], TOKEN),
        (INSTANCE, &TOKEN[..63]),
        (
            "g123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            TOKEN,
        ),
        (
            INSTANCE,
            "g123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        ),
    ] {
        assert!(publish_instance(&directory, instance, token).is_err());
    }
    drop(listener);
    fs::remove_dir_all(directory.parent().unwrap().parent().unwrap()).unwrap();
}

#[test]
fn discovery_rejects_each_descriptor_identity_field_independently() {
    let (root, directory, listener) = fixture("descriptor-fields");
    publish_instance(&directory, INSTANCE, TOKEN).unwrap();
    let descriptor_path = directory.join("instance.json");
    let original: serde_json::Value =
        serde_json::from_slice(&fs::read(&descriptor_path).unwrap()).unwrap();
    for mutation in [
        serde_json::json!({"schemaVersion": 2}),
        serde_json::json!({"applicationApiVersion": 2}),
        serde_json::json!({"processStartTicks": current_process_start_ticks() + 1}),
        serde_json::json!({"socketPath": directory.join("other.sock")}),
    ] {
        let mut candidate = original.clone();
        candidate
            .as_object_mut()
            .unwrap()
            .extend(mutation.as_object().unwrap().clone());
        fs::write(&descriptor_path, serde_json::to_vec(&candidate).unwrap()).unwrap();
        fs::set_permissions(&descriptor_path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(discover_instances(&root).unwrap().is_empty());
    }
    drop(listener);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn discovery_rejects_replaced_endpoints_and_stale_process_identity() {
    let (root, directory, listener) = fixture("replacement");
    publish_instance(&directory, INSTANCE, TOKEN).unwrap();

    let descriptor_path = directory.join("instance.json");
    let mut descriptor: serde_json::Value =
        serde_json::from_slice(&fs::read(&descriptor_path).unwrap()).unwrap();
    descriptor["processStartTicks"] =
        serde_json::json!(descriptor["processStartTicks"].as_u64().unwrap() + 1);
    fs::write(&descriptor_path, serde_json::to_vec(&descriptor).unwrap()).unwrap();
    fs::set_permissions(&descriptor_path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(discover_instances(&root).unwrap().is_empty());

    publish_instance(&directory, INSTANCE, TOKEN).unwrap();
    drop(listener);
    fs::remove_file(directory.join("instance.sock")).unwrap();
    fs::write(directory.join("instance.sock"), b"not a socket").unwrap();
    fs::set_permissions(
        directory.join("instance.sock"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    assert!(discover_instances(&root).unwrap().is_empty());
    fs::remove_dir_all(root).unwrap();
}

fn current_process_start_ticks() -> u64 {
    let stat = fs::read_to_string(format!("/proc/{}/stat", std::process::id())).unwrap();
    let end = stat.rfind(')').unwrap();
    stat[end + 2..]
        .split_whitespace()
        .nth(19)
        .unwrap()
        .parse()
        .unwrap()
}
