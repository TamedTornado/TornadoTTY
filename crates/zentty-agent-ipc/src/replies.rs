//! Bounded transport ownership while the GUI decides a command's reply.
//! No product state or retry policy lives here. Readers return to admission
//! immediately; one nonblocking writer services pending replies fairly.
use super::{
    AgentIpcError, AgentIpcServer, ApplicationErrorCategory, ProductIpcReply, ProductReply,
    ReceivedResponse, ServerIpcReply, TmuxCompatReply, response_bytes,
};
use std::collections::BTreeMap;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests;

pub(super) const CAPACITY: usize = 32;
// Preserve the existing eight-client burst contract while reserving capacity
// for other panes. This bounds in-flight replies, not just queued GUI commands.
const PER_PANE_CAPACITY: usize = 8;

#[derive(Default)]
struct ReplySlots {
    total: usize,
    by_pane: BTreeMap<String, usize>,
}

pub(super) struct ReplyQueue {
    sender: mpsc::SyncSender<PendingConnection>,
    occupied: Arc<Mutex<ReplySlots>>,
}

pub(super) struct Reservation {
    occupied: Arc<Mutex<ReplySlots>>,
    pane: String,
}

impl Drop for Reservation {
    fn drop(&mut self) {
        let mut slots = self
            .occupied
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        slots.total -= 1;
        if let Some(count) = slots.by_pane.get_mut(&self.pane) {
            *count -= 1;
            if *count == 0 {
                slots.by_pane.remove(&self.pane);
            }
        }
    }
}

pub(super) enum ReplyReceiver {
    Tmux(mpsc::Receiver<TmuxCompatReply>),
    Server(mpsc::Receiver<ServerIpcReply>),
    Product(mpsc::Receiver<ProductIpcReply>),
}

impl ReplyReceiver {
    fn take(&self) -> Result<ProductReply, mpsc::TryRecvError> {
        match self {
            Self::Tmux(receiver) => receiver.try_recv().map(Into::into),
            Self::Server(receiver) => receiver.try_recv().map(Into::into),
            Self::Product(receiver) => receiver.try_recv().map(Into::into),
        }
    }

    fn timeout_message(&self) -> &'static str {
        match self {
            Self::Tmux(_) => "tmux compatibility response timed out",
            Self::Server(_) => "development-server response timed out",
            Self::Product(_) => "product command response timed out",
        }
    }
}

pub(super) struct PendingReply {
    id: String,
    receiver: ReplyReceiver,
    deadline: Instant,
    _reservation: Reservation,
}

impl PendingReply {
    pub(super) fn new(id: String, receiver: ReplyReceiver, reservation: Reservation) -> Self {
        let timeout = match receiver {
            ReplyReceiver::Product(_) => AgentIpcServer::APPLICATION_REPLY_TIMEOUT,
            _ => AgentIpcServer::TMUX_REPLY_TIMEOUT,
        };
        Self {
            id,
            receiver,
            deadline: Instant::now() + timeout,
            _reservation: reservation,
        }
    }
}

pub(super) struct PendingConnection {
    stream: UnixStream,
    reply: PendingReply,
    bytes: Option<Vec<u8>>,
    written: usize,
    write_deadline: Option<Instant>,
}

impl PendingConnection {
    /// One nonblocking write per pass. A client that stops reading cannot
    /// hold up unrelated replies; offsets preserve exact response bytes.
    fn advance(&mut self, now: Instant) -> bool {
        if self.bytes.is_none() {
            let result = match self.reply.receiver.take() {
                Ok(reply) => Ok(ReceivedResponse {
                    id: self.reply.id.clone(),
                    reply: Some(reply),
                }),
                Err(mpsc::TryRecvError::Empty) if now < self.reply.deadline => return false,
                Err(_) => Err(AgentIpcError::Rejected(
                    self.reply.receiver.timeout_message().to_owned(),
                )),
            };
            self.bytes = Some(response_bytes(result));
            self.write_deadline = Some(now + AgentIpcServer::CONNECTION_TIMEOUT);
        }
        if self.write_deadline.is_some_and(|deadline| now >= deadline) {
            return true;
        }
        let bytes = self.bytes.as_ref().expect("reply serialization completed");
        match self.stream.write(&bytes[self.written..]) {
            Ok(0) => true,
            Ok(count) => {
                self.written += count;
                self.written == bytes.len()
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) =>
            {
                false
            }
            Err(_) => true,
        }
    }
}

impl ReplyQueue {
    pub(super) fn new() -> (Self, mpsc::Receiver<PendingConnection>) {
        let (sender, receiver) = mpsc::sync_channel(CAPACITY);
        (
            Self {
                sender,
                occupied: Arc::new(Mutex::new(ReplySlots::default())),
            },
            receiver,
        )
    }

    /// Reserve BEFORE submitting a non-idempotent command to the product.
    /// Saturation rejects admission, never an already accepted operation.
    pub(super) fn reserve(&self, pane: &str) -> Result<Reservation, AgentIpcError> {
        let mut slots = self
            .occupied
            .lock()
            .map_err(|_| AgentIpcError::Rejected("reply capacity unavailable".to_owned()))?;
        if slots.total >= CAPACITY
            || slots.by_pane.get(pane).copied().unwrap_or(0) >= PER_PANE_CAPACITY
        {
            return Err(AgentIpcError::Remote {
                category: ApplicationErrorCategory::ProductUnavailable,
                code: "ingress_full".to_owned(),
                message: format!(
                    "product reply capacity exhausted for pane {pane}; command was not admitted"
                ),
            });
        }
        slots.total += 1;
        *slots.by_pane.entry(pane.to_owned()).or_default() += 1;
        Ok(Reservation {
            occupied: Arc::clone(&self.occupied),
            pane: pane.to_owned(),
        })
    }

    pub(super) fn enqueue(&self, stream: UnixStream, reply: PendingReply) {
        if stream.set_nonblocking(true).is_err() {
            return;
        }
        // Each queue member owns a reservation, so the queue cannot be full
        // while a newly reserved reader holds an item outside it.
        let _ = self.sender.try_send(PendingConnection {
            stream,
            reply,
            bytes: None,
            written: 0,
            write_deadline: None,
        });
    }

    pub(super) fn run(receiver: mpsc::Receiver<PendingConnection>, running: &AtomicBool) {
        let mut pending = Vec::with_capacity(CAPACITY);
        while running.load(Ordering::Acquire) {
            pending.extend(receiver.try_iter().take(CAPACITY - pending.len()));
            pending.retain_mut(|connection| !connection.advance(Instant::now()));
            // Same transport polling cadence as accept/read workers. This is
            // socket service, not an application-state timer or retry.
            thread::sleep(Duration::from_millis(5));
        }
        // Closing these streams wakes callers on shutdown; no GUI reply waits.
        drop(receiver);
    }
}
