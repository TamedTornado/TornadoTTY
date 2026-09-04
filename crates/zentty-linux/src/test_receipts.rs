use tornadotty_test_receipts::{
    ActionName, ActionOutcome, FailureCode, FocusTarget, GeometrySnapshot, LifecycleState,
    PaneColumn, ReceiptEvent, ReceiptId, WidgetName, WorklaneTopology,
};
use zentty_core::WorkspaceState;

pub(crate) fn initialize() -> Result<(), String> {
    tornadotty_test_receipts::initialize_from_environment()
        .map(|_| ())
        .map_err(|error| format!("could not initialize test receipt contract: {error}"))
}

pub(crate) fn finish() -> Result<(), String> {
    tornadotty_test_receipts::finish()
        .map(|_| ())
        .map_err(|error| format!("could not finish test receipt contract: {error}"))
}

pub(crate) fn lifecycle(state: LifecycleState, pane_id: Option<&str>) {
    let pane_id = match pane_id.map(ReceiptId::new).transpose() {
        Ok(pane_id) => pane_id,
        Err(error) => return report_error(&error),
    };
    emit(ReceiptEvent::Lifecycle { state, pane_id });
}

pub(crate) fn pane_focus(pane_id: &str) {
    let pane_id = match ReceiptId::new(pane_id) {
        Ok(pane_id) => pane_id,
        Err(error) => return report_error(&error),
    };
    emit(ReceiptEvent::Focus {
        focus: FocusTarget::Pane { pane_id },
    });
}

pub(crate) fn widget_focus(widget_name: &str) {
    let widget = match widget_name {
        "notification-sound-import" => WidgetName::NotificationSoundImport,
        _ => return,
    };
    emit(ReceiptEvent::Focus {
        focus: FocusTarget::Widget { widget },
    });
}

pub(crate) fn settings_focus() {
    emit(ReceiptEvent::Focus {
        focus: FocusTarget::Widget {
            widget: WidgetName::SettingsWindow,
        },
    });
}

pub(crate) fn window_geometry(window_id: &str, width: i32, height: i32) {
    let (Ok(window_id), Ok(width), Ok(height)) = (
        ReceiptId::new(window_id),
        u32::try_from(width),
        u32::try_from(height),
    ) else {
        eprintln!("tornadotty-test-receipts: invalid bounded window geometry");
        return;
    };
    emit(ReceiptEvent::Geometry {
        geometry: GeometrySnapshot::Window {
            window_id,
            width,
            height,
        },
    });
}

pub(crate) fn workspace(window_id: &str, state: &WorkspaceState) {
    if let Err(error) = emit_workspace(window_id, state) {
        report_error(&error);
    }
}

pub(crate) fn action(action: ActionName, outcome: ActionOutcome, target_id: Option<&str>) {
    let target_id = match target_id.map(ReceiptId::new).transpose() {
        Ok(target_id) => target_id,
        Err(error) => return report_error(&error),
    };
    emit(ReceiptEvent::ActionCompletion {
        action,
        outcome,
        target_id,
    });
}

pub(crate) fn failure(code: FailureCode, target_id: Option<&str>) {
    let target_id = match target_id.map(ReceiptId::new).transpose() {
        Ok(target_id) => target_id,
        Err(error) => return report_error(&error),
    };
    emit(ReceiptEvent::Failure { code, target_id });
}

fn emit_workspace(window_id: &str, state: &WorkspaceState) -> tornadotty_test_receipts::Result<()> {
    let window_id = ReceiptId::new(window_id)?;
    let focused_pane_id = required_id(state.focused_pane_id(), "focused pane")?;
    let mut worklanes = Vec::with_capacity(state.worklanes().len());
    for worklane in state.worklanes() {
        let selected_pane_id = worklane
            .columns
            .iter()
            .find(|column| column.id == worklane.focused_column_id)
            .map(|column| column.focused_pane_id.as_str());
        worklanes.push(WorklaneTopology {
            worklane_id: ReceiptId::new(&worklane.id)?,
            pane_ids: worklane
                .columns
                .iter()
                .flat_map(|column| &column.panes)
                .map(|pane| ReceiptId::new(&pane.id))
                .collect::<tornadotty_test_receipts::Result<Vec<_>>>()?,
            selected_pane_id: required_id(selected_pane_id, "selected pane")?,
        });
    }
    tornadotty_test_receipts::emit(ReceiptEvent::Topology {
        window_id: window_id.clone(),
        worklanes,
        focused_pane_id,
    })?;
    for worklane in state.worklanes() {
        tornadotty_test_receipts::emit(ReceiptEvent::Geometry {
            geometry: GeometrySnapshot::PaneLayout {
                window_id: window_id.clone(),
                worklane_id: ReceiptId::new(&worklane.id)?,
                columns: worklane
                    .columns
                    .iter()
                    .map(|column| {
                        Ok(PaneColumn {
                            column_id: ReceiptId::new(&column.id)?,
                            pane_ids: column
                                .panes
                                .iter()
                                .map(|pane| ReceiptId::new(&pane.id))
                                .collect::<tornadotty_test_receipts::Result<Vec<_>>>()?,
                        })
                    })
                    .collect::<tornadotty_test_receipts::Result<Vec<_>>>()?,
            },
        })?;
    }
    Ok(())
}

fn required_id(value: Option<&str>, field: &str) -> tornadotty_test_receipts::Result<ReceiptId> {
    let Some(value) = value else {
        return Err(tornadotty_test_receipts::ReceiptError::invalid_event(
            format!("workspace has no {field}"),
        ));
    };
    ReceiptId::new(value)
}

fn emit(event: ReceiptEvent) {
    if let Err(error) = tornadotty_test_receipts::emit(event) {
        report_error(&error);
    }
}

fn report_error(error: &tornadotty_test_receipts::ReceiptError) {
    eprintln!(
        "tornadotty-test-receipts: write-failed kind={:?}",
        error.kind()
    );
}
