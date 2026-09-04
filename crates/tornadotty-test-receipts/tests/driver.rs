use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{self, Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use tornadotty_test_receipts::{LifecycleState, ReceiptEvent, ReceiptId, ReceiptWriter};

static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "tornadotty-driver-test-{}-{sequence}",
            process::id()
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        Self { root }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

fn driver() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tornadotty-journey-driver"))
}

fn spawn_wait(
    path: &std::path::Path,
    expected_transient: &str,
) -> (Child, BufReader<std::process::ChildStderr>) {
    let mut child = driver()
        .args(["wait"])
        .arg(path)
        .args(["1000", "1", "process-started"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stderr = BufReader::new(child.stderr.take().unwrap());
    let mut boundary = String::new();
    stderr.read_line(&mut boundary).unwrap();
    assert!(boundary.starts_with("journey-phase=receipt-wait "));
    boundary.clear();
    stderr.read_line(&mut boundary).unwrap();
    assert_eq!(
        boundary.trim_end(),
        format!("journey-phase=receipt-transient state={expected_transient}")
    );
    (child, stderr)
}

#[test]
fn driver_validates_and_matches_typed_running_and_complete_streams() {
    let fixture = Fixture::new();
    let receipt = fixture.root.join("events.ndjson");
    let mut writer = ReceiptWriter::create(&receipt).unwrap();
    writer
        .write(ReceiptEvent::Lifecycle {
            state: LifecycleState::ProcessStarted,
            pane_id: None,
        })
        .unwrap();
    writer
        .write(ReceiptEvent::Lifecycle {
            state: LifecycleState::TerminalReady,
            pane_id: Some(ReceiptId::new("pane-1").unwrap()),
        })
        .unwrap();

    let active = driver().arg("validate").arg(&receipt).output().unwrap();
    assert!(
        active.status.success(),
        "{}",
        String::from_utf8_lossy(&active.stderr)
    );
    assert!(String::from_utf8_lossy(&active.stdout).contains("complete=false"));

    let matched = driver()
        .args(["wait"])
        .arg(&receipt)
        .args(["200", "1", "terminal-ready", "pane-1"])
        .output()
        .unwrap();
    assert!(
        matched.status.success(),
        "{}",
        String::from_utf8_lossy(&matched.stderr)
    );
    assert!(String::from_utf8_lossy(&matched.stdout).contains("\"pane_id\":\"pane-1\""));

    let incomplete = driver()
        .arg("validate")
        .arg(&receipt)
        .arg("--complete")
        .output()
        .unwrap();
    assert!(!incomplete.status.success());
    writer
        .write(ReceiptEvent::Lifecycle {
            state: LifecycleState::ProcessStopped,
            pane_id: None,
        })
        .unwrap();
    drop(writer);
    assert!(
        driver()
            .arg("validate")
            .arg(&receipt)
            .arg("--complete")
            .status()
            .unwrap()
            .success()
    );
}

#[test]
fn driver_rejects_malformed_evidence_and_expires_a_missing_match() {
    let fixture = Fixture::new();
    let malformed = fixture.root.join("malformed.ndjson");
    fs::write(&malformed, b"{\n").unwrap();
    fs::set_permissions(&malformed, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(
        !driver()
            .arg("validate")
            .arg(&malformed)
            .status()
            .unwrap()
            .success()
    );
    let malformed_wait = driver()
        .args(["wait"])
        .arg(&malformed)
        .args(["1000", "1", "process-started"])
        .output()
        .unwrap();
    assert!(!malformed_wait.status.success());
    assert!(!String::from_utf8_lossy(&malformed_wait.stderr).contains("deadline expired"));

    let receipt = fixture.root.join("events.ndjson");
    let mut writer = ReceiptWriter::create(&receipt).unwrap();
    writer
        .write(ReceiptEvent::Lifecycle {
            state: LifecycleState::ProcessStarted,
            pane_id: None,
        })
        .unwrap();
    let timeout = driver()
        .args(["wait"])
        .arg(&receipt)
        .args(["80", "1", "terminal-ready", "pane-missing"])
        .output()
        .unwrap();
    assert!(!timeout.status.success());
    assert!(String::from_utf8_lossy(&timeout.stderr).contains("receipt deadline expired"));
}

#[test]
fn driver_waits_for_typed_not_created_and_partial_record_states_without_timers() {
    let fixture = Fixture::new();

    let absent = fixture.root.join("created-after-wait.ndjson");
    let (waiting, _stderr) = spawn_wait(&absent, "NotCreated");
    let mut writer = ReceiptWriter::create(&absent).unwrap();
    writer
        .write(ReceiptEvent::Lifecycle {
            state: LifecycleState::ProcessStarted,
            pane_id: None,
        })
        .unwrap();
    drop(writer);
    assert!(waiting.wait_with_output().unwrap().status.success());

    let partial = fixture.root.join("partial-at-wait.ndjson");
    fs::write(&partial, b"{").unwrap();
    fs::set_permissions(&partial, fs::Permissions::from_mode(0o600)).unwrap();
    let (waiting, _stderr) = spawn_wait(&partial, "PartialRecord");
    fs::write(
        &partial,
        b"{\"schema_version\":1,\"sequence\":1,\"event\":{\"category\":\"lifecycle\",\"state\":\"process_started\"}}\n",
    )
    .unwrap();
    assert!(waiting.wait_with_output().unwrap().status.success());
}
