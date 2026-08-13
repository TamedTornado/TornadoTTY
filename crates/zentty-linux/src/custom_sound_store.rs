use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{ErrorKind, Read, Seek, SeekFrom};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

const CUSTOM_PREFIX: &str = "zentty-custom-";
const CUSTOM_SUFFIX: &str = ".wav";
const MAX_SOURCE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_INSTALLED_BYTES: u64 = 3 * 1024 * 1024;
const MAX_DURATION_SECONDS: f64 = 30.0;
const PROCESS_TIMEOUT: Duration = Duration::from_secs(15);
const LOCK_TIMEOUT: Duration = Duration::from_secs(2);
const FFMPEG: &str = "/usr/bin/ffmpeg";
const FFPROBE: &str = "/usr/bin/ffprobe";
pub(crate) const APLAY: &str = "/usr/bin/aplay";

static TRANSACTION_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(crate) struct PreparedSound {
    internal_name: String,
    display_name: String,
    installed_path: PathBuf,
    transaction_directory: PathBuf,
    created_installed_file: bool,
    _lock: File,
}

impl PreparedSound {
    pub(crate) fn internal_name(&self) -> &str {
        &self.internal_name
    }

    pub(crate) fn display_name(&self) -> &str {
        &self.display_name
    }
}

impl Drop for PreparedSound {
    fn drop(&mut self) {
        if self.created_installed_file {
            let _ = fs::remove_file(&self.installed_path);
        }
        let _ = fs::remove_dir_all(&self.transaction_directory);
    }
}

pub(crate) struct CustomSoundStore;

impl CustomSoundStore {
    pub(crate) fn is_custom_name(name: &str) -> bool {
        let Some(digest) = name
            .strip_prefix(CUSTOM_PREFIX)
            .and_then(|name| name.strip_suffix(CUSTOM_SUFFIX))
        else {
            return false;
        };
        digest.len() == 32
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }

    pub(crate) fn path_for_name(name: &str) -> Result<PathBuf, String> {
        if !Self::is_custom_name(name) {
            return Err("custom sound name is not a Zentty-owned asset".into());
        }
        let path = ensure_private_sounds_directory()?.join(name);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("custom sound is unavailable: {error}"))?;
        if !metadata.file_type().is_file() || metadata.len() == 0 {
            return Err("custom sound is not a nonempty regular file".into());
        }
        if metadata.len() > MAX_INSTALLED_BYTES {
            return Err(format!(
                "custom sound exceeds the {MAX_INSTALLED_BYTES} byte installed limit"
            ));
        }
        Ok(path)
    }

    pub(crate) fn prepare(source: &Path) -> Result<PreparedSound, String> {
        let sounds = ensure_private_sounds_directory()?;
        let lock = acquire_lock(&sounds)?;
        let transaction_directory = sounds.join(format!(
            ".zentty-sound-{}-{}",
            std::process::id(),
            TRANSACTION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        create_private_directory(&transaction_directory)?;

        let prepared = prepare_in_transaction(source, &sounds, &transaction_directory, lock);
        if prepared.is_err() {
            let _ = fs::remove_dir_all(&transaction_directory);
        }
        prepared
    }

    pub(crate) fn finish(mut prepared: PreparedSound) -> Result<(), String> {
        // Configuration has already committed this asset before `finish` is
        // called. Disarm rollback first so a prune or transaction-cleanup
        // failure cannot delete the now-active sound.
        prepared.created_installed_file = false;
        let prune_result = prune_locked(
            prepared
                .installed_path
                .parent()
                .ok_or_else(|| "installed custom sound has no containing directory".to_owned())?,
            Some(&prepared.internal_name),
        );
        let cleanup_result = fs::remove_dir_all(&prepared.transaction_directory).map_err(|error| {
            format!(
                "could not remove completed sound transaction {}: {error}",
                prepared.transaction_directory.display()
            )
        });
        prune_result.and(cleanup_result)
    }

    pub(crate) fn rollback(prepared: PreparedSound) {
        drop(prepared);
    }

    pub(crate) fn prune(keeping: Option<&str>) -> Result<(), String> {
        if keeping.is_some_and(|name| !Self::is_custom_name(name)) {
            return Err("refusing an invalid custom sound retention name".into());
        }
        let sounds = ensure_private_sounds_directory()?;
        let _lock = acquire_lock(&sounds)?;
        prune_locked(&sounds, keeping)
    }
}

fn prepare_in_transaction(
    source: &Path,
    sounds: &Path,
    transaction_directory: &Path,
    lock: File,
) -> Result<PreparedSound, String> {
    let copied_source = transaction_directory.join("source.audio");
    copy_source_no_follow(source, &copied_source)?;
    let source_duration = probe_duration(&copied_source, transaction_directory, "source")?;
    validate_duration(source_duration)?;

    let converted = transaction_directory.join("converted.wav");
    let mut command = Command::new(FFMPEG);
    command.args(["-nostdin", "-v", "error", "-y", "-i"]);
    command.arg(&copied_source).args([
        "-map_metadata",
        "-1",
        "-ac",
        "1",
        "-ar",
        "44100",
        "-c:a",
        "pcm_s16le",
    ]);
    command.arg(&converted);
    run_bounded_process(&mut command, "audio conversion")?;
    let converted_duration = probe_duration(&converted, transaction_directory, "converted")?;
    validate_duration(converted_duration)?;
    validate_wav(&converted)?;

    let mut converted_file = File::open(&converted)
        .map_err(|error| format!("could not open converted audio: {error}"))?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut converted_file)
        .take(installed_read_limit())
        .read_to_end(&mut bytes)
        .map_err(|error| format!("could not read converted audio: {error}"))?;
    validate_installed_size(bytes.len() as u64, "converted audio")?;
    let digest = Sha256::digest(&bytes);
    let mut digest_name = String::with_capacity(32);
    for byte in &digest[..16] {
        write!(&mut digest_name, "{byte:02x}").expect("writing to a String cannot fail");
    }
    let internal_name = format!("{CUSTOM_PREFIX}{digest_name}{CUSTOM_SUFFIX}");
    let installed_path = sounds.join(&internal_name);
    let created_installed_file = match fs::symlink_metadata(&installed_path) {
        Ok(metadata) => {
            if !metadata.file_type().is_file() {
                return Err("custom sound destination is not a regular file".into());
            }
            validate_installed_size(metadata.len(), "existing custom sound")?;
            let existing = fs::read(&installed_path)
                .map_err(|error| format!("could not verify existing custom sound: {error}"))?;
            if existing != bytes {
                return Err("custom sound digest collision did not match existing bytes".into());
            }
            false
        }
        Err(error) if destination_is_missing(error.kind()) => {
            fs::set_permissions(&converted, fs::Permissions::from_mode(0o600))
                .map_err(|error| format!("could not secure converted sound: {error}"))?;
            File::open(&converted)
                .and_then(|file| file.sync_all())
                .map_err(|error| format!("could not sync converted sound: {error}"))?;
            fs::rename(&converted, &installed_path)
                .map_err(|error| format!("could not publish converted sound: {error}"))?;
            if let Err(error) = File::open(sounds).and_then(|directory| directory.sync_all()) {
                let _ = fs::remove_file(&installed_path);
                return Err(format!("could not sync sound directory: {error}"));
            }
            true
        }
        Err(error) => {
            return Err(format!(
                "could not inspect custom sound destination: {error}"
            ));
        }
    };

    let display_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Custom audio")
        .chars()
        .take(160)
        .collect::<String>();
    Ok(PreparedSound {
        internal_name,
        display_name,
        installed_path,
        transaction_directory: transaction_directory.to_owned(),
        created_installed_file,
        _lock: lock,
    })
}

fn installed_read_limit() -> u64 {
    MAX_INSTALLED_BYTES + 1
}

fn validate_installed_size(length: u64, label: &str) -> Result<(), String> {
    if length == 0 {
        return Err(format!("{label} is empty"));
    }
    if length > MAX_INSTALLED_BYTES {
        return Err(format!(
            "{label} exceeds the {MAX_INSTALLED_BYTES} byte installed limit"
        ));
    }
    Ok(())
}

fn destination_is_missing(kind: ErrorKind) -> bool {
    kind == ErrorKind::NotFound
}

fn sounds_directory() -> Result<PathBuf, String> {
    sounds_directory_from(std::env::var_os("XDG_DATA_HOME"), std::env::var_os("HOME"))
}

fn sounds_directory_from(
    xdg_data_home: Option<OsString>,
    home: Option<OsString>,
) -> Result<PathBuf, String> {
    let root = xdg_data_home
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            home.filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .map(|home| home.join(".local/share"))
        })
        .ok_or_else(|| {
            "could not resolve custom sounds: XDG_DATA_HOME and HOME are unset".to_owned()
        })?;
    if !root.is_absolute() {
        return Err("custom sound data root must be absolute".into());
    }
    Ok(root.join("zentty/sounds"))
}

fn ensure_private_sounds_directory() -> Result<PathBuf, String> {
    let sounds = sounds_directory()?;
    let data_root = sounds
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| "custom sound data root is invalid".to_owned())?;
    if !data_root.exists() {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true).mode(0o700);
        builder
            .create(data_root)
            .map_err(|error| format!("could not create XDG data root: {error}"))?;
    }
    create_private_directory(sounds.parent().unwrap())?;
    create_private_directory(&sounds)?;
    Ok(sounds)
}

fn create_private_directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(format!(
                "refusing symlinked owned directory: {}",
                path.display()
            ));
        }
        Ok(metadata) if !metadata.is_dir() => {
            return Err(format!("owned path is not a directory: {}", path.display()));
        }
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(path)
                .map_err(|error| format!("could not create {}: {error}", path.display()))?;
        }
        Err(error) => return Err(format!("could not inspect {}: {error}", path.display())),
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("could not secure {}: {error}", path.display()))
}

fn acquire_lock(sounds: &Path) -> Result<File, String> {
    acquire_lock_with_timeout(sounds, LOCK_TIMEOUT)
}

fn acquire_lock_with_timeout(sounds: &Path, timeout: Duration) -> Result<File, String> {
    let path = sounds.join(".zentty-sounds.lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(&path)
        .map_err(|error| format!("could not open custom sound lock: {error}"))?;
    lock.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("could not secure custom sound lock: {error}"))?;
    let deadline = Instant::now() + timeout;
    loop {
        match lock.try_lock() {
            Ok(()) => return Ok(lock),
            Err(TryLockError::WouldBlock) if lock_wait_remains(deadline) => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(TryLockError::WouldBlock) => {
                return Err("timed out waiting for custom sound transaction".into());
            }
            Err(TryLockError::Error(error)) => {
                return Err(format!("could not lock custom sound transaction: {error}"));
            }
        }
    }
}

// The strict-vs-inclusive comparison at one exact `Instant` is not an
// observable behavioral mutant. The surrounding timeout and successful-wakeup
// branches remain mutation-tested.
#[cfg_attr(any(), mutants::skip)]
fn lock_wait_remains(deadline: Instant) -> bool {
    Instant::now() < deadline
}

fn copy_source_no_follow(source: &Path, destination: &Path) -> Result<(), String> {
    let mut input = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(source)
        .map_err(|error| {
            format!("could not open selected audio without following links: {error}")
        })?;
    let metadata = input
        .metadata()
        .map_err(|error| format!("could not inspect selected audio: {error}"))?;
    validate_source_file(metadata.is_file(), metadata.len())?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(destination)
        .map_err(|error| format!("could not create private audio input: {error}"))?;
    let copied = std::io::copy(
        &mut Read::by_ref(&mut input).take(source_copy_limit()),
        &mut output,
    )
    .map_err(|error| format!("could not copy selected audio: {error}"))?;
    validate_copied_size(copied, MAX_SOURCE_BYTES)?;
    output
        .sync_all()
        .map_err(|error| format!("could not sync private audio input: {error}"))
}

fn source_copy_limit() -> u64 {
    MAX_SOURCE_BYTES + 1
}

fn validate_copied_size(copied: u64, maximum: u64) -> Result<(), String> {
    if copied == 0 || copied > maximum {
        return Err("selected audio changed size while it was being copied".into());
    }
    Ok(())
}

fn validate_source_file(is_file: bool, length: u64) -> Result<(), String> {
    if !is_file || length == 0 {
        return Err("selected audio must be a nonempty regular file".into());
    }
    if length > MAX_SOURCE_BYTES {
        return Err(format!(
            "selected audio exceeds the {MAX_SOURCE_BYTES} byte source limit"
        ));
    }
    Ok(())
}

fn probe_duration(path: &Path, transaction: &Path, label: &str) -> Result<f64, String> {
    let output_path = transaction.join(format!("{label}.duration"));
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&output_path)
        .map_err(|error| format!("could not create duration receipt: {error}"))?;
    let mut command = Command::new(FFPROBE);
    command
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .stdout(Stdio::from(output));
    run_bounded_process(&mut command, "audio inspection")?;
    let mut receipt = File::open(&output_path)
        .map_err(|error| format!("could not open duration receipt: {error}"))?;
    let mut text = String::new();
    Read::by_ref(&mut receipt)
        .take(129)
        .read_to_string(&mut text)
        .map_err(|error| format!("could not read duration receipt: {error}"))?;
    if text.len() > 128 {
        return Err("audio duration receipt exceeded its limit".into());
    }
    text.trim()
        .parse::<f64>()
        .map_err(|_| "could not determine a finite audio duration".into())
}

fn validate_duration(duration: f64) -> Result<(), String> {
    if !duration.is_finite() || duration <= 0.0 {
        return Err("audio duration must be finite and greater than zero".into());
    }
    if duration > MAX_DURATION_SECONDS {
        return Err(format!(
            "selected audio is longer than {MAX_DURATION_SECONDS:.0} seconds"
        ));
    }
    Ok(())
}

fn validate_wav(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("could not inspect converted audio: {error}"))?;
    if !metadata.file_type().is_file() || metadata.len() < 44 {
        return Err("converted audio is not a complete WAV file".into());
    }
    if metadata.len() > MAX_INSTALLED_BYTES {
        return Err(format!(
            "converted audio exceeds the {MAX_INSTALLED_BYTES} byte installed limit"
        ));
    }
    let mut header = [0_u8; 12];
    let mut file =
        File::open(path).map_err(|error| format!("could not open converted audio: {error}"))?;
    file.read_exact(&mut header)
        .map_err(|error| format!("could not read converted audio header: {error}"))?;
    if &header[..4] != b"RIFF" || &header[8..] != b"WAVE" {
        return Err("converted audio does not have a PCM WAV container".into());
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|error| format!("could not rewind converted audio: {error}"))?;
    Ok(())
}

fn run_bounded_process(command: &mut Command, operation: &str) -> Result<(), String> {
    let mut child = command
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("could not start {operation}: {error}"))?;
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => return Err(format!("{operation} exited with {status}")),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{operation} exceeded its time limit"));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("could not wait for {operation}: {error}"));
            }
        }
    }
}

fn prune_locked(sounds: &Path, keeping: Option<&str>) -> Result<(), String> {
    for entry in
        fs::read_dir(sounds).map_err(|error| format!("could not inspect custom sounds: {error}"))?
    {
        let entry = entry.map_err(|error| format!("could not inspect custom sound: {error}"))?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !CustomSoundStore::is_custom_name(&name) || keeping == Some(name.as_str()) {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| format!("could not inspect custom sound candidate: {error}"))?;
        if metadata.file_type().is_file() {
            fs::remove_file(entry.path())
                .map_err(|error| format!("could not prune custom sound: {error}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        CustomSoundStore, MAX_INSTALLED_BYTES, MAX_SOURCE_BYTES, acquire_lock,
        acquire_lock_with_timeout, copy_source_no_follow, create_private_directory,
        destination_is_missing, installed_read_limit, prepare_in_transaction, prune_locked,
        sounds_directory_from, source_copy_limit, validate_copied_size, validate_duration,
        validate_installed_size, validate_source_file, validate_wav,
    };
    use std::ffi::OsString;
    use std::fs;
    use std::io::ErrorKind;
    use std::os::unix::fs::{MetadataExt, symlink};
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::mpsc;
    use std::time::Duration;

    static SEQUENCE: AtomicU64 = AtomicU64::new(1);

    fn root(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "zentty-custom-sound-{label}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn names_and_xdg_paths_are_strict() {
        let valid = "zentty-custom-0123456789abcdef0123456789abcdef.wav";
        assert!(CustomSoundStore::is_custom_name(valid));
        for invalid in [
            "",
            "zentty-custom-0123.wav",
            "zentty-custom-0123456789ABCDEF0123456789ABCDEF.wav",
            "zentty-custom-0123456789abcdef0123456789abcdef.ogg",
            "../zentty-custom-0123456789abcdef0123456789abcdef.wav",
        ] {
            assert!(!CustomSoundStore::is_custom_name(invalid));
        }
        assert_eq!(
            sounds_directory_from(Some(OsString::from("/xdg")), None).unwrap(),
            Path::new("/xdg/zentty/sounds")
        );
        assert_eq!(
            sounds_directory_from(Some(OsString::new()), Some(OsString::from("/home"))).unwrap(),
            Path::new("/home/.local/share/zentty/sounds")
        );
        assert!(sounds_directory_from(None, None).is_err());
        assert!(sounds_directory_from(Some(OsString::from("relative")), None).is_err());
    }

    #[test]
    fn private_directory_and_source_copy_reject_symlinks_and_bounds() {
        let root = root("security");
        let owned = root.join("owned");
        fs::create_dir_all(&root).unwrap();
        create_private_directory(&owned).unwrap();
        assert_eq!(fs::metadata(&owned).unwrap().mode() & 0o777, 0o700);

        let source = root.join("source.wav");
        fs::write(&source, b"source").unwrap();
        let copied = root.join("copied.wav");
        copy_source_no_follow(&source, &copied).unwrap();
        assert_eq!(fs::read(&copied).unwrap(), b"source");
        assert_eq!(fs::metadata(&copied).unwrap().mode() & 0o777, 0o600);

        let link = root.join("source-link.wav");
        symlink(&source, &link).unwrap();
        assert!(copy_source_no_follow(&link, &root.join("linked-copy")).is_err());
        assert!(
            copy_source_no_follow(&root.join("vanished.wav"), &root.join("vanished-copy")).is_err()
        );
        assert_eq!(
            copy_source_no_follow(&owned, &root.join("directory-copy")).unwrap_err(),
            "selected audio must be a nonempty regular file"
        );
        let empty = root.join("empty.wav");
        fs::write(&empty, []).unwrap();
        assert_eq!(
            copy_source_no_follow(&empty, &root.join("empty-copy")).unwrap_err(),
            "selected audio must be a nonempty regular file"
        );

        let oversized = root.join("oversized.wav");
        let file = fs::File::create(&oversized).unwrap();
        file.set_len(MAX_SOURCE_BYTES + 1).unwrap();
        assert!(copy_source_no_follow(&oversized, &root.join("large-copy")).is_err());
        validate_source_file(true, MAX_SOURCE_BYTES).unwrap();
        assert!(validate_source_file(true, MAX_SOURCE_BYTES + 1).is_err());
        assert_eq!(source_copy_limit(), MAX_SOURCE_BYTES + 1);
        assert!(validate_copied_size(0, MAX_SOURCE_BYTES).is_err());
        validate_copied_size(MAX_SOURCE_BYTES, MAX_SOURCE_BYTES).unwrap();
        assert!(validate_copied_size(MAX_SOURCE_BYTES + 1, MAX_SOURCE_BYTES).is_err());

        let target = root.join("target");
        fs::create_dir(&target).unwrap();
        let linked_owned = root.join("linked-owned");
        symlink(&target, &linked_owned).unwrap();
        assert!(create_private_directory(&linked_owned).is_err());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn wav_duration_and_pruning_contracts_are_independently_observable() {
        for rejected in [f64::NAN, f64::INFINITY, -1.0, 0.0, 30.01] {
            assert!(validate_duration(rejected).is_err());
        }
        validate_duration(0.01).unwrap();
        validate_duration(30.0).unwrap();
        assert_eq!(installed_read_limit(), MAX_INSTALLED_BYTES + 1);
        assert!(validate_installed_size(0, "fixture").is_err());
        validate_installed_size(MAX_INSTALLED_BYTES, "fixture").unwrap();
        assert!(validate_installed_size(MAX_INSTALLED_BYTES + 1, "fixture").is_err());
        assert!(destination_is_missing(ErrorKind::NotFound));
        assert!(!destination_is_missing(ErrorKind::PermissionDenied));

        let root = root("prune");
        fs::create_dir_all(&root).unwrap();
        let keep = "zentty-custom-00000000000000000000000000000000.wav";
        let remove = "zentty-custom-11111111111111111111111111111111.wav";
        fs::write(root.join(keep), b"keep").unwrap();
        fs::write(root.join(remove), b"remove").unwrap();
        fs::write(root.join("not-owned.wav"), b"outside").unwrap();
        let symlink_name = "zentty-custom-22222222222222222222222222222222.wav";
        symlink(root.join("not-owned.wav"), root.join(symlink_name)).unwrap();
        prune_locked(&root, Some(keep)).unwrap();
        assert!(root.join(keep).is_file());
        assert!(!root.join(remove).exists());
        assert!(root.join("not-owned.wav").is_file());
        assert!(root.join(symlink_name).is_symlink());

        let bad_wav = root.join("bad.wav");
        fs::write(&bad_wav, [0_u8; 44]).unwrap();
        assert_eq!(
            validate_wav(&bad_wav).unwrap_err(),
            "converted audio does not have a PCM WAV container"
        );
        let truncated_wav = root.join("truncated.wav");
        fs::write(&truncated_wav, b"RIFF").unwrap();
        assert_eq!(
            validate_wav(&truncated_wav).unwrap_err(),
            "converted audio is not a complete WAV file"
        );
        assert_eq!(
            validate_wav(&root).unwrap_err(),
            "converted audio is not a complete WAV file"
        );

        for (name, header) in [
            ("riff-only.wav", *b"RIFF00000000"),
            ("wave-only.wav", *b"00000000WAVE"),
        ] {
            let path = root.join(name);
            fs::write(&path, header).unwrap();
            fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .unwrap()
                .set_len(44)
                .unwrap();
            assert_eq!(
                validate_wav(&path).unwrap_err(),
                "converted audio does not have a PCM WAV container"
            );
        }

        let maximum = root.join("maximum.wav");
        fs::write(&maximum, b"RIFF0000WAVE").unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&maximum)
            .unwrap()
            .set_len(MAX_INSTALLED_BYTES)
            .unwrap();
        validate_wav(&maximum).unwrap();
        let oversized_wav = root.join("oversized.wav");
        fs::copy(&maximum, &oversized_wav).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&oversized_wav)
            .unwrap()
            .set_len(MAX_INSTALLED_BYTES + 1)
            .unwrap();
        assert_eq!(
            validate_wav(&oversized_wav).unwrap_err(),
            format!("converted audio exceeds the {MAX_INSTALLED_BYTES} byte installed limit")
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn real_conversion_deduplicates_and_rollback_preserves_the_committed_asset() {
        let root = root("conversion");
        let sounds = root.join("sounds");
        fs::create_dir_all(&sounds).unwrap();
        let source = root.join("Friendly tone.wav");
        let status = std::process::Command::new("/usr/bin/ffmpeg")
            .args([
                "-nostdin",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=0.1",
                "-y",
            ])
            .arg(&source)
            .status()
            .unwrap();
        assert!(status.success());

        let first_transaction = root.join("transaction-1");
        create_private_directory(&first_transaction).unwrap();
        let first = prepare_in_transaction(
            &source,
            &sounds,
            &first_transaction,
            acquire_lock(&sounds).unwrap(),
        )
        .unwrap();
        let name = first.internal_name().to_owned();
        assert_eq!(first.display_name(), "Friendly tone.wav");
        assert!(CustomSoundStore::is_custom_name(&name));
        CustomSoundStore::finish(first).unwrap();
        let installed = sounds.join(&name);
        let installed_bytes = fs::read(&installed).unwrap();

        let second_transaction = root.join("transaction-2");
        create_private_directory(&second_transaction).unwrap();
        let second = prepare_in_transaction(
            &source,
            &sounds,
            &second_transaction,
            acquire_lock(&sounds).unwrap(),
        )
        .unwrap();
        assert_eq!(second.internal_name(), name);
        CustomSoundStore::rollback(second);
        assert_eq!(fs::read(&installed).unwrap(), installed_bytes);

        let uncommitted_source = root.join("Uncommitted tone.wav");
        let status = std::process::Command::new("/usr/bin/ffmpeg")
            .args([
                "-nostdin",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=880:duration=0.1",
                "-y",
            ])
            .arg(&uncommitted_source)
            .status()
            .unwrap();
        assert!(status.success());
        let uncommitted_transaction = root.join("transaction-uncommitted");
        create_private_directory(&uncommitted_transaction).unwrap();
        let uncommitted = prepare_in_transaction(
            &uncommitted_source,
            &sounds,
            &uncommitted_transaction,
            acquire_lock(&sounds).unwrap(),
        )
        .unwrap();
        let uncommitted_path = uncommitted.installed_path.clone();
        assert!(uncommitted_path.is_file());
        drop(uncommitted);
        assert!(!uncommitted_path.exists());
        assert!(!uncommitted_transaction.exists());
        assert!(installed.is_file());

        let invalid_store = root.join("not-a-sounds-directory");
        fs::write(&invalid_store, b"regular file").unwrap();
        let invalid_transaction = root.join("transaction-invalid-store");
        create_private_directory(&invalid_transaction).unwrap();
        let Err(error) = prepare_in_transaction(
            &source,
            &invalid_store,
            &invalid_transaction,
            acquire_lock(&sounds).unwrap(),
        ) else {
            panic!("regular-file store unexpectedly accepted");
        };
        assert!(error.starts_with("could not inspect custom sound destination:"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn corrupt_audio_is_rejected_without_publishing_an_asset() {
        let root = root("corrupt");
        let sounds = root.join("sounds");
        let transaction = root.join("transaction");
        fs::create_dir_all(&sounds).unwrap();
        create_private_directory(&transaction).unwrap();
        let source = root.join("not-audio.bin");
        fs::write(&source, b"this is not an audio stream").unwrap();

        let result = prepare_in_transaction(
            &source,
            &sounds,
            &transaction,
            acquire_lock(&sounds).unwrap(),
        );
        assert!(result.is_err());
        assert_eq!(
            fs::read_dir(&sounds)
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| CustomSoundStore::is_custom_name(
                    &entry.file_name().to_string_lossy()
                ))
                .count(),
            0
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn store_lock_serializes_concurrent_transactions() {
        let root = root("locking");
        let sounds = root.join("sounds");
        fs::create_dir_all(&sounds).unwrap();
        let first = acquire_lock(&sounds).unwrap();
        let second_sounds = sounds.clone();
        let (sender, receiver) = mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let lock = acquire_lock(&second_sounds).unwrap();
            sender.send(()).unwrap();
            drop(lock);
        });

        assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());
        drop(first);
        receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        waiter.join().unwrap();

        let held = acquire_lock(&sounds).unwrap();
        assert_eq!(
            acquire_lock_with_timeout(&sounds, Duration::from_millis(30)).unwrap_err(),
            "timed out waiting for custom sound transaction"
        );
        drop(held);
        fs::remove_dir_all(&root).unwrap();
    }
}
