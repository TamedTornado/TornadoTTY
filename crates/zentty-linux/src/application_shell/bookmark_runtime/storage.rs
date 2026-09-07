//! One admitted bookmark operation at a time, from disk read through UI
//! publication. Window snapshots are projections of the same default store.
use super::{ApplicationShell, BookmarkRuntime, now_iso8601};
use gtk::{gio, glib};
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicBool, Ordering};

thread_local! {
    static WINDOWS: RefCell<Vec<Weak<RefCell<ApplicationShell>>>> = const { RefCell::new(Vec::new()) };
}
static BUSY: AtomicBool = AtomicBool::new(false);
pub(super) fn is_busy() -> bool {
    BUSY.load(Ordering::Acquire)
}
pub(super) struct Permit;
impl Permit {
    fn acquire() -> Option<Self> {
        BUSY.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| Self)
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}

pub(super) fn initialize(shell: &Rc<RefCell<ApplicationShell>>) {
    WINDOWS.with(|windows| windows.borrow_mut().push(Rc::downgrade(shell)));
    // Another window's in-flight operation publishes its result to this one
    // too. Do not create a second loader or queue blocked filesystem workers.
    if BUSY.load(Ordering::Acquire) {
        return;
    }
    if let Err(error) = run(shell, "load-bookmarks", |_, _| Ok(()), |_, ()| Ok(())) {
        fail(shell, "load-bookmarks", &error);
    }
}

fn publish(runtime: &BookmarkRuntime) {
    WINDOWS.with(|windows| {
        windows.borrow_mut().retain(|window| {
            let Some(shell) = window.upgrade() else {
                return false;
            };
            if shell.borrow().shutting_down {
                return false;
            }
            let changed = shell.borrow().bookmark_runtime.snapshot != runtime.snapshot;
            shell.borrow_mut().bookmark_runtime = runtime.clone();
            if changed {
                shell.borrow().render_sidebar();
            }
            true
        });
    });
}

pub(super) fn fail(shell: &Rc<RefCell<ApplicationShell>>, action: &str, error: &str) {
    ApplicationShell::report_action_error(shell, action, error);
}

type After = Box<dyn FnOnce(&mut BookmarkRuntime, &str) -> Result<(), String> + Send>;

pub(super) fn run<T: Send + 'static>(
    shell: &Rc<RefCell<ApplicationShell>>,
    action: &'static str,
    operation: impl FnOnce(&mut BookmarkRuntime, &str) -> Result<T, String> + Send + 'static,
    apply: impl FnOnce(&Rc<RefCell<ApplicationShell>>, T) -> Result<(), String> + 'static,
) -> Result<(), String> {
    run_with_after(shell, action, operation, apply, None)
}

pub(super) fn run_with_after<T: Send + 'static>(
    shell: &Rc<RefCell<ApplicationShell>>,
    action: &'static str,
    operation: impl FnOnce(&mut BookmarkRuntime, &str) -> Result<T, String> + Send + 'static,
    apply: impl FnOnce(&Rc<RefCell<ApplicationShell>>, T) -> Result<(), String> + 'static,
    after: Option<After>,
) -> Result<(), String> {
    let Some(permit) = Permit::acquire() else {
        let error = "Bookmark storage is busy. This request was not queued or saved; retry when the current operation completes.";
        fail(shell, action, error);
        return Err(error.to_owned());
    };
    let mut runtime = shell.borrow().bookmark_runtime.clone();
    let weak = Rc::downgrade(shell);
    let now = now_iso8601()?;
    let requires_window = after.is_some();
    glib::spawn_future_local(async move {
        let stamp = now.clone();
        let result = gio::spawn_blocking(move || {
            let loaded = runtime.reload();
            let snapshot_valid = loaded.is_ok();
            let result = loaded.and_then(|()| operation(&mut runtime, &stamp));
            (runtime, result, permit, snapshot_valid)
        })
        .await;
        let Ok((mut runtime, result, permit, snapshot_valid)) = result else {
            if let Some(shell) = weak.upgrade() {
                fail(
                    &shell,
                    action,
                    "Bookmark worker failed. Retry the operation.",
                );
            }
            return;
        };
        let result = result.and_then(|value| {
            let shell = weak.upgrade().filter(|shell| !shell.borrow().shutting_down);
            let Some(shell) = shell else {
                return if requires_window {
                    Err("Window closed before bookmark restore completed".to_owned())
                } else {
                    Ok(())
                };
            };
            apply(&shell, value)
        });
        let result = if snapshot_valid {
            gio::spawn_blocking(move || {
                let result =
                    result.and_then(|()| after.map_or(Ok(()), |after| after(&mut runtime, &now)));
                let refreshed = runtime.reload();
                let snapshot_valid = refreshed.is_ok();
                (runtime, result.and(refreshed), permit, snapshot_valid)
            })
            .await
        } else {
            Ok((runtime, result, permit, false))
        };
        match result {
            Ok((runtime, result, _permit, snapshot_valid)) => {
                if snapshot_valid {
                    publish(&runtime);
                }
                match result {
                    Ok(()) => eprintln!("zentty-linux: action={action} result=ok"),
                    Err(error) => {
                        if let Some(shell) = weak.upgrade() {
                            fail(&shell, action, &error);
                        } else {
                            eprintln!("zentty-linux: action={action} failed: {error}");
                        }
                    }
                }
            }
            Err(_) => {
                if let Some(shell) = weak.upgrade() {
                    fail(
                        &shell,
                        action,
                        "Bookmark completion worker failed. Reopen to refresh storage.",
                    );
                }
            }
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Permit;

    #[test]
    fn storage_admission_rejects_overlap_until_worker_finishes_and_recovers_after_panic() {
        let permit = Permit::acquire().expect("first operation admitted");
        std::thread::spawn(move || {
            assert!(Permit::acquire().is_none(), "overlapping mutation admitted");
            drop(permit);
            let _permit = Permit::acquire().expect("completed operation released slot");
            assert!(Permit::acquire().is_none());
        })
        .join()
        .expect("worker assertions must pass, not be mistaken for the expected panic");
        let permit = Permit::acquire().expect("worker exit released slot");
        std::thread::spawn(move || {
            let _permit = permit;
            panic!("worker failure");
        })
        .join()
        .expect_err("worker deliberately failed");
        let permit = Permit::acquire().expect("failed worker released slot");
        assert!(Permit::acquire().is_none());
        drop(permit);
        assert!(Permit::acquire().is_some());
    }
}
