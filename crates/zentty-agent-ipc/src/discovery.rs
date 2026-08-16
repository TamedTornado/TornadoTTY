use crate::APPLICATION_API_VERSION;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{FileTypeExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

const DISCOVERY_SCHEMA_VERSION: u32 = 1;
const DESCRIPTOR_NAME: &str = "instance.json";
const CREDENTIAL_NAME: &str = "automation.token";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstanceDescriptor {
    schema_version: u32,
    application_api_version: u32,
    instance_id: String,
    process_id: u32,
    process_start_ticks: u64,
    socket_path: PathBuf,
}

#[derive(Clone, Eq, PartialEq)]
pub struct InstanceCredential(String);

impl InstanceCredential {
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for InstanceCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("InstanceCredential([REDACTED])")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredInstance {
    pub instance_id: String,
    pub process_id: u32,
    pub socket_path: PathBuf,
    pub credential: InstanceCredential,
}

/// Publishes owner-private, split metadata and credential artifacts for one
/// running instance. The non-secret descriptor never contains the capability.
///
/// # Errors
///
/// Returns an error for an invalid runtime directory, process identity,
/// endpoint, capability, or atomic write failure.
pub fn publish_instance(
    runtime_directory: &Path,
    instance_id: &str,
    credential: &str,
) -> Result<(), String> {
    validate_identity(instance_id, credential)?;
    validate_private_directory(runtime_directory)?;
    let socket_path = runtime_directory.join("instance.sock");
    validate_private_socket(&socket_path)?;
    let descriptor = InstanceDescriptor {
        schema_version: DISCOVERY_SCHEMA_VERSION,
        application_api_version: APPLICATION_API_VERSION,
        instance_id: instance_id.to_owned(),
        process_id: std::process::id(),
        process_start_ticks: process_start_ticks(std::process::id())?,
        socket_path,
    };
    let descriptor = serde_json::to_vec(&descriptor).map_err(|error| error.to_string())?;
    atomic_private_write(runtime_directory, CREDENTIAL_NAME, credential.as_bytes())?;
    if let Err(error) = atomic_private_write(runtime_directory, DESCRIPTOR_NAME, &descriptor) {
        let _ = fs::remove_file(runtime_directory.join(CREDENTIAL_NAME));
        return Err(error);
    }
    Ok(())
}

/// Finds valid live instances beneath one XDG runtime root. Invalid, stale,
/// symlinked, or non-private candidates are ignored rather than trusted.
///
/// # Errors
///
/// Returns an error only when the discovery root exists but cannot be read.
pub fn discover_instances(runtime_root: &Path) -> Result<Vec<DiscoveredInstance>, String> {
    let root = runtime_root.join("zentty");
    let metadata = match fs::symlink_metadata(&root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("could not inspect discovery root: {error}")),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("discovery root is not a real directory".to_owned());
    }
    let mut instances = fs::read_dir(&root)
        .map_err(|error| format!("could not read discovery root: {error}"))?
        .filter_map(Result::ok)
        .filter_map(|entry| read_candidate(&entry.path()).ok())
        .collect::<Vec<_>>();
    instances.sort_by(|left, right| left.instance_id.cmp(&right.instance_id));
    Ok(instances)
}

fn read_candidate(runtime_directory: &Path) -> Result<DiscoveredInstance, String> {
    validate_private_directory(runtime_directory)?;
    let descriptor_path = runtime_directory.join(DESCRIPTOR_NAME);
    validate_private_file(&descriptor_path)?;
    let descriptor: InstanceDescriptor =
        serde_json::from_slice(&fs::read(&descriptor_path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    if descriptor.schema_version != DISCOVERY_SCHEMA_VERSION
        || descriptor.application_api_version != APPLICATION_API_VERSION
        || descriptor.process_start_ticks != process_start_ticks(descriptor.process_id)?
        || descriptor.socket_path != runtime_directory.join("instance.sock")
    {
        return Err("stale or incompatible instance descriptor".to_owned());
    }
    validate_private_socket(&descriptor.socket_path)?;
    let credential_path = runtime_directory.join(CREDENTIAL_NAME);
    validate_private_file(&credential_path)?;
    let credential = fs::read_to_string(&credential_path).map_err(|error| error.to_string())?;
    validate_identity(&descriptor.instance_id, credential.trim())?;
    Ok(DiscoveredInstance {
        instance_id: descriptor.instance_id,
        process_id: descriptor.process_id,
        socket_path: descriptor.socket_path,
        credential: InstanceCredential(credential.trim().to_owned()),
    })
}

fn validate_identity(instance_id: &str, credential: &str) -> Result<(), String> {
    if instance_id.len() != 64
        || credential.len() != 64
        || !instance_id.bytes().all(|byte| byte.is_ascii_hexdigit())
        || !credential.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("instance identity or credential is malformed".to_owned());
    }
    Ok(())
}

fn validate_private_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.permissions().mode() & 0o777 != 0o700
    {
        return Err("instance runtime directory is not private".to_owned());
    }
    Ok(())
}

fn validate_private_file(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.permissions().mode() & 0o777 != 0o600
    {
        return Err("instance discovery file is not private".to_owned());
    }
    Ok(())
}

fn validate_private_socket(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.file_type().is_socket() || metadata.permissions().mode() & 0o777 != 0o600 {
        return Err("instance discovery socket is not private".to_owned());
    }
    Ok(())
}

fn atomic_private_write(directory: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    let temporary = directory.join(format!(".{name}.tmp-{}", std::process::id()));
    let destination = directory.join(name);
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    let result = (|| {
        file.write_all(bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        fs::rename(&temporary, &destination).map_err(|error| error.to_string())?;
        fs::set_permissions(&destination, fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
        fs::File::open(directory)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| error.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

fn process_start_ticks(process_id: u32) -> Result<u64, String> {
    let stat = fs::read_to_string(format!("/proc/{process_id}/stat"))
        .map_err(|error| error.to_string())?;
    let end = stat
        .rfind(')')
        .ok_or_else(|| "process stat is malformed".to_owned())?;
    stat.get(end + 2..)
        .and_then(|fields| fields.split_whitespace().nth(19))
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| "process start time is malformed".to_owned())
}
