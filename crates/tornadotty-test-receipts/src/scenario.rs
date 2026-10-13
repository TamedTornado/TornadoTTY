//! Named real-product scenarios owned by the single journey driver.

mod layout_restore;
mod notification_settings;
mod pane_control_bypass;

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::symlink;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DEADLINE: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(20);

#[derive(Clone, Copy, Debug)]
enum Backend {
    X11,
    Wayland,
}

struct RestoreInputs {
    state: PathBuf,
    session: PathBuf,
    fake_bin: PathBuf,
    project: PathBuf,
    receipt: PathBuf,
    actor_receipt: PathBuf,
}

/// Runs one named real-product scenario.
///
/// # Errors
///
/// Returns a deterministic error if arguments, environment ownership, a real
/// system observation, product behavior, or cleanup violates the scenario.
pub fn run(arguments: &[String]) -> Result<(), String> {
    match arguments {
        [name, product, backend, application_id, desktop_entry] if name == "window-identity" => {
            window_identity(
                Path::new(product),
                parse_backend(backend)?,
                application_id,
                Path::new(desktop_entry),
            )
        }
        [name, product, backend] if name == "divider-layout" => {
            divider_layout(Path::new(product), parse_backend(backend)?)
        }
        [name, product, backend, fixture, actor] if name == "session-restore" => session_restore(
            Path::new(product),
            parse_backend(backend)?,
            Path::new(fixture),
            Path::new(actor),
        ),
        [name, product, backend, daemon] if name == "notification-settings" => {
            notification_settings::run(
                Path::new(product),
                parse_backend(backend)?,
                Path::new(daemon),
            )
        }
        _ => Err(usage().to_owned()),
    }
}

fn session_restore(
    product: &Path,
    backend: Backend,
    fixture: &Path,
    actor: &Path,
) -> Result<(), String> {
    if !product.is_absolute()
        || !product.is_file()
        || !fixture.is_absolute()
        || !fixture.is_file()
        || !actor.is_absolute()
        || !actor.is_file()
    {
        return Err(
            "session-restore product, fixture, and actor must be absolute regular files".to_owned(),
        );
    }
    let session_id = controlled_input_session_id(backend)?;
    let run_root = create_named_run_root("session-restore")?;
    let result = run_session_restore(product, backend, fixture, actor, &session_id, &run_root);
    if let Err(error) = fs::remove_dir_all(&run_root)
        && result.is_ok()
    {
        return Err(format!("could not remove scenario directory: {error}"));
    }
    result?;
    println!(
        "rust-session-restore-driver-{}: PASS persisted-topology controlled-codex typed-readiness physical-input",
        backend_name(backend)
    );
    Ok(())
}

fn run_session_restore(
    product: &Path,
    backend: Backend,
    fixture: &Path,
    actor: &Path,
    session_id: &str,
    root: &Path,
) -> Result<(), String> {
    let inputs = prepare_restore_inputs(root, fixture, actor)?;
    let driver = std::env::current_exe()
        .map_err(|error| format!("could not locate journey driver: {error}"))?;
    let resource = format!("display={}:{}", backend_name(backend), session_id);
    let path = std::env::var_os("PATH").ok_or_else(|| "PATH is absent".to_owned())?;
    let mut joined_path = inputs.fake_bin.as_os_str().to_os_string();
    joined_path.push(":");
    joined_path.push(path);
    let mut supervisor = Command::new(&driver)
        .args(["session", "supervise"])
        .arg(&inputs.session)
        .arg(resource_root()?)
        .args(["--resource", &resource, "--"])
        .arg(product)
        .arg("--state-directory")
        .arg(&inputs.state)
        .env("PATH", joined_path)
        .env("ZENTTY_CONTROLLED_AGENT_PROFILE", "codex-restore")
        .env("ZENTTY_CONTROLLED_AGENT_SESSIONS", "session-codex")
        .env("ZENTTY_CONTROLLED_AGENT_SLEEP_SECONDS", "60")
        .env("ZENTTY_CONTROLLED_AGENT_RECEIPT", &inputs.actor_receipt)
        .env("TORNADOTTY_TEST_RECEIPT_FILE", &inputs.receipt)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("could not start restore supervisor: {error}"))?;

    let scenario_result = observe_session_restore(
        &driver,
        &inputs.session,
        &inputs.receipt,
        &inputs.actor_receipt,
        &inputs.project,
        backend,
    )
    .and_then(|()| send_input_key(&driver, &inputs.session, backend, "ctrl+q"))
    .and_then(|()| {
        driver_success(
            &driver,
            &["session", "wait"],
            &[&inputs.session],
            &["10000", "exited"],
        )
    })
    .and_then(|()| {
        let status = supervisor
            .wait()
            .map_err(|error| format!("could not wait for restore supervisor: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("restore supervisor failed with {status}"))
        }
    })
    .and_then(|()| driver_success(&driver, &["validate"], &[&inputs.receipt], &["--complete"]))
    .and_then(|()| {
        driver_success(
            &driver,
            &["session", "validate-journal"],
            &[&inputs.session],
            &[],
        )
    });
    if scenario_result.is_err() {
        stop_supervisor(&driver, &inputs.session, &mut supervisor);
        report_product_log(&inputs.session.join("product.log"));
    }
    scenario_result
}

fn prepare_restore_inputs(
    root: &Path,
    fixture: &Path,
    actor: &Path,
) -> Result<RestoreInputs, String> {
    let inputs = RestoreInputs {
        state: root.join("state"),
        session: root.join("session"),
        fake_bin: root.join("bin"),
        project: root.join("project"),
        receipt: root.join("state/product-events.ndjson"),
        actor_receipt: root.join("controlled-agent.receipt"),
    };
    for directory in [
        &inputs.state,
        &inputs.session,
        &inputs.fake_bin,
        &inputs.project,
    ] {
        create_owner_directory(directory)?;
    }
    symlink(actor, inputs.fake_bin.join("codex"))
        .map_err(|error| format!("could not install controlled Codex actor: {error}"))?;
    let mut snapshot: serde_json::Value = serde_json::from_slice(
        &fs::read(fixture).map_err(|error| format!("could not read restore fixture: {error}"))?,
    )
    .map_err(|error| format!("restore fixture is malformed: {error}"))?;
    replace_json_string(
        &mut snapshot,
        "/tmp/project",
        &inputs.project.to_string_lossy(),
    );
    fs::write(
        inputs.state.join("restore-snapshot.json"),
        serde_json::to_vec(&snapshot)
            .map_err(|error| format!("could not encode restore fixture: {error}"))?,
    )
    .map_err(|error| format!("could not publish restore fixture: {error}"))?;
    write_restore_config()?;
    Ok(inputs)
}

fn write_restore_config() -> Result<(), String> {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .ok_or_else(|| "session-restore requires isolated XDG_CONFIG_HOME".to_owned())?;
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
            "[restore]\n",
            "restore_workspace_on_launch = true\n"
        ),
    )
    .map_err(|error| format!("could not write isolated restore config: {error}"))
}

fn observe_session_restore(
    driver: &Path,
    session: &Path,
    receipt: &Path,
    actor_receipt: &Path,
    project: &Path,
    backend: Backend,
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
        &["10000", "1", "terminal-ready", "pane-agent"],
    )?;
    driver_success(
        driver,
        &["wait"],
        &[receipt],
        &["10000", "1", "focus-pane", "pane-agent"],
    )?;
    wait_for_log(actor_receipt, "resume:session-codex pid:")?;
    wait_for_log(
        actor_receipt,
        &format!("cwd:session-codex value:{}", project.display()),
    )?;
    focus_input_target(driver, session, backend)
}

fn replace_json_string(value: &mut serde_json::Value, from: &str, to: &str) {
    match value {
        serde_json::Value::String(current) if current == from => to.clone_into(current),
        serde_json::Value::Array(values) => {
            for value in values {
                replace_json_string(value, from, to);
            }
        }
        serde_json::Value::Object(values) => {
            for value in values.values_mut() {
                replace_json_string(value, from, to);
            }
        }
        _ => {}
    }
}

fn divider_layout(product: &Path, backend: Backend) -> Result<(), String> {
    if !product.is_absolute() || !product.is_file() {
        return Err("divider-layout product must be an absolute regular file".to_owned());
    }
    let session_id = controlled_input_session_id(backend)?;
    let run_root = create_named_run_root("divider-layout")?;
    let result = run_divider_layout(product, backend, &session_id, &run_root);
    if result.is_ok()
        && let Err(error) = fs::remove_dir_all(&run_root)
    {
        return Err(format!("could not remove scenario directory: {error}"));
    }
    result?;
    println!(
        "rust-divider-layout-{}: PASS real-ghostty real-gtk physical-input typed-layout",
        backend_name(backend)
    );
    Ok(())
}

fn write_layout_config(config_home: &Path, restore_widths: bool) -> Result<(), String> {
    let config_dir = config_home.join("zentty");
    fs::create_dir_all(&config_dir)
        .map_err(|error| format!("could not create isolated config directory: {error}"))?;
    let mut config = concat!(
        "[confirmations]\n",
        "confirm_before_closing_pane = false\n",
        "confirm_before_closing_window = false\n",
        "confirm_before_quitting = false\n",
        "[pane_layout]\n",
        "right_split_behavior = \"alwaysSplit\"\n",
        "[[shortcuts.bindings]]\ncommand_id = \"pane.arrange.height.full\"\nshortcut = \"command+option+1\"\n",
        "[[shortcuts.bindings]]\ncommand_id = \"pane.arrange.height.two_per_column\"\nshortcut = \"command+option+2\"\n",
        "[[shortcuts.bindings]]\ncommand_id = \"pane.arrange.height.three_per_column\"\nshortcut = \"command+option+3\"\n",
        "[[shortcuts.bindings]]\ncommand_id = \"pane.focus.right\"\nshortcut = \"command+option+right\"\n"
    ).to_owned();
    if restore_widths {
        config.push_str("[sidebar]\nwidth = 262\n[restore]\nrestore_workspace_on_launch = true\n");
    }
    fs::write(config_dir.join("config.toml"), config)
        .map_err(|error| format!("could not write isolated layout config: {error}"))
}

fn run_divider_layout(
    product: &Path,
    backend: Backend,
    session_id: &str,
    root: &Path,
) -> Result<(), String> {
    let state = root.join("state");
    let session = root.join("session");
    create_owner_directory(&state)?;
    create_owner_directory(&session)?;
    let receipt = state.join("product-events.ndjson");
    let actor = root.join("pane-layout-actor");
    let restore_widths = std::env::var("TORNADOTTY_LAYOUT_RESTORE_ONLY").as_deref() == Ok("true");
    if restore_widths {
        layout_restore::prepare(&state)?;
    }
    fs::write(
        &actor,
        include_str!("../../../linux/tests/fixtures/pane-layout-actor"),
    )
    .map_err(|error| format!("could not prepare layout PTY actor: {error}"))?;
    fs::write(
        root.join("visual-evidence"),
        include_str!("../../../linux/tests/lib/visual-evidence"),
    )
    .map_err(|error| format!("could not prepare shared capture helper: {error}"))?;
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .ok_or_else(|| "divider-layout requires isolated XDG_CONFIG_HOME".to_owned())?;
    let config_home = PathBuf::from(config_home);
    write_layout_config(&config_home, restore_widths)?;
    let ghostty_dir = config_home.join("ghostty");
    fs::create_dir_all(&ghostty_dir)
        .map_err(|error| format!("could not create isolated terminal config: {error}"))?;
    // Extra bottom padding can conceal a viewport over-allocation. The last
    // row must fit even when the user chooses a zero-padding terminal.
    fs::write(
        ghostty_dir.join("config"),
        "window-padding-x = 0\nwindow-padding-y = 0\n",
    )
    .map_err(|error| format!("could not write isolated terminal config: {error}"))?;

    let driver = std::env::current_exe()
        .map_err(|error| format!("could not locate journey driver: {error}"))?;
    let resources = resource_root()?;
    let resource = format!("display={}:{}", backend_name(backend), session_id);
    let mut supervisor = Command::new(&driver)
        .args(["session", "supervise"])
        .arg(&session)
        .arg(resources)
        .args(["--resource", &resource, "--", "env"])
        .arg(format!(
            "TORNADOTTY_TEST_RECEIPT_FILE={}",
            receipt.display()
        ))
        .arg(format!("TORNADOTTY_LAYOUT_ROOT={}", root.display()))
        .arg("GTK_OVERLAY_SCROLLING=0")
        // Do not inherit the source checkout: asynchronous Git chrome can
        // legitimately change the available height between measurements.
        .current_dir(root)
        .arg(product)
        .args([
            "--command",
            &format!("bash '{}'", actor.display()),
            "--state-directory",
        ])
        .arg(&state)
        .args(if restore_widths {
            vec![]
        } else {
            vec!["--no-session-restore"]
        })
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("could not start product supervisor: {error}"))?;

    let observation = if std::env::var("TORNADOTTY_LAYOUT_CONTROL_BYPASS").as_deref() == Ok("true")
    {
        pane_control_bypass::observe(&driver, &session, &receipt, backend, root)
    } else if restore_widths {
        layout_restore::observe(&driver, &session, &receipt, backend, root)
    } else {
        observe_divider_layout(&driver, &session, &receipt, backend, root)
    };
    let scenario_result = observation
        .and_then(|()| send_input_key(&driver, &session, backend, "ctrl+q"))
        .and_then(|()| {
            driver_success(
                &driver,
                &["session", "wait"],
                &[&session],
                &["10000", "exited"],
            )
        })
        .and_then(|()| {
            let status = supervisor
                .wait()
                .map_err(|error| format!("could not wait for product supervisor: {error}"))?;
            if status.success() {
                Ok(())
            } else {
                Err(format!("product supervisor failed with {status}"))
            }
        })
        .and_then(|()| driver_success(&driver, &["validate"], &[&receipt], &["--complete"]))
        .and_then(|()| driver_success(&driver, &["session", "validate-journal"], &[&session], &[]));
    if scenario_result.is_err() {
        stop_supervisor(&driver, &session, &mut supervisor);
        report_product_log(&session.join("product.log"));
    }
    scenario_result
}

fn prepare_divider_observation(
    driver: &Path,
    session: &Path,
    receipt: &Path,
    backend: Backend,
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
        if std::env::var("ZENTTY_NESTED_WAYLAND_INPUT_COMPOSITOR").as_deref() == Ok("labwc") {
            send_input_key(driver, session, backend, "F11")?;
        }
        driver_success(
            driver,
            &["wait"],
            &[receipt],
            &["10000", "1", "window-geometry", "window-1", "1024", "768"],
        )?;
    }
    focus_input_target(driver, session, backend)
}

fn observe_divider_layout(
    driver: &Path,
    session: &Path,
    receipt: &Path,
    backend: Backend,
    root: &Path,
) -> Result<(), String> {
    prepare_divider_observation(driver, session, receipt, backend)?;
    let initial = layout_geometry(driver, session, backend, root, "pane-1", "initial")?;
    send_input_key(driver, session, backend, "ctrl+shift+d")?;
    driver_success(
        driver,
        &["wait"],
        &[receipt],
        &[
            "10000",
            "1",
            "action",
            "split-pane-below",
            "completed",
            "pane-2",
        ],
    )?;
    send_input_key(driver, session, backend, "ctrl+d")?;
    driver_success(
        driver,
        &["wait"],
        &[receipt],
        &["10000", "1", "terminal-ready", "pane-3"],
    )?;
    driver_success(
        driver,
        &["wait"],
        &[receipt],
        &[
            "10000",
            "1",
            "pane-layout",
            "window-1",
            "worklane-1",
            "column-worklane-1=pane-1,pane-2;column-pane-3=pane-3",
        ],
    )?;
    let beside = layout_geometry(driver, session, backend, root, "pane-3", "beside")?;
    visible_last_row(
        driver,
        session,
        backend,
        root,
        "beside",
        beside[0] / beside[2],
    )?;
    send_input_key(driver, session, backend, "ctrl+t")?;
    driver_success(
        driver,
        &["wait"],
        &[receipt],
        &["10000", "1", "terminal-ready", "pane-4"],
    )?;
    send_input_key(driver, session, backend, "ctrl+shift+Tab")?;
    let returned = layout_geometry(driver, session, backend, root, "pane-3", "returned")?;
    visible_last_row(
        driver,
        session,
        backend,
        root,
        "returned",
        returned[0] / returned[2],
    )?;
    println!(
        "layout PTY text-area heights: initial={} new-neighbor={} after-switch={}",
        initial[0], beside[0], returned[0]
    );
    if initial[0] != beside[0] || beside[0] != returned[0] {
        return Err(format!(
            "full-height pane geometry changed after split/worklane switch: initial={initial:?}, beside={beside:?}, returned={returned:?}"
        ));
    }
    check_resized_last_rows(
        driver,
        session,
        receipt,
        backend,
        root,
        returned[0] / returned[2],
    )?;
    check_reflow_return(driver, session, backend, root)
}

fn check_reflow_return(
    driver: &Path,
    session: &Path,
    backend: Backend,
    root: &Path,
) -> Result<(), String> {
    // Expand [1,2],[3] into full-width columns, then return to stacked
    // panes. Previously the expansion reused column 3's identity twice,
    // stranding an empty overlay in the viewport after the next reflow.
    send_input_key(driver, session, backend, "ctrl+alt+1")?;
    click_layout_sidebar(driver, session, backend, 170)?;
    send_input_key(driver, session, backend, "ctrl+alt+3")?;
    click_layout_sidebar(driver, session, backend, 290)?;
    capture_layout(driver, session, backend, root, "stacked-settled")?;
    let stacked = layout_geometry(driver, session, backend, root, "pane-3", "stacked")?;
    capture_layout(driver, session, backend, root, "stacked")?;
    click_layout_sidebar(driver, session, backend, 357)?;
    click_layout_sidebar(driver, session, backend, 110)?;
    // Inspect the pixels before sending input or selecting another pane.
    visible_last_row(
        driver,
        session,
        backend,
        root,
        "stacked-return",
        stacked[0] / stacked[2],
    )?;
    let returned = layout_geometry(driver, session, backend, root, "pane-3", "stacked-return")?;
    if returned != stacked {
        return Err(format!(
            "stacked pane geometry changed on return: {stacked:?} -> {returned:?}"
        ));
    }
    send_input_key(driver, session, backend, "ctrl+alt+2")?;
    send_input_key(driver, session, backend, "ctrl+alt+1")?;
    send_input_key(driver, session, backend, "ctrl+1")?;
    for (pane, y, color) in [
        ("pane-1", 170, "#ff0000"),
        ("pane-2", 230, "#00ff00"),
        ("pane-3", 290, "#0000ff"),
    ] {
        click_layout_sidebar(driver, session, backend, y)?;
        let phase = format!("full-width-{pane}");
        capture_layout(driver, session, backend, root, &format!("{phase}-settled"))?;
        let geometry = layout_geometry(driver, session, backend, root, pane, &phase)?;
        visible_colored_row(
            driver,
            session,
            backend,
            root,
            &phase,
            geometry[0] / geometry[2],
            color,
        )?;
    }
    println!(
        "layout reflow: stacked header round-trip pixels and all three full-width sidebar selections PASS"
    );
    check_stationary_divider(driver, session, backend, root)
}

fn check_stationary_divider(
    driver: &Path,
    session: &Path,
    backend: Backend,
    root: &Path,
) -> Result<(), String> {
    send_input_key(driver, session, backend, "ctrl+alt+2")?;
    capture_layout(driver, session, backend, root, "two-pane-reflow-settled")?;
    click_layout_sidebar(driver, session, backend, 170)?;
    capture_layout(driver, session, backend, root, "two-pane-divider")?;
    let before_geometry =
        layout_geometry(driver, session, backend, root, "pane-1", "divider-before")?;
    let (transport, window) = input_target(driver, session, backend)?;
    driver_success(
        driver,
        &["input", "verify"],
        &[session],
        &[transport, &window],
    )?;
    let log = session.join("product.log");
    let pointer = "pane-divider-pointer id=pane-divider-column-worklane-1-after-pane-1 state=enter";
    let resize = "pane-divider-resize axis=vertical column=column-worklane-1 after=pane-1 ";
    let count = |needle: &str| -> Result<usize, String> {
        fs::read_to_string(&log)
            .map(|text| text.matches(needle).count())
            .map_err(|error| error.to_string())
    };
    let before = count(pointer)?;
    let mut found = false;
    for y in (70..590).step_by(3) {
        require_command(
            Command::new("xdotool").args([
                "mousemove",
                "--sync",
                "--window",
                &window,
                "700",
                &y.to_string(),
            ]),
            "locate divider",
        )?;
        thread::sleep(Duration::from_millis(20));
        if count(pointer)? > before {
            found = true;
            break;
        }
    }
    if !found {
        return Err("physical pointer did not find the two-pane divider".to_owned());
    }
    let before = count(resize)?;
    require_command(
        Command::new("xdotool").args(["mousedown", "1"]),
        "press divider",
    )?;
    // Move slowly across multiple allocations, as a person does when nudging
    // a split. One instantaneous motion cannot expose moving-origin feedback.
    for _ in 0..10 {
        require_command(
            Command::new("xdotool").args(["mousemove_relative", "--sync", "--", "0", "2"]),
            "drag divider",
        )?;
        thread::sleep(Duration::from_millis(30));
    }
    thread::sleep(Duration::from_millis(250));
    let settled = count(resize)?;
    thread::sleep(Duration::from_millis(300));
    require_command(
        Command::new("xdotool").args(["mouseup", "1"]),
        "release divider",
    )?;
    if settled <= before || count(resize)? != settled {
        return Err("divider did not move once and settle under the stationary pointer".to_owned());
    }
    click_layout_sidebar(driver, session, backend, 170)?;
    capture_layout(driver, session, backend, root, "divider-after-settled")?;
    let after_geometry =
        layout_geometry(driver, session, backend, root, "pane-1", "divider-after")?;
    if after_geometry[0] <= before_geometry[0] || after_geometry[0] - before_geometry[0] > 40 {
        return Err(format!(
            "small divider drag produced wrong PTY size: {before_geometry:?} -> {after_geometry:?}"
        ));
    }
    println!("layout divider: physical drag settles under stationary pointer PASS");
    Ok(())
}

fn click_layout_sidebar(
    driver: &Path,
    session: &Path,
    backend: Backend,
    y: u32,
) -> Result<(), String> {
    let (transport, window) = match backend {
        Backend::X11 => input_target(driver, session, backend)?,
        Backend::Wayland => (
            "outer-x11",
            std::env::var("ZENTTY_NESTED_WAYLAND_OUTER_X11_WINDOW")
                .map_err(|_| "missing private compositor pointer window".to_owned())?,
        ),
    };
    driver_success(
        driver,
        &["input", "click"],
        &[session],
        &[transport, &window, "120", &y.to_string()],
    )
}

fn check_resized_last_rows(
    driver: &Path,
    session: &Path,
    receipt: &Path,
    backend: Backend,
    root: &Path,
    cell_height: u32,
) -> Result<(), String> {
    // Cover every row-boundary alignment rather than allowing cell rounding
    // to hide a small over-allocation. Resizing is confined to our owned X11
    // product window, or the private compositor output for native Wayland.
    let window = match backend {
        Backend::X11 => input_target(driver, session, backend)?.1,
        Backend::Wayland => std::env::var("ZENTTY_NESTED_WAYLAND_OUTER_X11_WINDOW")
            .map_err(|_| "missing private compositor resize window".to_owned())?,
    };
    if !(1..=128).contains(&cell_height) {
        return Err(format!("invalid terminal cell height: {cell_height}"));
    }
    send_input_key(driver, session, backend, "ctrl+1")?;
    for offset in 0..cell_height {
        let height = (600 + offset).to_string();
        require_command(
            Command::new("xdotool").args(["windowsize", "--sync", &window, "1024", &height]),
            "resize controlled layout",
        )?;
        driver_success(
            driver,
            &["wait"],
            &[receipt],
            &["10000", "1", "window-geometry", "window-1", "1024", &height],
        )?;
        let phase = format!("resize-{offset}");
        let resized = layout_geometry(driver, session, backend, root, "pane-3", &phase)?;
        visible_last_row(
            driver,
            session,
            backend,
            root,
            &phase,
            resized[0] / resized[2],
        )?;
    }
    Ok(())
}

fn visible_last_row(
    driver: &Path,
    session: &Path,
    backend: Backend,
    root: &Path,
    phase: &str,
    cell_height: u32,
) -> Result<(), String> {
    visible_colored_row(
        driver,
        session,
        backend,
        root,
        phase,
        cell_height,
        "#ff00ff",
    )
}

fn visible_colored_row(
    driver: &Path,
    session: &Path,
    backend: Backend,
    root: &Path,
    phase: &str,
    cell_height: u32,
    color: &str,
) -> Result<(), String> {
    let screenshot = capture_layout(driver, session, backend, root, phase)?;
    let output = Command::new("convert")
        .arg(&screenshot)
        .args([
            "-fill", "black", "+opaque", color, "-fill", "white", "-opaque", color, "-format",
            "%@", "info:",
        ])
        .output()
        .map_err(|error| format!("could not inspect last row: {error}"))?;
    if !output.status.success() {
        return Err("last-row pixel inspection failed".to_owned());
    }
    let bounds = String::from_utf8_lossy(&output.stdout);
    let height = bounds
        .split('x')
        .nth(1)
        .and_then(|v| v.split('+').next())
        .and_then(|v| v.parse::<u32>().ok());
    if height != Some(cell_height) {
        return Err(format!(
            "terminal last row clipped in {phase}: visible={bounds}, expected-height={cell_height}"
        ));
    }
    Ok(())
}

fn capture_layout(
    driver: &Path,
    session: &Path,
    backend: Backend,
    root: &Path,
    phase: &str,
) -> Result<PathBuf, String> {
    let window = match backend {
        Backend::X11 => input_target(driver, session, backend)?.1,
        Backend::Wayland => std::env::var("ZENTTY_NESTED_WAYLAND_OUTER_X11_WINDOW")
            .map_err(|_| "missing private compositor capture window".to_owned())?,
    };
    let screenshot = root.join(format!("{phase}.png"));
    require_command(
        Command::new("bash")
            .args([
                "-c",
                "source \"$1\"; zentty_visual_capture_stable \"$2\" timeout --foreground --kill-after=1s 5s import -window \"$3\"",
                "capture",
            ])
            .arg(root.join("visual-evidence"))
            .arg(&screenshot)
            .arg(window),
        "capture last terminal row",
    )?;
    Ok(screenshot)
}

fn layout_geometry(
    driver: &Path,
    session: &Path,
    backend: Backend,
    root: &Path,
    pane: &str,
    phase: &str,
) -> Result<Vec<u32>, String> {
    send_input_text(driver, session, backend, phase)?;
    send_input_key(driver, session, backend, "Return")?;
    let path = root.join(format!("{pane}-{phase}"));
    let deadline = Instant::now() + DEADLINE;
    loop {
        if let Ok(value) = fs::read_to_string(&path) {
            let geometry = value
                .split_whitespace()
                .map(str::parse::<u32>)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("invalid PTY geometry: {error}"))?;
            if geometry.len() != 4 || geometry.contains(&0) {
                return Err(format!("incomplete PTY geometry: {value}"));
            }
            return Ok(geometry);
        }
        if Instant::now() >= deadline {
            return Err(format!("no PTY geometry for {pane}/{phase}"));
        }
        thread::sleep(POLL);
    }
}

fn focus_input_target(driver: &Path, session: &Path, backend: Backend) -> Result<(), String> {
    let (transport, window) = input_target(driver, session, backend)?;
    if transport == "outer-x11" {
        driver_success(
            driver,
            &["input", "click"],
            &[session],
            &[transport, &window, "700", "400"],
        )?;
    }
    Ok(())
}

fn window_identity(
    product: &Path,
    backend: Backend,
    application_id: &str,
    desktop_entry: &Path,
) -> Result<(), String> {
    require_safe_identifier(application_id)?;
    if !product.is_absolute() || !product.is_file() {
        return Err("window-identity product must be an absolute regular file".to_owned());
    }
    let desktop = fs::read_to_string(desktop_entry)
        .map_err(|error| format!("could not read desktop entry: {error}"))?;
    if !desktop
        .lines()
        .any(|line| line == format!("StartupWMClass={application_id}"))
    {
        return Err(format!(
            "packaged StartupWMClass does not match {application_id}"
        ));
    }

    let session_id = controlled_session_id(backend)?;
    let run_root = create_run_root()?;
    let result = run_window_identity(product, backend, application_id, &session_id, &run_root);
    if let Err(error) = fs::remove_dir_all(&run_root)
        && result.is_ok()
    {
        return Err(format!("could not remove scenario directory: {error}"));
    }
    result?;
    println!(
        "desktop-window-identity: PASS backend={} application-id={application_id}",
        backend_name(backend)
    );
    Ok(())
}

fn run_window_identity(
    product: &Path,
    backend: Backend,
    application_id: &str,
    session_id: &str,
    root: &Path,
) -> Result<(), String> {
    let state = root.join("state");
    let session = root.join("session");
    let resources = resource_root()?;
    create_owner_directory(&state)?;
    create_owner_directory(&session)?;
    let receipt = state.join("product-events.ndjson");
    let fifo = state.join("exit.fifo");
    require_command(
        Command::new("mkfifo").args(["-m", "600"]).arg(&fifo),
        "mkfifo",
    )?;

    let driver = std::env::current_exe()
        .map_err(|error| format!("could not locate journey driver: {error}"))?;
    let resource = format!("display={}:{}", backend_name(backend), session_id);
    let shell = format!("IFS= read -r _ < '{}'", fifo.display());
    let mut supervisor = Command::new(&driver)
        .args(["session", "supervise"])
        .arg(&session)
        .arg(resources)
        .args(["--resource", &resource, "--", "env"])
        .arg(format!(
            "WAYLAND_DEBUG={}",
            if matches!(backend, Backend::Wayland) {
                "client"
            } else {
                "0"
            }
        ))
        .arg(format!(
            "TORNADOTTY_TEST_RECEIPT_FILE={}",
            receipt.display()
        ))
        .arg(product)
        .args(["--state-directory"])
        .arg(&state)
        .args(["--command", &shell, "--no-session-restore"])
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("could not start product supervisor: {error}"))?;

    let scenario_result =
        observe_window_identity(&driver, &session, &receipt, backend, application_id)
            .and_then(|()| {
                OpenOptions::new()
                    .write(true)
                    .open(&fifo)
                    .and_then(|mut file| file.write_all(b"exit\n"))
                    .map_err(|error| format!("could not release real PTY child: {error}"))
            })
            .and_then(|()| {
                driver_success(
                    &driver,
                    &["session", "wait"],
                    &[&session],
                    &["10000", "exited"],
                )
            })
            .and_then(|()| {
                let status = supervisor
                    .wait()
                    .map_err(|error| format!("could not wait for product supervisor: {error}"))?;
                if status.success() {
                    Ok(())
                } else {
                    Err(format!("product supervisor failed with {status}"))
                }
            })
            .and_then(|()| driver_success(&driver, &["validate"], &[&receipt], &["--complete"]))
            .and_then(|()| {
                driver_success(&driver, &["session", "validate-journal"], &[&session], &[])
            });

    if scenario_result.is_err() {
        stop_supervisor(&driver, &session, &mut supervisor);
        report_product_log(&session.join("product.log"));
    }
    scenario_result
}

fn observe_window_identity(
    driver: &Path,
    session: &Path,
    receipt: &Path,
    backend: Backend,
    application_id: &str,
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
        &["10000", "1", "process-started"],
    )?;
    driver_success(
        driver,
        &["wait"],
        &[receipt],
        &["10000", "1", "terminal-ready", "pane-1"],
    )?;
    let product_pid = driver_stdout(driver, &["session", "product-pid"], &[session], &[])?;
    match backend {
        Backend::Wayland => wait_for_log(
            &session.join("product.log"),
            &format!("set_app_id(\"{application_id}\")"),
        ),
        Backend::X11 => verify_x11_identity(product_pid.trim(), application_id),
    }
}

fn verify_x11_identity(product_pid: &str, application_id: &str) -> Result<(), String> {
    let window = find_x11_window(product_pid)?;
    let output = Command::new("xprop")
        .args(["-id", &window, "WM_CLASS"])
        .output()
        .map_err(|error| format!("could not inspect X11 WM_CLASS: {error}"))?;
    if !output.status.success() {
        return Err(format!("xprop WM_CLASS failed with {}", output.status));
    }
    let value = String::from_utf8_lossy(&output.stdout);
    if !value.contains(&format!("\"{application_id}\",")) {
        return Err(format!("X11 WM_CLASS instance differs: {}", value.trim()));
    }
    Ok(())
}

fn find_x11_window(product_pid: &str) -> Result<String, String> {
    let deadline = Instant::now() + DEADLINE;
    loop {
        let output = Command::new("xdotool")
            .args(["search", "--onlyvisible", "--pid", product_pid])
            .output()
            .map_err(|error| format!("could not search X11 windows: {error}"))?;
        if output.status.success()
            && let Some(window) = String::from_utf8_lossy(&output.stdout)
                .lines()
                .rfind(|line| line.parse::<u64>().is_ok_and(|value| value > 0))
        {
            return Ok(window.to_owned());
        }
        if Instant::now() >= deadline {
            return Err("X11 product window was not mapped before deadline".to_owned());
        }
        thread::sleep(POLL);
    }
}

fn send_input_key(
    driver: &Path,
    session: &Path,
    backend: Backend,
    chord: &str,
) -> Result<(), String> {
    let (transport, window) = input_target(driver, session, backend)?;
    driver_success(
        driver,
        &["input", "key"],
        &[session],
        &[transport, &window, chord],
    )
}

fn send_input_text(
    driver: &Path,
    session: &Path,
    backend: Backend,
    value: &str,
) -> Result<(), String> {
    let (transport, window) = input_target(driver, session, backend)?;
    driver_success(
        driver,
        &["input", "type"],
        &[session],
        &[transport, &window, value],
    )
}

fn input_target(
    driver: &Path,
    session: &Path,
    backend: Backend,
) -> Result<(&'static str, String), String> {
    match backend {
        Backend::X11 => {
            let product_pid = driver_stdout(driver, &["session", "product-pid"], &[session], &[])?;
            Ok(("x11", find_x11_window(product_pid.trim())?))
        }
        Backend::Wayland
            if std::env::var("ZENTTY_NESTED_WAYLAND_OUTER_X11_INPUT").as_deref() == Ok("1") =>
        {
            let window = std::env::var("ZENTTY_NESTED_WAYLAND_OUTER_X11_WINDOW")
                .map_err(|_| "controlled outer-X11 window is absent".to_owned())?;
            Ok(("outer-x11", window))
        }
        Backend::Wayland => Ok(("wayland", "-".to_owned())),
    }
}

fn wait_for_log(path: &Path, expected: &str) -> Result<(), String> {
    let deadline = Instant::now() + DEADLINE;
    loop {
        let mut value = String::new();
        if let Ok(file) = File::open(path) {
            file.take(1024 * 1024)
                .read_to_string(&mut value)
                .map_err(|error| format!("could not read product log: {error}"))?;
        }
        if value.contains(expected) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!("product log did not publish {expected:?}"));
        }
        thread::sleep(POLL);
    }
}

fn driver_success(
    driver: &Path,
    prefix: &[&str],
    paths: &[&Path],
    suffix: &[&str],
) -> Result<(), String> {
    driver_output(driver, prefix, paths, suffix).map(|_| ())
}

fn driver_stdout(
    driver: &Path,
    prefix: &[&str],
    paths: &[&Path],
    suffix: &[&str],
) -> Result<String, String> {
    let output = driver_output(driver, prefix, paths, suffix)?;
    String::from_utf8(output.stdout).map_err(|_| "journey driver output was not UTF-8".to_owned())
}

fn driver_output(
    driver: &Path,
    prefix: &[&str],
    paths: &[&Path],
    suffix: &[&str],
) -> Result<std::process::Output, String> {
    let output = Command::new(driver)
        .args(prefix)
        .args(paths)
        .args(suffix)
        .output()
        .map_err(|error| format!("could not invoke journey driver: {error}"))?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(format!(
            "journey driver phase failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn stop_supervisor(driver: &Path, session: &Path, supervisor: &mut Child) {
    let _ = Command::new(driver)
        .args(["session", "stop"])
        .arg(session)
        .arg("500")
        .status();
    let _ = supervisor.wait();
}

fn report_product_log(path: &Path) {
    if let Ok(value) = fs::read_to_string(path) {
        eprintln!("--- product.log ---\n{value}");
    }
}

fn parse_backend(value: &str) -> Result<Backend, String> {
    match value {
        "x11" => Ok(Backend::X11),
        "wayland" => Ok(Backend::Wayland),
        _ => Err("window-identity backend must be x11 or wayland".to_owned()),
    }
}

const fn backend_name(backend: Backend) -> &'static str {
    match backend {
        Backend::X11 => "x11",
        Backend::Wayland => "wayland",
    }
}

fn controlled_session_id(backend: Backend) -> Result<String, String> {
    let name = match backend {
        Backend::X11 => "ZENTTY_NESTED_X11_SESSION_ID",
        Backend::Wayland => "ZENTTY_NESTED_WAYLAND_SESSION_ID",
    };
    let value = std::env::var(name).map_err(|_| {
        format!(
            "{} scenario must run through its controlled wrapper",
            backend_name(backend)
        )
    })?;
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("{name} is malformed"));
    }
    Ok(value)
}

fn controlled_input_session_id(backend: Backend) -> Result<String, String> {
    let name = match backend {
        Backend::X11 => "ZENTTY_NESTED_X11_SESSION_ID",
        Backend::Wayland => "ZENTTY_NESTED_WAYLAND_INPUT_SESSION_ID",
    };
    let value = std::env::var(name).map_err(|_| {
        format!(
            "{} input scenario must run through its controlled wrapper",
            backend_name(backend)
        )
    })?;
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("{name} is malformed"));
    }
    Ok(value)
}

fn create_run_root() -> Result<PathBuf, String> {
    create_named_run_root("window-identity")
}

fn create_named_run_root(name: &str) -> Result<PathBuf, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock precedes Unix epoch: {error}"))?
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "tornadotty-{name}-{}-{timestamp}",
        std::process::id()
    ));
    create_owner_directory(&path)?;
    Ok(path)
}

fn resource_root() -> Result<PathBuf, String> {
    let path = std::env::var_os("TORNADOTTY_JOURNEY_RESOURCE_ROOT").map_or_else(
        || {
            std::env::var_os("XDG_RUNTIME_DIR").map_or_else(
                || {
                    std::env::temp_dir().join(format!(
                        "tornadotty-journey-resources-{}",
                        rustix::process::getuid().as_raw()
                    ))
                },
                |runtime| PathBuf::from(runtime).join("tornadotty-journey-resources"),
            )
        },
        PathBuf::from,
    );
    match fs::symlink_metadata(&path) {
        Ok(_) => require_owner_directory(&path)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            create_owner_directory(&path)?;
        }
        Err(error) => {
            return Err(format!(
                "could not inspect journey resource root {}: {error}",
                path.display()
            ));
        }
    }
    Ok(path)
}

fn create_owner_directory(path: &Path) -> Result<(), String> {
    fs::create_dir(path)
        .map_err(|error| format!("could not create {}: {error}", path.display()))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("could not secure {}: {error}", path.display()))?;
    require_owner_directory(path)
}

fn require_owner_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != rustix::process::getuid().as_raw()
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(format!("{} is not an owner-only directory", path.display()));
    }
    Ok(())
}

fn require_command(command: &mut Command, label: &str) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|error| format!("could not execute {label}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{label} failed with {status}"))
    }
}

fn require_safe_identifier(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err("application ID must be bounded safe ASCII".to_owned());
    }
    Ok(())
}

fn usage() -> &'static str {
    "scenario usage:\n  tornadotty-journey-driver scenario window-identity PRODUCT x11|wayland APPLICATION_ID DESKTOP_ENTRY\n  tornadotty-journey-driver scenario divider-layout PRODUCT x11|wayland\n  tornadotty-journey-driver scenario session-restore PRODUCT x11|wayland FIXTURE ACTOR\n  tornadotty-journey-driver scenario notification-settings PRODUCT x11|wayland NOTIFICATION_DAEMON"
}
