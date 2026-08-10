use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use zentty_core::{
    RemoteTransferFailure, RemoteUploadPath, RemoteVerificationPlan, SshDestination,
    scp_connection_arguments, ssh_connection_arguments,
};

const MAXIMUM_DIAGNOSTIC_BYTES: u64 = 64 * 1024;
const CHILD_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Debug)]
pub struct RemoteTransferRequest {
    pub source: PathBuf,
    pub destination: SshDestination,
    pub upload_path: RemoteUploadPath,
    pub maximum_bytes: u64,
    pub timeout: Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedLocalUpload {
    byte_count: u64,
    sha256: String,
}

impl PreparedLocalUpload {
    #[must_use]
    pub const fn byte_count(&self) -> u64 {
        self.byte_count
    }

    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteTransferReceipt {
    remote_path: String,
    byte_count: u64,
    sha256: String,
}

impl RemoteTransferReceipt {
    #[must_use]
    pub fn remote_path(&self) -> &str {
        &self.remote_path
    }

    #[must_use]
    pub const fn byte_count(&self) -> u64 {
        self.byte_count
    }

    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteTransferError {
    pub failure: RemoteTransferFailure,
    pub detail: String,
}

impl RemoteTransferError {
    fn new(failure: RemoteTransferFailure, detail: impl Into<String>) -> Self {
        Self {
            failure,
            detail: detail.into(),
        }
    }
}

struct OpenLocalUpload {
    file: File,
    prepared: PreparedLocalUpload,
}

/// Opens and hashes a regular local file without following a final-component
/// symbolic link. The same open file description is retained for transfer so
/// a later path substitution cannot change the uploaded bytes.
///
/// # Errors
///
/// Returns a classified transfer error for an unreadable, non-regular, or
/// oversized source.
pub fn prepare_local_upload(
    source: &Path,
    maximum_bytes: u64,
) -> Result<PreparedLocalUpload, RemoteTransferError> {
    open_local_upload(source, maximum_bytes, None, None).map(|upload| upload.prepared)
}

fn open_local_upload(
    source: &Path,
    maximum_bytes: u64,
    cancelled: Option<&AtomicBool>,
    deadline: Option<Instant>,
) -> Result<OpenLocalUpload, RemoteTransferError> {
    check_preparation_control(cancelled, deadline)?;
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(source)
        .map_err(|error| {
            RemoteTransferError::new(
                RemoteTransferFailure::PermissionDenied,
                format!("could not securely open local upload: {error}"),
            )
        })?;
    let metadata = file.metadata().map_err(|error| {
        RemoteTransferError::new(
            RemoteTransferFailure::Ambiguous,
            format!("could not inspect local upload: {error}"),
        )
    })?;
    if !metadata.is_file() {
        return Err(RemoteTransferError::new(
            RemoteTransferFailure::PermissionDenied,
            "local upload is not a regular file",
        ));
    }
    if metadata.len() > maximum_bytes {
        return Err(RemoteTransferError::new(
            RemoteTransferFailure::PermissionDenied,
            format!(
                "local upload is {} bytes; limit is {maximum_bytes}",
                metadata.len()
            ),
        ));
    }

    let mut digest = Sha256::new();
    let mut copied = 0u64;
    let mut buffer = vec![0u8; 64 * 1024].into_boxed_slice();
    loop {
        check_preparation_control(cancelled, deadline)?;
        let count = file.read(&mut buffer).map_err(|error| {
            RemoteTransferError::new(
                RemoteTransferFailure::Ambiguous,
                format!("could not hash local upload: {error}"),
            )
        })?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
        copied = copied.saturating_add(u64::try_from(count).unwrap_or(u64::MAX));
    }
    if copied != metadata.len() {
        return Err(RemoteTransferError::new(
            RemoteTransferFailure::IntegrityMismatch,
            "local upload changed while it was being hashed",
        ));
    }
    file.seek(SeekFrom::Start(0)).map_err(|error| {
        RemoteTransferError::new(
            RemoteTransferFailure::Ambiguous,
            format!("could not rewind local upload: {error}"),
        )
    })?;
    Ok(OpenLocalUpload {
        file,
        prepared: PreparedLocalUpload {
            byte_count: copied,
            sha256: format!("{:x}", digest.finalize()),
        },
    })
}

/// Executes one transactional remote upload.
///
/// # Errors
///
/// Returns a classified error without publishing a remote path when source
/// validation, transport, integrity verification, or atomic publication fails.
pub fn execute_remote_transfer(
    request: &RemoteTransferRequest,
    cancelled: &AtomicBool,
) -> Result<RemoteTransferReceipt, RemoteTransferError> {
    let deadline = Instant::now().checked_add(request.timeout).ok_or_else(|| {
        RemoteTransferError::new(
            RemoteTransferFailure::Timeout,
            "upload timeout is out of range",
        )
    })?;
    let mut upload = open_local_upload(
        &request.source,
        request.maximum_bytes,
        Some(cancelled),
        Some(deadline),
    )?;
    let upload_path = request
        .upload_path
        .with_transport_nonce(&secure_nonce()?)
        .map_err(|_| {
            RemoteTransferError::new(
                RemoteTransferFailure::Ambiguous,
                "internal transport nonce validation failed",
            )
        })?;
    let plan = RemoteVerificationPlan::new(
        upload_path,
        upload.prepared.byte_count,
        &upload.prepared.sha256,
    )
    .map_err(|_| {
        RemoteTransferError::new(
            RemoteTransferFailure::Ambiguous,
            "internal SHA-256 validation failed",
        )
    })?;
    run_ssh_script(
        &request.destination,
        &preflight_script(plan.upload_path()),
        None,
        cancelled,
        deadline,
    )?;

    let scp_result = stage_for_scp(&mut upload.file, cancelled, deadline).and_then(|staging| {
        run_scp(
            &request.destination,
            &staging.path,
            plan.upload_path().partial_path(),
            cancelled,
            deadline,
        )
    });
    match scp_result {
        Ok(()) => {}
        Err(error) if error.failure.permits_stream_fallback() => {
            upload.file.seek(SeekFrom::Start(0)).map_err(|seek_error| {
                RemoteTransferError::new(
                    RemoteTransferFailure::Ambiguous,
                    format!("could not rewind local upload for fallback: {seek_error}"),
                )
            })?;
            let script = stream_script(&plan);
            if let Err(stream_error) = run_ssh_script(
                &request.destination,
                &script,
                Some(upload.file),
                cancelled,
                deadline,
            ) {
                cleanup_remote_partial(&request.destination, plan.upload_path());
                return Err(stream_error);
            }
            return Ok(receipt(&plan, &upload.prepared));
        }
        Err(error) => {
            cleanup_remote_partial(&request.destination, plan.upload_path());
            return Err(error);
        }
    }

    if let Err(error) = run_ssh_script(
        &request.destination,
        &plan.script(),
        None,
        cancelled,
        deadline,
    ) {
        cleanup_remote_partial(&request.destination, plan.upload_path());
        return Err(error);
    }
    Ok(receipt(&plan, &upload.prepared))
}

/// Removes final paths published by earlier successful transfers in a product
/// batch that subsequently failed. Only receipts returned by this module can
/// identify deletion targets.
///
/// # Errors
///
/// Returns a classified SSH error if the bounded rollback cannot remove every
/// supplied final path.
pub fn rollback_remote_transfers(
    destination: &SshDestination,
    receipts: &[RemoteTransferReceipt],
) -> Result<(), RemoteTransferError> {
    if receipts.is_empty() {
        return Ok(());
    }
    let paths = receipts
        .iter()
        .map(|receipt| zentty_core::escape_remote_path_for_shell(receipt.remote_path()))
        .collect::<Vec<_>>()
        .join(" ");
    let script = format!("rm -f {paths}");
    let never_cancelled = AtomicBool::new(false);
    run_ssh_script(
        destination,
        &script,
        None,
        &never_cancelled,
        Instant::now() + Duration::from_secs(10),
    )
}

fn receipt(plan: &RemoteVerificationPlan, prepared: &PreparedLocalUpload) -> RemoteTransferReceipt {
    RemoteTransferReceipt {
        remote_path: plan.upload_path().final_path().to_owned(),
        byte_count: prepared.byte_count,
        sha256: prepared.sha256.clone(),
    }
}

fn preflight_script(path: &RemoteUploadPath) -> String {
    format!(
        "set -eu; umask 077; set -C; : > {} || exit 71",
        zentty_core::escape_remote_path_for_shell(path.partial_path())
    )
}

fn stream_script(plan: &RemoteVerificationPlan) -> String {
    let partial = zentty_core::escape_remote_path_for_shell(plan.upload_path().partial_path());
    format!(
        "set -eu; p={partial}; cleanup() {{ rm -f \"$p\"; }}; \
         trap cleanup EXIT HUP INT TERM; cat > \"$p\"; \
         trap - EXIT HUP INT TERM; {}",
        plan.script()
    )
}

fn cleanup_remote_partial(destination: &SshDestination, path: &RemoteUploadPath) {
    let script = format!(
        "rm -f {}",
        zentty_core::escape_remote_path_for_shell(path.partial_path())
    );
    let never_cancelled = AtomicBool::new(false);
    let _ = run_ssh_script(
        destination,
        &script,
        None,
        &never_cancelled,
        Instant::now() + Duration::from_secs(10),
    );
}

struct StagingFile {
    path: PathBuf,
}

impl Drop for StagingFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn stage_for_scp(
    source: &mut File,
    cancelled: &AtomicBool,
    deadline: Instant,
) -> Result<StagingFile, RemoteTransferError> {
    let nonce = secure_nonce()?;
    let path = std::env::temp_dir().join(format!("zentty-upload-{}-{nonce}", std::process::id()));
    let staging_guard = StagingFile { path };
    let mut staging = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&staging_guard.path)
        .map_err(|error| {
            RemoteTransferError::new(
                RemoteTransferFailure::Ambiguous,
                format!("could not create private local staging file: {error}"),
            )
        })?;
    let mut buffer = vec![0u8; 64 * 1024].into_boxed_slice();
    loop {
        check_preparation_control(Some(cancelled), Some(deadline))?;
        let count = source.read(&mut buffer).map_err(|error| {
            RemoteTransferError::new(
                RemoteTransferFailure::Ambiguous,
                format!("could not read local upload for staging: {error}"),
            )
        })?;
        if count == 0 {
            break;
        }
        staging.write_all(&buffer[..count]).map_err(|error| {
            RemoteTransferError::new(
                RemoteTransferFailure::DiskFull,
                format!("could not stage local upload: {error}"),
            )
        })?;
    }
    staging.flush().map_err(|error| {
        RemoteTransferError::new(
            RemoteTransferFailure::DiskFull,
            format!("could not flush local staging file: {error}"),
        )
    })?;
    Ok(staging_guard)
}

fn check_preparation_control(
    cancelled: Option<&AtomicBool>,
    deadline: Option<Instant>,
) -> Result<(), RemoteTransferError> {
    if cancelled.is_some_and(|cancelled| cancelled.load(Ordering::Acquire)) {
        return Err(cancelled_error());
    }
    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        return Err(RemoteTransferError::new(
            RemoteTransferFailure::Timeout,
            "remote upload exceeded its deadline",
        ));
    }
    Ok(())
}

fn secure_nonce() -> Result<String, RemoteTransferError> {
    let mut entropy = [0u8; 16];
    File::open("/dev/urandom")
        .and_then(|mut random| random.read_exact(&mut entropy))
        .map_err(|error| {
            RemoteTransferError::new(
                RemoteTransferFailure::Ambiguous,
                format!("could not obtain staging entropy: {error}"),
            )
        })?;
    let nonce = entropy.iter().fold(String::new(), |mut nonce, byte| {
        std::fmt::Write::write_fmt(&mut nonce, format_args!("{byte:02x}"))
            .expect("formatting into a string cannot fail");
        nonce
    });
    Ok(nonce)
}

fn run_scp(
    destination: &SshDestination,
    source: &Path,
    remote_path: &str,
    cancelled: &AtomicBool,
    deadline: Instant,
) -> Result<(), RemoteTransferError> {
    let mut arguments = scp_connection_arguments(destination)
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    arguments.push("--".into());
    arguments.push(source.as_os_str().to_owned());
    arguments.push(format!("{}:{remote_path}", destination.target).into());
    let outcome = run_child("scp", &arguments, None, cancelled, deadline)
        .map_err(|error| classify_scp_spawn_error(&error))?;
    classify_outcome(outcome, true)
}

fn classify_scp_spawn_error(error: &std::io::Error) -> RemoteTransferError {
    if error.kind() == std::io::ErrorKind::NotFound {
        RemoteTransferError::new(
            RemoteTransferFailure::LocalScpMissing,
            "local scp executable is unavailable",
        )
    } else {
        RemoteTransferError::new(
            RemoteTransferFailure::Ambiguous,
            format!("could not start scp: {error}"),
        )
    }
}

fn run_ssh_script(
    destination: &SshDestination,
    script: &str,
    stdin: Option<File>,
    cancelled: &AtomicBool,
    deadline: Instant,
) -> Result<(), RemoteTransferError> {
    let mut arguments = ssh_connection_arguments(destination)
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    arguments.push("--".into());
    arguments.push(destination.target.clone().into());
    arguments.push(script.into());
    let outcome = run_child("ssh", &arguments, stdin, cancelled, deadline).map_err(|error| {
        RemoteTransferError::new(
            RemoteTransferFailure::Ambiguous,
            format!("could not start ssh: {error}"),
        )
    })?;
    classify_outcome(outcome, false)
}

struct ChildOutcome {
    status: ExitStatus,
    stderr: String,
    write_error: Option<String>,
    termination: Option<RemoteTransferFailure>,
}

fn run_child(
    program: &str,
    arguments: &[OsString],
    stdin: Option<File>,
    cancelled: &AtomicBool,
    deadline: Instant,
) -> std::io::Result<ChildOutcome> {
    let mut child = Command::new(program)
        .args(arguments)
        .env("LC_ALL", "C")
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    let stderr = child.stderr.take().expect("piped child stderr");
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr
            .take(MAXIMUM_DIAGNOSTIC_BYTES)
            .read_to_end(&mut bytes)
            .map(|_| String::from_utf8_lossy(&bytes).into_owned())
    });
    let writer = stdin.map(|mut source| {
        let mut child_stdin = child.stdin.take().expect("piped child stdin");
        thread::spawn(move || {
            std::io::copy(&mut source, &mut child_stdin)
                .and_then(|_| child_stdin.flush())
                .map_err(|error| error.to_string())
        })
    });
    let (status, termination) = wait_for_child(&mut child, cancelled, deadline)?;
    let write_error = writer.and_then(|writer| match writer.join() {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error),
        Err(_) => Some("upload writer panicked".to_owned()),
    });
    let stderr = stderr_reader
        .join()
        .ok()
        .and_then(Result::ok)
        .unwrap_or_else(|| "child diagnostics unavailable".to_owned());
    Ok(ChildOutcome {
        status,
        stderr,
        write_error,
        termination,
    })
}

fn wait_for_child(
    child: &mut Child,
    cancelled: &AtomicBool,
    deadline: Instant,
) -> std::io::Result<(ExitStatus, Option<RemoteTransferFailure>)> {
    loop {
        if cancelled.load(Ordering::Acquire) {
            let _ = child.kill();
            return child
                .wait()
                .map(|status| (status, Some(RemoteTransferFailure::Cancelled)));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            return child
                .wait()
                .map(|status| (status, Some(RemoteTransferFailure::Timeout)));
        }
        if let Some(status) = child.try_wait()? {
            return Ok((status, None));
        }
        thread::sleep(CHILD_POLL_INTERVAL);
    }
}

fn classify_outcome(outcome: ChildOutcome, scp: bool) -> Result<(), RemoteTransferError> {
    if let Some(termination) = outcome.termination {
        return Err(RemoteTransferError::new(
            termination,
            if termination == RemoteTransferFailure::Cancelled {
                "remote upload was cancelled"
            } else {
                "remote upload exceeded its deadline"
            },
        ));
    }
    if outcome.status.success() {
        if let Some(error) = outcome.write_error {
            return Err(RemoteTransferError::new(
                RemoteTransferFailure::IntegrityMismatch,
                format!("upload input failed: {error}"),
            ));
        }
        return Ok(());
    }
    let stderr = outcome.stderr.to_ascii_lowercase();
    let code = outcome.status.code();
    let (failure, detail) = if code == Some(70) {
        (
            RemoteTransferFailure::IntegrityMismatch,
            "remote integrity check failed",
        )
    } else if code == Some(71) {
        (
            RemoteTransferFailure::DestinationCollision,
            "remote upload path already exists",
        )
    } else if code == Some(72) {
        (
            RemoteTransferFailure::Ambiguous,
            "remote SHA-256 tool is unavailable",
        )
    } else if scp && stderr.contains("subsystem request failed") {
        (
            RemoteTransferFailure::SftpSubsystemUnavailable,
            "remote SFTP subsystem is unavailable",
        )
    } else if stderr.contains("host key verification failed")
        || stderr.contains("remote host identification has changed")
    {
        (
            RemoteTransferFailure::HostVerification,
            "SSH host verification failed",
        )
    } else if stderr.contains("permission denied") {
        (
            RemoteTransferFailure::Authentication,
            "SSH authentication failed",
        )
    } else if stderr.contains("connection timed out") {
        (RemoteTransferFailure::Timeout, "SSH connection timed out")
    } else if stderr.contains("could not resolve hostname")
        || stderr.contains("no route to host")
        || stderr.contains("connection refused")
    {
        (
            RemoteTransferFailure::HostUnreachable,
            "SSH host is unreachable",
        )
    } else {
        (
            RemoteTransferFailure::Ambiguous,
            "remote transfer command failed",
        )
    };
    Err(RemoteTransferError::new(failure, detail))
}

fn cancelled_error() -> RemoteTransferError {
    RemoteTransferError::new(
        RemoteTransferFailure::Cancelled,
        "remote upload was cancelled",
    )
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::process::ExitStatusExt;
    use std::sync::Arc;

    use zentty_core::{RemoteUploadPath, RemoteVerificationPlan};

    use super::{
        ChildOutcome, MAXIMUM_DIAGNOSTIC_BYTES, RemoteTransferFailure, classify_outcome,
        classify_scp_spawn_error, preflight_script, run_child, stream_script,
    };

    fn failed(code: i32, stderr: &str) -> ChildOutcome {
        ChildOutcome {
            status: std::process::ExitStatus::from_raw(code << 8),
            stderr: stderr.to_owned(),
            write_error: None,
            termination: None,
        }
    }

    #[test]
    fn command_scripts_and_diagnostic_bound_are_exact() {
        assert_eq!(MAXIMUM_DIAGNOSTIC_BYTES, 65_536);
        let path = RemoteUploadPath::for_file("payload", 1, "1234abcd").unwrap();
        assert_eq!(
            preflight_script(&path),
            "set -eu; umask 077; set -C; : > /tmp/zentty-paste-1-1234abcd-payload.partial-1234abcd || exit 71"
        );
        let plan = RemoteVerificationPlan::new(
            path,
            7,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        .unwrap();
        let stream = stream_script(&plan);
        assert!(stream.starts_with("set -eu; p=/tmp/zentty-paste-1-1234abcd-payload.partial"));
        assert!(stream.contains("trap cleanup EXIT HUP INT TERM; cat > \"$p\";"));
        assert!(stream.ends_with("rm -f \"$p\"; trap - EXIT HUP INT TERM"));
    }

    #[test]
    fn every_process_failure_classification_is_distinct() {
        for (outcome, scp, expected) in [
            (
                failed(70, ""),
                false,
                RemoteTransferFailure::IntegrityMismatch,
            ),
            (
                failed(71, ""),
                false,
                RemoteTransferFailure::DestinationCollision,
            ),
            (failed(72, ""), false, RemoteTransferFailure::Ambiguous),
            (
                failed(255, "subsystem request failed on channel 0"),
                true,
                RemoteTransferFailure::SftpSubsystemUnavailable,
            ),
            (
                failed(255, "subsystem request failed on channel 0"),
                false,
                RemoteTransferFailure::Ambiguous,
            ),
            (
                failed(255, "Host key verification failed"),
                false,
                RemoteTransferFailure::HostVerification,
            ),
            (
                failed(255, "REMOTE HOST IDENTIFICATION HAS CHANGED"),
                false,
                RemoteTransferFailure::HostVerification,
            ),
            (
                failed(255, "Permission denied (publickey)"),
                false,
                RemoteTransferFailure::Authentication,
            ),
            (
                failed(255, "Connection timed out"),
                false,
                RemoteTransferFailure::Timeout,
            ),
            (
                failed(255, "Could not resolve hostname"),
                false,
                RemoteTransferFailure::HostUnreachable,
            ),
            (
                failed(255, "No route to host"),
                false,
                RemoteTransferFailure::HostUnreachable,
            ),
            (
                failed(255, "Connection refused"),
                false,
                RemoteTransferFailure::HostUnreachable,
            ),
            (
                failed(1, "unknown"),
                false,
                RemoteTransferFailure::Ambiguous,
            ),
        ] {
            assert_eq!(
                classify_outcome(outcome, scp).unwrap_err().failure,
                expected
            );
        }
    }

    #[test]
    fn missing_scp_is_the_only_spawn_error_that_permits_fallback() {
        assert_eq!(
            classify_scp_spawn_error(&std::io::ErrorKind::NotFound.into()).failure,
            RemoteTransferFailure::LocalScpMissing
        );
        assert_eq!(
            classify_scp_spawn_error(&std::io::ErrorKind::PermissionDenied.into()).failure,
            RemoteTransferFailure::Ambiguous
        );
    }

    #[test]
    fn real_child_cancellation_is_distinct_from_timeout() {
        let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let signal = Arc::clone(&cancelled);
        let signal_thread = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(30));
            signal.store(true, std::sync::atomic::Ordering::Release);
        });
        let outcome = run_child(
            "sh",
            &[OsString::from("-c"), OsString::from("exec sleep 5")],
            None,
            &cancelled,
            std::time::Instant::now() + std::time::Duration::from_secs(2),
        )
        .unwrap();
        signal_thread.join().unwrap();
        let error = classify_outcome(outcome, false).unwrap_err();
        assert_eq!(error.failure, RemoteTransferFailure::Cancelled);
        assert_eq!(error.detail, "remote upload was cancelled");

        let outcome = run_child(
            "sh",
            &[OsString::from("-c"), OsString::from("exec sleep 5")],
            None,
            &std::sync::atomic::AtomicBool::new(false),
            std::time::Instant::now() + std::time::Duration::from_millis(30),
        )
        .unwrap();
        let error = classify_outcome(outcome, false).unwrap_err();
        assert_eq!(error.failure, RemoteTransferFailure::Timeout);
        assert_eq!(error.detail, "remote upload exceeded its deadline");
    }
}
