//! Bounded application ingress: FIFO per pane, round-robin between panes.
//! Admission never waits for the GUI. Rejected messages remain with the caller.
use std::collections::{BTreeMap, VecDeque};
use std::sync::mpsc::{RecvTimeoutError, TryRecvError};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

pub trait IngressMessage {
    fn pane_id(&self) -> &str;
}

impl IngressMessage for zentty_core::AuthenticatedAgentEvent {
    fn pane_id(&self) -> &str {
        &self.target.pane_id
    }
}
impl IngressMessage for crate::AuthenticatedTmuxRequest {
    fn pane_id(&self) -> &str {
        &self.target.pane_id
    }
}
impl IngressMessage for crate::AuthenticatedServerRequest {
    fn pane_id(&self) -> &str {
        &self.target.pane_id
    }
}
impl IngressMessage for crate::AuthenticatedProductRequest {
    fn pane_id(&self) -> &str {
        &self.target.pane_id
    }
}

#[derive(Debug, PartialEq)]
pub enum IngressSendError<T> {
    Full(T),
    Disconnected(T),
}

struct State<T> {
    panes: BTreeMap<String, VecDeque<T>>,
    ready: VecDeque<String>,
    len: usize,
    senders: usize,
    receiver_alive: bool,
    pressure: IngressPressure,
}

/// Aggregate overload diagnostics; retains no event contents or credentials.
#[derive(Debug, Default, PartialEq)]
pub struct IngressPressure {
    pub queued: usize,
    pub high_water: usize,
    pub rejected: u64,
    pub last_rejected_pane: Option<String>,
}

struct Shared<T> {
    state: Mutex<State<T>>,
    changed: Condvar,
    capacity: usize,
    per_pane: usize,
}

pub struct IngressSender<T>(Arc<Shared<T>>);
pub struct IngressReceiver<T>(Arc<Shared<T>>);

/// Creates one bounded route. Limits count authenticated, size-validated messages.
///
/// # Panics
/// Panics if either limit is zero or the per-pane limit exceeds global capacity.
#[must_use]
pub fn ingress_channel<T: IngressMessage>(
    capacity: usize,
    per_pane: usize,
) -> (IngressSender<T>, IngressReceiver<T>) {
    assert!(capacity > 0 && per_pane > 0 && per_pane <= capacity);
    let shared = Arc::new(Shared {
        state: Mutex::new(State {
            panes: BTreeMap::new(),
            ready: VecDeque::new(),
            len: 0,
            senders: 1,
            receiver_alive: true,
            pressure: IngressPressure::default(),
        }),
        changed: Condvar::new(),
        capacity,
        per_pane,
    });
    (IngressSender(Arc::clone(&shared)), IngressReceiver(shared))
}

impl<T> Clone for IngressSender<T> {
    fn clone(&self) -> Self {
        self.0.state.lock().unwrap().senders += 1;
        Self(Arc::clone(&self.0))
    }
}

impl<T: IngressMessage> IngressSender<T> {
    /// Admits once without waiting for capacity. Never retries or silently drops.
    ///
    /// # Errors
    /// Returns the original message when the route/pane is full or disconnected.
    /// # Panics
    /// Panics if the internal queue mutex was poisoned.
    pub fn send(&self, message: T) -> Result<(), IngressSendError<T>> {
        let mut state = self.0.state.lock().unwrap();
        if !state.receiver_alive {
            return Err(IngressSendError::Disconnected(message));
        }
        let pane = message.pane_id();
        if state.len >= self.0.capacity
            || state
                .panes
                .get(pane)
                .is_some_and(|queue| queue.len() >= self.0.per_pane)
        {
            state.pressure.rejected = state.pressure.rejected.saturating_add(1);
            state.pressure.last_rejected_pane = Some(pane.to_owned());
            return Err(IngressSendError::Full(message));
        }
        let pane = pane.to_owned();
        if !state.panes.contains_key(&pane) {
            state.ready.push_back(pane.clone());
        }
        state.panes.entry(pane).or_default().push_back(message);
        state.len += 1;
        state.pressure.high_water = state.pressure.high_water.max(state.len);
        drop(state);
        self.0.changed.notify_one();
        Ok(())
    }
}

impl<T> State<T> {
    fn pop(&mut self) -> Option<T> {
        let pane = self.ready.pop_front()?;
        let queue = self.panes.get_mut(&pane).expect("ready pane has a queue");
        let message = queue.pop_front().expect("ready pane is nonempty");
        if queue.is_empty() {
            self.panes.remove(&pane);
        } else {
            self.ready.push_back(pane);
        }
        self.len -= 1;
        Some(message)
    }
}

impl<T> IngressReceiver<T> {
    /// Takes aggregate rejection counts; queue high-water remains lifetime-scoped.
    /// # Panics
    /// Panics if the internal queue mutex was poisoned.
    #[must_use]
    pub fn take_pressure(&self) -> IngressPressure {
        let mut state = self.0.state.lock().unwrap();
        IngressPressure {
            queued: state.len,
            high_water: state.pressure.high_water,
            rejected: std::mem::take(&mut state.pressure.rejected),
            last_rejected_pane: state.pressure.last_rejected_pane.take(),
        }
    }

    /// # Errors
    /// Reports empty or disconnected without blocking the GUI.
    /// # Panics
    /// Panics if the internal queue mutex was poisoned.
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        let mut state = self.0.state.lock().unwrap();
        state.pop().ok_or(if state.senders == 0 {
            TryRecvError::Disconnected
        } else {
            TryRecvError::Empty
        })
    }

    pub fn try_iter(&self) -> impl Iterator<Item = T> + '_ {
        std::iter::from_fn(|| self.try_recv().ok())
    }

    /// Returns at most this iteration's work budget, retaining the remainder.
    #[must_use]
    pub fn drain_batch(&self, limit: usize) -> Vec<T> {
        self.try_iter().take(limit).collect()
    }

    /// Blocking receive for non-GUI consumers and transport integration tests.
    /// # Errors
    /// Reports timeout or disconnected after draining accepted messages.
    /// # Panics
    /// Panics if the internal queue mutex was poisoned.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, RecvTimeoutError> {
        let started = Instant::now();
        let mut state = self.0.state.lock().unwrap();
        loop {
            if let Some(message) = state.pop() {
                return Ok(message);
            }
            if state.senders == 0 {
                return Err(RecvTimeoutError::Disconnected);
            }
            let remaining = timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Err(RecvTimeoutError::Timeout);
            }
            state = self.0.changed.wait_timeout(state, remaining).unwrap().0;
        }
    }
}

impl<T> Drop for IngressSender<T> {
    fn drop(&mut self) {
        self.0.state.lock().unwrap().senders -= 1;
        self.0.changed.notify_all();
    }
}

impl<T> Drop for IngressReceiver<T> {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap();
        state.receiver_alive = false;
        let pending = std::mem::take(&mut state.panes);
        state.ready.clear();
        state.len = 0;
        drop(state);
        drop(pending);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AuthenticatedProductRequest, AuthenticatedServerRequest, AuthenticatedTmuxRequest,
    };
    use std::sync::mpsc;
    use zentty_core::{AgentEvent, AgentTarget, AuthenticatedAgentEvent};

    fn independent<T: IngressMessage>(messages: [T; 2]) {
        let (sender, receiver) = ingress_channel(2, 1);
        for message in messages {
            assert!(sender.send(message).is_ok());
        }
        assert_eq!(receiver.try_iter().count(), 2);
    }

    #[test]
    fn each_authenticated_route_preserves_independent_pane_capacity() {
        independent(["first", "second"].map(|pane| {
            AuthenticatedAgentEvent {
                target: AgentTarget::new("window", "lane", pane),
                pane_token: "token".into(),
                event: AgentEvent::parse(
                    br#"{"version":1,"event":"agent.running","session":{"id":"a"}}"#,
                )
                .unwrap(),
            }
        }));
        independent(["first", "second"].map(|pane| {
            AuthenticatedTmuxRequest {
                target: AgentTarget::new("window", "lane", pane),
                request: zentty_tmux_compat::TmuxCompatRequest::new(
                    1,
                    "display-message",
                    vec![],
                    None,
                )
                .unwrap(),
                responder: mpsc::sync_channel(1).0,
            }
        }));
        independent(["first", "second"].map(|pane| AuthenticatedServerRequest {
            target: AgentTarget::new("window", "lane", pane),
            request: crate::ServerIpcRequest::new("server-list", vec![]).unwrap(),
            responder: mpsc::sync_channel(1).0,
        }));
        independent(["first", "second"].map(|pane| {
            AuthenticatedProductRequest {
                target: crate::ApplicationTarget::new("window", "lane", pane),
                authority: crate::ApplicationAuthority::Pane,
                request: crate::ProductIpcRequest::new(
                    crate::ProductIpcKind::Discover,
                    "panes",
                    vec![],
                )
                .unwrap(),
                responder: mpsc::sync_channel(1).0,
            }
        }));
    }
}
