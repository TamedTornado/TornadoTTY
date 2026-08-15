#![forbid(unsafe_code)]

use gtk::gio;
use gtk::gio::prelude::*;
use serde_json::json;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use zentty_linux::platform::{
    ChildStdio, ProcessLaunch, UserDirectory, open_file, open_uri, resolve_user_directory,
    resolve_user_path, runtime_directory, spawn_process,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("platform-contract: error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = std::env::args_os().collect::<Vec<_>>();
    match arguments.get(1).and_then(|argument| argument.to_str()) {
        Some("xdg") => xdg_contract(),
        Some("process") => process_contract(),
        Some("open") => open_contract(),
        Some("--process-child") => process_child(&arguments),
        Some("--sleep-child") => {
            sleep_child();
            Ok(())
        }
        Some("--open-child") => open_child(&arguments),
        _ => Err("usage: platform_contract xdg|process|open".to_owned()),
    }
}

fn xdg_contract() -> Result<(), String> {
    let home = required_absolute_environment("HOME")?;
    let cases = [
        (
            UserDirectory::Config,
            "XDG_CONFIG_HOME",
            "zentty/config.toml",
        ),
        (UserDirectory::Data, "XDG_DATA_HOME", "zentty/sounds"),
        (
            UserDirectory::Cache,
            "XDG_CACHE_HOME",
            "zentty/cache-receipt",
        ),
        (UserDirectory::State, "XDG_STATE_HOME", "zentty/session"),
    ];
    for (directory, variable, relative) in cases {
        let expected = required_absolute_environment(variable)?;
        let resolved = resolve_user_directory(
            directory,
            std::env::var_os(variable).as_deref(),
            Some(home.as_os_str()),
        )?;
        if resolved != expected {
            return Err(format!(
                "{variable} resolved to {} instead of {}",
                resolved.display(),
                expected.display()
            ));
        }
        let owned = resolve_user_path(
            directory,
            std::env::var_os(variable).as_deref(),
            Some(home.as_os_str()),
            Path::new(relative),
        )?;
        if !owned.starts_with(&expected) {
            return Err(format!("{variable} owned path escaped its root"));
        }
    }

    let runtime = runtime_directory(std::env::var_os("XDG_RUNTIME_DIR").as_deref())?
        .ok_or_else(|| "XDG_RUNTIME_DIR is absent".to_owned())?;
    if runtime != required_absolute_environment("XDG_RUNTIME_DIR")? {
        return Err("runtime directory did not preserve the isolated root".to_owned());
    }
    if resolve_user_path(
        UserDirectory::Config,
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        Some(home.as_os_str()),
        Path::new("../escape"),
    )
    .is_ok()
    {
        return Err("XDG traversal was accepted".to_owned());
    }
    println!("platform-contract-xdg: PASS private-roots=true traversal-rejected=true");
    Ok(())
}

fn process_contract() -> Result<(), String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("could not resolve test actor: {error}"))?;
    let root = private_root("process")?;
    let working_directory = root.join("working directory");
    fs::create_dir(&working_directory)
        .map_err(|error| format!("could not create process CWD: {error}"))?;
    let receipt = root.join("process receipt.json");
    let hostile_arguments = [
        "literal space",
        "quote'\"value",
        "line\nbreak",
        "$(not-a-shell)",
    ];
    let mut arguments = vec![
        OsString::from("--process-child"),
        receipt.as_os_str().to_owned(),
    ];
    arguments.extend(hostile_arguments.iter().map(OsString::from));
    let mut specification = ProcessLaunch::detached(&executable, arguments);
    specification.current_directory = Some(working_directory.clone());
    specification.environment = vec![
        (
            OsString::from("ZENTTY_PLATFORM_SET"),
            Some(OsString::from("exact=value\nnext")),
        ),
        (OsString::from("ZENTTY_PLATFORM_REMOVED"), None),
    ];
    specification.stdout = ChildStdio::Inherit;
    specification.stderr = ChildStdio::Inherit;
    let mut child = spawn_process(&specification)?;
    let status = child
        .wait()
        .map_err(|error| format!("could not wait for process actor: {error}"))?;
    if !status.success() {
        return Err(format!("process actor exited with {status}"));
    }
    let value: serde_json::Value = serde_json::from_slice(
        &fs::read(&receipt).map_err(|error| format!("could not read process receipt: {error}"))?,
    )
    .map_err(|error| format!("invalid process receipt: {error}"))?;
    if value["arguments"] != json!(hostile_arguments)
        || value["current_directory"] != json!(working_directory)
        || value["set_environment"] != json!("exact=value\nnext")
        || value["removed_environment"] != json!(true)
    {
        return Err(format!("process boundaries changed: {value}"));
    }

    let missing = ProcessLaunch::detached(root.join("missing-program"), Vec::new());
    if spawn_process(&missing).is_ok() {
        return Err("missing executable launch unexpectedly succeeded".to_owned());
    }
    let mut relative_cwd = ProcessLaunch::detached(&executable, Vec::new());
    relative_cwd.current_directory = Some(PathBuf::from("relative"));
    if spawn_process(&relative_cwd).is_ok() {
        return Err("relative process CWD unexpectedly succeeded".to_owned());
    }

    let mut sleeper = spawn_process(&ProcessLaunch::detached(
        &executable,
        vec![OsString::from("--sleep-child")],
    ))?;
    sleeper
        .kill()
        .map_err(|error| format!("could not signal process actor: {error}"))?;
    let killed = sleeper
        .wait()
        .map_err(|error| format!("could not reap signalled process actor: {error}"))?;
    if killed.success() {
        return Err("signalled process actor reported success".to_owned());
    }
    fs::remove_dir_all(&root).map_err(|error| format!("could not remove process root: {error}"))?;
    println!(
        "platform-contract-process: PASS argv=true environment=true cwd=true failure=true signal=true teardown=true"
    );
    Ok(())
}

fn process_child(arguments: &[OsString]) -> Result<(), String> {
    let receipt = arguments
        .get(2)
        .map(PathBuf::from)
        .ok_or_else(|| "process child receipt is missing".to_owned())?;
    let value = json!({
        "arguments": arguments.iter().skip(3).map(|value| value.to_string_lossy()).collect::<Vec<_>>(),
        "current_directory": std::env::current_dir().map_err(|error| error.to_string())?,
        "set_environment": std::env::var("ZENTTY_PLATFORM_SET").map_err(|error| error.to_string())?,
        "removed_environment": std::env::var_os("ZENTTY_PLATFORM_REMOVED").is_none(),
    });
    fs::write(
        receipt,
        serde_json::to_vec(&value).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("could not write process receipt: {error}"))
}

fn sleep_child() {
    thread::sleep(Duration::from_mins(1));
}

fn open_contract() -> Result<(), String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("could not resolve open actor: {error}"))?;
    let root = private_root("open")?;
    let receipt = root.join("open receipt with spaces.jsonl");
    let commandline = format!(
        "{} --open-child {} %u",
        gtk::glib::shell_quote(&executable).to_string_lossy(),
        gtk::glib::shell_quote(&receipt).to_string_lossy()
    );
    let application = gio::AppInfo::create_from_commandline(
        &commandline,
        Some("Zentty platform contract actor"),
        gio::AppInfoCreateFlags::SUPPORTS_URIS,
    )
    .map_err(|error| format!("could not create desktop actor: {error}"))?;
    application
        .set_as_default_for_type("x-scheme-handler/zentty-contract")
        .map_err(|error| format!("could not register URI actor: {error}"))?;
    application
        .set_as_default_for_type("text/plain")
        .map_err(|error| format!("could not register file actor: {error}"))?;

    let hostile_uri = "zentty-contract://host/path?literal=%24%28safe%29&unicode=caf%C3%A9";
    open_uri(hostile_uri)?;
    wait_for_receipts(&receipt, 1)?;
    let file = root.join("file with spaces.txt");
    fs::write(&file, "platform contract\n")
        .map_err(|error| format!("could not write open target: {error}"))?;
    open_file(&file)?;
    wait_for_receipts(&receipt, 2)?;

    let lines = fs::read_to_string(&receipt)
        .map_err(|error| format!("could not read open receipt: {error}"))?;
    if !lines.lines().any(|line| line == hostile_uri) {
        return Err("desktop actor did not receive the exact hostile URI".to_owned());
    }
    if !lines.lines().any(|line| line == file.to_string_lossy()) {
        return Err(format!(
            "desktop actor did not receive exact canonical file path {}; receipts={lines:?}",
            file.display()
        ));
    }
    if open_uri("no-scheme").is_ok() || open_file(Path::new("relative.txt")).is_ok() {
        return Err("ambiguous open target was accepted".to_owned());
    }
    fs::remove_dir_all(&root).map_err(|error| format!("could not remove open root: {error}"))?;
    println!(
        "platform-contract-open: PASS real-gio-default=true uri=true file=true hostile=true failure=true"
    );
    Ok(())
}

fn open_child(arguments: &[OsString]) -> Result<(), String> {
    let receipt = arguments
        .get(2)
        .map(PathBuf::from)
        .ok_or_else(|| "open child receipt is missing".to_owned())?;
    let target = arguments
        .get(3)
        .ok_or_else(|| "open child target is missing".to_owned())?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(receipt)
        .map_err(|error| format!("could not open receipt: {error}"))?;
    writeln!(file, "{}", target.to_string_lossy())
        .map_err(|error| format!("could not append receipt: {error}"))
}

fn wait_for_receipts(path: &Path, count: usize) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let observed = fs::read_to_string(path).unwrap_or_default().lines().count();
        if observed >= count {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(20));
    }
    Err(format!("desktop actor did not produce receipt {count}"))
}

fn private_root(label: &str) -> Result<PathBuf, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before the Unix epoch: {error}"))?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "zentty-platform-{label}-{}-{}",
        std::process::id(),
        timestamp
    ));
    fs::create_dir(&root).map_err(|error| format!("could not create private root: {error}"))?;
    Ok(root)
}

fn required_absolute_environment(name: &str) -> Result<PathBuf, String> {
    let path = std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| format!("{name} is unset"))?;
    if !path.is_absolute() {
        return Err(format!("{name} is not absolute"));
    }
    Ok(path)
}
