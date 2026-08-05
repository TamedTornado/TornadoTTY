use super::ApplicationShell;
use crate::tmux_compat::{SplitDisposition, SplitPlan, TmuxProductAction};
use std::cell::RefCell;
use std::rc::Rc;
use zentty_agent_ipc::AuthenticatedTmuxRequest;
use zentty_ghostty::TextExtent;
use zentty_tmux_compat::{Command as TmuxCommand, TmuxCompatReply};

impl ApplicationShell {
    pub(super) fn execute_tmux_command(
        shell: &Rc<RefCell<Self>>,
        command: AuthenticatedTmuxRequest,
    ) {
        eprintln!(
            "zentty-linux: tmux-command pane={} worklane={} command={:?}",
            command.target.pane_id,
            command.target.worklane_id,
            command.request.command()
        );
        let reply = match command.request.command() {
            TmuxCommand::SendKeys => Self::execute_tmux_send_keys(shell, &command),
            TmuxCommand::SplitWindow => Self::execute_tmux_split(shell, &command),
            TmuxCommand::CapturePane => Self::execute_tmux_capture(shell, &command),
            _ => {
                let mut shell = shell.borrow_mut();
                let Self {
                    tmux_compat, state, ..
                } = &mut *shell;
                tmux_compat.handle(state, &command.target, &command.request)
            }
        };
        if let Err(error) = command.respond(reply) {
            eprintln!("zentty-linux: tmux-response failed={error}");
        }
    }

    fn execute_tmux_send_keys(
        shell: &Rc<RefCell<Self>>,
        command: &AuthenticatedTmuxRequest,
    ) -> TmuxCompatReply {
        let action = {
            let shell = shell.borrow();
            crate::tmux_compat::TmuxCompatProduct::prepare_send_keys(
                &shell.state,
                &command.target,
                &command.request,
            )
        };
        match action {
            Ok(TmuxProductAction::Noop) => TmuxCompatReply::success(String::new())
                .expect("empty compatibility output fits protocol limits"),
            Ok(TmuxProductAction::SendText { pane_id, text }) => shell
                .borrow()
                .surfaces
                .get(&pane_id)
                .ok_or_else(|| format!("pane {pane_id} has no live terminal surface"))
                .and_then(|surface| surface.send_text(&text).map_err(|error| error.to_string()))
                .map_or_else(
                    |message| {
                        TmuxCompatReply::failure("delivery_failed", message)
                            .expect("bounded product diagnostic fits protocol limits")
                    },
                    |()| {
                        TmuxCompatReply::success(String::new())
                            .expect("empty compatibility output fits protocol limits")
                    },
                ),
            Err((code, message)) => TmuxCompatReply::failure(code, message)
                .expect("bounded product diagnostic fits protocol limits"),
        }
    }

    fn execute_tmux_split(
        shell: &Rc<RefCell<Self>>,
        command: &AuthenticatedTmuxRequest,
    ) -> TmuxCompatReply {
        let plan = {
            let shell = shell.borrow();
            shell
                .tmux_compat
                .prepare_split(&shell.state, &command.target, &command.request)
        };
        let plan = match plan {
            Ok(plan) => plan,
            Err((code, message)) => {
                return TmuxCompatReply::failure(code, message)
                    .expect("bounded product diagnostic fits protocol limits");
            }
        };
        match Self::create_tmux_split_surface(shell, &plan) {
            Ok((pane_id, pre_team_leader_width)) => {
                let mut shell = shell.borrow_mut();
                shell
                    .tmux_compat
                    .record_split(&plan, &pane_id, pre_team_leader_width);
                let reply = shell.tmux_compat.split_reply(&shell.state, &plan, &pane_id);
                shell.render();
                shell.focus_selected_surface();
                reply
            }
            Err(message) => TmuxCompatReply::failure("split_failed", message)
                .expect("bounded product diagnostic fits protocol limits"),
        }
    }

    fn execute_tmux_capture(
        shell: &Rc<RefCell<Self>>,
        command: &AuthenticatedTmuxRequest,
    ) -> TmuxCompatReply {
        let plan = {
            let shell = shell.borrow();
            crate::tmux_compat::TmuxCompatProduct::prepare_capture(
                &shell.state,
                &command.target,
                &command.request,
            )
        };
        let plan = match plan {
            Ok(plan) => plan,
            Err((code, message)) => {
                return TmuxCompatReply::failure(code, message)
                    .expect("bounded product diagnostic fits protocol limits");
            }
        };
        let text = shell
            .borrow()
            .surfaces
            .get(&plan.pane_id)
            .ok_or_else(|| format!("pane {} has no live terminal surface", plan.pane_id))
            .and_then(|surface| {
                surface
                    .read_text(TextExtent::Screen)
                    .map_err(|error| error.to_string())
            });
        match text {
            Ok(text) => shell
                .borrow_mut()
                .tmux_compat
                .complete_capture(&plan, &text),
            Err(message) => TmuxCompatReply::failure("capture_failed", message)
                .expect("bounded product diagnostic fits protocol limits"),
        }
    }

    fn create_tmux_split_surface(
        shell: &Rc<RefCell<Self>>,
        plan: &SplitPlan,
    ) -> Result<(String, Option<u32>), String> {
        let original = shell.borrow().current_pane_reference();
        let (pane_id, pre_team_leader_width) = {
            let mut shell = shell.borrow_mut();
            if !shell
                .state
                .select_worklane_and_pane(&plan.worklane_id, &plan.insertion_pane_id)
            {
                return Err("split insertion pane is unavailable".to_owned());
            }
            let pre_team_leader_width = (plan.disposition == SplitDisposition::RightGolden)
                .then(|| u32::try_from(shell.focused_column_render_width()).ok())
                .flatten();
            let pane_id = shell.take_pane_id();
            let inserted = match plan.disposition {
                SplitDisposition::RightGolden => {
                    shell.state.split_focused_pane_right(pane_id.clone())
                }
                SplitDisposition::StackBelow => {
                    shell.state.split_focused_pane_below(pane_id.clone())
                }
            };
            if !inserted {
                if let Some(original) = &original {
                    let _ = shell
                        .state
                        .select_worklane_and_pane(&original.worklane_id, &original.pane_id);
                }
                return Err("generated duplicate pane identity".to_owned());
            }
            if plan.disposition == SplitDisposition::StackBelow {
                let _ = shell.state.equalize_pane_heights_in_column(&pane_id);
            }
            if !shell
                .state
                .select_worklane_and_pane(&plan.worklane_id, &plan.leader_pane_id)
            {
                let _ = shell.state.close_pane_after_child_exit(&pane_id);
                if let Some(original) = &original {
                    let _ = shell
                        .state
                        .select_worklane_and_pane(&original.worklane_id, &original.pane_id);
                }
                return Err("team leader pane is unavailable".to_owned());
            }
            let viewport_width = f64::from(shell.pane_viewport_width());
            let _ = shell.state.arrange_golden_width(true, viewport_width);
            if let Some(original) = &original
                && original.worklane_id != plan.worklane_id
            {
                let _ = shell
                    .state
                    .select_worklane_and_pane(&original.worklane_id, &original.pane_id);
            }
            (pane_id, pre_team_leader_width)
        };
        if let Err(error) = Self::create_surface(shell, &pane_id) {
            let mut shell = shell.borrow_mut();
            let _ = shell.state.close_pane_after_child_exit(&pane_id);
            if let Some(original) = original {
                let _ = shell
                    .state
                    .select_worklane_and_pane(&original.worklane_id, &original.pane_id);
            }
            return Err(error);
        }
        Ok((pane_id, pre_team_leader_width))
    }
}
