//! Real Notifications settings workflow for the journey driver.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::{
    Backend, backend_name, controlled_input_session_id, create_named_run_root,
    create_owner_directory, driver_success, focus_input_target, report_product_log, resource_root,
    send_input_key, send_input_text, stop_supervisor,
};

const DEADLINE: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(20);

struct NotificationInputs {
    state: PathBuf,
    product_session: PathBuf,
    daemon_session: PathBuf,
    receipt: PathBuf,
}

pub(super) fn run(product: &Path, backend: Backend, daemon: &Path) -> Result<(), String> {
    if !product.is_absolute() || !product.is_file() || !daemon.is_absolute() || !daemon.is_file() {
        return Err(
            "notification-settings product and daemon must be absolute regular files".to_owned(),
        );
    }
    let session_id = controlled_input_session_id(backend)?;
    let run_root = create_named_run_root("notification-settings")?;
    let result = run_scenario(product, backend, daemon, &session_id, &run_root);
    if let Err(error) = fs::remove_dir_all(&run_root)
        && result.is_ok()
    {
        return Err(format!("could not remove scenario directory: {error}"));
    }
    result?;
    println!(
        "rust-notifications-settings-driver-{}: PASS real-gtk-settings physical-input private-dbus real-freedesktop-notify typed-evidence",
        backend_name(backend)
    );
    Ok(())
}

fn run_scenario(
    product: &Path,
    backend: Backend,
    daemon: &Path,
    session_id: &str,
    root: &Path,
) -> Result<(), String> {
    let inputs = prepare_inputs(root)?;
    let driver = std::env::current_exe()
        .map_err(|error| format!("could not locate journey driver: {error}"))?;
    let resources = resource_root()?;
    let mut daemon_supervisor = spawn_daemon(
        &driver,
        daemon,
        &inputs.daemon_session,
        &resources,
        session_id,
    )?;
    let mut product_supervisor = None;

    let scenario_result = wait_for_notification_service().and_then(|()| {
        let supervisor = spawn_product(&driver, product, &inputs, &resources, backend, session_id)?;
        product_supervisor = Some(supervisor);
        observe_product(&driver, &inputs, backend)
    });
    if scenario_result.is_err() {
        if let Some(supervisor) = &mut product_supervisor {
            stop_supervisor(&driver, &inputs.product_session, supervisor);
            report_product_log(&inputs.product_session.join("product.log"));
        }
        stop_supervisor(&driver, &inputs.daemon_session, &mut daemon_supervisor);
        report_product_log(&inputs.daemon_session.join("product.log"));
        return scenario_result;
    }

    let mut product_supervisor = product_supervisor
        .ok_or_else(|| "notification product supervisor was not started".to_owned())?;
    let finish_result =
        finish_clean_session(&driver, &inputs.product_session, &mut product_supervisor)
            .and_then(|()| {
                finish_stopped_session(&driver, &inputs.daemon_session, &mut daemon_supervisor)
            })
            .and_then(|()| {
                driver_success(&driver, &["validate"], &[&inputs.receipt], &["--complete"])
            });
    if finish_result.is_err() {
        stop_supervisor(&driver, &inputs.product_session, &mut product_supervisor);
        stop_supervisor(&driver, &inputs.daemon_session, &mut daemon_supervisor);
        report_product_log(&inputs.product_session.join("product.log"));
        report_product_log(&inputs.daemon_session.join("product.log"));
    }
    finish_result
}

fn prepare_inputs(root: &Path) -> Result<NotificationInputs, String> {
    let inputs = NotificationInputs {
        state: root.join("state"),
        product_session: root.join("product-session"),
        daemon_session: root.join("daemon-session"),
        receipt: root.join("state/product-events.ndjson"),
    };
    for directory in [
        &inputs.state,
        &inputs.product_session,
        &inputs.daemon_session,
    ] {
        create_owner_directory(directory)?;
    }
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .ok_or_else(|| "notification-settings requires isolated XDG_CONFIG_HOME".to_owned())?;
    let config_dir = PathBuf::from(config_home).join("zentty");
    fs::create_dir_all(&config_dir)
        .map_err(|error| format!("could not create isolated config directory: {error}"))?;
    fs::write(
        config_dir.join("config.toml"),
        concat!(
            "[confirmations]\n",
            "confirm_before_closing_pane = false\n",
            "confirm_before_closing_window = false\n",
            "confirm_before_quitting = false\n",
            "[notifications]\n",
            "sound_name = \"\"\n"
        ),
    )
    .map_err(|error| format!("could not write isolated notification config: {error}"))?;
    Ok(inputs)
}

fn spawn_daemon(
    driver: &Path,
    daemon: &Path,
    session: &Path,
    resources: &Path,
    session_id: &str,
) -> Result<Child, String> {
    let resource = format!("notification-bus={session_id}");
    let mut child = Command::new(driver)
        .args(["session", "supervise"])
        .arg(session)
        .arg(resources)
        .args(["--resource", &resource, "--"])
        .arg(daemon)
        .env("GDK_BACKEND", "x11")
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("could not start notification daemon supervisor: {error}"))?;
    if let Err(error) = driver_success(
        driver,
        &["session", "wait"],
        &[session],
        &["10000", "running"],
    ) {
        stop_supervisor(driver, session, &mut child);
        report_product_log(&session.join("product.log"));
        return Err(error);
    }
    Ok(child)
}

fn spawn_product(
    driver: &Path,
    product: &Path,
    inputs: &NotificationInputs,
    resources: &Path,
    backend: Backend,
    session_id: &str,
) -> Result<Child, String> {
    let resource = format!("display={}:{}", backend_name(backend), session_id);
    let mut child = Command::new(driver)
        .args(["session", "supervise"])
        .arg(&inputs.product_session)
        .arg(resources)
        .args(["--resource", &resource, "--"])
        .arg(product)
        .args(["--command", "sleep 60", "--state-directory"])
        .arg(&inputs.state)
        .arg("--no-session-restore")
        .env("TORNADOTTY_TEST_RECEIPT_FILE", &inputs.receipt)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("could not start notification product supervisor: {error}"))?;
    if let Err(error) = driver_success(
        driver,
        &["session", "wait"],
        &[&inputs.product_session],
        &["10000", "running"],
    ) {
        stop_supervisor(driver, &inputs.product_session, &mut child);
        report_product_log(&inputs.product_session.join("product.log"));
        return Err(error);
    }
    Ok(child)
}

fn observe_product(
    driver: &Path,
    inputs: &NotificationInputs,
    backend: Backend,
) -> Result<(), String> {
    driver_success(
        driver,
        &["wait"],
        &[&inputs.receipt],
        &["10000", "1", "terminal-ready", "pane-1"],
    )?;
    if matches!(backend, Backend::Wayland) {
        driver_success(
            driver,
            &["wait"],
            &[&inputs.receipt],
            &["10000", "1", "window-geometry", "window-1", "1024", "768"],
        )?;
    }
    focus_input_target(driver, &inputs.product_session, backend)?;
    send_input_key(driver, &inputs.product_session, backend, "ctrl+shift+p")?;
    send_input_text(
        driver,
        &inputs.product_session,
        backend,
        "Notifications Settings",
    )?;
    driver_success(
        driver,
        &["wait"],
        &[&inputs.receipt],
        &[
            "10000",
            "1",
            "action",
            "resolve-command-palette",
            "completed",
            "notifications",
        ],
    )?;
    send_input_key(driver, &inputs.product_session, backend, "Return")?;
    driver_success(
        driver,
        &["wait"],
        &[&inputs.receipt],
        &[
            "10000",
            "1",
            "action",
            "select-notifications-settings",
            "completed",
            "-",
        ],
    )?;
    driver_success(
        driver,
        &["wait"],
        &[&inputs.receipt],
        &["10000", "1", "focus-widget", "settings-window"],
    )?;
    send_input_key(driver, &inputs.product_session, backend, "alt+t")?;
    driver_success(
        driver,
        &["wait"],
        &[&inputs.receipt],
        &[
            "10000",
            "1",
            "action",
            "send-test-notification",
            "completed",
            "-",
        ],
    )?;
    send_input_key(driver, &inputs.product_session, backend, "Escape")?;
    driver_success(
        driver,
        &["wait"],
        &[&inputs.receipt],
        &[
            "10000",
            "1",
            "action",
            "close-settings-window",
            "completed",
            "-",
        ],
    )?;
    send_input_key(driver, &inputs.product_session, backend, "ctrl+q")?;
    Ok(())
}

fn wait_for_notification_service() -> Result<(), String> {
    let deadline = Instant::now() + DEADLINE;
    loop {
        let output = Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest",
                "org.freedesktop.DBus",
                "--object-path",
                "/org/freedesktop/DBus",
                "--method",
                "org.freedesktop.DBus.NameHasOwner",
                "org.freedesktop.Notifications",
            ])
            .output()
            .map_err(|error| format!("could not query notification service: {error}"))?;
        if output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "(true,)" {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(
                "notification daemon did not own its D-Bus name before deadline".to_owned(),
            );
        }
        thread::sleep(POLL);
    }
}

fn finish_clean_session(driver: &Path, session: &Path, child: &mut Child) -> Result<(), String> {
    driver_success(
        driver,
        &["session", "wait"],
        &[session],
        &["10000", "exited"],
    )?;
    let status = child
        .wait()
        .map_err(|error| format!("could not wait for product supervisor: {error}"))?;
    if !status.success() {
        return Err(format!("product supervisor failed with {status}"));
    }
    driver_success(driver, &["session", "validate-journal"], &[session], &[])
}

fn finish_stopped_session(driver: &Path, session: &Path, child: &mut Child) -> Result<(), String> {
    driver_success(driver, &["session", "stop"], &[session], &["1000"])?;
    let status = child
        .wait()
        .map_err(|error| format!("could not wait for daemon supervisor: {error}"))?;
    if !status.success() {
        return Err(format!("daemon supervisor failed with {status}"));
    }
    driver_success(driver, &["session", "validate-journal"], &[session], &[])
}
