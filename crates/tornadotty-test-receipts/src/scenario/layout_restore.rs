//! Restored inactive-column coverage within the existing real layout journey.
use super::{
    Backend, capture_layout, driver_success, focus_input_target, layout_geometry, send_input_key,
};
use serde_json::json;
use std::{fs, path::Path};

pub(super) fn prepare(state: &Path) -> Result<(), String> {
    let column = |pane: &str| {
        json!({
            "id": format!("column-{pane}"), "width": 754.0,
            "focusedPaneID": pane, "paneHeights": [1.0],
            "panes": [{"id": pane}]
        })
    };
    let snapshot = json!({
        "schemaVersion": 1, "savedAt": 807_321_600.5, "reason": "cleanExit",
        "workspace": {
            "schemaVersion": 3, "activeWindowID": "window-1",
            "windows": [{
                "id": "window-1", "frame": {"x": 0.0, "y": 0.0, "width": 1024.0, "height": 768.0},
                "activeWorklaneID": "worklane-1",
                "worklanes": [
                    {"id": "worklane-1", "nextPaneNumber": 2,
                     "focusedColumnID": "column-pane-1", "columns": [column("pane-1")]},
                    {"id": "worklane-2", "nextPaneNumber": 5,
                     "focusedColumnID": "column-pane-2",
                     "columns": [column("pane-2"), column("pane-3"), column("pane-4")]}
                ]
            }]
        }, "restoreDraftWindows": []
    });
    fs::write(state.join("restore-snapshot.json"), snapshot.to_string())
        .map_err(|error| format!("could not seed isolated restore layout: {error}"))
}

pub(super) fn observe(
    driver: &Path,
    session: &Path,
    receipt: &Path,
    backend: Backend,
    root: &Path,
) -> Result<(), String> {
    driver_success(
        driver,
        &["session", "wait"],
        &[session],
        &["10000", "running"],
    )?;
    driver_success(
        driver,
        &["wait"],
        &[receipt],
        &["10000", "1", "terminal-ready", "pane-1"],
    )?;
    if matches!(backend, Backend::Wayland) {
        driver_success(
            driver,
            &["wait"],
            &[receipt],
            &["10000", "1", "window-geometry", "window-1", "1024", "768"],
        )?;
        driver_success(
            driver,
            &["wait"],
            &[receipt],
            &["10000", "1", "focus-pane", "pane-1"],
        )?;
    }
    focus_input_target(driver, session, backend)?;
    capture_layout(driver, session, backend, root, "restored-startup-settled")?;
    send_input_key(driver, session, backend, "ctrl+Tab")?;
    for (pane, next) in [("pane-2", false), ("pane-3", true), ("pane-4", true)] {
        if next {
            send_input_key(driver, session, backend, "ctrl+alt+Right")?;
        }
        driver_success(
            driver,
            &["wait"],
            &[receipt],
            &["10000", "1", "focus-pane", pane],
        )?;
        capture_layout(
            driver,
            session,
            backend,
            root,
            &format!("{pane}-restored-settled"),
        )?;
        let geometry = layout_geometry(driver, session, backend, root, pane, "restored-full")?;
        // The restored window is 1024 wide, with a 262 sidebar and 8px
        // separator: 754 viewport pixels. Text-area pixels round to cells.
        let cell_width = geometry[1] / geometry[3];
        if cell_width == 0 {
            return Err(format!("invalid PTY cell width: {geometry:?}"));
        }
        let expected = 754 / cell_width * cell_width;
        if geometry[1] != expected {
            return Err(format!(
                "restored full-width {pane} has {} PTY pixels, expected {expected} for 754px viewport",
                geometry[1]
            ));
        }
    }
    println!("restored inactive full-width columns: all three PTY widths fill the 754px viewport");
    Ok(())
}
