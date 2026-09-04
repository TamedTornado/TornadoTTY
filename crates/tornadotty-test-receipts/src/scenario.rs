//! Named real-product scenarios owned by the single journey driver.

mod notification_settings;

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
    if let Err(error) = fs::remove_dir_all(&run_root)
        && result.is_ok()
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
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .ok_or_else(|| "divider-layout requires isolated XDG_CONFIG_HOME".to_owned())?;
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
            "[pane_layout]\n",
            "right_split_behavior = \"alwaysSplit\"\n"
        ),
    )
    .map_err(|error| format!("could not write isolated layout config: {error}"))?;

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
        .arg(product)
        .args(["--command", "sleep 60", "--state-directory"])
        .arg(&state)
        .arg("--no-session-restore")
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("could not start product supervisor: {error}"))?;

    let scenario_result = observe_divider_layout(&driver, &session, &receipt, backend)
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

fn observe_divider_layout(
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
        driver_success(
            driver,
            &["wait"],
            &[receipt],
            &["10000", "1", "window-geometry", "window-1", "1024", "768"],
        )?;
    }
    focus_input_target(driver, session, backend)?;
    send_input_key(driver, session, backend, "ctrl+d")?;
    driver_success(
        driver,
        &["wait"],
        &[receipt],
        &[
            "10000",
            "1",
            "action",
            "split-pane-right",
            "completed",
            "pane-2",
        ],
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
            "column-worklane-1=pane-1;column-pane-2=pane-2",
        ],
    )
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
