use std::rc::Rc;

use gtk::glib;
use zentty_core::{
    AgentPhase, CloseDecision, ClosePaneEvidence, CloseReason, CloseTarget, decide_close,
};

use super::ApplicationShell;

mod inspection;
pub(crate) use inspection::CloseSnapshot;
use inspection::{ClosePaneSnapshot, InspectedClose, Permit};

impl ApplicationShell {
    fn focus_terminal_after_confirmation(shell: &Rc<std::cell::RefCell<Self>>) {
        let window = shell.borrow().window().clone();
        // Dismissing a modal transient does not guarantee that a Wayland
        // compositor reactivates its parent. Request the parent explicitly;
        // focus restoration below still waits for the real active transition
        // and never treats present() itself as compositor acceptance.
        gtk::prelude::GtkWindowExt::present(&window);
        eprintln!("zentty-linux: confirmation parent-presented=true");
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

    pub(super) fn pane_close_evidence(&self, pane_id: &str) -> CloseSnapshot {
        let worklane_id = self.state.worklane_id_for_pane(pane_id);
        let panes = worklane_id
            .and_then(|_| self.close_pane_evidence(pane_id))
            .into_iter()
            .collect();
        CloseSnapshot::new(
            CloseTarget::Pane {
                window_id: self.window_template.id.clone(),
                worklane_id: worklane_id.unwrap_or_default().to_owned(),
                pane_id: pane_id.to_owned(),
            },
            panes,
        )
    }

    pub(super) fn worklane_close_evidence(&self, worklane_id: &str) -> CloseSnapshot {
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
        CloseSnapshot::new(
            CloseTarget::Worklane {
                window_id: self.window_template.id.clone(),
                worklane_id: worklane_id.to_owned(),
            },
            panes,
        )
    }

    pub(crate) fn window_close_evidence(&self) -> CloseSnapshot {
        let panes = self
            .state
            .worklanes()
            .iter()
            .flat_map(|worklane| &worklane.columns)
            .flat_map(|column| &column.panes)
            .filter_map(|pane| self.close_pane_evidence(&pane.id))
            .collect();
        CloseSnapshot::new(
            CloseTarget::Window {
                window_id: self.window_template.id.clone(),
            },
            panes,
        )
    }

    fn close_pane_evidence(&self, pane_id: &str) -> Option<ClosePaneSnapshot> {
        let pane = self.state.pane(pane_id)?;
        let agent_status = self.state.pane_agent_status(pane_id);
        let has_active_agent = agent_status.as_ref().is_some_and(|status| {
            matches!(
                status.phase,
                AgentPhase::Starting
                    | AgentPhase::Running
                    | AgentPhase::NeedsInput
                    | AgentPhase::UnresolvedStop
            )
        });
        Some(ClosePaneSnapshot {
            identity: super::pane_context::PaneContext::capture(self, pane_id)?,
            window_id: self.window_template.id.clone(),
            evidence: ClosePaneEvidence {
                pane_id: pane_id.to_owned(),
                has_running_process: false,
                has_active_agent,
                has_session_history: pane
                    .last_run_command
                    .as_deref()
                    .is_some_and(has_user_session_history)
                    || agent_status.is_some(),
            },
        })
    }

    fn current_close_evidence(&self, target: &CloseTarget) -> CloseSnapshot {
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
        snapshot: &CloseSnapshot,
        confirmation_enabled: bool,
        action: Rc<dyn Fn()>,
    ) {
        {
            let shell_ref = shell.borrow();
            let mut pending = shell_ref.pending_close_evidence.borrow_mut();
            if pending.is_some() {
                eprintln!(
                    "zentty-linux: close-request ignored=pending target={:?}",
                    snapshot.target
                );
                return;
            }
            if snapshot.panes.is_empty() || shell_ref.shutting_down {
                eprintln!(
                    "zentty-linux: close-request ignored=stale target={:?}",
                    snapshot.target
                );
                return;
            }
            // Disabled confirmation does not require process classification.
            // No asynchronous boundary is introduced on this explicit policy.
            if confirmation_enabled {
                *pending = Some(snapshot.clone());
            }
        }
        if !confirmation_enabled {
            action();
            return;
        }
        let snapshot = snapshot.clone();
        Self::inspect_close(shell, snapshot.clone(), move |shell, inspected| {
            Self::present_close_decision(shell, snapshot, inspected, action);
        });
    }

    fn reject_pending_close(shell: &Rc<std::cell::RefCell<Self>>, reason: &str) {
        let shell = shell.borrow();
        shell.pending_close_evidence.borrow_mut().take();
        eprintln!("zentty-linux: close-request ignored={reason}");
        if !shell.shutting_down {
            shell.restore_notice.show("Close was not applied. The pane changed or process inspection was unavailable. Retry the close action.");
        }
    }

    fn inspect_close(
        shell: &Rc<std::cell::RefCell<Self>>,
        snapshot: CloseSnapshot,
        ready: impl FnOnce(&Rc<std::cell::RefCell<Self>>, InspectedClose) + 'static,
    ) {
        let Some(permit) = Permit::acquire() else {
            Self::reject_pending_close(shell, "inspection-busy");
            return;
        };
        let weak = Rc::downgrade(shell);
        glib::spawn_future_local(async move {
            let source = snapshot.clone();
            let result = gtk::gio::spawn_blocking(move || {
                let _permit = permit;
                source.inspect()
            })
            .await;
            let Some(shell) = weak.upgrade() else { return };
            let current = {
                let shell = shell.borrow();
                !shell.shutting_down
                    && shell.pending_close_evidence.borrow().as_ref() == Some(&snapshot)
                    && shell.current_close_evidence(&snapshot.target) == snapshot
            };
            if !current {
                Self::reject_pending_close(&shell, "stale-inspection");
                return;
            }
            match result {
                Ok(inspected) => ready(&shell, inspected),
                Err(_) => Self::reject_pending_close(&shell, "inspection-worker-panic"),
            }
        });
    }

    fn present_close_decision(
        shell: &Rc<std::cell::RefCell<Self>>,
        snapshot: CloseSnapshot,
        inspected: InspectedClose,
        action: Rc<dyn Fn()>,
    ) {
        match decide_close(inspected.evidence.clone(), true) {
            CloseDecision::StaleTarget => {
                Self::reject_pending_close(shell, "stale-target");
            }
            CloseDecision::CloseNow => {
                shell.borrow().pending_close_evidence.borrow_mut().take();
                action();
            }
            CloseDecision::Confirm { reason, evidence } => {
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
                            .is_some_and(|pending| pending == &snapshot);
                        let accepted = response == Ok(1);
                        eprintln!("zentty-linux: confirmation accepted={accepted}");
                        if !was_pending {
                            return;
                        }
                        if !accepted
                            || matches!(
                                snapshot.target,
                                CloseTarget::Pane { .. } | CloseTarget::Worklane { .. }
                            )
                        {
                            let weak = Rc::downgrade(&shell);
                            glib::idle_add_local_once(move || {
                                // Return pane input while inspection is pending,
                                // but never re-present a window being closed.
                                if let Some(shell) = weak.upgrade() {
                                    Self::focus_terminal_after_confirmation(&shell);
                                }
                            });
                        }
                        if !accepted {
                            shell.borrow().pending_close_evidence.borrow_mut().take();
                            return;
                        }
                        Self::inspect_close(&shell, snapshot.clone(), move |shell, current| {
                            if current != inspected {
                                Self::reject_pending_close(shell, "stale-callback");
                                Self::focus_terminal_after_confirmation(shell);
                                return;
                            }
                            let weak = Rc::downgrade(shell);
                            glib::idle_add_local_once(move || {
                                let Some(shell) = weak.upgrade() else {
                                    return;
                                };
                                let still_pending = shell
                                    .borrow()
                                    .pending_close_evidence
                                    .borrow_mut()
                                    .take()
                                    .is_some_and(|pending| pending == snapshot);
                                let unchanged = !shell.borrow().shutting_down
                                    && shell.borrow().current_close_evidence(&snapshot.target)
                                        == snapshot;
                                if still_pending && unchanged {
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
                                } else {
                                    Self::reject_pending_close(&shell, "stale-before-commit");
                                }
                            });
                        });
                    },
                );
                eprintln!("zentty-linux: confirmation shown title={title:?}");
            }
        }
    }
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
            "Quit Tornado TTY?",
            "All windows, panes, and running processes will be terminated.",
            "Quit",
        ),
    }
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
    use super::{has_user_session_history, looks_like_idle_shell};

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
}
