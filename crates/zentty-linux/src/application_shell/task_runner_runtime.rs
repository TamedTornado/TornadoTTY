use std::cell::RefCell;
use std::rc::Rc;

use gtk::{gio, glib};
use zentty_core::{discover_task_runners, revalidate_task_runner};

use super::pane_context::PaneContext;
use super::{ApplicationShell, pane_runtime::PaneRuntimeCoordinator};

mod worker;

pub(super) struct Catalog {
    source: PaneContext,
    pub(super) actions: std::collections::BTreeMap<String, zentty_core::TaskRunnerAction>,
}

fn focused_source(shell: &ApplicationShell) -> Option<PaneContext> {
    let mut source = PaneContext::capture(shell, shell.state.focused_pane_id()?)?;
    // Tasks belong to the pane's project, not a transient prompt-hook process.
    // A normal shell hook must not invalidate an otherwise unchanged catalog.
    source.foreground_pid = None;
    Some(source)
}

fn source_is_current(source: &PaneContext, shell: &ApplicationShell) -> bool {
    !shell.shutting_down && source.matches(focused_source(shell).as_ref())
}

pub(super) fn discover(shell: &ApplicationShell) {
    let Some(source) = focused_source(shell) else {
        return;
    };
    let Some(directory) = source.working_directory.clone() else {
        return;
    };
    let Some(permit) = worker::Permit::discovery() else {
        eprintln!("zentty-linux: task-discovery rejected=busy capacity=1");
        shell.restore_notice.show("Task discovery is still busy. Other commands are available; reopen the palette to retry.");
        return;
    };
    let generation = shell.command_palette.generation();
    let weak = shell.self_handle.borrow().clone();
    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(move || {
            let _permit = permit;
            discover_task_runners(&directory)
        })
        .await;
        let Some(shell) = weak.upgrade() else { return };
        let mut shell = shell.borrow_mut();
        if !source_is_current(&source, &shell)
            || !shell.command_palette.is_visible()
            || shell.command_palette.generation() != generation
        {
            eprintln!("zentty-linux: task-discovery rejected=stale-context");
            return;
        }
        let actions = match result {
            Ok(Ok(actions)) => actions,
            Ok(Err(error)) => {
                eprintln!("zentty-linux: task-discovery error={error}");
                return;
            }
            Err(_) => {
                eprintln!("zentty-linux: task-discovery error=worker-panic");
                return;
            }
        };
        shell.task_runner_catalog = Some(Catalog {
            source,
            actions: actions
                .into_iter()
                .map(|action| (action.id.clone(), action))
                .collect(),
        });
        let items = shell.command_palette_task_items();
        shell.command_palette.replace_task_items(items);
    });
}

pub(super) fn run_task(shell: &Rc<RefCell<ApplicationShell>>, id: &str) {
    let (action, source) = {
        let shell_ref = shell.borrow();
        let Some(catalog) = &shell_ref.task_runner_catalog else {
            return;
        };
        let Some(action) = catalog.actions.get(id) else {
            eprintln!("zentty-linux: action=run-task rejected=unknown-snapshot id={id:?}");
            return;
        };
        let source = catalog.source.clone();
        if !source_is_current(&source, &shell_ref) {
            eprintln!("zentty-linux: action=run-task rejected=stale-context");
            return;
        }
        (action.clone(), source)
    };
    let Some(permit) = worker::Permit::validation() else {
        ApplicationShell::report_action_error(
            shell,
            super::action_router::ACTION_RUN_TASK,
            "Another task launch is still being checked. Retry when it completes.",
        );
        return;
    };
    let weak = Rc::downgrade(shell);
    let id = id.to_owned();
    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(move || {
            // Retain capacity through GTK application of the result, too.
            (revalidate_task_runner(&action), permit)
        })
        .await;
        let Some(shell) = weak.upgrade() else { return };
        if !source_is_current(&source, &shell.borrow()) {
            eprintln!("zentty-linux: action=run-task rejected=stale-context id={id:?}");
            return;
        }
        let Ok((result, _permit)) = result else {
            ApplicationShell::report_action_error(
                &shell,
                super::action_router::ACTION_RUN_TASK,
                "Task validation worker failed. Reopen the palette to retry.",
            );
            return;
        };
        match result {
            Ok(action) => activate_validated(&shell, &action),
            Err(error) => {
                eprintln!("zentty-linux: action=run-task rejected=stale id={id:?} error={error}");
                ApplicationShell::report_action_error(
                    &shell,
                    super::action_router::ACTION_RUN_TASK,
                    &error,
                );
            }
        }
    });
}

fn activate_validated(
    shell: &Rc<RefCell<ApplicationShell>>,
    action: &zentty_core::TaskRunnerAction,
) {
    if !action.is_enabled() {
        super::open_with_runtime::open_local_path_primary(
            shell,
            &action.source_path,
            "task-source",
        );
        return;
    }
    if let Err(error) = launch_in_new_pane(shell, action) {
        ApplicationShell::report_action_error(shell, super::action_router::ACTION_RUN_TASK, &error);
    }
}

fn launch_in_new_pane(
    shell: &Rc<RefCell<ApplicationShell>>,
    action: &zentty_core::TaskRunnerAction,
) -> Result<(), String> {
    let previous = shell.borrow().state.clone();
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
            shell_ref.state = previous;
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
        ApplicationShell::rollback_pane_creation(shell, previous, &pane_id);
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
