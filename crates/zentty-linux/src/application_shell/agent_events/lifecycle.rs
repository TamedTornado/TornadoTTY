//! Liveness I/O stays outside GTK; only current status identities may consume it.
use super::{ApplicationShell, Rc, RefCell, unix_time_ms};
use gtk::{gio, glib};
use std::sync::{Arc, LazyLock};

static WORKERS: LazyLock<Arc<crate::worker_budget::WorkerBudget<2>>> =
    LazyLock::new(|| Arc::new(crate::worker_budget::WorkerBudget::default()));

pub(super) fn request(shell: &Rc<RefCell<ApplicationShell>>, now: u64) {
    let mut state = shell.borrow_mut();
    // Existing lifecycle deadlines still advance if an I/O worker is delayed.
    if state.state.apply_agent_process_observations(now, &[]) {
        state.refresh_sidebar_metadata();
    }
    if state.shutting_down || state.agent_events.lifecycle_in_flight {
        return;
    }
    let Some(permit) = WORKERS.acquire() else {
        return;
    };
    let probes = state.state.agent_process_probes();
    if probes.is_empty() {
        return;
    }
    let identities = probes
        .iter()
        .filter_map(|probe| {
            super::super::pane_context::PaneContext::capture(&state, &probe.pane_id)
                .map(|identity| (probe.pane_id.clone(), identity))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    state.agent_events.lifecycle_in_flight = true;
    drop(state);
    let weak = Rc::downgrade(shell);
    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(move || {
            let _permit = permit;
            probes
                .into_iter()
                .map(|probe| {
                    let alive = probe.pid > 0
                        && std::path::Path::new("/proc")
                            .join(probe.pid.to_string())
                            .try_exists()
                            .unwrap_or(true);
                    (probe, alive)
                })
                .collect::<Vec<_>>()
        })
        .await;
        let Some(shell) = weak.upgrade() else {
            return;
        };
        let mut shell = shell.borrow_mut();
        shell.agent_events.lifecycle_in_flight = false;
        if shell.shutting_down {
            return;
        }
        match result {
            Ok(mut observations) => {
                observations.retain(|(probe, _)| {
                    identities
                        .get(&probe.pane_id)
                        .is_some_and(|identity| identity.is_current(&shell))
                });
                if shell
                    .state
                    .apply_agent_process_observations(unix_time_ms(), &observations)
                {
                    eprintln!("tornadotty: agent-lifecycle-sweep changed=true");
                    shell.refresh_sidebar_metadata();
                }
            }
            Err(_) => eprintln!("tornadotty: agent-lifecycle-sweep error=worker-panic"),
        }
    });
}
