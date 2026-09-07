//! One native-monitor owner, outside GTK. Registration and cancellation retain
//! their slots until the owner processes them, including a blocked filesystem.
use gtk::{gio, glib, prelude::*};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};

type Permit = crate::worker_budget::WorkerPermit<128>;
thread_local! {
    static MONITORS: RefCell<BTreeMap<u64, (Vec<gio::FileMonitor>, Permit)>> = RefCell::default();
}
static SLOTS: LazyLock<Arc<crate::worker_budget::WorkerBudget<128>>> =
    LazyLock::new(|| Arc::new(crate::worker_budget::WorkerBudget::default()));
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static OWNER: LazyLock<Option<glib::MainContext>> = LazyLock::new(|| {
    let context = glib::MainContext::new();
    let worker_context = context.clone();
    let (ready, started) = std::sync::mpsc::sync_channel(1);
    let thread = std::thread::Builder::new()
        .name("project-watches".into())
        .spawn(move || {
            let _ = worker_context.with_thread_default(|| {
                if ready.send(()).is_err() {
                    return;
                }
                glib::MainLoop::new(Some(&worker_context), false).run();
            });
        });
    if let Err(error) = thread {
        eprintln!("tornadotty: project-watch owner=unavailable detail={error}");
        return None;
    }
    // Only waits for context ownership, before any filesystem operation. This
    // prevents MainContext::invoke executing registration inline on GTK.
    started.recv().ok()?;
    Some(context)
});

pub(super) struct ProjectWatch {
    pub(super) targets: Vec<PathBuf>,
    id: u64,
    changed: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
}

impl ProjectWatch {
    #[cfg(test)]
    pub(super) fn id_for_test(&self) -> u64 {
        self.id
    }

    #[cfg(test)]
    pub(super) fn wait_registered_for_test(&self) {
        let id = self.id;
        let (send, receive) = std::sync::mpsc::channel();
        OWNER.as_ref().unwrap().invoke(move || {
            send.send(MONITORS.with_borrow(|entries| entries.contains_key(&id)))
                .unwrap();
        });
        assert!(
            receive
                .recv_timeout(std::time::Duration::from_secs(3))
                .unwrap()
        );
    }

    #[cfg(test)]
    pub(super) fn assert_cancelled_for_test(id: u64) {
        let (send, receive) = std::sync::mpsc::channel();
        OWNER.as_ref().unwrap().invoke(move || {
            send.send(MONITORS.with_borrow(|entries| entries.contains_key(&id)))
                .unwrap();
        });
        assert!(
            !receive
                .recv_timeout(std::time::Duration::from_secs(3))
                .unwrap()
        );
    }

    pub(super) fn new(targets: Vec<PathBuf>) -> Option<Self> {
        let permit = SLOTS.acquire()?;
        let owner = OWNER.as_ref()?;
        let watch = Self {
            targets,
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            changed: Arc::new(AtomicBool::new(false)),
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        let (id, paths, changed, cancelled) = (
            watch.id,
            watch.targets.clone(),
            Arc::clone(&watch.changed),
            Arc::clone(&watch.cancelled),
        );
        owner.invoke(move || {
            if cancelled.load(Ordering::Acquire) {
                return;
            }
            let mut monitors = Vec::new();
            for (index, path) in paths.iter().enumerate() {
                if cancelled.load(Ordering::Acquire) {
                    break;
                }
                let file = gio::File::for_path(path);
                let result = if index == 0 {
                    file.monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE)
                } else {
                    file.monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE)
                };
                match result {
                    Ok(monitor) => {
                        let changed = Arc::clone(&changed);
                        monitor.connect_changed(move |_, _, _, _| {
                            changed.store(true, Ordering::Release);
                        });
                        monitors.push(monitor);
                    }
                    Err(error) => eprintln!(
                        "tornadotty: project-watch registration=unavailable detail={error}"
                    ),
                }
            }
            MONITORS.with_borrow_mut(|entries| {
                entries.insert(id, (monitors, permit));
            });
        });
        Some(watch)
    }

    pub(super) fn take_changed(&self) -> bool {
        self.changed.swap(false, Ordering::AcqRel)
    }
}

impl Drop for ProjectWatch {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        let id = self.id;
        let Some(owner) = OWNER.as_ref() else {
            return;
        };
        owner.invoke(move || {
            MONITORS.with_borrow_mut(|entries| {
                if let Some((monitors, _permit)) = entries.remove(&id) {
                    for monitor in monitors {
                        monitor.cancel();
                    }
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn busy_native_owner_does_not_block_registration_or_cancellation() {
        let (entered, started) = std::sync::mpsc::channel();
        let (release, blocked) = std::sync::mpsc::channel();
        OWNER.as_ref().unwrap().invoke(move || {
            entered.send(()).unwrap();
            blocked
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap();
        });
        started
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        let start = std::time::Instant::now();
        let watch = ProjectWatch::new(vec![PathBuf::from("/nonexistent-private-watch")]).unwrap();
        let id = watch.id;
        drop(watch);
        let elapsed = start.elapsed();
        release.send(()).unwrap();
        assert!(elapsed < std::time::Duration::from_millis(250));
        ProjectWatch::assert_cancelled_for_test(id);
    }
}
