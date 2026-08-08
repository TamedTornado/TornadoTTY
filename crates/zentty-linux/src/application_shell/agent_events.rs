use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use zentty_core::WorkspaceState;
use zentty_tmux_compat::Command as TmuxCommand;

use crate::agent_runtime::AgentRuntime;
use crate::codex_enrichment::CodexTranscriptEnricher;

use super::{ApplicationShell, unix_time_ms};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum EventRouteDecision {
    ApplyAuthenticatedTarget,
    RetargetMovedPane,
    #[default]
    DropRemovedPane,
}

fn event_route_decision(
    current_worklane_id: Option<&str>,
    authenticated_worklane_id: &str,
) -> EventRouteDecision {
    match current_worklane_id {
        None => EventRouteDecision::DropRemovedPane,
        Some(current) if current == authenticated_worklane_id => {
            EventRouteDecision::ApplyAuthenticatedTarget
        }
        Some(_) => EventRouteDecision::RetargetMovedPane,
    }
}

/// Owns authenticated agent-event ingestion and bounded transcript enrichment
/// for one application window. `AgentRuntime` remains the sole transport and
/// token authority; `WorkspaceState` remains the sole status reducer.
pub(super) struct AgentEventCoordinator {
    runtime: AgentRuntime,
    transcript_enricher: CodexTranscriptEnricher,
}

impl AgentEventCoordinator {
    pub(super) fn start(window_id: impl Into<String>) -> Result<Self, String> {
        let runtime = AgentRuntime::start(window_id)?;
        let codex_home = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))
            .unwrap_or_else(|| runtime.missing_codex_home());
        Ok(Self {
            runtime,
            transcript_enricher: CodexTranscriptEnricher::new(codex_home),
        })
    }

    pub(super) fn environment_for_pane(
        &mut self,
        worklane_id: &str,
        pane_id: &str,
    ) -> Result<Vec<(String, String)>, String> {
        self.runtime.environment_for_pane(worklane_id, pane_id)
    }

    pub(super) fn unregister_pane(&mut self, pane_id: &str) {
        self.transcript_enricher.cancel_pane(pane_id);
        self.runtime.unregister_pane(pane_id);
    }

    pub(super) fn sync_targets(&mut self, state: &WorkspaceState) -> Result<(), String> {
        let topology = state
            .worklanes()
            .iter()
            .flat_map(|worklane| {
                worklane.columns.iter().flat_map(move |column| {
                    column
                        .panes
                        .iter()
                        .map(move |pane| (worklane.id.clone(), pane.id.clone()))
                })
            })
            .collect::<Vec<_>>();
        self.runtime.retarget_registered_panes(
            topology
                .iter()
                .map(|(worklane_id, pane_id)| (worklane_id.as_str(), pane_id.as_str())),
        )
    }

    pub(super) fn schedule_for_pane(&mut self, state: &WorkspaceState, pane_id: &str) {
        let fallback_working_directory = std::env::current_dir()
            .ok()
            .map(|path| path.to_string_lossy().into_owned());
        let Some(candidate) = state
            .codex_transcript_enrichment_candidate(pane_id, fallback_working_directory.as_deref())
        else {
            return;
        };
        if self.transcript_enricher.schedule(candidate.clone()) {
            eprintln!(
                "zentty-linux: codex-transcript-enrichment-scheduled pane={} session={}",
                candidate.pane_id, candidate.session_id
            );
        }
    }

    pub(super) fn shutdown(&mut self) {
        self.transcript_enricher.shutdown();
    }

    pub(super) fn drain(shell: &Rc<RefCell<ApplicationShell>>) {
        let tmux_commands = shell.borrow().agent_events.runtime.drain_tmux();
        let mut tmux_changed_product_state = false;
        for command in tmux_commands {
            let changes_state = matches!(command.request.command(), TmuxCommand::SelectPane);
            ApplicationShell::execute_tmux_command(shell, command);
            tmux_changed_product_state |= changes_state;
        }
        if tmux_changed_product_state {
            shell.borrow().render();
        }

        let events = shell.borrow().agent_events.runtime.drain();
        let now = unix_time_ms();
        let mut sidebar_changed = false;
        for mut event in events {
            let pane_id = event.target.pane_id.clone();
            let current_worklane_id = {
                let shell = shell.borrow();
                shell
                    .state
                    .worklane_id_for_pane(&pane_id)
                    .map(str::to_owned)
            };
            let route =
                event_route_decision(current_worklane_id.as_deref(), &event.target.worklane_id);
            match route {
                EventRouteDecision::DropRemovedPane => {
                    eprintln!("zentty-linux: agent-event-dropped pane={pane_id} reason=removed");
                    continue;
                }
                EventRouteDecision::RetargetMovedPane => {
                    current_worklane_id
                        .as_ref()
                        .expect("route decision proved the pane exists")
                        .clone_into(&mut event.target.worklane_id);
                }
                EventRouteDecision::ApplyAuthenticatedTarget => {}
            }
            eprintln!(
                "zentty-linux: agent-event pane={} worklane={} kind={} session={}",
                event.target.pane_id,
                event.target.worklane_id,
                event.event_kind(),
                event.session_id().unwrap_or("pane-default")
            );
            let mut shell = shell.borrow_mut();
            shell.state.apply_agent_event(event, now);
            let ApplicationShell {
                state,
                agent_events,
                ..
            } = &mut *shell;
            agent_events.schedule_for_pane(state, &pane_id);
            sidebar_changed = true;
        }

        let enrichments = shell.borrow_mut().agent_events.transcript_enricher.drain();
        for enrichment in enrichments {
            let mut shell = shell.borrow_mut();
            if shell.state.apply_codex_transcript_enrichment(
                &enrichment.candidate,
                &enrichment.question,
                now,
            ) {
                eprintln!(
                    "zentty-linux: codex-transcript-enriched pane={} session={}",
                    enrichment.candidate.pane_id, enrichment.candidate.session_id
                );
                sidebar_changed = true;
            }
        }
        if sidebar_changed {
            shell.borrow().render_sidebar();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{EventRouteDecision, event_route_decision};

    #[test]
    fn authenticated_events_apply_only_to_current_pane_identity() {
        assert_eq!(
            event_route_decision(Some("lane-a"), "lane-a"),
            EventRouteDecision::ApplyAuthenticatedTarget
        );
        assert_eq!(
            event_route_decision(Some("lane-b"), "lane-a"),
            EventRouteDecision::RetargetMovedPane
        );
        assert_eq!(
            event_route_decision(None, "lane-a"),
            EventRouteDecision::DropRemovedPane
        );
    }
}
