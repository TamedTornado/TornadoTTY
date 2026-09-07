use super::WorkloadId;
use gtk::{gio, glib, prelude::*};

const DESTINATION: &str = "org.freedesktop.systemd1";
const PATH: &str = "/org/freedesktop/systemd1";
const MANAGER: &str = "org.freedesktop.systemd1.Manager";
const CALL_TIMEOUT_MS: i32 = 5_000;

/// Values are bytes/task counts, not shell fragments or percentages sent to
/// systemd. Policy calculation/approval belongs above this transport boundary.
#[derive(Clone, Copy, Debug)]
pub struct ScopeLimits {
    pub memory_high: u64,
    pub memory_max: u64,
    pub tasks_max: u64,
    pub cpu_weight: u16,
    pub io_weight: u16,
}

#[derive(Debug)]
pub struct ScopeMembership {
    pub path: std::path::PathBuf,
    /// Request acceptance is insufficient: many desktops do not delegate the
    /// I/O controller to the user manager. Callers must surface this limitation.
    pub io_weight_available: bool,
}

pub struct Manager {
    connection: gio::DBusConnection,
    slice: String,
}

impl Manager {
    /// Create the workload-only parent before launching any pane. The GUI and
    /// its lifetime observer must never enter this memory budget.
    pub fn start_aggregate(
        &self,
        policy: zentty_core::WorkloadPolicy,
        capacity_bytes: u64,
    ) -> Result<(), String> {
        let budget = policy.budgets(capacity_bytes)?.aggregate;
        let properties: Vec<(&str, glib::Variant)> = vec![
            (
                "Description",
                "TornadoTTY aggregate pane workloads".to_variant(),
            ),
            ("MemoryHigh", budget.high.to_variant()),
            ("MemoryMax", budget.max.to_variant()),
            ("CPUWeight", u64::from(policy.cpu_weight).to_variant()),
            ("IOWeight", u64::from(policy.io_weight).to_variant()),
        ];
        let auxiliary: Vec<(&str, Vec<(&str, glib::Variant)>)> = Vec::new();
        super::unit_job::start(
            &self.connection,
            &(self.slice.as_str(), "fail", properties, auxiliary).to_variant(),
            None,
        )
    }

    /// Call only from the pre-exec helper or an existing background worker.
    pub fn connect(slice: &str) -> Result<Self, String> {
        let stem = slice
            .strip_suffix(".slice")
            .ok_or("workload hierarchy must be a slice")?;
        if !stem.starts_with("tornadotty-")
            || stem.len() > 200
            || !stem
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err("invalid TornadoTTY workload hierarchy".into());
        }
        let connection = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE)
            .map_err(|error| format!("cannot connect to user manager bus: {error}"))?;
        let subscribed = connection.call_sync(
            Some(DESTINATION),
            PATH,
            MANAGER,
            "Subscribe",
            None,
            None,
            gio::DBusCallFlags::NONE,
            CALL_TIMEOUT_MS,
            gio::Cancellable::NONE,
        );
        if let Err(error) = subscribed {
            // GIO intentionally shares the session connection. This exact
            // protocol result means JobRemoved delivery is already enabled;
            // access denial, disconnect, and other failures remain errors.
            if gio::DBusError::remote_error(&error).as_deref()
                != Some("org.freedesktop.systemd1.AlreadySubscribed")
            {
                return Err(format!("cannot subscribe to user manager: {error}"));
            }
        }
        Ok(Self {
            connection,
            slice: slice.to_owned(),
        })
    }

    /// Keep the lifetime observer out of the pane aggregate. Only the pidfd,
    /// not a numeric PID or inherited pipe writer, determines owner lifetime.
    pub fn start_owner(&self, id: &WorkloadId, executable: &std::path::Path) -> Result<(), String> {
        let executable = executable.to_str().ok_or("owner executable is not UTF-8")?;
        if !std::path::Path::new(executable).is_absolute() {
            return Err("owner executable must be absolute".into());
        }
        let pidfd = rustix::process::pidfd_open(
            rustix::process::getpid(),
            rustix::process::PidfdFlags::empty(),
        )
        .map_err(|error| format!("cannot pin GUI process identity: {error}"))?;
        let descriptors = gio::UnixFDList::new();
        let handle = descriptors
            .append(pidfd)
            .map_err(|error| error.to_string())?;
        let properties: Vec<(&str, glib::Variant)> = vec![
            (
                "Description",
                "TornadoTTY GUI lifetime observer".to_variant(),
            ),
            ("Type", "exec".to_variant()),
            (
                "ExecStart",
                vec![(
                    executable,
                    vec![executable, "--internal-workload-owner"],
                    false,
                )]
                .to_variant(),
            ),
            (
                "StandardInputFileDescriptor",
                glib::variant::Handle(handle).to_variant(),
            ),
            ("StandardOutput", "null".to_variant()),
            ("StandardError", "journal".to_variant()),
            ("CollectMode", "inactive-or-failed".to_variant()),
        ];
        let auxiliary: Vec<(&str, Vec<(&str, glib::Variant)>)> = Vec::new();
        super::unit_job::start(
            &self.connection,
            &(id.owner(), "fail", properties, auxiliary).to_variant(),
            Some(&descriptors),
        )
    }

    /// The only attach operation accepts the *calling process*, never an
    /// arbitrary PID supplied by a pane or asynchronous GUI observation.
    pub fn join_current_process(
        &self,
        id: &WorkloadId,
        limits: ScopeLimits,
    ) -> Result<ScopeMembership, String> {
        if limits.memory_high == 0
            || limits.memory_high > limits.memory_max
            || limits.tasks_max == 0
            || !(1..=10_000).contains(&limits.cpu_weight)
            || !(1..=10_000).contains(&limits.io_weight)
        {
            return Err("invalid workload resource limits".into());
        }
        let properties: Vec<(&str, glib::Variant)> = vec![
            ("Description", "TornadoTTY pane workload".to_variant()),
            ("Slice", self.slice.to_variant()),
            ("PIDs", vec![std::process::id()].to_variant()),
            ("MemoryHigh", limits.memory_high.to_variant()),
            ("MemoryMax", limits.memory_max.to_variant()),
            ("TasksMax", limits.tasks_max.to_variant()),
            ("CPUWeight", u64::from(limits.cpu_weight).to_variant()),
            ("IOWeight", u64::from(limits.io_weight).to_variant()),
            ("OOMPolicy", "kill".to_variant()),
            ("KillMode", "control-group".to_variant()),
            ("BindsTo", vec![id.owner()].to_variant()),
            ("After", vec![id.owner()].to_variant()),
        ];
        let auxiliary: Vec<(&str, Vec<(&str, glib::Variant)>)> = Vec::new();
        super::unit_job::start(
            &self.connection,
            &(id.scope(), "fail", properties, auxiliary).to_variant(),
            None,
        )?;
        // A queued systemd job is not proof of migration. Before exec, verify
        // kernel membership, independently of systemd's request acceptance.
        let membership = std::fs::read_to_string("/proc/self/cgroup")
            .map_err(|error| format!("cannot verify own cgroup: {error}"))?;
        let path = membership
            .lines()
            .find_map(|line| {
                line.strip_prefix("0::").filter(|path| {
                    std::path::Path::new(path)
                        .file_name()
                        .is_some_and(|name| name == id.scope().as_str())
                })
            })
            .ok_or_else(|| {
                format!(
                    "pane scope {} has not acquired this process; refusing exec",
                    id.scope()
                )
            })?;
        let path = std::path::Path::new("/sys/fs/cgroup").join(path.trim_start_matches('/'));
        for required in ["memory.max", "memory.high", "pids.max", "cpu.weight"] {
            std::fs::metadata(path.join(required)).map_err(|error| {
                format!("pane scope lacks required controller {required}: {error}")
            })?;
        }
        let io_weight_available = match std::fs::metadata(path.join("io.weight")) {
            Ok(_) => true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => return Err(format!("cannot inspect I/O controller: {error}")),
        };
        Ok(ScopeMembership {
            path,
            io_weight_available,
        })
    }
}
