//! Named real-product scenarios owned by the single journey driver.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
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
        _ => Err(usage().to_owned()),
    }
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
    let deadline = Instant::now() + DEADLINE;
    let window = loop {
        let output = Command::new("xdotool")
            .args(["search", "--onlyvisible", "--pid", product_pid])
            .output()
            .map_err(|error| format!("could not search X11 windows: {error}"))?;
        if output.status.success()
            && let Some(window) = String::from_utf8_lossy(&output.stdout)
                .lines()
                .rfind(|line| line.parse::<u64>().is_ok_and(|value| value > 0))
        {
            break window.to_owned();
        }
        if Instant::now() >= deadline {
            return Err("X11 product window was not mapped before deadline".to_owned());
        }
        thread::sleep(POLL);
    };
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

fn create_run_root() -> Result<PathBuf, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock precedes Unix epoch: {error}"))?
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "tornadotty-window-identity-{}-{timestamp}",
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
    "scenario usage:\n  tornadotty-journey-driver scenario window-identity PRODUCT x11|wayland APPLICATION_ID DESKTOP_ENTRY"
}
