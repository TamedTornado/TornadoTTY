use std::fs::{self, File};
use std::io::Write;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use zentty_core::{RemoteTransferFailure, RemoteUploadPath, SshDestination};
use zentty_linux::remote_transfer::{
    RemoteTransferRequest, execute_remote_transfer, prepare_local_upload, rollback_remote_transfers,
};

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "zentty-remote-transfer-{label}-{}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("scratch directory must be unique");
        Self(path)
    }
}

struct DisposableSshd {
    child: Child,
    root: PathBuf,
    target: String,
    port: u16,
}

impl DisposableSshd {
    fn start(sftp_mode: &str) -> Self {
        let library =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../linux/tests/lib/disposable-sshd");
        let script = r#"
source "$1"
zentty_disposable_sshd_start
printf '%s\n%s\n%s\n' "$ZENTTY_DISPOSABLE_SSHD_ROOT" "$ZENTTY_DISPOSABLE_SSH_TARGET" "$ZENTTY_DISPOSABLE_SSHD_PORT"
IFS= read -r _
zentty_disposable_sshd_stop
"#;
        let mut child = Command::new("bash")
            .args(["-c", script, "zentty-real-sshd"])
            .arg(library)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .env("ZENTTY_DISPOSABLE_SSHD_SFTP", sftp_mode)
            .spawn()
            .expect("disposable sshd supervisor must start");
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        let root = PathBuf::from(line.trim());
        line.clear();
        stdout.read_line(&mut line).unwrap();
        let target = line.trim().to_owned();
        line.clear();
        stdout.read_line(&mut line).unwrap();
        let port = line.trim().parse().unwrap();
        assert!(root.is_dir(), "disposable sshd did not become ready");
        Self {
            child,
            root,
            target,
            port,
        }
    }

    fn destination(&self) -> SshDestination {
        let port = self.port.to_string();
        let identity = self.root.join("client-key");
        zentty_core::parse_ssh_destination(&[
            "ssh",
            "-F",
            "/dev/null",
            "-o",
            "StrictHostKeyChecking=no",
            "-o",
            "UserKnownHostsFile=/dev/null",
            "-o",
            "IdentitiesOnly=yes",
            "-i",
            identity.to_str().unwrap(),
            "-p",
            &port,
            &self.target,
        ])
        .unwrap()
    }
}

impl Drop for DisposableSshd {
    fn drop(&mut self) {
        self.child
            .stdin
            .take()
            .unwrap()
            .write_all(b"stop\n")
            .unwrap();
        assert!(self.child.wait().unwrap().success());
    }
}

#[test]
fn cancellation_and_expired_deadline_precede_all_network_activity() {
    let scratch = ScratchDirectory::new("control");
    let source = scratch.0.join("missing-payload");
    let request = RemoteTransferRequest {
        source,
        destination: SshDestination::new("unreachable.invalid", None, "unreachable.invalid", None),
        upload_path: RemoteUploadPath::for_file("payload", 1, "1234abcd").unwrap(),
        maximum_bytes: 100,
        timeout: Duration::from_secs(1),
    };
    assert_eq!(
        execute_remote_transfer(&request, &AtomicBool::new(true))
            .unwrap_err()
            .failure,
        RemoteTransferFailure::Cancelled
    );
    let expired = RemoteTransferRequest {
        timeout: Duration::ZERO,
        ..request
    };
    assert_eq!(
        execute_remote_transfer(&expired, &AtomicBool::new(false))
            .unwrap_err()
            .failure,
        RemoteTransferFailure::Timeout
    );
}

#[test]
#[ignore = "real loopback OpenSSH integration; qualification runs linux/tests/remote-transfer"]
fn production_executor_publishes_verified_bytes_through_real_openssh() {
    let server = DisposableSshd::start("true");
    let scratch = ScratchDirectory::new("real-ssh");
    let source = scratch.0.join("payload");
    fs::write(&source, b"mutation-visible real SSH payload\n").unwrap();
    let expected = prepare_local_upload(&source, 1_024).unwrap();
    let upload_path = RemoteUploadPath::for_file(
        "payload",
        1_900_000_000 + u64::from(std::process::id()),
        &format!("{:08x}", std::process::id()),
    )
    .unwrap();
    let final_path = PathBuf::from(upload_path.final_path());
    let receipt = execute_remote_transfer(
        &RemoteTransferRequest {
            source,
            destination: server.destination(),
            upload_path,
            maximum_bytes: 1_024,
            timeout: Duration::from_secs(20),
        },
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(receipt.remote_path(), final_path.to_str().unwrap());
    assert_eq!(receipt.byte_count(), expected.byte_count());
    assert_eq!(receipt.sha256(), expected.sha256());
    assert_eq!(
        fs::read(&final_path).unwrap(),
        b"mutation-visible real SSH payload\n"
    );
    let local_staging_prefix = format!("zentty-upload-{}-", std::process::id());
    assert!(fs::read_dir("/tmp").unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(&local_staging_prefix)
    }));
    rollback_remote_transfers(&server.destination(), &[receipt]).unwrap();
    assert!(!final_path.exists());
}

#[test]
#[ignore = "real loopback OpenSSH integration; mutation qualification includes ignored tests"]
fn fallback_is_used_only_for_an_unavailable_sftp_subsystem() {
    let scratch = ScratchDirectory::new("fallback-policy");
    let source = scratch.0.join("payload");
    fs::write(&source, b"fallback policy payload\n").unwrap();

    let unavailable = DisposableSshd::start("false");
    let upload_path = unique_upload_path("unavailable");
    let final_path = PathBuf::from(upload_path.final_path());
    execute_remote_transfer(
        &RemoteTransferRequest {
            source: source.clone(),
            destination: unavailable.destination(),
            upload_path,
            maximum_bytes: 1_024,
            timeout: Duration::from_secs(20),
        },
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(fs::read(&final_path).unwrap(), b"fallback policy payload\n");
    fs::remove_file(final_path).unwrap();
    drop(unavailable);

    let failing = DisposableSshd::start("failing");
    let upload_path = unique_upload_path("failing");
    let final_path = PathBuf::from(upload_path.final_path());
    let error = execute_remote_transfer(
        &RemoteTransferRequest {
            source,
            destination: failing.destination(),
            upload_path,
            maximum_bytes: 1_024,
            timeout: Duration::from_secs(20),
        },
        &AtomicBool::new(false),
    )
    .unwrap_err();
    assert_eq!(error.failure, RemoteTransferFailure::Ambiguous);
    assert!(!final_path.exists(), "ambiguous SFTP failure fell back");
    let partial_prefix = format!("{}.", final_path.to_string_lossy());
    assert!(
        fs::read_dir("/tmp").unwrap().all(|entry| {
            !entry
                .unwrap()
                .path()
                .to_string_lossy()
                .starts_with(&partial_prefix)
        }),
        "ambiguous SFTP failure retained a remote partial"
    );
}

fn unique_upload_path(label: &str) -> RemoteUploadPath {
    RemoteUploadPath::for_file(
        label,
        1_910_000_000 + u64::from(std::process::id()),
        &format!("{:08x}", std::process::id()),
    )
    .unwrap()
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("scratch directory should be removable");
    }
}

#[test]
fn local_preparation_hashes_exact_regular_file_bytes() {
    let scratch = ScratchDirectory::new("hash");
    let source = scratch.0.join("payload");
    File::create(&source)
        .unwrap()
        .write_all(b"real remote payload\n")
        .unwrap();

    let prepared = prepare_local_upload(&source, 20).unwrap();
    assert_eq!(prepared.byte_count(), 20);
    assert_eq!(
        prepared.sha256(),
        "fb11570f49cd73e4aab1bef0be28e1b18e72e9ae3fd525f8dd2bfd3e2f6e1114"
    );
}

#[test]
fn local_preparation_rejects_symlinks_directories_and_oversize_files() {
    let scratch = ScratchDirectory::new("reject");
    let source = scratch.0.join("payload");
    fs::write(&source, b"12345").unwrap();
    let link = scratch.0.join("link");
    symlink(&source, &link).unwrap();

    assert_eq!(
        prepare_local_upload(&link, 10).unwrap_err().failure,
        RemoteTransferFailure::PermissionDenied
    );
    assert_eq!(
        prepare_local_upload(&scratch.0, 10).unwrap_err().failure,
        RemoteTransferFailure::PermissionDenied
    );
    assert_eq!(
        prepare_local_upload(&source, 4).unwrap_err().failure,
        RemoteTransferFailure::PermissionDenied
    );
}
