//! Separate, process-wide limits for discovery and launch validation. A blocked
//! filesystem must not cause palette reopenings to accumulate worker threads.
use std::sync::atomic::{AtomicBool, Ordering};

static DISCOVERY: AtomicBool = AtomicBool::new(false);
static VALIDATION: AtomicBool = AtomicBool::new(false);

pub(super) struct Permit(&'static AtomicBool);

impl Permit {
    pub(super) fn discovery() -> Option<Self> {
        Self::acquire(&DISCOVERY)
    }

    pub(super) fn validation() -> Option<Self> {
        Self::acquire(&VALIDATION)
    }

    fn acquire(busy: &'static AtomicBool) -> Option<Self> {
        busy.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| Self(busy))
    }
}

impl Drop for Permit {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_discovery_is_bounded_independent_and_releases_on_worker_exit() {
        let permit = Permit::discovery().unwrap();
        let (release, blocked) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let _permit = permit;
            blocked.recv().unwrap();
        });
        assert!(Permit::discovery().is_none());
        let validation = Permit::validation().expect("discovery cannot starve validation");
        assert!(Permit::validation().is_none());
        drop(validation);
        release.send(()).unwrap();
        worker.join().unwrap();
        assert!(Permit::discovery().is_some());
        let panic = std::panic::catch_unwind(|| {
            let _permit = Permit::validation().unwrap();
            panic!("controlled worker panic");
        });
        assert!(panic.is_err());
        assert!(Permit::validation().is_some());
    }
}
