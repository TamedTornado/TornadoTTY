use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread;
use zentty_core::{PersistenceRequest, SessionRestoreStore, SnapshotPersistence};

#[cfg(test)]
mod tests;

struct Save {
    request: PersistenceRequest,
    generation: u64,
}

enum Control {
    Save {
        save: Save,
        clean_exit_timestamp: Option<f64>,
        response: mpsc::SyncSender<Result<bool, String>>,
    },
    #[cfg(test)]
    Flush(mpsc::SyncSender<()>),
}

#[derive(Default)]
struct Mailbox {
    // Latest-value semantics apply only to ordinary snapshots, never barriers.
    pending: Option<Save>,
    control: Option<Control>,
    phase: WorkerPhase,
    latest_generation: u64,
    coalesced: u64,
    failure: Option<(u64, String)>,
}

#[derive(Default, Eq, PartialEq)]
enum WorkerPhase {
    #[default]
    Open,
    Barrier,
    Closing,
    Stopped,
}

#[derive(Default)]
struct Shared {
    mailbox: Mutex<Mailbox>,
    ready: Condvar,
}

// Disconnect outstanding responses even if the disk worker unwinds.
struct WorkerExit(Arc<Shared>);

impl Drop for WorkerExit {
    fn drop(&mut self) {
        let mut mailbox = self
            .0
            .mailbox
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        mailbox.phase = WorkerPhase::Stopped;
        let pending = mailbox.pending.take();
        let control = mailbox.control.take();
        drop(mailbox);
        drop((pending, control));
    }
}

pub(super) struct PersistenceWorker {
    shared: Arc<Shared>,
    thread: Option<thread::JoinHandle<()>>,
}

impl PersistenceWorker {
    pub(super) fn spawn(store: SessionRestoreStore) -> Result<Self, String> {
        let shared = Arc::new(Shared::default());
        let worker_shared = Arc::clone(&shared);
        let thread = thread::Builder::new()
            .name("zentty-session-persistence".to_owned())
            .spawn(move || run(&worker_shared, store))
            .map_err(|error| format!("could not start session-persistence worker: {error}"))?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    pub(super) fn persist(
        &self,
        request: PersistenceRequest,
        generation: u64,
    ) -> Result<(), String> {
        let mut mailbox = self.shared.mailbox.lock().unwrap();
        if mailbox.phase != WorkerPhase::Open {
            return Err("session-persistence worker is closed or completing a barrier".to_owned());
        }
        if generation < mailbox.latest_generation {
            return Err("session-persistence rejected a stale snapshot generation".to_owned());
        }
        mailbox.latest_generation = generation;
        let replaced = mailbox.pending.replace(Save {
            request,
            generation,
        });
        if replaced.is_some() {
            mailbox.coalesced = mailbox.coalesced.saturating_add(1);
        }
        drop(mailbox);
        // Reclaim a potentially large superseded topology outside the worker lock.
        drop(replaced);
        self.shared.ready.notify_one();
        Ok(())
    }

    // Only called after the GUI loop has finished. Do not move this durability
    // barrier into a GTK callback or mark the launch clean before it succeeds.
    pub(super) fn persist_synchronously(
        &self,
        request: PersistenceRequest,
        generation: u64,
        clean_exit_timestamp: Option<f64>,
    ) -> Result<bool, String> {
        let (response, receiver) = mpsc::sync_channel(1);
        self.control(Control::Save {
            save: Save {
                request,
                generation,
            },
            clean_exit_timestamp,
            response,
        })?;
        receiver.recv().map_err(|_| {
            "session-persistence worker stopped before completing a synchronous save".to_owned()
        })?
    }

    fn control(&self, control: Control) -> Result<(), String> {
        let mut mailbox = self.shared.mailbox.lock().unwrap();
        if mailbox.phase != WorkerPhase::Open {
            return Err("session-persistence worker is closed or completing a barrier".to_owned());
        }
        mailbox.phase = WorkerPhase::Barrier;
        mailbox.control = Some(control);
        drop(mailbox);
        self.shared.ready.notify_one();
        Ok(())
    }

    pub(super) fn drain_errors(&self) -> Vec<String> {
        let mut mailbox = self.shared.mailbox.lock().unwrap();
        let failure = mailbox.failure.take();
        drop(mailbox);
        if let Some(coalesced) = self.take_pressure() {
            eprintln!(
                "zentty-linux: persistence-backpressure boundary=workspace coalesced={coalesced} pending-capacity=1"
            );
        }
        failure
            .into_iter()
            .map(|(count, error)| {
                format!("session persistence: {count} failed operation(s); latest: {error}")
            })
            .collect()
    }

    fn take_pressure(&self) -> Option<u64> {
        let coalesced = std::mem::take(&mut self.shared.mailbox.lock().unwrap().coalesced);
        (coalesced != 0).then_some(coalesced)
    }

    #[cfg(test)]
    pub(super) fn synchronize(&self) {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.control(Control::Flush(sender)).unwrap();
        receiver.recv().unwrap();
    }
}

impl Drop for PersistenceWorker {
    fn drop(&mut self) {
        self.shared.mailbox.lock().unwrap().phase = WorkerPhase::Closing;
        self.shared.ready.notify_one();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run(shared: &Arc<Shared>, store: SessionRestoreStore) {
    let _exit = WorkerExit(Arc::clone(shared));
    let mut persistence = SnapshotPersistence::new(store);
    loop {
        let mut mailbox = shared.mailbox.lock().unwrap();
        while mailbox.pending.is_none()
            && mailbox.control.is_none()
            && mailbox.phase != WorkerPhase::Closing
        {
            mailbox = shared.ready.wait(mailbox).unwrap();
        }
        // Prior live state must reach storage before a final save merges its
        // missing restore drafts. No later live write can follow a clean marker.
        if let Some(save) = mailbox.pending.take() {
            drop(mailbox);
            if let Err(error) = persistence.persist(save.request, save.generation) {
                let mut mailbox = shared.mailbox.lock().unwrap();
                let count = mailbox
                    .failure
                    .as_ref()
                    .map_or(1, |(count, _)| count.saturating_add(1));
                mailbox.failure = Some((count, error.to_string()));
            }
            continue;
        }
        let control = mailbox.control.take();
        if control.is_none() && mailbox.phase == WorkerPhase::Closing {
            break;
        }
        drop(mailbox);
        match control {
            Some(Control::Save {
                save,
                clean_exit_timestamp,
                response,
            }) => {
                let result = persistence
                    .persist(save.request, save.generation)
                    .map_err(|error| error.to_string())
                    .and_then(|accepted| {
                        if accepted && let Some(updated_at) = clean_exit_timestamp {
                            persistence
                                .store()
                                .mark_clean_exit(updated_at)
                                .map_err(|error| error.to_string())?;
                        }
                        Ok(accepted)
                    });
                // A final save is terminal, including on failure.
                shared.mailbox.lock().unwrap().phase = WorkerPhase::Closing;
                let _ = response.send(result);
            }
            #[cfg(test)]
            Some(Control::Flush(response)) => {
                shared.mailbox.lock().unwrap().phase = WorkerPhase::Open;
                let _ = response.send(());
            }
            None => unreachable!("worker woke without work or shutdown"),
        }
    }
}
