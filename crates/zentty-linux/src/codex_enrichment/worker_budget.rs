use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A slot follows the actual thread, not its window or `JoinHandle`. Dropping a
/// window cannot cancel a filesystem syscall, so it cannot release this slot.
#[derive(Default)]
pub(super) struct WorkerBudget(AtomicUsize);

impl WorkerBudget {
    pub(super) fn acquire(self: &Arc<Self>) -> Option<WorkerPermit> {
        let mut active = self.0.load(Ordering::Acquire);
        while active < super::MAX_WORKERS {
            match self.0.compare_exchange_weak(
                active,
                active + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Some(WorkerPermit(Arc::clone(self))),
                Err(current) => active = current,
            }
        }
        None
    }
}

pub(super) struct WorkerPermit(Arc<WorkerBudget>);

impl Drop for WorkerPermit {
    fn drop(&mut self) {
        self.0.0.fetch_sub(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detached_worker_retains_its_slot_until_exit_including_panic() {
        for panic in [false, true] {
            let budget = Arc::new(WorkerBudget::default());
            let held = (1..crate::codex_enrichment::MAX_WORKERS)
                .map(|_| budget.acquire().unwrap())
                .collect::<Vec<_>>();
            let permit = budget.acquire().unwrap();
            let (release, blocked) = std::sync::mpsc::channel();
            let (exited, completion) = std::sync::mpsc::channel();
            let worker = std::thread::spawn(move || {
                let result = std::panic::catch_unwind(move || {
                    let _permit = permit;
                    blocked.recv().unwrap();
                    assert!(!panic, "controlled worker panic");
                });
                exited.send(result.is_err()).unwrap();
            });
            drop(worker); // Exactly what happens when a window drops its handle.
            assert!(budget.acquire().is_none());
            release.send(()).unwrap();
            assert_eq!(
                completion
                    .recv_timeout(std::time::Duration::from_secs(2))
                    .unwrap(),
                panic
            );
            assert!(budget.acquire().is_some());
            drop(held);
            assert_eq!(budget.0.load(Ordering::Acquire), 0);
        }
    }
}
