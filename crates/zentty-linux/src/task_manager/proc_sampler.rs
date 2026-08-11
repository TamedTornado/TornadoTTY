use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use super::model::{ProcessMetric, ProcessTree};

const MAX_PROCESSES_PER_TREE: usize = 4096;
const MAX_TREE_DEPTH: usize = 256;
const MAX_STAT_BYTES: u64 = 4096;
const MAX_CHILDREN_BYTES: u64 = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ProcessIdentity {
    pid: u32,
    start_time_ticks: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProbeSample {
    identity: ProcessIdentity,
    parent_pid: Option<u32>,
    name: String,
    cpu_time_ticks: u64,
    memory_bytes: u64,
}

#[derive(Clone, Copy, Debug)]
struct PreviousSample {
    cpu_time_ticks: u64,
    sampled_at: Duration,
}

pub(crate) struct ProcSampler {
    proc_root: PathBuf,
    clock_ticks_per_second: u64,
    page_size: u64,
    epoch: Instant,
    previous: BTreeMap<ProcessIdentity, PreviousSample>,
}

impl ProcSampler {
    pub(crate) fn system() -> Result<Self, String> {
        Ok(Self::at(
            PathBuf::from("/proc"),
            getconf("CLK_TCK")?,
            getconf("PAGESIZE")?,
        ))
    }

    fn at(proc_root: PathBuf, clock_ticks_per_second: u64, page_size: u64) -> Self {
        Self {
            proc_root,
            clock_ticks_per_second,
            page_size,
            epoch: Instant::now(),
            previous: BTreeMap::new(),
        }
    }

    pub(crate) fn sample(&mut self, root_pids: &[u32]) -> BTreeMap<u32, ProcessTree> {
        self.sample_at(root_pids, self.epoch.elapsed())
    }

    fn sample_at(&mut self, root_pids: &[u32], sampled_at: Duration) -> BTreeMap<u32, ProcessTree> {
        let mut next = BTreeMap::new();
        let mut trees = BTreeMap::new();
        for root_pid in root_pids.iter().copied().filter(|pid| *pid > 1) {
            let samples = self.read_tree(root_pid);
            if samples.is_empty() {
                continue;
            }
            let processes = samples
                .into_iter()
                .map(|sample| {
                    let previous = self.previous.get(&sample.identity).copied();
                    next.insert(
                        sample.identity,
                        PreviousSample {
                            cpu_time_ticks: sample.cpu_time_ticks,
                            sampled_at,
                        },
                    );
                    ProcessMetric {
                        pid: sample.identity.pid,
                        parent_pid: sample.parent_pid,
                        name: sample.name,
                        cpu_percent: cpu_percent(
                            sample.cpu_time_ticks,
                            previous,
                            sampled_at,
                            self.clock_ticks_per_second,
                        ),
                        memory_bytes: sample.memory_bytes,
                    }
                })
                .collect();
            trees.insert(
                root_pid,
                ProcessTree {
                    root_pid,
                    processes,
                },
            );
        }
        self.previous = next;
        trees
    }

    fn read_tree(&self, root_pid: u32) -> Vec<ProbeSample> {
        let mut samples = Vec::new();
        let mut queue = VecDeque::from([(root_pid, 0_usize, None)]);
        let mut visited = BTreeSet::new();
        while let Some((pid, depth, expected_parent)) = queue.pop_front() {
            if samples.len() >= MAX_PROCESSES_PER_TREE
                || depth > MAX_TREE_DEPTH
                || !visited.insert(pid)
            {
                continue;
            }
            let Some(sample) = read_process(&self.proc_root, pid, self.page_size) else {
                continue;
            };
            if expected_parent.is_some() && sample.parent_pid != expected_parent {
                continue;
            }
            samples.push(sample);
            queue.extend(
                read_children(&self.proc_root, pid)
                    .into_iter()
                    .map(|child| (child, depth + 1, Some(pid))),
            );
        }
        samples
    }
}

fn getconf(name: &str) -> Result<u64, String> {
    let output = Command::new("getconf")
        .arg(name)
        .output()
        .map_err(|error| format!("could not run getconf {name}: {error}"))?;
    if !output.status.success() {
        return Err(format!("getconf {name} exited with {}", output.status));
    }
    parse_getconf_value(name, &output.stdout)
}

fn parse_getconf_value(name: &str, stdout: &[u8]) -> Result<u64, String> {
    let value = String::from_utf8(stdout.to_vec())
        .map_err(|error| format!("getconf {name} returned invalid UTF-8: {error}"))?;
    value
        .trim()
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("getconf {name} returned an invalid value"))
}

fn read_process(proc_root: &Path, pid: u32, page_size: u64) -> Option<ProbeSample> {
    let stat = read_bounded(
        &proc_root.join(pid.to_string()).join("stat"),
        MAX_STAT_BYTES,
    )?;
    parse_stat(&stat, page_size)
}

fn read_children(proc_root: &Path, pid: u32) -> Vec<u32> {
    let path = proc_root
        .join(pid.to_string())
        .join("task")
        .join(pid.to_string())
        .join("children");
    read_bounded(&path, MAX_CHILDREN_BYTES)
        .map(|text| {
            text.split_whitespace()
                .filter_map(|value| value.parse::<u32>().ok())
                .filter(|pid| *pid > 1)
                .collect()
        })
        .unwrap_or_default()
}

fn read_bounded(path: &Path, limit: u64) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.by_ref().take(limit + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > limit {
        return None;
    }
    String::from_utf8(bytes).ok()
}

fn parse_stat(stat: &str, page_size: u64) -> Option<ProbeSample> {
    let open = stat.find('(')?;
    let close = stat.rfind(')')?;
    if close <= open {
        return None;
    }
    let pid = stat.get(..open)?.trim().parse::<u32>().ok()?;
    let name = stat.get(open + 1..close)?.to_owned();
    let fields = stat
        .get(close + 1..)?
        .split_whitespace()
        .collect::<Vec<_>>();
    let parent_pid = fields.get(1)?.parse::<u32>().ok().filter(|pid| *pid > 0);
    let user_ticks = fields.get(11)?.parse::<u64>().ok()?;
    let system_ticks = fields.get(12)?.parse::<u64>().ok()?;
    let start_time_ticks = fields.get(19)?.parse::<u64>().ok()?;
    let resident_pages = fields.get(21)?.parse::<i64>().ok()?.max(0).cast_unsigned();
    Some(ProbeSample {
        identity: ProcessIdentity {
            pid,
            start_time_ticks,
        },
        parent_pid,
        name,
        cpu_time_ticks: user_ticks.saturating_add(system_ticks),
        memory_bytes: resident_pages.saturating_mul(page_size),
    })
}

#[allow(clippy::cast_precision_loss)] // Kernel tick counters require a fractional percentage.
fn cpu_percent(
    current_ticks: u64,
    previous: Option<PreviousSample>,
    sampled_at: Duration,
    clock_ticks_per_second: u64,
) -> f64 {
    let Some(previous) = previous else {
        return 0.0;
    };
    let Some(elapsed) = sampled_at.checked_sub(previous.sampled_at) else {
        return 0.0;
    };
    let elapsed = elapsed.as_secs_f64();
    if elapsed <= 0.0 || clock_ticks_per_second == 0 {
        return 0.0;
    }
    let Some(delta) = current_ticks.checked_sub(previous.cpu_time_ticks) else {
        return 0.0;
    };
    delta as f64 / clock_ticks_per_second as f64 / elapsed * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir()
                .join(format!("zentty-task-manager-{}-{id}", std::process::id()));
            fs::create_dir_all(&root).expect("fixture root");
            Self { root }
        }

        #[allow(clippy::too_many_arguments)]
        fn process(
            &self,
            pid: u32,
            parent: u32,
            name: &str,
            user_ticks: u64,
            system_ticks: u64,
            start_time: u64,
            rss_pages: i64,
            children: &[u32],
        ) {
            let process = self.root.join(pid.to_string());
            let task = process.join("task").join(pid.to_string());
            fs::create_dir_all(&task).expect("task root");
            let mut fields = vec!["0".to_owned(); 22];
            fields[0] = "S".to_owned();
            fields[1] = parent.to_string();
            fields[11] = user_ticks.to_string();
            fields[12] = system_ticks.to_string();
            fields[19] = start_time.to_string();
            fields[21] = rss_pages.to_string();
            fs::write(
                process.join("stat"),
                format!("{pid} ({name}) {}\n", fields.join(" ")),
            )
            .expect("stat");
            fs::write(
                task.join("children"),
                children
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(" "),
            )
            .expect("children");
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).expect("remove fixture");
        }
    }

    fn assert_percent(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
    }

    #[test]
    fn stat_parser_handles_spaces_and_parentheses() {
        let fixture = Fixture::new();
        fixture.process(100, 10, "worker (busy)", 25, 5, 900, 12, &[]);
        let sample = read_process(&fixture.root, 100, 4096).expect("sample");
        assert_eq!(
            sample.identity,
            ProcessIdentity {
                pid: 100,
                start_time_ticks: 900
            }
        );
        assert_eq!(sample.parent_pid, Some(10));
        assert_eq!(sample.name, "worker (busy)");
        assert_eq!(sample.cpu_time_ticks, 30);
        assert_eq!(sample.memory_bytes, 49_152);
    }

    #[test]
    fn tree_is_cycle_safe_and_rejects_stale_parent_links() {
        let fixture = Fixture::new();
        fixture.process(100, 1, "root", 0, 0, 1, 1, &[101, 102, 100]);
        fixture.process(101, 100, "child", 0, 0, 2, 1, &[100]);
        fixture.process(102, 999, "reparented", 0, 0, 3, 1, &[]);
        let sampler = ProcSampler::at(fixture.root.clone(), 100, 4096);
        assert_eq!(
            sampler
                .read_tree(100)
                .iter()
                .map(|sample| sample.identity.pid)
                .collect::<Vec<_>>(),
            [100, 101]
        );
    }

    #[test]
    fn sibling_trees_keep_cpu_history_and_prune_exits_once_per_tick() {
        let fixture = Fixture::new();
        fixture.process(100, 1, "one", 0, 0, 1, 1, &[]);
        fixture.process(200, 1, "two", 0, 0, 2, 1, &[]);
        let mut sampler = ProcSampler::at(fixture.root.clone(), 100, 4096);
        sampler.sample_at(&[100, 200], Duration::ZERO);
        fixture.process(100, 1, "one", 100, 0, 1, 2, &[]);
        fixture.process(200, 1, "two", 50, 0, 2, 3, &[]);
        let trees = sampler.sample_at(&[100, 200], Duration::from_secs(1));
        assert_percent(trees[&100].processes[0].cpu_percent, 100.0);
        assert_percent(trees[&200].processes[0].cpu_percent, 50.0);
        fs::remove_dir_all(fixture.root.join("200")).expect("exit second process");
        fixture.process(100, 1, "one", 125, 0, 1, 2, &[]);
        let trees = sampler.sample_at(&[100, 200], Duration::from_secs(2));
        assert_percent(trees[&100].processes[0].cpu_percent, 25.0);
        assert!(!trees.contains_key(&200));
    }

    #[test]
    fn pid_reuse_and_counter_rollback_never_inherit_cpu() {
        let fixture = Fixture::new();
        fixture.process(100, 1, "old", 100, 0, 1, 1, &[]);
        let mut sampler = ProcSampler::at(fixture.root.clone(), 100, 4096);
        sampler.sample_at(&[100], Duration::ZERO);
        fixture.process(100, 1, "new", 5, 0, 2, 1, &[]);
        let reused = sampler.sample_at(&[100], Duration::from_secs(1));
        assert_percent(reused[&100].processes[0].cpu_percent, 0.0);
        fixture.process(100, 1, "new", 1, 0, 2, 1, &[]);
        let rollback = sampler.sample_at(&[100], Duration::from_secs(2));
        assert_percent(rollback[&100].processes[0].cpu_percent, 0.0);
    }

    #[test]
    fn malformed_and_oversized_inputs_are_unavailable() {
        let fixture = Fixture::new();
        let process = fixture.root.join("100");
        fs::create_dir_all(&process).expect("process");
        fs::write(process.join("stat"), "100 (truncated) S 1\n").expect("stat");
        assert!(read_process(&fixture.root, 100, 4096).is_none());
        fs::write(
            process.join("stat"),
            "x".repeat(usize::try_from(MAX_STAT_BYTES).expect("small test limit") + 1),
        )
        .expect("large stat");
        assert!(read_process(&fixture.root, 100, 4096).is_none());
    }

    #[test]
    fn exact_io_bounds_and_children_pid_floor_are_enforced() {
        let fixture = Fixture::new();
        let exact = fixture.root.join("exact");
        fs::write(&exact, "x".repeat(16)).expect("exact file");
        assert_eq!(
            read_bounded(&exact, 16).as_deref(),
            Some("xxxxxxxxxxxxxxxx")
        );
        fs::write(&exact, "x".repeat(17)).expect("oversized file");
        assert!(read_bounded(&exact, 16).is_none());
        fs::write(
            &exact,
            "x".repeat(usize::try_from(MAX_CHILDREN_BYTES).expect("small limit") + 1),
        )
        .expect("children-boundary file");
        assert!(read_bounded(&exact, MAX_CHILDREN_BYTES).is_none());
        fs::write(&exact, "x".repeat(2_048)).expect("within children bound");
        assert_eq!(
            read_bounded(&exact, MAX_CHILDREN_BYTES)
                .expect("within bound")
                .len(),
            2_048
        );

        let task = fixture.root.join("9/task/9");
        fs::create_dir_all(&task).expect("children parent");
        fs::write(task.join("children"), "0 1 2 3 invalid").expect("children");
        assert_eq!(read_children(&fixture.root, 9), [2, 3]);
        fs::write(
            task.join("children"),
            "2".repeat(usize::try_from(MAX_CHILDREN_BYTES).expect("small limit") + 1),
        )
        .expect("oversized children");
        assert!(read_children(&fixture.root, 9).is_empty());
    }

    #[test]
    fn root_floor_depth_and_duplicate_limits_are_independent() {
        let fixture = Fixture::new();
        fixture.process(1, 0, "init", 0, 0, 1, 1, &[]);
        fixture.process(2, 0, "root", 0, 0, 2, 1, &[3, 3]);
        fixture.process(3, 2, "child", 0, 0, 3, 1, &[]);
        let mut sampler = ProcSampler::at(fixture.root.clone(), 100, 4096);
        assert!(sampler.sample_at(&[1], Duration::ZERO).is_empty());
        assert_eq!(
            read_process(&fixture.root, 1, 4096).unwrap().parent_pid,
            None
        );
        assert_eq!(sampler.read_tree(2).len(), 2);
        assert!(sampler.sample(&[2]).contains_key(&2));

        for pid in 10..=u32::try_from(MAX_TREE_DEPTH).expect("small depth") + 11 {
            let child = (pid < u32::try_from(MAX_TREE_DEPTH).expect("small depth") + 11)
                .then_some(pid + 1)
                .into_iter()
                .collect::<Vec<_>>();
            fixture.process(
                pid,
                pid.saturating_sub(1),
                "depth",
                0,
                0,
                u64::from(pid),
                1,
                &child,
            );
        }
        assert_eq!(sampler.read_tree(10).len(), MAX_TREE_DEPTH + 1);
    }

    #[test]
    fn clock_discovery_and_elapsed_cpu_boundaries_are_exact() {
        assert!(getconf("CLK_TCK").expect("system clock ticks") > 1);
        assert!(getconf("ZENTTY_UNKNOWN_GETCONF_VALUE").is_err());
        assert!(parse_getconf_value("test", b"0\n").is_err());
        assert_eq!(parse_getconf_value("test", b"1\n").unwrap(), 1);
        let previous = PreviousSample {
            cpu_time_ticks: 100,
            sampled_at: Duration::from_secs(1),
        };
        assert_percent(
            cpu_percent(200, Some(previous), Duration::from_secs(2), 100),
            100.0,
        );
        assert_percent(
            cpu_percent(200, Some(previous), Duration::from_secs(3), 100),
            50.0,
        );
        assert_percent(
            cpu_percent(100, Some(previous), Duration::from_secs(1), 100),
            0.0,
        );
        assert_percent(
            cpu_percent(101, Some(previous), Duration::from_secs(2), 0),
            0.0,
        );
    }
}
