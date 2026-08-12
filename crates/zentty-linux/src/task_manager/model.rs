use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::PathBuf;

const MEMORY_UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PaneSource {
    pub(crate) window_id: String,
    pub(crate) window_title: String,
    pub(crate) worklane_id: String,
    pub(crate) worklane_title: String,
    pub(crate) pane_id: String,
    pub(crate) pane_title: String,
    pub(crate) status_text: Option<String>,
    pub(crate) root_pid: Option<u32>,
    pub(crate) is_remote: bool,
    pub(crate) is_worklane_active: bool,
    pub(crate) working_directory: Option<PathBuf>,
}

impl PaneSource {
    pub(crate) fn stable_id(&self) -> String {
        format!("{}|{}", self.window_id, self.pane_id)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ProcessMetric {
    pub(crate) pid: u32,
    pub(crate) parent_pid: Option<u32>,
    pub(crate) name: String,
    pub(crate) cpu_percent: f64,
    pub(crate) memory_bytes: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ProcessTree {
    pub(crate) root_pid: u32,
    pub(crate) processes: Vec<ProcessMetric>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Availability {
    Available,
    Unavailable(String),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PaneRow {
    pub(crate) source: PaneSource,
    pub(crate) availability: Availability,
    pub(crate) cpu_percent: Option<f64>,
    pub(crate) peak_cpu_percent: Option<f64>,
    pub(crate) memory_bytes: Option<u64>,
    pub(crate) peak_memory_bytes: Option<u64>,
    pub(crate) hottest_process: Option<ProcessMetric>,
    pub(crate) processes: Vec<ProcessMetric>,
}

impl PaneRow {
    pub(crate) fn project(
        source: PaneSource,
        tree: Option<ProcessTree>,
        previous: Option<&Self>,
    ) -> Self {
        if source.is_remote {
            return Self::unavailable(source, "Remote pane", previous);
        }
        if source.root_pid.is_none() {
            let reason = if source.is_worklane_active {
                "Waiting for shell PID"
            } else {
                "Inactive — shell not started"
            };
            return Self::unavailable(source, reason, previous);
        }
        let Some(tree) = tree.filter(|tree| !tree.processes.is_empty()) else {
            return Self::unavailable(source, "Metrics unavailable", previous);
        };
        let mut processes = tree.processes;
        processes.sort_by(process_order);
        let cpu_percent = processes.iter().map(|process| process.cpu_percent).sum();
        let memory_bytes = processes.iter().map(|process| process.memory_bytes).sum();
        let hottest_process = processes.first().cloned();
        Self {
            source,
            availability: Availability::Available,
            cpu_percent: Some(cpu_percent),
            peak_cpu_percent: Some(
                previous
                    .and_then(|row| row.peak_cpu_percent)
                    .map_or(cpu_percent, |peak| peak.max(cpu_percent)),
            ),
            memory_bytes: Some(memory_bytes),
            peak_memory_bytes: Some(
                previous
                    .and_then(|row| row.peak_memory_bytes)
                    .map_or(memory_bytes, |peak| peak.max(memory_bytes)),
            ),
            hottest_process,
            processes,
        }
    }

    fn unavailable(source: PaneSource, reason: &str, previous: Option<&Self>) -> Self {
        Self {
            source,
            availability: Availability::Unavailable(reason.to_owned()),
            cpu_percent: None,
            peak_cpu_percent: previous.and_then(|row| row.peak_cpu_percent),
            memory_bytes: None,
            peak_memory_bytes: previous.and_then(|row| row.peak_memory_bytes),
            hottest_process: None,
            processes: Vec::new(),
        }
    }

    pub(crate) fn status_text(&self) -> &str {
        match &self.availability {
            Availability::Available => self.source.status_text.as_deref().unwrap_or(""),
            Availability::Unavailable(reason) => reason,
        }
    }

    pub(crate) fn matches(&self, query: &str) -> bool {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return true;
        }
        let mut values = vec![
            self.source.window_title.as_str(),
            self.source.worklane_title.as_str(),
            self.source.pane_title.as_str(),
            self.status_text(),
        ];
        if let Some(cwd) = self
            .source
            .working_directory
            .as_deref()
            .and_then(|path| path.to_str())
        {
            values.push(cwd);
        }
        values
            .iter()
            .any(|value| value.to_lowercase().contains(&needle))
            || self.processes.iter().any(|process| {
                process.name.to_lowercase().contains(&needle)
                    || process.pid.to_string().contains(&needle)
                    || process
                        .parent_pid
                        .is_some_and(|pid| pid.to_string().contains(&needle))
            })
            || self
                .source
                .root_pid
                .is_some_and(|pid| pid.to_string().contains(&needle))
    }
}

fn process_order(left: &ProcessMetric, right: &ProcessMetric) -> Ordering {
    right
        .cpu_percent
        .total_cmp(&left.cpu_percent)
        .then_with(|| right.memory_bytes.cmp(&left.memory_bytes))
        .then_with(|| left.pid.cmp(&right.pid))
}

pub(crate) fn stable_hot_sort(rows: &mut [PaneRow], previous_order: &[String]) {
    const CPU_HYSTERESIS_PERCENT: f64 = 1.0;
    let previous = previous_order
        .iter()
        .enumerate()
        .map(|(index, pane_id)| (pane_id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    rows.sort_by(|left, right| {
        let left_cpu = left.cpu_percent.unwrap_or(-1.0);
        let right_cpu = right.cpu_percent.unwrap_or(-1.0);
        if (left_cpu - right_cpu).abs() <= CPU_HYSTERESIS_PERCENT
            && let (Some(left_index), Some(right_index)) = (
                previous.get(left.source.stable_id().as_str()),
                previous.get(right.source.stable_id().as_str()),
            )
            && left_index != right_index
        {
            return left_index.cmp(right_index);
        }
        right_cpu
            .total_cmp(&left_cpu)
            .then_with(|| {
                right
                    .memory_bytes
                    .unwrap_or(0)
                    .cmp(&left.memory_bytes.unwrap_or(0))
            })
            .then_with(|| left.source.pane_title.cmp(&right.source.pane_title))
    });
}

pub(crate) fn stable_hot_sort_within_worklanes(rows: &mut [PaneRow], previous_order: &[String]) {
    let mut start = 0;
    while start < rows.len() {
        let window_id = rows[start].source.window_id.clone();
        let worklane_id = rows[start].source.worklane_id.clone();
        let mut end = start + 1;
        while end < rows.len()
            && rows[end].source.window_id == window_id
            && rows[end].source.worklane_id == worklane_id
        {
            end += 1;
        }
        stable_hot_sort(&mut rows[start..end], previous_order);
        start = end;
    }
}

pub(crate) fn format_cpu(value: Option<f64>) -> String {
    value.map_or_else(|| "-".to_owned(), |value| format!("{value:.1}%"))
}

pub(crate) fn format_memory(value: Option<u64>) -> String {
    let Some(bytes) = value else {
        return "-".to_owned();
    };
    let mut amount = bytes;
    let mut remainder = 0;
    let mut divisor = 1;
    let mut unit = 0;
    while amount >= 1024 && unit + 1 < MEMORY_UNITS.len() {
        remainder = amount % 1024;
        amount /= 1024;
        divisor = 1024;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        let tenth = remainder.saturating_mul(10) / divisor;
        format!("{amount}.{tenth} {}", MEMORY_UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(id: &str, pid: Option<u32>) -> PaneSource {
        PaneSource {
            window_id: "window".to_owned(),
            window_title: "Main Window".to_owned(),
            worklane_id: "lane".to_owned(),
            worklane_title: "API".to_owned(),
            pane_id: id.to_owned(),
            pane_title: id.to_owned(),
            status_text: Some("Running".to_owned()),
            root_pid: pid,
            is_remote: false,
            is_worklane_active: true,
            working_directory: Some(PathBuf::from("/repo/api")),
        }
    }

    fn process(pid: u32, cpu: f64, memory: u64, name: &str) -> ProcessMetric {
        ProcessMetric {
            pid,
            parent_pid: None,
            name: name.to_owned(),
            cpu_percent: cpu,
            memory_bytes: memory,
        }
    }

    #[test]
    fn projection_aggregates_and_sorts_real_process_metrics() {
        let row = PaneRow::project(
            source("pane", Some(100)),
            Some(ProcessTree {
                root_pid: 100,
                processes: vec![
                    process(100, 1.0, 20, "bash"),
                    process(101, 175.0, 700, "cargo"),
                    process(102, 40.0, 200, "rustc"),
                ],
            }),
            None,
        );
        assert_eq!(row.cpu_percent, Some(216.0));
        assert_eq!(row.memory_bytes, Some(920));
        assert_eq!(
            row.hottest_process
                .as_ref()
                .map(|process| process.name.as_str()),
            Some("cargo")
        );
        assert_eq!(
            row.processes
                .iter()
                .map(|process| process.pid)
                .collect::<Vec<_>>(),
            [101, 102, 100]
        );
    }

    #[test]
    fn unavailable_and_remote_panes_stay_visible_with_peaks() {
        let previous = PaneRow::project(
            source("pane", Some(100)),
            Some(ProcessTree {
                root_pid: 100,
                processes: vec![process(100, 20.0, 400, "bash")],
            }),
            None,
        );
        let missing = PaneRow::project(source("pane", None), None, Some(&previous));
        assert_eq!(
            missing.availability,
            Availability::Unavailable("Waiting for shell PID".to_owned())
        );
        assert_eq!(missing.peak_cpu_percent, Some(20.0));
        let mut inactive = source("inactive", None);
        inactive.is_worklane_active = false;
        assert_eq!(
            PaneRow::project(inactive, None, None).status_text(),
            "Inactive — shell not started"
        );
        let mut remote = source("remote", Some(200));
        remote.is_remote = true;
        assert_eq!(
            PaneRow::project(remote, None, None).status_text(),
            "Remote pane"
        );
    }

    #[test]
    fn filter_searches_context_processes_and_pids() {
        let row = PaneRow::project(
            source("Server", Some(100)),
            Some(ProcessTree {
                root_pid: 100,
                processes: vec![process(101, 10.0, 20, "node")],
            }),
            None,
        );
        for query in ["api", "repo/api", "node", "101", "100", "running"] {
            assert!(row.matches(query), "query {query:?}");
        }
        assert!(!row.matches("unrelated"));
    }

    #[test]
    fn stable_sort_uses_hysteresis_then_hotness() {
        let mut rows = vec![
            PaneRow::project(
                source("B", Some(2)),
                Some(ProcessTree {
                    root_pid: 2,
                    processes: vec![process(2, 50.1, 10, "b")],
                }),
                None,
            ),
            PaneRow::project(
                source("A", Some(1)),
                Some(ProcessTree {
                    root_pid: 1,
                    processes: vec![process(1, 50.0, 10, "a")],
                }),
                None,
            ),
        ];
        stable_hot_sort(&mut rows, &["window|A".to_owned(), "window|B".to_owned()]);
        assert_eq!(
            rows.iter()
                .map(|row| row.source.pane_id.as_str())
                .collect::<Vec<_>>(),
            ["A", "B"]
        );
        rows[0].cpu_percent = Some(10.0);
        stable_hot_sort(&mut rows, &["window|A".to_owned(), "window|B".to_owned()]);
        assert_eq!(rows[0].source.pane_id, "B");

        rows[0].cpu_percent = None;
        rows[1].cpu_percent = Some(0.0);
        stable_hot_sort(&mut rows, &[]);
        assert_eq!(rows[0].source.pane_id, "A");

        rows[0].cpu_percent = Some(49.0);
        rows[1].cpu_percent = Some(50.1);
        stable_hot_sort(&mut rows, &["window|A".to_owned(), "window|B".to_owned()]);
        assert_eq!(rows[0].source.pane_id, "B");

        let mut ratio_trap = vec![
            PaneRow::project(
                source("B", Some(2)),
                Some(ProcessTree {
                    root_pid: 2,
                    processes: vec![process(2, 12.0, 0, "b")],
                }),
                None,
            ),
            PaneRow::project(
                source("A", Some(1)),
                Some(ProcessTree {
                    root_pid: 1,
                    processes: vec![process(1, 10.0, 0, "a")],
                }),
                None,
            ),
        ];
        stable_hot_sort(
            &mut ratio_trap,
            &["window|A".to_owned(), "window|B".to_owned()],
        );
        assert_eq!(ratio_trap[0].source.pane_id, "B");

        for panes in [["A", "B"], ["B", "A"]] {
            let mut unavailable = panes
                .into_iter()
                .map(|pane| PaneRow::project(source(pane, None), None, None))
                .collect::<Vec<_>>();
            unavailable[0].cpu_percent = None;
            unavailable[1].cpu_percent = Some(0.0);
            stable_hot_sort(&mut unavailable, &[]);
            assert_eq!(unavailable[0].cpu_percent, Some(0.0));
        }
        let mut unavailable_on_right = vec![
            PaneRow::project(source("hot", None), None, None),
            PaneRow::project(source("missing", None), None, None),
        ];
        unavailable_on_right[0].cpu_percent = Some(0.0);
        unavailable_on_right[1].cpu_percent = None;
        stable_hot_sort(&mut unavailable_on_right, &[]);
        assert_eq!(unavailable_on_right[0].source.pane_id, "hot");
    }

    #[test]
    fn worklane_group_order_is_preserved_while_panes_sort_hot() {
        let mut first_cool = PaneRow::project(
            source("first-cool", Some(10)),
            Some(ProcessTree {
                root_pid: 10,
                processes: vec![process(10, 1.0, 100, "cool")],
            }),
            None,
        );
        first_cool.source.worklane_id = "first".to_owned();
        let mut first_hot = PaneRow::project(
            source("first-hot", Some(11)),
            Some(ProcessTree {
                root_pid: 11,
                processes: vec![process(11, 80.0, 100, "hot")],
            }),
            None,
        );
        first_hot.source.worklane_id = "first".to_owned();
        let mut second = PaneRow::project(
            source("second", Some(12)),
            Some(ProcessTree {
                root_pid: 12,
                processes: vec![process(12, 200.0, 100, "hottest")],
            }),
            None,
        );
        second.source.worklane_id = "second".to_owned();
        let mut rows = vec![first_cool, first_hot, second];

        stable_hot_sort_within_worklanes(&mut rows, &[]);

        assert_eq!(
            rows.iter()
                .map(|row| (row.source.worklane_id.as_str(), row.source.pane_id.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("first", "first-hot"),
                ("first", "first-cool"),
                ("second", "second")
            ]
        );
    }

    #[test]
    fn formatters_are_stable() {
        assert_eq!(format_cpu(Some(12.34)), "12.3%");
        assert_eq!(format_cpu(None), "-");
        assert_eq!(format_memory(Some(1_023)), "1023 B");
        assert_eq!(format_memory(Some(1_024)), "1.0 KiB");
        assert_eq!(format_memory(Some(1_536)), "1.5 KiB");
        assert_eq!(format_memory(Some(1_048_576)), "1.0 MiB");
        assert_eq!(format_memory(Some(1_073_741_824)), "1.0 GiB");
        assert_eq!(format_memory(Some(u64::MAX)), "16777215.9 TiB");
        assert_eq!(format_memory(None), "-");
    }
}
