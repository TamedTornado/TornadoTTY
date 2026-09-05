use std::sync::mpsc::{RecvTimeoutError, TryRecvError};
use std::time::Duration;
use zentty_agent_ipc::{IngressMessage, IngressSendError, ingress_channel};

#[derive(Debug, PartialEq)]
struct Message(&'static str, usize);
impl IngressMessage for Message {
    fn pane_id(&self) -> &str {
        self.0
    }
}

#[test]
fn a_full_pane_cannot_consume_another_panes_capacity_or_service_turn() {
    let (sender, receiver) = ingress_channel(4, 2);
    sender.send(Message("busy", 1)).unwrap();
    sender.send(Message("busy", 2)).unwrap();
    assert_eq!(
        sender.send(Message("busy", 3)),
        Err(IngressSendError::Full(Message("busy", 3)))
    );
    sender.send(Message("quiet", 4)).unwrap();
    assert_eq!(receiver.try_recv(), Ok(Message("busy", 1)));
    assert_eq!(receiver.try_recv(), Ok(Message("quiet", 4)));
    assert_eq!(receiver.try_recv(), Ok(Message("busy", 2)));
    assert_eq!(receiver.try_recv(), Err(TryRecvError::Empty));
    sender.send(Message("busy", 5)).unwrap();
    assert_eq!(receiver.try_recv(), Ok(Message("busy", 5)));
}

#[test]
fn global_capacity_rejects_without_losing_or_reordering_accepted_messages() {
    let (sender, receiver) = ingress_channel(2, 2);
    sender.send(Message("a", 1)).unwrap();
    sender.send(Message("b", 2)).unwrap();
    assert_eq!(
        sender.send(Message("c", 3)),
        Err(IngressSendError::Full(Message("c", 3)))
    );
    assert_eq!(receiver.try_recv(), Ok(Message("a", 1)));
    sender.send(Message("c", 4)).unwrap();
    assert_eq!(receiver.try_recv(), Ok(Message("b", 2)));
    assert_eq!(receiver.try_recv(), Ok(Message("c", 4)));
}

#[test]
fn disconnect_preserves_accepted_messages_then_wakes_receiver() {
    let (sender, receiver) = ingress_channel(2, 2);
    let other = sender.clone();
    sender.send(Message("a", 1)).unwrap();
    drop(sender);
    assert_eq!(receiver.try_recv(), Ok(Message("a", 1)));
    assert_eq!(
        receiver.recv_timeout(Duration::ZERO),
        Err(RecvTimeoutError::Timeout)
    );
    drop(other);
    assert_eq!(
        receiver.recv_timeout(Duration::from_secs(1)),
        Err(RecvTimeoutError::Disconnected)
    );
}

#[test]
fn receiver_drop_rejects_without_blocking_producer() {
    let (sender, receiver) = ingress_channel(1, 1);
    sender.send(Message("a", 1)).unwrap();
    drop(receiver);
    assert_eq!(
        sender.send(Message("a", 2)),
        Err(IngressSendError::Disconnected(Message("a", 2)))
    );
}

#[test]
fn waiting_receiver_observes_real_thread_send_and_disconnect() {
    let (sender, receiver) = ingress_channel(1, 1);
    let worker = std::thread::spawn(move || {
        sender.send(Message("a", 7)).unwrap();
    });
    assert_eq!(
        receiver.recv_timeout(Duration::from_secs(1)),
        Ok(Message("a", 7))
    );
    worker.join().unwrap();
    assert_eq!(receiver.try_recv(), Err(TryRecvError::Disconnected));
}

#[test]
fn concurrent_producers_cannot_overbook_global_or_per_pane_capacity() {
    let (sender, receiver) = ingress_channel(6, 2);
    let accepted = std::thread::scope(|scope| {
        let workers = ["a", "b", "c", "d"].map(|pane| {
            let sender = sender.clone();
            scope.spawn(move || {
                (0..20)
                    .filter(|id| sender.send(Message(pane, *id)).is_ok())
                    .count()
            })
        });
        workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .sum::<usize>()
    });
    assert_eq!(accepted, 6);
    let messages = receiver.try_iter().collect::<Vec<_>>();
    assert_eq!(messages.len(), 6);
    for pane in ["a", "b", "c", "d"] {
        let ids = messages
            .iter()
            .filter(|message| message.0 == pane)
            .map(|message| message.1)
            .collect::<Vec<_>>();
        assert!(ids.len() <= 2);
        assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
    }
}

#[test]
fn admission_requires_nonzero_and_consistent_limits() {
    for (total, per_pane) in [(0, 1), (1, 0), (1, 2)] {
        assert!(std::panic::catch_unwind(|| ingress_channel::<Message>(total, per_pane)).is_err());
    }
}

#[test]
fn overload_diagnostics_are_aggregate_and_do_not_drain_work() {
    let (sender, receiver) = ingress_channel(3, 2);
    sender.send(Message("a", 1)).unwrap();
    sender.send(Message("a", 2)).unwrap();
    for id in 3..6 {
        assert!(sender.send(Message("a", id)).is_err());
    }
    let pressure = receiver.take_pressure();
    assert_eq!(pressure.queued, 2);
    assert_eq!(pressure.high_water, 2);
    assert_eq!(pressure.rejected, 3);
    assert_eq!(pressure.last_rejected_pane.as_deref(), Some("a"));
    assert_eq!(receiver.try_recv(), Ok(Message("a", 1)));
    let next = receiver.take_pressure();
    assert_eq!(next.queued, 1);
    assert_eq!(next.high_water, 2);
    assert_eq!(next.rejected, 0);
    assert_eq!(next.last_rejected_pane, None);
}

#[test]
fn gui_batch_budget_retains_remainder_in_fair_order() {
    let (sender, receiver) = ingress_channel(4, 3);
    for id in 1..=3 {
        sender.send(Message("busy", id)).unwrap();
    }
    sender.send(Message("quiet", 4)).unwrap();
    assert!(receiver.drain_batch(0).is_empty());
    assert_eq!(
        receiver.drain_batch(2),
        vec![Message("busy", 1), Message("quiet", 4)]
    );
    assert_eq!(
        receiver.drain_batch(2),
        vec![Message("busy", 2), Message("busy", 3)]
    );
    assert!(receiver.drain_batch(2).is_empty());
}
