//! One bounded Open With request, from validation to external launch.
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

use gtk::{gio, glib};
use zentty_core::OpenWithTarget;

use super::{ApplicationShell, focused_context, launch, local_directory};

static BUSY: AtomicBool = AtomicBool::new(false);

struct Permit;
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

pub(super) fn submit(
    shell: &Rc<RefCell<ApplicationShell>>,
    target: OpenWithTarget,
    path: Option<PathBuf>,
    action: &str,
) {
    let Some(source) = focused_context(&shell.borrow()) else {
        return;
    };
    let Some(permit) = Permit::acquire() else {
        eprintln!(
            "zentty-linux: action=open-{action} pane={} rejected=busy boundary=open-with capacity=1",
            source.pane_id
        );
        return;
    };
    let weak = Rc::downgrade(shell);
    let action = action.to_owned();
    let target_id = target.id.clone();
    glib::spawn_future_local(async move {
        let probe_source = source.clone();
        let plan = gio::spawn_blocking(move || {
            let plan = if let Some(path) = path {
                target
                    .launch_local_path_plan(&path)
                    .map(|plan| (plan, path))
                    .map_err(|error| format!("{error:?}"))
            } else {
                let directory = local_directory(&probe_source).map_err(str::to_owned)?;
                target
                    .launch_plan(&directory)
                    .map(|plan| (plan, directory))
                    .map_err(|error| format!("{error:?}"))
            };
            // Capacity follows the actual worker, including cancellation of
            // its awaiting GTK future. Transfer it with a successful result.
            plan.map(|plan| (plan, permit))
        })
        .await;
        let Some(shell) = weak.upgrade() else {
            return;
        };
        if !source.is_current(&shell.borrow()) {
            eprintln!(
                "zentty-linux: action=open-{action} pane={} rejected=stale-context",
                source.pane_id
            );
            return;
        }
        let ((plan, path), permit) = match plan {
            Ok(Ok(plan)) => plan,
            Ok(Err(error)) => {
                eprintln!(
                    "zentty-linux: action=open-{action} id={target_id} unavailable={error} pane={}",
                    source.pane_id
                );
                return;
            }
            Err(_) => {
                eprintln!(
                    "zentty-linux: action=open-{action} id={target_id} error=worker-panic pane={}",
                    source.pane_id
                );
                return;
            }
        };
        let result = gio::spawn_blocking(move || {
            let _permit = permit;
            launch(plan)
        })
        .await;
        match result {
            Ok(Ok(())) => eprintln!(
                "zentty-linux: action=open-{action} id={target_id} result=launched path={} pane={}",
                path.display(),
                source.pane_id
            ),
            Ok(Err(error)) => eprintln!(
                "zentty-linux: action=open-{action} id={target_id} error={error} pane={}",
                source.pane_id
            ),
            Err(_) => eprintln!(
                "zentty-linux: action=open-{action} id={target_id} error=worker-panic pane={}",
                source.pane_id
            ),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_requests_do_not_queue_and_capacity_returns_on_unwind() {
        let result = std::panic::catch_unwind(|| {
            let _permit = Permit::acquire().expect("first request");
            assert!(Permit::acquire().is_none());
            panic!("controlled validation panic");
        });
        assert!(result.is_err());
        assert!(Permit::acquire().is_some());

        let permit = Permit::acquire().unwrap();
        let (release, pending) = std::sync::mpsc::channel();
        let (exited, completed) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let permit = permit;
            pending.recv().unwrap();
            drop(permit);
            exited.send(()).unwrap();
        });
        drop(worker);
        assert!(Permit::acquire().is_none());
        release.send(()).unwrap();
        completed
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        assert!(Permit::acquire().is_some());
    }
}
