//! Payload-free, bounded evidence that can be emitted while GTK is blocked.
use std::cell::RefCell;
use std::io::Read;
use std::os::unix::net::UnixDatagram;
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Phase {
    MainLoop,
    ApplicationTick,
    Layout,
    Chrome,
    ServerRanking,
    ServerProjection,
    ServerTooltip,
    ServerAccessibility,
    ServerMenuBuild,
    ServerPopoverReplace,
}

#[derive(Clone, Copy)]
struct State {
    heartbeat: Instant,
    phase: Phase,
    phase_since: Instant,
    sequence: u64,
    next_sequence: u64,
}

thread_local! {
    static CURRENT: RefCell<Option<Arc<Mutex<State>>>> = const { RefCell::new(None) };
}

pub(crate) struct Span {
    state: Option<Arc<Mutex<State>>>,
    previous: Option<(Phase, Instant, u64)>,
}

pub(crate) fn enter(phase: Phase) -> Span {
    let state = CURRENT.with(|current| current.borrow().clone());
    let previous = state.as_ref().map(|state| {
        let mut state = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let previous = (state.phase, state.phase_since, state.sequence);
        state.phase = phase;
        state.phase_since = Instant::now();
        state.next_sequence = state.next_sequence.wrapping_add(1);
        state.sequence = state.next_sequence;
        previous
    });
    Span { state, previous }
}

impl Drop for Span {
    fn drop(&mut self) {
        if let (Some(state), Some((phase, since, sequence))) = (&self.state, self.previous) {
            let mut state = state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.phase = phase;
            state.phase_since = since;
            state.sequence = sequence;
        }
    }
}

struct Budget {
    window: Instant,
    emitted: usize,
    stalled_heartbeat: Option<(Instant, u64)>,
    samples: usize,
}

impl Budget {
    fn new(now: Instant) -> Self {
        Self {
            window: now,
            emitted: 0,
            stalled_heartbeat: None,
            samples: 0,
        }
    }

    fn observe(&mut self, state: State, now: Instant) -> Option<(&'static str, u64, Duration)> {
        if now.duration_since(self.window) >= Duration::from_secs(60) {
            self.window = now;
            self.emitted = 0;
        }
        let episode = self
            .stalled_heartbeat
            .unwrap_or((state.heartbeat, state.sequence));
        let recovered = self
            .stalled_heartbeat
            .is_some_and(|(beat, _)| beat != state.heartbeat);
        if recovered {
            self.stalled_heartbeat = None;
            self.samples = 0;
        }
        let thresholds = [500, 2000, 10000];
        let stalled = self.samples < thresholds.len()
            && now.duration_since(state.heartbeat)
                >= Duration::from_millis(thresholds[self.samples]);
        let kind = if recovered {
            "recovered"
        } else if stalled {
            "stalled"
        } else {
            return None;
        };
        if stalled && !recovered {
            self.stalled_heartbeat
                .get_or_insert((state.heartbeat, state.sequence));
            self.samples += 1;
        }
        if self.emitted >= 12 {
            return None;
        }
        self.emitted += 1;
        Some((kind, episode.1, now.duration_since(episode.0)))
    }
}

pub(crate) struct Observer {
    state: Arc<Mutex<State>>,
    stop: mpsc::SyncSender<()>,
    worker: Option<JoinHandle<()>>,
    source: Option<gtk::glib::SourceId>,
}

impl Observer {
    #[cfg(test)]
    pub(crate) fn for_test(sink: impl Fn(String) + Send + 'static) -> Self {
        Self::start(sink).expect("controlled diagnostic observer")
    }

    pub(crate) fn install() -> Result<Self, String> {
        // Nonblocking local journal datagrams: a blocked stderr pipe must not
        // block this observer or make GUI shutdown wait for log delivery.
        let journal = UnixDatagram::unbound().map_err(|error| error.to_string())?;
        journal
            .connect("/run/systemd/journal/socket")
            .map_err(|error| error.to_string())?;
        journal
            .set_nonblocking(true)
            .map_err(|error| error.to_string())?;
        let mut observer = Self::start(move |message| {
            let _ = send_journal(&journal, &message);
        })?;
        let state = Arc::clone(&observer.state);
        observer.source = Some(gtk::glib::timeout_add_local(
            Duration::from_millis(100),
            move || {
                state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .heartbeat = Instant::now();
                gtk::glib::ControlFlow::Continue
            },
        ));
        Ok(observer)
    }

    fn start(sink: impl Fn(String) + Send + 'static) -> Result<Self, String> {
        let now = Instant::now();
        let state = Arc::new(Mutex::new(State {
            heartbeat: now,
            phase: Phase::MainLoop,
            phase_since: now,
            sequence: 0,
            next_sequence: 0,
        }));
        let observed = Arc::clone(&state);
        let (stop, receiver) = mpsc::sync_channel(1);
        let tid = std::fs::read_link("/proc/thread-self")
            .ok()
            .and_then(|path| path.file_name()?.to_str()?.parse::<u32>().ok());
        let worker = thread::Builder::new().name("ui-diagnostics".to_owned()).spawn(move || {
            let mut budget = Budget::new(Instant::now());
            while receiver.recv_timeout(Duration::from_millis(100)) == Err(mpsc::RecvTimeoutError::Timeout) {
                let snapshot = *observed.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                let now = Instant::now();
                if let Some((kind, episode, elapsed)) = budget.observe(snapshot, now) {
                    // No GTK calls, locks held during I/O, raw syscall arguments,
                    // terminal contents, object pointers or user-provided strings.
                    let (wait, syscall) = kernel_wait(tid);
                    sink(format!("ui-progress state={kind} phase={:?} sequence={} stall-sequence={episode} stall-age-ms={} phase-age-ms={} wait={wait} syscall={syscall}",
                        snapshot.phase, snapshot.sequence, elapsed.as_millis(), now.duration_since(snapshot.phase_since).as_millis()));
                }
            }
        }).map_err(|error| error.to_string())?;
        CURRENT.with(|current| *current.borrow_mut() = Some(Arc::clone(&state)));
        Ok(Self {
            state,
            stop,
            worker: Some(worker),
            source: None,
        })
    }
}

impl Drop for Observer {
    fn drop(&mut self) {
        if let Some(source) = self.source.take() {
            source.remove();
        }
        CURRENT.with(|current| {
            let mut current = current.borrow_mut();
            if current
                .as_ref()
                .is_some_and(|state| Arc::ptr_eq(state, &self.state))
            {
                current.take();
            }
        });
        let _ = self.stop.try_send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn send_journal(journal: &UnixDatagram, message: &str) -> std::io::Result<usize> {
    let record =
        format!("PRIORITY=4\nSYSLOG_IDENTIFIER=tornadotty-ui-diagnostics\nMESSAGE={message}");
    journal.send(record.as_bytes())
}

fn kernel_wait(tid: Option<u32>) -> (String, String) {
    let read = |name| -> Option<String> {
        let mut text = String::new();
        std::fs::File::open(format!("/proc/self/task/{}/{name}", tid?))
            .ok()?
            .take(256)
            .read_to_string(&mut text)
            .ok()?;
        Some(text)
    };
    let wait = read("wchan")
        .filter(|text| {
            text.len() <= 64
                && text
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c.is_ascii_whitespace())
        })
        .map_or_else(|| "unavailable".to_owned(), |text| text.trim().to_owned());
    let syscall = read("syscall")
        .and_then(|text| {
            let first = text.split_whitespace().next()?;
            (first == "running" || first.parse::<i64>().is_ok()).then(|| first.to_owned())
        })
        .unwrap_or_else(|| "unavailable".to_owned());
    (wait, syscall)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_phase_is_reported_before_it_returns_and_observer_stops() {
        let (sender, receiver) = mpsc::sync_channel(16);
        let observer = Observer::start(move |report| {
            let _ = sender.try_send(report);
        })
        .unwrap();
        let outer = enter(Phase::Chrome);
        let inner = enter(Phase::ServerPopoverReplace);
        let report = receiver.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(report.contains("state=stalled"));
        assert!(report.contains("phase=ServerPopoverReplace"));
        assert!(report.len() < 512);
        drop(inner);
        assert_eq!(observer.state.lock().unwrap().phase, Phase::Chrome);
        drop(outer);
        observer.state.lock().unwrap().heartbeat = Instant::now();
        assert!(
            receiver
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .contains("state=recovered")
        );
        drop(observer);
        assert!(matches!(
            receiver.recv_timeout(Duration::from_secs(1)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    }

    #[test]
    fn reports_are_capped_per_stall_and_per_minute() {
        let start = Instant::now();
        let mut budget = Budget::new(start);
        let state = State {
            heartbeat: start,
            phase: Phase::ServerRanking,
            phase_since: start,
            sequence: 1,
            next_sequence: 1,
        };
        let reports = (0..600)
            .filter_map(|step| budget.observe(state, start + Duration::from_millis(step * 100)))
            .count();
        assert_eq!(reports, 3);
        let mut budget = Budget::new(start);
        let mut reports = 0;
        for step in 1..50 {
            let now = start + Duration::from_millis(step * 1000);
            let state = State {
                heartbeat: now - Duration::from_millis(600),
                ..state
            };
            reports += usize::from(budget.observe(state, now).is_some());
            reports += usize::from(
                budget
                    .observe(
                        State {
                            heartbeat: now,
                            ..state
                        },
                        now,
                    )
                    .is_some(),
            );
        }
        assert_eq!(reports, 12);
    }

    #[test]
    fn healthy_progress_is_silent_and_nested_scope_ids_do_not_repeat() {
        let start = Instant::now();
        let mut budget = Budget::new(start);
        for step in 0..1000 {
            let now = start + Duration::from_millis(step * 100);
            let state = State {
                heartbeat: now,
                phase: Phase::MainLoop,
                phase_since: now,
                sequence: 0,
                next_sequence: 0,
            };
            assert!(budget.observe(state, now).is_none());
        }
        let observer = Observer::start(|_| {}).unwrap();
        let first = enter(Phase::ServerTooltip);
        let first_id = observer.state.lock().unwrap().sequence;
        drop(first);
        let second = enter(Phase::ServerAccessibility);
        assert!(observer.state.lock().unwrap().sequence > first_id);
        drop(second);
        assert_eq!(observer.state.lock().unwrap().phase, Phase::MainLoop);
    }

    #[test]
    fn unavailable_kernel_metadata_does_not_leak_raw_errors_or_paths() {
        assert_eq!(
            kernel_wait(None),
            ("unavailable".to_owned(), "unavailable".to_owned())
        );
    }

    #[test]
    fn journal_backpressure_drops_instead_of_blocking_delivery() {
        let (sender, _receiver) = UnixDatagram::pair().unwrap();
        sender.set_nonblocking(true).unwrap();
        let error = (0..4096)
            .find_map(|_| {
                send_journal(
                    &sender,
                    "ui-progress state=stalled phase=ServerPopoverReplace",
                )
                .err()
            })
            .expect("bounded socket buffer should fill without a reader");
        assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
    }
}
