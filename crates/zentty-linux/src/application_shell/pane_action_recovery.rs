//! Recovery for synchronous, user-requested pane mutations. The temporary
//! workspace snapshot is discarded on success; it is not another state owner.
use std::cell::RefCell;
use std::rc::Rc;

use zentty_core::WorkspaceState;

use super::{ApplicationShell, action_router};

impl ApplicationShell {
    pub(super) fn rollback_pane_creation(
        shell: &Rc<RefCell<Self>>,
        previous: WorkspaceState,
        pane_id: &str,
    ) {
        let mut shell = shell.borrow_mut();
        // Surface creation has not installed a surface on these error paths.
        // Drop launch credentials and transient intent, not a sibling surface.
        shell.agent_events.unregister_pane(pane_id);
        shell.pane_runtime.cancel_failed_creation(pane_id);
        shell.restored_pane_commands.remove(pane_id);
        shell.failed_restore_commands.remove(pane_id);
        shell.state = previous;
        shell.render();
        shell.focus_selected_surface();
        eprintln!("zentty-linux: pane-action-rollback pane={pane_id} topology=restored");
    }

    pub(super) fn report_action_error(shell: &Rc<RefCell<Self>>, action: &str, error: &str) {
        let title = action_router::ACTION_SPECS
            .iter()
            .find(|spec| spec.name == action)
            .and_then(|spec| match spec.palette {
                action_router::PaletteDisposition::Ordinary(metadata) => Some(metadata.title),
                _ => None,
            })
            .unwrap_or(action);
        let shell_ref = shell.borrow();
        eprintln!("zentty-linux: action={action} failed: {error}");
        shell_ref.restore_notice.show(&format!(
            "{title} failed.\n{error}\nYour other panes are still available. Fix the reported problem, then retry the action."
        ));
        eprintln!("zentty-linux: action-error-notice action={action} visible=true");
        drop(shell_ref);
        // The originating palette/menu may finish closing after its action
        // callback. Restore terminal focus on the next main-loop turn.
        Self::focus_terminal_after_present(shell);
    }
}
