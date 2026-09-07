use super::{ApplicationShell, live_capture_context, read_proc_command};
use std::collections::BTreeMap;
use zentty_core::{
    TemplateKind, WorklaneRecipe, WorkspaceTemplate, WorkspaceTemplateCaptureContext,
};

pub(super) struct Capture {
    worklane: WorklaneRecipe,
    pids: BTreeMap<String, u64>,
    environments: BTreeMap<String, BTreeMap<String, String>>,
    width: f64,
}

impl Capture {
    pub(super) fn snapshot(shell: &ApplicationShell) -> Result<Self, String> {
        let projected = shell.state.to_window_recipe(&shell.window_template);
        let worklane = projected
            .worklanes
            .into_iter()
            .find(|worklane| worklane.id == shell.state.active_worklane_id())
            .ok_or("active worklane is absent from the workspace projection")?;
        let (pids, environments) = live_capture_context(shell, &worklane);
        Ok(Self {
            worklane,
            pids,
            environments,
            width: f64::from(shell.pane_viewport_width()),
        })
    }

    // Worker-only: process arguments are read here, never during GTK capture.
    pub(super) fn finish(
        &self,
        id: &str,
        name: &str,
        kind: TemplateKind,
        now: &str,
    ) -> WorkspaceTemplate {
        let commands = self
            .pids
            .iter()
            .filter_map(|(pane, pid)| {
                read_proc_command(*pid).map(|command| (pane.clone(), command))
            })
            .collect();
        WorkspaceTemplate::capture(
            &self.worklane,
            kind,
            name,
            WorkspaceTemplateCaptureContext {
                id,
                now,
                captured_readable_width: Some(self.width),
                commands: &commands,
                environments: &self.environments,
            },
        )
    }
}
