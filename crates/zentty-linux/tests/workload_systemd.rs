//! Real user-manager checks; never attach the test runner or an existing pane.
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use zentty_linux::workload::{Manager, ScopeLimits, WorkloadId};

fn active(unit: &str) -> bool {
    let output = Command::new("systemctl")
        .args(["--user", "show", "--property=ActiveState", "--value", unit])
        .output()
        .unwrap();
    output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "active"
}

fn until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "real systemd transition deadline"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

struct Cleanup {
    child: std::process::Child,
    unit: String,
    scope: String,
    slice: String,
    directory: std::path::PathBuf,
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = Command::new("systemctl")
            .args(["--user", "stop", &self.unit, &self.scope, &self.slice])
            .output();
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[test]
#[ignore = "requires real systemd user manager and TORNADOTTY_OWNER_BINARY staged build"]
fn lifetime_observer_exits_when_its_gui_process_dies() {
    if let Ok(instance) = std::env::var("TORNADOTTY_TEST_PANE_CHILD") {
        let id = WorkloadId::new(&instance, 1).unwrap();
        let joined = Manager::connect(&format!("tornadotty-tests-{instance}.slice"))
            .unwrap()
            .join_current_process(
                &id,
                ScopeLimits {
                    memory_high: 512 * 1024 * 1024,
                    memory_max: 1024 * 1024 * 1024,
                    tasks_max: 128,
                    cpu_weight: 200,
                    io_weight: 300,
                },
            )
            .unwrap();
        // The shell exits and its background grandchild starts a different
        // session. Process-group ownership alone cannot clean this up.
        let output = Command::new("setsid")
            .args([
                "/bin/sh",
                "-c",
                "sleep 60 </dev/null >/dev/null 2>&1 & echo $!",
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        let descendant: u32 = String::from_utf8(output.stdout)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let membership = std::fs::read_to_string(format!("/proc/{descendant}/cgroup")).unwrap();
        assert!(membership.trim_end().ends_with(&format!("/{}", id.scope())));
        let own_membership = std::fs::read_to_string("/proc/self/cgroup").unwrap();
        let path = own_membership
            .lines()
            .find_map(|line| line.strip_prefix("0::"))
            .unwrap();
        let cgroup = std::path::Path::new("/sys/fs/cgroup").join(path.trim_start_matches('/'));
        let property = |path: &std::path::Path, name: &str| {
            std::fs::read_to_string(path.join(name))
                .unwrap()
                .trim()
                .to_owned()
        };
        assert_eq!(
            property(&cgroup, "memory.max"),
            (1024_u64.pow(3)).to_string()
        );
        assert_eq!(
            property(&cgroup, "memory.high"),
            (512 * 1024 * 1024_u64).to_string()
        );
        assert_eq!(property(&cgroup, "pids.max"), "128");
        assert_eq!(property(&cgroup, "cpu.weight"), "200");
        assert_eq!(joined.path, cgroup);
        let controllers = property(&cgroup, "cgroup.controllers");
        assert_eq!(
            joined.io_weight_available,
            controllers.split_whitespace().any(|name| name == "io")
        );
        if joined.io_weight_available {
            assert_eq!(property(&cgroup, "io.weight"), "default 300");
        } else {
            eprintln!(
                "I/O weight enforcement BLOCKED: io controller not delegated to user manager"
            );
        }
        assert!(property(&cgroup, "cpu.max").starts_with("max "));
        let aggregate = cgroup.parent().unwrap();
        let policy = zentty_core::WorkloadPolicy::default();
        let budget = policy.budgets(2 * 1024_u64.pow(3)).unwrap().aggregate;
        // The kernel rounds memory thresholds down to page boundaries.
        let page = rustix::param::page_size() as u64;
        assert_eq!(
            property(aggregate, "memory.max"),
            (budget.max / page * page).to_string()
        );
        assert_eq!(
            property(aggregate, "memory.high"),
            (budget.high / page * page).to_string()
        );
        assert_eq!(property(aggregate, "cpu.weight"), "100");
        if joined.io_weight_available {
            assert_eq!(property(aggregate, "io.weight"), "default 100");
        }
        assert!(property(aggregate, "cpu.max").starts_with("max "));
        std::fs::write(
            std::env::var_os("TORNADOTTY_TEST_PANE_READY").unwrap(),
            serde_json::json!({"descendant":descendant}).to_string(),
        )
        .unwrap();
        let _ = std::io::stdin().read(&mut [0_u8]);
        return;
    }
    if let Ok(instance) = std::env::var("TORNADOTTY_TEST_OWNER_CHILD") {
        let id = WorkloadId::new(&instance, 1).unwrap();
        let binary = std::env::var_os("TORNADOTTY_OWNER_BINARY").expect("staged owner binary");
        let manager = Manager::connect(&format!("tornadotty-tests-{instance}.slice")).unwrap();
        // GIO shares a session-bus connection. A second owner object must not
        // break subscription or prevent the first one's unit jobs completing.
        let _second_manager =
            Manager::connect(&format!("tornadotty-tests-{instance}.slice")).unwrap();
        // A tiny isolated hierarchy exercises the real approved percentage
        // calculation without applying any limits to Jason's live workloads.
        manager
            .start_aggregate(zentty_core::WorkloadPolicy::default(), 2 * 1024_u64.pow(3))
            .unwrap();
        manager
            .start_owner(&id, std::path::Path::new(&binary))
            .unwrap();
        let ready =
            std::path::PathBuf::from(std::env::var_os("TORNADOTTY_TEST_OWNER_READY").unwrap());
        let pane_ready = ready.with_file_name("pane-ready");
        let mut pane = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "lifetime_observer_exits_when_its_gui_process_dies",
                "--nocapture",
            ])
            .env("TORNADOTTY_TEST_PANE_CHILD", &instance)
            .env("TORNADOTTY_TEST_PANE_READY", &pane_ready)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        until(|| pane_ready.exists() || pane.try_wait().unwrap().is_some());
        assert!(
            pane_ready.exists(),
            "pane did not establish actual scope membership"
        );
        std::fs::write(ready, std::fs::read(pane_ready).unwrap()).unwrap();
        let _ = std::io::stdin().read(&mut [0_u8]);
        return;
    }
    let instance = format!(
        "{:032x}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let id = WorkloadId::new(&instance, 1).unwrap();
    let directory = std::env::temp_dir().join(format!("tornadotty-owner-{instance}"));
    std::fs::create_dir(&directory).unwrap();
    let ready = directory.join("ready");
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "lifetime_observer_exits_when_its_gui_process_dies",
            "--nocapture",
        ])
        .env("TORNADOTTY_TEST_OWNER_CHILD", &instance)
        .env("TORNADOTTY_TEST_OWNER_READY", &ready)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut cleanup = Cleanup {
        child,
        unit: id.owner(),
        scope: id.scope(),
        slice: format!("tornadotty-tests-{instance}.slice"),
        directory,
    };
    until(|| ready.exists() || cleanup.child.try_wait().unwrap().is_some());
    assert!(
        ready.exists(),
        "disposable GUI could not establish its owner service"
    );
    assert!(
        active(&cleanup.unit),
        "observer exited while its actual GUI was alive"
    );
    assert!(active(&cleanup.scope));
    let gui_cgroup =
        std::fs::read_to_string(format!("/proc/{}/cgroup", cleanup.child.id())).unwrap();
    assert!(
        !gui_cgroup.contains(&cleanup.slice),
        "GUI was charged to the pane budget"
    );
    let evidence: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&ready).unwrap()).unwrap();
    let descendant = evidence["descendant"].as_u64().unwrap();
    cleanup.child.kill().unwrap();
    cleanup.child.wait().unwrap();
    until(|| !active(&cleanup.unit));
    until(|| !active(&cleanup.scope));
    until(
        || match std::fs::read_to_string(format!("/proc/{descendant}/stat")) {
            Ok(stat) => stat
                .rsplit_once(") ")
                .is_some_and(|(_, tail)| tail.starts_with('Z')),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
            Err(error) => panic!("cannot inspect owned descendant: {error}"),
        },
    );
}
