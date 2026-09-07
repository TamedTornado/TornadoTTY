//! Host-relative workload budgets. These are containment limits, not estimates
//! of application memory use, and CPU/I/O weights never impose a CPU quota.
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct WorkloadPolicy {
    pub aggregate_memory_high_percent: u8,
    pub aggregate_memory_max_percent: u8,
    pub pane_memory_high_percent: u8,
    pub pane_memory_max_percent: u8,
    pub cpu_weight: u16,
    pub io_weight: u16,
}

impl Default for WorkloadPolicy {
    fn default() -> Self {
        Self {
            aggregate_memory_high_percent: 70,
            aggregate_memory_max_percent: 80,
            pane_memory_high_percent: 50,
            pane_memory_max_percent: 60,
            cpu_weight: 100,
            io_weight: 100,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryBudget {
    pub high: u64,
    pub max: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkloadBudgets {
    pub aggregate: MemoryBudget,
    pub pane: MemoryBudget,
}

impl WorkloadPolicy {
    pub fn validated(self) -> Result<Self, String> {
        for (name, high, max) in [
            (
                "aggregate",
                self.aggregate_memory_high_percent,
                self.aggregate_memory_max_percent,
            ),
            (
                "pane",
                self.pane_memory_high_percent,
                self.pane_memory_max_percent,
            ),
        ] {
            if high == 0 || high >= max || max > 100 {
                return Err(format!(
                    "workloads.{name} memory percentages require 0 < high < max <= 100"
                ));
            }
        }
        if self.pane_memory_max_percent > self.aggregate_memory_max_percent
            || self.pane_memory_high_percent > self.aggregate_memory_high_percent
        {
            return Err("workloads pane budgets must not exceed aggregate budgets".into());
        }
        if !(1..=10_000).contains(&self.cpu_weight) || !(1..=10_000).contains(&self.io_weight) {
            return Err("workloads CPU and I/O weights must be between 1 and 10000".into());
        }
        Ok(self)
    }

    /// Pass physical RAM capped by any tighter ancestor cgroup memory limit.
    /// Do not use currently free memory: identical settings must not fluctuate
    /// with the workload running at the moment a new pane opens.
    pub fn budgets(self, available_bytes: u64) -> Result<WorkloadBudgets, String> {
        self.validated()?;
        if available_bytes < 100 {
            return Err(
                "cannot calculate workload budgets without a usable memory capacity".into(),
            );
        }
        let percent = |value: u8| {
            // Avoid overflow even if a platform reports an unlimited sentinel.
            available_bytes / 100 * u64::from(value)
                + available_bytes % 100 * u64::from(value) / 100
        };
        Ok(WorkloadBudgets {
            aggregate: MemoryBudget {
                high: percent(self.aggregate_memory_high_percent),
                max: percent(self.aggregate_memory_max_percent),
            },
            pane: MemoryBudget {
                high: percent(self.pane_memory_high_percent),
                max: percent(self.pane_memory_max_percent),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppConfig;

    #[test]
    fn default_budgets_leave_gui_headroom_and_scale_with_capacity() {
        let policy = AppConfig::parse_toml("").unwrap().workloads;
        for capacity in [1024_u64.pow(3), 96 * 1024_u64.pow(3), u64::MAX] {
            let budgets = policy.budgets(capacity).unwrap();
            assert_eq!(
                u128::from(budgets.aggregate.max),
                u128::from(capacity) * 80 / 100
            );
            assert_eq!(
                u128::from(budgets.pane.max),
                u128::from(capacity) * 60 / 100
            );
            assert!(budgets.pane.high < budgets.pane.max);
            assert!(budgets.aggregate.high < budgets.aggregate.max);
            assert!(budgets.aggregate.max < capacity);
        }
        assert!(policy.budgets(0).is_err());
    }

    #[test]
    fn configured_budgets_and_invalid_reload_use_existing_config_contract() {
        let source = "[workloads]\naggregate_memory_high_percent=60\naggregate_memory_max_percent=75\npane_memory_high_percent=30\npane_memory_max_percent=40\ncpu_weight=200\nio_weight=500\n";
        let config = AppConfig::parse_toml(source).unwrap();
        let budgets = config.workloads.budgets(10_000).unwrap();
        assert_eq!(
            budgets.aggregate,
            MemoryBudget {
                high: 6000,
                max: 7500
            }
        );
        assert_eq!(
            budgets.pane,
            MemoryBudget {
                high: 3000,
                max: 4000
            }
        );
        assert_eq!(config.workloads.cpu_weight, 200);
        assert_eq!(config.workloads.io_weight, 500);
        for invalid in [
            "pane_memory_high_percent=0",
            "pane_memory_high_percent=60",
            "aggregate_memory_max_percent=101",
            "pane_memory_max_percent=90",
            "cpu_weight=0",
            "io_weight=10001",
            "cpu_quota=50",
        ] {
            let source = format!("[workloads]\n{invalid}\n");
            assert!(AppConfig::parse_toml(&source).is_err(), "{invalid}");
            let partial = AppConfig::parse_toml_partial(&source, &config).unwrap();
            assert_eq!(partial.config.workloads, config.workloads);
            assert_eq!(partial.retained_sections, vec!["workloads"]);
        }
    }
}
