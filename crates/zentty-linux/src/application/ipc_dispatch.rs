//! Cooperative IPC service on the existing 10 ms application tick. Accepted
//! messages remain in the transport's fair queues until individually admitted.
use std::time::{Duration, Instant};

use super::ApplicationCoordinator;
use crate::agent_runtime::{IngressInput, IngressRoute};
use crate::application_shell::ApplicationShell;

const WORK_BUDGET: Duration = Duration::from_millis(4);
const COUNT_LIMITS: [usize; 4] = [32, 4, 4, 4];

#[derive(Default)]
pub(super) struct DispatchState {
    next: usize,
    yields: u64,
    last_report: Option<Instant>,
}

struct Turn {
    next: usize,
    remaining: [usize; 4],
}

impl Turn {
    fn new(next: usize) -> Self {
        Self {
            next,
            remaining: COUNT_LIMITS,
        }
    }

    fn take_route(&mut self, elapsed: Duration) -> Option<IngressRoute> {
        if elapsed >= WORK_BUDGET {
            return None;
        }
        for _ in 0..IngressRoute::ALL.len() {
            let index = self.next;
            self.next = (index + 1) % IngressRoute::ALL.len();
            if self.remaining[index] > 0 {
                self.remaining[index] -= 1;
                return Some(IngressRoute::ALL[index]);
            }
        }
        None
    }

    fn empty(&mut self, route: IngressRoute) {
        self.remaining[route as usize] = 0;
    }
}

impl ApplicationCoordinator {
    pub(super) fn dispatch_ipc(&mut self) -> Result<(), String> {
        self.agent_runtime.borrow().log_ingress_pressure();
        let started = Instant::now();
        let mut turn = Turn::new(self.ipc_dispatch.next);
        let mut last = None;
        while let Some(route) = turn.take_route(started.elapsed()) {
            let input = self.agent_runtime.borrow().take_input(route);
            let Some(input) = input else {
                turn.empty(route);
                continue;
            };
            last = Some((route, input.pane_id().to_owned()));
            // Store fairness progress before executing an operation that can
            // fail or consume the remaining budget. No second pending queue.
            self.ipc_dispatch.next = turn.next;
            self.dispatch_input(input)?;
        }
        self.ipc_dispatch.next = turn.next;
        let elapsed = started.elapsed();
        if elapsed >= WORK_BUDGET
            && let Some((route, pane)) = last
        {
            let state = &mut self.ipc_dispatch;
            state.yields = state.yields.saturating_add(1);
            if state
                .last_report
                .is_none_or(|last| last.elapsed() >= Duration::from_secs(5))
            {
                eprintln!(
                    "tornadotty: ipc-dispatch yield=budget count={} last-route={route:?} last-pane={pane} elapsed-us={} budget-us={}",
                    state.yields,
                    elapsed.as_micros(),
                    WORK_BUDGET.as_micros()
                );
                state.yields = 0;
                state.last_report = Some(Instant::now());
            }
        }
        Ok(())
    }

    fn dispatch_input(&mut self, input: IngressInput) -> Result<(), String> {
        let window = input.window_id().to_owned();
        let shell = self.shells.get(&window);
        match input {
            IngressInput::Product(command) => self.handle_product_commands(vec![command]),
            IngressInput::Event(event) => {
                if let Some(shell) = shell {
                    ApplicationShell::apply_agent_event(shell, *event);
                } else {
                    eprintln!(
                        "zentty-linux: agent-event-dropped pane={} reason=stale-window window={window}",
                        event.target.pane_id
                    );
                }
            }
            IngressInput::Tmux(command) => {
                if let Some(shell) = shell {
                    ApplicationShell::apply_tmux_input(shell, command);
                } else {
                    let reply = zentty_tmux_compat::TmuxCompatReply::failure(
                        "stale_target",
                        format!("window {window:?} is no longer available"),
                    )
                    .map_err(|error| error.to_string())?;
                    if let Err(error) = command.respond(reply) {
                        eprintln!(
                            "zentty-linux: tmux-stale-target-response window={window} error={error}"
                        );
                    }
                }
            }
            IngressInput::Server(command) => {
                let reply = if let Some(shell) = shell {
                    crate::application_shell::server_runtime::handle_ipc(
                        shell,
                        &command.target,
                        &command.request,
                    )
                } else {
                    zentty_agent_ipc::ServerIpcReply::failure(
                        "stale_target",
                        format!("window {window:?} is no longer available"),
                    )
                    .map_err(|error| error.to_string())?
                };
                if let Err(error) = command.respond(reply) {
                    eprintln!("zentty-linux: server-response window={window} error={error}");
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elapsed_budget_yields_before_dequeue_and_next_turn_starts_at_next_route() {
        let mut turn = Turn::new(0);
        assert_eq!(turn.take_route(Duration::ZERO), Some(IngressRoute::Events));
        assert_eq!(
            turn.take_route(Duration::from_millis(3)),
            Some(IngressRoute::Tmux)
        );
        assert_eq!(turn.take_route(WORK_BUDGET), None);
        assert_eq!(turn.remaining, [31, 3, 4, 4]);
        let mut next = Turn::new(turn.next);
        assert_eq!(next.take_route(Duration::ZERO), Some(IngressRoute::Servers));
        assert_eq!(next.take_route(Duration::from_secs(1)), None);
    }

    #[test]
    fn cheap_continuous_work_cannot_exceed_existing_count_caps() {
        let mut turn = Turn::new(0);
        let mut counts = [0; 4];
        while let Some(route) = turn.take_route(Duration::ZERO) {
            counts[route as usize] += 1;
        }
        assert_eq!(counts, COUNT_LIMITS);
    }

    #[test]
    fn empty_routes_do_not_spin_or_prevent_other_routes_from_using_their_budget() {
        let mut turn = Turn::new(0);
        assert_eq!(turn.take_route(Duration::ZERO), Some(IngressRoute::Events));
        turn.empty(IngressRoute::Events);
        let mut counts = [0; 4];
        while let Some(route) = turn.take_route(Duration::ZERO) {
            counts[route as usize] += 1;
        }
        assert_eq!(counts, [0, 4, 4, 4]);
    }

    #[test]
    fn repeated_slow_turns_preserve_queued_work_and_service_every_route() {
        use zentty_agent_ipc::{IngressMessage, ingress_channel};

        #[derive(Debug, PartialEq)]
        struct Message(&'static str, usize);
        impl IngressMessage for Message {
            fn pane_id(&self) -> &str {
                self.0
            }
        }
        let queues = IngressRoute::ALL.map(|_| ingress_channel(3, 2));
        for (sender, _) in &queues {
            sender.send(Message("busy", 1)).unwrap();
            sender.send(Message("busy", 2)).unwrap();
            sender.send(Message("quiet", 3)).unwrap();
        }
        let mut cursor = 0;
        let mut received: [Vec<Message>; 4] = std::array::from_fn(|_| Vec::new());
        for service_turn in 0..12 {
            let mut turn = Turn::new(cursor);
            let route = turn.take_route(Duration::ZERO).unwrap();
            assert_eq!(route, IngressRoute::ALL[service_turn % 4]);
            let index = route as usize;
            received[index].push(queues[index].1.try_recv().unwrap());
            // One expensive request exhausts this turn. Nothing else may be
            // removed from any transport queue until the next GTK tick.
            assert_eq!(turn.take_route(WORK_BUDGET), None);
            cursor = turn.next;
            let pending_count: usize = queues.iter().map(|(_, rx)| rx.take_pressure().queued).sum();
            assert_eq!(pending_count, 11 - service_turn);
        }
        for messages in received {
            assert_eq!(
                messages,
                vec![Message("busy", 1), Message("quiet", 3), Message("busy", 2)]
            );
        }
    }
}
