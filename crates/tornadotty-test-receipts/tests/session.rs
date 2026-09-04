use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{self, Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use rustix::process::{Pid, Signal, kill_process};
use serde_json::Value;

static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct Fixture {
    root: PathBuf,
    resource_root: PathBuf,
    supervisors: Vec<(PathBuf, Child)>,
}

impl Fixture {
    fn new() -> Self {
        let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "tornadotty-session-test-{}-{sequence}",
            process::id()
        ));
        let resource_root = root.join("resources");
        create_owner_directory(&root);
        create_owner_directory(&resource_root);
        Self {
            root,
            resource_root,
            supervisors: Vec::new(),
        }
    }

    fn session(&self, name: &str) -> PathBuf {
        let path = self.root.join(name);
        create_owner_directory(&path);
        path
    }

    fn supervise(&mut self, session: &Path, resources: &[&str], command: &[&str]) {
        let mut driver = driver();
        driver
            .args(["session", "supervise"])
            .arg(session)
            .arg(&self.resource_root);
        for resource in resources {
            driver.args(["--resource", resource]);
        }
        let child = driver
            .arg("--")
            .args(command)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        self.supervisors.push((session.to_path_buf(), child));
    }

    fn wait_supervisor(&mut self, session: &Path) -> Output {
        let index = self
            .supervisors
            .iter()
            .position(|(candidate, _)| candidate == session)
            .unwrap();
        let (_, child) = self.supervisors.swap_remove(index);
        child.wait_with_output().unwrap()
    }

    fn stop(session: &Path) -> Output {
        driver()
            .args(["session", "stop"])
            .arg(session)
            .arg("1000")
            .output()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for (session, child) in &mut self.supervisors {
            let _ = driver()
                .args(["session", "stop"])
                .arg(session)
                .arg("200")
                .output();
            let _ = child.wait();
        }
        fs::remove_dir_all(&self.root).unwrap();
    }
}

fn create_owner_directory(path: &Path) {
    fs::create_dir(path).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

fn driver() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tornadotty-journey-driver"))
}

fn wait_running(session: &Path) {
    let output = driver()
        .args(["session", "wait"])
        .arg(session)
        .args(["2000", "running"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn wait_for_file(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "{} was not created",
            path.display()
        );
        thread::yield_now();
    }
}

fn process_is_running(pid: u32) -> bool {
    let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return false;
    };
    stat.rfind(')')
        .and_then(|close| stat.get(close + 1..))
        .and_then(|fields| fields.split_ascii_whitespace().next())
        .is_some_and(|state| state != "Z")
}

fn product_pid(session: &Path) -> u32 {
    let output = driver()
        .args(["session", "product-pid"])
        .arg(session)
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap()
}

#[test]
fn supervisor_records_clean_process_exit_and_bounded_phase_state() {
    let mut fixture = Fixture::new();
    let session = fixture.session("clean");
    fixture.supervise(&session, &[], &["/bin/true"]);
    let output = fixture.wait_supervisor(&session);
    assert!(output.status.success());

    let state = read_json(&session.join("state.json"));
    assert_eq!(state["schema_version"], 1);
    assert_eq!(state["phase"], "exited");
    assert_eq!(state["exit"]["code"], 0);
    let journal = fs::read_to_string(session.join("journey.ndjson")).unwrap();
    assert!(journal.contains(r#""kind":"product_started""#));
    assert!(journal.contains(r#""kind":"session_completed""#));
    assert!(!journal.contains(r#""kind":"failure""#));

    let validated = driver()
        .args(["session", "validate-journal"])
        .arg(&session)
        .output()
        .unwrap();
    assert!(validated.status.success());
    assert_eq!(
        String::from_utf8(validated.stdout).unwrap(),
        format!(
            "journey-journal-valid path={} records=4\n",
            session.join("journey.ndjson").display()
        )
    );
}

#[test]
fn journal_validation_rejects_partial_unknown_and_out_of_order_evidence() {
    let fixture = Fixture::new();
    let session = fixture.session("malformed-journal");
    let journal = session.join("journey.ndjson");

    fs::write(&journal, b"{\"schema_version\":1").unwrap();
    fs::set_permissions(&journal, fs::Permissions::from_mode(0o600)).unwrap();
    let partial = driver()
        .args(["session", "validate-journal"])
        .arg(&session)
        .output()
        .unwrap();
    assert!(!partial.status.success());
    assert!(String::from_utf8_lossy(&partial.stderr).contains("partial record"));

    fs::write(
        &journal,
        b"{\"schema_version\":1,\"sequence\":1,\"event\":{\"kind\":\"invented\"}}\n",
    )
    .unwrap();
    let unknown = driver()
        .args(["session", "validate-journal"])
        .arg(&session)
        .output()
        .unwrap();
    assert!(!unknown.status.success());
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("malformed"));

    fs::write(
        &journal,
        concat!(
            "{\"schema_version\":1,\"sequence\":1,\"event\":{\"kind\":\"session_started\"}}\n",
            "{\"schema_version\":1,\"sequence\":3,\"event\":{\"kind\":\"product_started\",\"pid\":123}}\n",
            "{\"schema_version\":1,\"sequence\":2,\"event\":{\"kind\":\"product_exited\",\"code\":0,\"signal\":null}}\n",
            "{\"schema_version\":1,\"sequence\":4,\"event\":{\"kind\":\"session_completed\"}}\n"
        ),
    )
    .unwrap();
    let out_of_order = driver()
        .args(["session", "validate-journal"])
        .arg(&session)
        .output()
        .unwrap();
    assert!(!out_of_order.status.success());
    assert!(String::from_utf8_lossy(&out_of_order.stderr).contains("sequence mismatch"));
}

#[test]
fn wait_deadline_and_stale_pid_are_hard_failures() {
    let mut fixture = Fixture::new();
    let session = fixture.session("stale");
    fixture.supervise(&session, &[], &["/bin/sleep", "60"]);
    wait_running(&session);

    let timeout = driver()
        .args(["session", "wait"])
        .arg(&session)
        .args(["30", "exited"])
        .output()
        .unwrap();
    assert!(!timeout.status.success());
    assert!(String::from_utf8_lossy(&timeout.stderr).contains("session deadline expired"));

    let state_path = session.join("state.json");
    let original = fs::read(&state_path).unwrap();
    let mut state = read_json(&state_path);
    state["product"]["start_ticks"] =
        Value::from(state["product"]["start_ticks"].as_u64().unwrap() + 1);
    fs::write(&state_path, serde_json::to_vec(&state).unwrap()).unwrap();
    let stale_output = driver()
        .args(["session", "product-pid"])
        .arg(&session)
        .output()
        .unwrap();
    assert!(!stale_output.status.success());
    assert!(String::from_utf8_lossy(&stale_output.stderr).contains("stale product PID identity"));
    fs::write(&state_path, original).unwrap();

    assert!(Fixture::stop(&session).status.success());
    assert!(fixture.wait_supervisor(&session).status.success());
}

#[test]
fn exclusive_resource_conflict_prevents_second_product_spawn() {
    let mut fixture = Fixture::new();
    let first = fixture.session("first");
    fixture.supervise(&first, &["display=x11:99"], &["/bin/sleep", "60"]);
    wait_running(&first);

    let second = fixture.session("second");
    let output = driver()
        .args(["session", "supervise"])
        .arg(&second)
        .arg(&fixture.resource_root)
        .args(["--resource", "display=x11:99", "--", "/bin/true"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("already claimed"));
    assert!(!second.join("product.log").exists());
    assert_eq!(read_json(&second.join("state.json"))["phase"], "failed");
    assert!(
        fs::read_to_string(second.join("journey.ndjson"))
            .unwrap()
            .contains(r#""code":"resource_conflict""#)
    );
    let evidence = driver()
        .args(["session", "validate-journal"])
        .arg(&second)
        .output()
        .unwrap();
    assert!(
        evidence.status.success(),
        "{}",
        String::from_utf8_lossy(&evidence.stderr)
    );

    assert!(Fixture::stop(&first).status.success());
    assert!(fixture.wait_supervisor(&first).status.success());
}

#[test]
fn stop_reaps_the_owned_process_group_and_is_not_reported_as_product_failure() {
    let mut fixture = Fixture::new();
    let session = fixture.session("cleanup");
    let descendant_path = fixture.root.join("descendant.pid");
    let script = "sleep 60 & child=$!; printf '%s' \"$child\" >\"$1\"; wait";
    fixture.supervise(
        &session,
        &[],
        &[
            "/bin/sh",
            "-c",
            script,
            "tornadotty-session-fixture",
            descendant_path.to_str().unwrap(),
        ],
    );
    wait_running(&session);
    wait_for_file(&descendant_path);
    let descendant = fs::read_to_string(&descendant_path)
        .unwrap()
        .parse::<u32>()
        .unwrap();
    assert!(process_is_running(descendant));

    assert!(Fixture::stop(&session).status.success());
    assert!(fixture.wait_supervisor(&session).status.success());
    let deadline = Instant::now() + Duration::from_secs(2);
    while process_is_running(descendant) {
        assert!(Instant::now() < deadline, "descendant {descendant} leaked");
        thread::yield_now();
    }
    let journal = fs::read_to_string(session.join("journey.ndjson")).unwrap();
    assert!(journal.contains(r#""kind":"stop_requested","signal":"term""#));
    assert!(!journal.contains(r#""code":"product_failed""#));
}

#[test]
fn input_driver_rejects_a_foreign_x11_window_before_sending_input() {
    let mut fixture = Fixture::new();
    let session = fixture.session("foreign-window");
    fixture.supervise(&session, &[], &["/bin/sleep", "60"]);
    wait_running(&session);
    let product = product_pid(&session);

    let fake_bin = fixture.root.join("fake-bin");
    create_owner_directory(&fake_bin);
    let calls = fixture.root.join("xdotool.calls");
    let xdotool = fake_bin.join("xdotool");
    fs::write(
        &xdotool,
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >>\"$MOCK_XDOTOOL_CALLS\"\nif [ \"$1\" = getwindowpid ]; then printf '%s\\n' \"$MOCK_XDOTOOL_OWNER\"; fi\n",
    )
    .unwrap();
    fs::set_permissions(&xdotool, fs::Permissions::from_mode(0o700)).unwrap();

    let foreign = driver()
        .args(["input", "verify"])
        .arg(&session)
        .args(["x11", "4242"])
        .env("PATH", &fake_bin)
        .env("MOCK_XDOTOOL_CALLS", &calls)
        .env("MOCK_XDOTOOL_OWNER", (product + 1).to_string())
        .output()
        .unwrap();
    assert!(!foreign.status.success());
    assert!(String::from_utf8_lossy(&foreign.stderr).contains("foreign X11 window rejected"));
    assert_eq!(fs::read_to_string(&calls).unwrap(), "getwindowpid 4242\n");

    fs::write(&calls, b"").unwrap();
    let owned = driver()
        .args(["input", "key"])
        .arg(&session)
        .args(["x11", "4242", "ctrl+t"])
        .env("PATH", &fake_bin)
        .env("MOCK_XDOTOOL_CALLS", &calls)
        .env("MOCK_XDOTOOL_OWNER", product.to_string())
        .output()
        .unwrap();
    assert!(owned.status.success());
    assert_eq!(
        fs::read_to_string(&calls).unwrap(),
        "getwindowpid 4242\nwindowfocus --sync 4242\nkey ctrl+t\n"
    );

    fs::write(&calls, b"").unwrap();
    let clicked = driver()
        .args(["input", "click"])
        .arg(&session)
        .args(["x11", "4242", "700", "400"])
        .env("PATH", &fake_bin)
        .env("MOCK_XDOTOOL_CALLS", &calls)
        .env("MOCK_XDOTOOL_OWNER", product.to_string())
        .output()
        .unwrap();
    assert!(clicked.status.success());
    assert_eq!(
        fs::read_to_string(&calls).unwrap(),
        "getwindowpid 4242\nwindowfocus --sync 4242\nmousemove --window 4242 700 400 click 1\n"
    );

    assert!(Fixture::stop(&session).status.success());
    assert!(fixture.wait_supervisor(&session).status.success());
}

#[test]
fn unexpected_product_kill_reaps_and_reports_a_live_child_leak() {
    let mut fixture = Fixture::new();
    let session = fixture.session("killed-product");
    let descendant_path = fixture.root.join("killed-descendant.pid");
    let script = "sleep 60 & child=$!; printf '%s' \"$child\" >\"$1\"; wait";
    fixture.supervise(
        &session,
        &[],
        &[
            "/bin/sh",
            "-c",
            script,
            "tornadotty-kill-fixture",
            descendant_path.to_str().unwrap(),
        ],
    );
    wait_running(&session);
    wait_for_file(&descendant_path);
    let descendant = fs::read_to_string(&descendant_path)
        .unwrap()
        .parse::<u32>()
        .unwrap();
    let product = product_pid(&session);
    kill_process(
        Pid::from_raw(i32::try_from(product).unwrap()).unwrap(),
        Signal::KILL,
    )
    .unwrap();

    let output = fixture.wait_supervisor(&session);
    assert!(!output.status.success());
    let deadline = Instant::now() + Duration::from_secs(2);
    while process_is_running(descendant) {
        assert!(Instant::now() < deadline, "descendant {descendant} leaked");
        thread::yield_now();
    }
    let journal = fs::read_to_string(session.join("journey.ndjson")).unwrap();
    assert!(journal.contains(r#""kind":"descendants_reaped","signal":"term","count":1"#));
    assert!(journal.contains(r#""code":"descendant_leak""#));
    assert!(journal.contains(r#""code":"product_failed""#));
    assert_eq!(read_json(&session.join("state.json"))["exit"]["signal"], 9);
}
