use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use zentty_agent_ipc::AuthenticatedTmuxRequest;
use zentty_core::{AuthenticatedAgentEvent, WorkspaceState};
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
    runtime: Rc<RefCell<AgentRuntime>>,
    window_id: String,
    transcript_enricher: CodexTranscriptEnricher,
}

impl AgentEventCoordinator {
    pub(super) fn start(window_id: impl Into<String>, runtime: Rc<RefCell<AgentRuntime>>) -> Self {
        let window_id = window_id.into();
        let codex_home = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))
            .unwrap_or_else(|| runtime.borrow().missing_codex_home());
        Self {
            runtime,
            window_id,
            transcript_enricher: CodexTranscriptEnricher::new(codex_home),
        }
    }

    pub(super) fn environment_for_pane(
        &mut self,
        worklane_id: &str,
        pane_id: &str,
    ) -> Result<Vec<(String, String)>, String> {
        self.runtime
            .borrow_mut()
            .environment_for_pane(&self.window_id, worklane_id, pane_id)
    }

    pub(super) fn unregister_pane(&mut self, pane_id: &str) {
        self.transcript_enricher.cancel_pane(pane_id);
        self.runtime.borrow_mut().unregister_pane(pane_id);
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
        self.runtime
            .borrow_mut()
            .retarget_registered_panes(topology.iter().map(|(worklane_id, pane_id)| {
                (
                    self.window_id.as_str(),
                    worklane_id.as_str(),
                    pane_id.as_str(),
                )
            }))
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

    pub(super) fn shutdown(&mut self, owned_live_pane_ids: &[String]) {
        for pane_id in owned_live_pane_ids {
            self.runtime.borrow_mut().unregister_pane(pane_id);
        }
        self.transcript_enricher.shutdown();
    }

    pub(super) fn apply_inputs(
        shell: &Rc<RefCell<ApplicationShell>>,
        tmux_commands: Vec<AuthenticatedTmuxRequest>,
        events: Vec<AuthenticatedAgentEvent>,
    ) {
        let mut tmux_changed_product_state = false;
        for command in tmux_commands {
            let changes_state = matches!(command.request.command(), TmuxCommand::SelectPane);
            ApplicationShell::execute_tmux_command(shell, command);
            tmux_changed_product_state |= changes_state;
        }
        if tmux_changed_product_state {
            shell.borrow().render();
        }

        let now = unix_time_ms();
        let mut sidebar_changed = false;
        let mut review_refresh_panes = Vec::new();
        for mut event in events {
            let pane_id = event.target.pane_id.clone();
            let refresh_review = matches!(event.event_kind(), "agent.idle" | "session.end");
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
            if refresh_review {
                review_refresh_panes.push(pane_id.clone());
            }
            let ApplicationShell {
                state,
                agent_events,
                ..
            } = &mut *shell;
            agent_events.schedule_for_pane(state, &pane_id);
            sidebar_changed = true;
        }

        if !review_refresh_panes.is_empty() {
            let mut shell = shell.borrow_mut();
            for pane_id in review_refresh_panes {
                super::project_context_runtime::mark_pane_for_refresh(&mut shell, &pane_id);
                eprintln!(
                    "zentty-linux: project-context pane={pane_id} refresh=agent-completion-requested"
                );
            }
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
