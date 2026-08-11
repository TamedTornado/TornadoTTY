use std::cell::RefCell;
use std::rc::Rc;

use zentty_core::revalidate_task_runner;

use super::{ApplicationShell, pane_runtime::PaneRuntimeCoordinator};

pub(super) fn run_task(shell: &Rc<RefCell<ApplicationShell>>, id: &str) {
    let action = {
        let shell_ref = shell.borrow();
        let Some(action) = shell_ref.task_runner_actions.get(id) else {
            eprintln!("zentty-linux: action=run-task rejected=unknown-snapshot id={id:?}");
            return;
        };
        action.clone()
    };
    if !action.is_enabled() {
        eprintln!("zentty-linux: action=run-task rejected=disabled id={id:?}");
        return;
    }
    let action = match revalidate_task_runner(&action) {
        Ok(action) => action,
        Err(error) => {
            eprintln!("zentty-linux: action=run-task rejected=stale id={id:?} error={error}");
            return;
        }
    };
    if let Err(error) = launch_in_new_pane(shell, &action) {
        ApplicationShell::report_action_error(shell, super::action_router::ACTION_RUN_TASK, &error);
    }
}

fn launch_in_new_pane(
    shell: &Rc<RefCell<ApplicationShell>>,
    action: &zentty_core::TaskRunnerAction,
) -> Result<(), String> {
    let pane_id = {
        let mut shell_ref = shell.borrow_mut();
        let pane_id = shell_ref.take_pane_id();
        let width = f64::from(shell_ref.focused_column_render_width());
        if !shell_ref
            .state
            .add_pane_right_without_resizing(pane_id.clone(), width)
        {
            return Err("generated duplicate task pane identity".to_owned());
        }
        if !shell_ref.state.configure_pane_launch(
            &pane_id,
            Some(action.working_directory.to_string_lossy().into_owned()),
            Some(action.execution_command.clone()),
        ) {
            let _ = shell_ref.state.close_focused_pane();
            return Err("new task pane could not retain its launch context".to_owned());
        }
        shell_ref.pane_runtime.queue_launch(
            &pane_id,
            action.execution_command.clone(),
            action
                .environment
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        );
        pane_id
    };
    if let Err(error) = PaneRuntimeCoordinator::create_surface(shell, &pane_id) {
        let mut shell_ref = shell.borrow_mut();
        shell_ref.pane_runtime.cancel_launch(&pane_id);
        let _ = shell_ref.state.close_focused_pane();
        return Err(error);
    }
    let shell_ref = shell.borrow();
    eprintln!(
        "zentty-linux: action=run-task pane={pane_id} title={:?} cwd={} command={:?}",
        action.title,
        action.working_directory.display(),
        action.execution_command
    );
    shell_ref.render();
    shell_ref.focus_selected_surface();
    drop(shell_ref);
    ApplicationShell::scroll_panes_to_end(shell);
    Ok(())
}
