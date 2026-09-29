//! Physical modifier/click coverage using the existing layout PTY actor.
use super::{
    Backend, DEADLINE, POLL, capture_layout, driver_success, input_target, require_command,
    wait_for_log,
};
use std::{fs, path::Path, process::Command, thread, time::Instant};

pub(super) fn observe(
    driver: &Path,
    session: &Path,
    receipt: &Path,
    backend: Backend,
    root: &Path,
) -> Result<(), String> {
    if !matches!(backend, Backend::X11) {
        return Err("pane-control bypass pointer coverage requires verified X11 targeting; native Wayland pointer ownership remains #198".into());
    }
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
    let deadline = Instant::now() + DEADLINE;
    while !root.join("pane-1-mouse-ready").exists() {
        if Instant::now() >= deadline {
            return Err("PTY mouse actor not ready".into());
        }
        thread::sleep(POLL);
    }
    let (_, window) = input_target(driver, session, backend)?;
    require_command(
        Command::new("xdotool").args(["windowfocus", "--sync", &window]),
        "focus private product window",
    )?;
    // Fix the private window size and target the New Pane Below icon center.
    require_command(
        Command::new("xdotool").args(["windowsize", "--sync", &window, "1080", "768"]),
        "size private window",
    )?;
    driver_success(
        driver,
        &["wait"],
        &[receipt],
        &["10000", "1", "window-geometry", "window-1", "1080", "768"],
    )?;
    require_command(
        Command::new("xdotool").args(["mousemove", "--sync", "--window", &window, "500", "350"]),
        "enter terminal pane",
    )?;
    wait_for_log(
        &session.join("product.log"),
        "pane-controls pane=pane-1 state=shown",
    )?;
    require_command(
        Command::new("xdotool").args(["mousemove", "--sync", "--window", &window, "1025", "70"]),
        "hover pane controls",
    )?;
    capture_layout(driver, session, backend, root, "controls-shown")?;
    let modifier =
        std::env::var("TORNADOTTY_LAYOUT_CONTROL_KEY").unwrap_or_else(|_| "Control_L".into());
    if !matches!(modifier.as_str(), "Control_L" | "Control_R") {
        return Err("control bypass fixture requires Control_L or Control_R".into());
    }
    let result = (|| {
        require_command(
            Command::new("xdotool").args(["keydown", &modifier]),
            "hold Ctrl",
        )?;
        capture_layout(driver, session, backend, root, "controls-bypassed")?;
        require_command(
            Command::new("xdotool").args(["click", "1"]),
            "click underlying terminal",
        )?;
        wait_for_ctrl_click(root)
    })();
    // Release even on assertion failure; never leave a held key in the fixture.
    let release = require_command(
        Command::new("xdotool").args(["keyup", &modifier]),
        "release Ctrl",
    );
    result?;
    release?;
    capture_layout(driver, session, backend, root, "controls-restored")?;
    require_command(
        Command::new("xdotool").args(["click", "1"]),
        "click restored pane control",
    )?;
    driver_success(
        driver,
        &["wait"],
        &[receipt],
        &["10000", "1", "terminal-ready", "pane-2"],
    )?;
    let reports = fs::read_to_string(root.join("pane-1-clicks"))
        .map_err(|error| format!("could not read controlled click receipt: {error}"))?;
    if reports.lines().count() != 1 {
        return Err("restored toolbar click also leaked into terminal input".into());
    }
    println!(
        "pane controls: Ctrl-click reached real PTY; release restored pane split at same pointer position PASS"
    );
    Ok(())
}

fn wait_for_ctrl_click(root: &Path) -> Result<(), String> {
    let deadline = Instant::now() + DEADLINE;
    loop {
        if let Ok(report) = fs::read_to_string(root.join("pane-1-clicks")) {
            return if report.lines().any(|line| line.starts_with("16 ")) {
                Ok(())
            } else {
                Err(format!(
                    "terminal did not receive Ctrl-left mouse report: {report}"
                ))
            };
        }
        if Instant::now() >= deadline {
            return Err("toolbar intercepted Ctrl-click; no PTY mouse report".into());
        }
        thread::sleep(POLL);
    }
}
