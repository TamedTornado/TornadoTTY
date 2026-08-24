use std::fs;
use std::path::Path;
use std::rc::Rc;

use gtk::glib;
use zentty_core::{
    AgentPhase, CloseDecision, CloseEvidence, ClosePaneEvidence, CloseReason, CloseTarget,
    decide_close,
};

use super::ApplicationShell;

impl ApplicationShell {
    fn focus_terminal_after_confirmation(shell: &Rc<std::cell::RefCell<Self>>) {
        let window = shell.borrow().window().clone();
        let restore = |shell: &Rc<std::cell::RefCell<Self>>| {
            let shell = shell.borrow();
            if shell.shutting_down {
                return;
            }
            let pane_id = shell.state.focused_pane_id().unwrap_or("none").to_owned();
            shell.focus_selected_surface_unchecked();
            eprintln!(
                "zentty-linux: confirmation focus-restored pane={pane_id} window-active={}",
                gtk::prelude::GtkWindowExt::is_active(&shell.window)
            );
        };
        if gtk::prelude::GtkWindowExt::is_active(&window) {
            restore(shell);
            return;
        }

        let weak = Rc::downgrade(shell);
        let handler = Rc::new(std::cell::RefCell::new(None));
        let callback_handler = Rc::clone(&handler);
        let handler_id =
            gtk::prelude::GtkWindowExt::connect_is_active_notify(&window, move |window| {
                if !gtk::prelude::GtkWindowExt::is_active(window) {
                    return;
                }
                if let Some(handler_id) = callback_handler.borrow_mut().take() {
                    gtk::prelude::ObjectExt::disconnect(window, handler_id);
                }
                let Some(shell) = weak.upgrade() else {
                    return;
                };
                restore(&shell);
            });
        *handler.borrow_mut() = Some(handler_id);
        eprintln!("zentty-linux: confirmation focus-pending reason=window-inactive");
    }

    pub(super) fn pane_close_evidence(&self, pane_id: &str) -> CloseEvidence {
        let worklane_id = self.state.worklane_id_for_pane(pane_id);
        let panes = worklane_id
            .and_then(|_| self.close_pane_evidence(pane_id))
            .into_iter()
            .collect();
        CloseEvidence::new(
            CloseTarget::Pane {
                window_id: self.window_template.id.clone(),
                worklane_id: worklane_id.unwrap_or_default().to_owned(),
                pane_id: pane_id.to_owned(),
            },
            panes,
        )
    }

    pub(super) fn worklane_close_evidence(&self, worklane_id: &str) -> CloseEvidence {
        let panes = self
            .state
            .worklanes()
            .iter()
            .find(|worklane| worklane.id == worklane_id)
            .into_iter()
            .flat_map(|worklane| &worklane.columns)
            .flat_map(|column| &column.panes)
            .filter_map(|pane| self.close_pane_evidence(&pane.id))
            .collect();
        CloseEvidence::new(
            CloseTarget::Worklane {
                window_id: self.window_template.id.clone(),
                worklane_id: worklane_id.to_owned(),
            },
            panes,
        )
    }

    pub(crate) fn window_close_evidence(&self) -> CloseEvidence {
        let panes = self
            .state
            .worklanes()
            .iter()
            .flat_map(|worklane| &worklane.columns)
            .flat_map(|column| &column.panes)
            .filter_map(|pane| self.close_pane_evidence(&pane.id))
            .collect();
        CloseEvidence::new(
            CloseTarget::Window {
                window_id: self.window_template.id.clone(),
            },
            panes,
        )
    }

    fn close_pane_evidence(&self, pane_id: &str) -> Option<ClosePaneEvidence> {
        let pane = self.state.pane(pane_id)?;
        let agent_status = self
            .state
            .sidebar_summaries()
            .into_iter()
            .flat_map(|worklane| worklane.pane_rows)
            .find(|summary| summary.pane_id == pane_id)
            .and_then(|summary| summary.agent_status);
        let has_active_agent = agent_status.as_ref().is_some_and(|status| {
            matches!(
                status.phase,
                AgentPhase::Starting
                    | AgentPhase::Running
                    | AgentPhase::NeedsInput
                    | AgentPhase::UnresolvedStop
            )
        });
        let has_running_process = self
            .pane_runtime
            .surface(pane_id)
            .and_then(zentty_ghostty::GhosttySurface::foreground_process_id)
            .is_some_and(process_requires_confirmation);
        Some(ClosePaneEvidence {
            pane_id: pane_id.to_owned(),
            has_running_process,
            has_active_agent,
            has_session_history: pane
                .last_run_command
                .as_deref()
                .is_some_and(has_user_session_history)
                || agent_status.is_some(),
        })
    }

    fn current_close_evidence(&self, target: &CloseTarget) -> CloseEvidence {
        match target {
            CloseTarget::Pane { pane_id, .. } => self.pane_close_evidence(pane_id),
            CloseTarget::Worklane { worklane_id, .. } => self.worklane_close_evidence(worklane_id),
            CloseTarget::Window { .. } => self.window_close_evidence(),
            CloseTarget::Application => self.quit_evidence_handler.as_ref().map_or_else(
                || {
                    let mut evidence = self.window_close_evidence();
                    evidence.target = CloseTarget::Application;
                    evidence
                },
                |handler| handler(),
            ),
        }
    }

    pub(super) fn request_close_action(
        shell: &Rc<std::cell::RefCell<Self>>,
        evidence: &CloseEvidence,
        confirmation_enabled: bool,
        action: Rc<dyn Fn()>,
    ) {
        match decide_close(evidence.clone(), confirmation_enabled) {
            CloseDecision::StaleTarget => {
                eprintln!(
                    "zentty-linux: close-request ignored=stale target={:?}",
                    evidence.target
                );
            }
            CloseDecision::CloseNow => action(),
            CloseDecision::Confirm { reason, evidence } => {
                let shell_ref = shell.borrow();
                let mut pending = shell_ref.pending_close_evidence.borrow_mut();
                if let Some(existing) = pending.as_ref() {
                    eprintln!(
                        "zentty-linux: close-request ignored=pending existing={:?} requested={:?}",
                        existing.target, evidence.target
                    );
                    return;
                }
                *pending = Some(evidence.clone());
                drop(pending);
                drop(shell_ref);
                let (title, detail, accept_label) = prompt_text(&evidence.target, reason);
                let dialog = gtk::AlertDialog::builder()
                    .modal(true)
                    .message(title)
                    .detail(detail)
                    .buttons(["Cancel", accept_label])
                    .cancel_button(0)
                    .default_button(1)
                    .build();
                let weak = Rc::downgrade(shell);
                dialog.choose(
                    Some(shell.borrow().window()),
                    None::<&gtk::gio::Cancellable>,
                    move |response| {
                        let Some(shell) = weak.upgrade() else {
                            return;
                        };
                        let was_pending = shell
                            .borrow()
                            .pending_close_evidence
                            .borrow()
                            .as_ref()
                            .is_some_and(|pending| pending == &evidence);
                        let accepted = response == Ok(1);
                        eprintln!("zentty-linux: confirmation accepted={accepted}");
                        if !was_pending {
                            return;
                        }
                        if !accepted {
                            shell.borrow().pending_close_evidence.borrow_mut().take();
                            let weak = Rc::downgrade(&shell);
                            glib::idle_add_local_once(move || {
                                // AlertDialog completes transient teardown
                                // after invoking the choose callback. Restore
                                // on the next main-loop turn, or wait for the
                                // compositor's parent-window activation event.
                                if let Some(shell) = weak.upgrade() {
                                    Self::focus_terminal_after_confirmation(&shell);
                                }
                            });
                            return;
                        }
                        let current = shell.borrow().current_close_evidence(&evidence.target);
                        if !same_close_evidence(&evidence, &current) {
                            shell.borrow().pending_close_evidence.borrow_mut().take();
                            eprintln!(
                                "zentty-linux: close-request ignored=stale-callback target={:?}",
                                evidence.target
                            );
                            return;
                        }
                        let weak = Rc::downgrade(&shell);
                        glib::idle_add_local_once(move || {
                            let Some(shell) = weak.upgrade() else {
                                return;
                            };
                            let still_pending = shell
                                .borrow()
                                .pending_close_evidence
                                .borrow_mut()
                                .take()
                                .is_some_and(|pending| pending == evidence);
                            if still_pending {
                                let restores_survivor = matches!(
                                    evidence.target,
                                    CloseTarget::Pane { .. } | CloseTarget::Worklane { .. }
                                );
                                action();
                                // A confirmed pane/worklane close may leave a
                                // surviving surface in this window. The modal
                                // transient owned focus while the action
                                // rendered that survivor, so restore it once
                                // the compositor reactivates the parent.
                                if restores_survivor {
                                    Self::focus_terminal_after_confirmation(&shell);
                                }
                            }
                        });
                    },
                );
                eprintln!("zentty-linux: confirmation shown title={title:?}");
            }
        }
    }
}

fn same_close_evidence(expected: &CloseEvidence, current: &CloseEvidence) -> bool {
    expected == current
}

fn prompt_text(
    target: &CloseTarget,
    reason: CloseReason,
) -> (&'static str, &'static str, &'static str) {
    match target {
        CloseTarget::Pane { .. } => (
            "Close this pane?",
            match reason {
                CloseReason::RunningProcess => {
                    "The running process in this pane will be terminated."
                }
                CloseReason::ActiveAgent => "The active agent in this pane will be terminated.",
                CloseReason::SessionHistory => "This pane's session history will be lost.",
            },
            "Close Pane",
        ),
        CloseTarget::Worklane { .. } => (
            "Close this worklane?",
            match reason {
                CloseReason::RunningProcess => {
                    "Running processes in this worklane will be terminated."
                }
                CloseReason::ActiveAgent => "Active agents in this worklane will be terminated.",
                CloseReason::SessionHistory => "This worklane's session history will be lost.",
            },
            "Close Worklane",
        ),
        CloseTarget::Window { .. } => (
            "Close this window?",
            "All panes and running processes in this window will be terminated.",
            "Close Window",
        ),
        CloseTarget::Application => (
            "Quit Zentty?",
            "All windows, panes, and running processes will be terminated.",
            "Quit",
        ),
    }
}

fn process_requires_confirmation(process_id: u64) -> bool {
    let Ok(process_id) = u32::try_from(process_id) else {
        return true;
    };
    let root = Path::new("/proc").join(process_id.to_string());
    let Ok(comm) = fs::read_to_string(root.join("comm")) else {
        return true;
    };
    let Ok(command_line) = fs::read(root.join("cmdline")) else {
        return true;
    };
    !looks_like_idle_shell(comm.trim(), &command_line)
}

fn looks_like_idle_shell(comm: &str, command_line: &[u8]) -> bool {
    let executable = comm.rsplit('/').next().unwrap_or(comm);
    let is_shell = matches!(
        executable,
        "bash" | "dash" | "fish" | "nu" | "nushell" | "sh" | "zsh"
    );
    is_shell
        && !command_line
            .split(|byte| *byte == 0)
            .any(|argument| argument == b"-c" || argument == b"--command")
}

fn has_user_session_history(command: &str) -> bool {
    let command = command.trim();
    !command.is_empty()
        && !command.starts_with("_zentty_")
        && !command.starts_with("PROMPT_COMMAND=")
}

#[cfg(test)]
mod tests {
    use super::{has_user_session_history, looks_like_idle_shell, same_close_evidence};
    use zentty_core::{CloseEvidence, ClosePaneEvidence, CloseTarget};

    fn evidence(panes: &[(&str, bool)]) -> CloseEvidence {
        CloseEvidence::new(
            CloseTarget::Window {
                window_id: "window-1".to_owned(),
            },
            panes
                .iter()
                .map(|(pane_id, running)| ClosePaneEvidence {
                    pane_id: (*pane_id).to_owned(),
                    has_running_process: *running,
                    has_active_agent: false,
                    has_session_history: false,
                })
                .collect(),
        )
    }

    #[test]
    fn interactive_shells_are_idle_but_command_shells_are_live_work() {
        assert!(looks_like_idle_shell("bash", b"/bin/bash\0-l\0"));
        assert!(looks_like_idle_shell("fish", b"fish\0"));
        assert!(!looks_like_idle_shell("sh", b"sh\0-c\0sleep 10\0"));
        assert!(!looks_like_idle_shell("vim", b"vim\0README.md\0"));
    }

    #[test]
    fn shell_integration_bootstrap_is_not_user_history() {
        assert!(!has_user_session_history(
            "PROMPT_COMMAND=\"_zentty_bash_prompt_hook\""
        ));
        assert!(!has_user_session_history("_zentty_ensure_wrapper_path"));
        assert!(has_user_session_history("cargo test"));
    }

    #[test]
    fn accepted_request_rejects_replaced_target_or_changed_risk() {
        assert!(same_close_evidence(
            &evidence(&[("pane-a", false), ("pane-b", false)]),
            &evidence(&[("pane-b", false), ("pane-a", false)])
        ));
        assert!(!same_close_evidence(
            &evidence(&[("pane-a", false)]),
            &evidence(&[("pane-a", false), ("pane-b", false)])
        ));
        assert!(!same_close_evidence(
            &evidence(&[("pane-a", false)]),
            &evidence(&[("pane-a", true)])
        ));
    }
}
