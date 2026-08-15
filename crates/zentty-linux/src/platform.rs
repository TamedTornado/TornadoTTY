//! Linux desktop contracts shared by the product and real-system test actors.
//!
//! This module owns platform mechanics only. Product policy (which target to
//! open, which process belongs to a pane, and what to show in settings) stays
//! with the product authority that requested the operation.

use gtk::{gio, gio::prelude::*};
use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UserDirectory {
    Config,
    Data,
    Cache,
    State,
}

impl UserDirectory {
    fn environment_name(self) -> &'static str {
        match self {
            Self::Config => "XDG_CONFIG_HOME",
            Self::Data => "XDG_DATA_HOME",
            Self::Cache => "XDG_CACHE_HOME",
            Self::State => "XDG_STATE_HOME",
        }
    }

    fn home_relative_default(self) -> &'static str {
        match self {
            Self::Config => ".config",
            Self::Data => ".local/share",
            Self::Cache => ".cache",
            Self::State => ".local/state",
        }
    }
}

/// Resolve one XDG user directory according to the base-directory contract.
///
/// Empty overrides are treated as unset. Set overrides and `HOME` must be
/// absolute; silently interpreting a relative root against the application
/// working directory would let configuration escape its intended owner.
///
/// # Errors
///
/// Returns an error when the selected override or fallback home is missing or
/// is not absolute.
pub fn resolve_user_directory(
    directory: UserDirectory,
    override_value: Option<&OsStr>,
    home: Option<&OsStr>,
) -> Result<PathBuf, String> {
    if let Some(value) = override_value.filter(|value| !value.is_empty()) {
        let path = PathBuf::from(value);
        if !path.is_absolute() {
            return Err(format!(
                "{} must be an absolute path",
                directory.environment_name()
            ));
        }
        return Ok(path);
    }

    let home = home
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| format!("{} and HOME are unset", directory.environment_name()))?;
    if !home.is_absolute() {
        return Err("HOME must be an absolute path".to_owned());
    }
    Ok(home.join(directory.home_relative_default()))
}

/// Resolve a validated relative path beneath one XDG user directory.
///
/// # Errors
///
/// Returns an error for invalid roots, empty paths, absolute paths, or path
/// traversal.
pub fn resolve_user_path(
    directory: UserDirectory,
    override_value: Option<&OsStr>,
    home: Option<&OsStr>,
    relative: &Path,
) -> Result<PathBuf, String> {
    validate_relative_path(relative)?;
    resolve_user_directory(directory, override_value, home).map(|root| root.join(relative))
}

/// Validate the optional XDG runtime directory without inventing a fallback.
///
/// # Errors
///
/// Returns an error when a non-empty runtime directory is not absolute.
pub fn runtime_directory(value: Option<&OsStr>) -> Result<Option<PathBuf>, String> {
    let Some(value) = value.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err("XDG_RUNTIME_DIR must be an absolute path".to_owned());
    }
    Ok(Some(path))
}

fn validate_relative_path(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(
            "XDG-owned path must be a non-empty relative path without traversal".to_owned(),
        );
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildStdio {
    Inherit,
    Null,
    Piped,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessLaunch {
    pub program: PathBuf,
    pub arguments: Vec<OsString>,
    pub current_directory: Option<PathBuf>,
    pub environment: Vec<(OsString, Option<OsString>)>,
    pub stdin: ChildStdio,
    pub stdout: ChildStdio,
    pub stderr: ChildStdio,
}

impl ProcessLaunch {
    #[must_use]
    pub fn detached(program: impl Into<PathBuf>, arguments: Vec<OsString>) -> Self {
        Self {
            program: program.into(),
            arguments,
            current_directory: None,
            environment: Vec::new(),
            stdin: ChildStdio::Null,
            stdout: ChildStdio::Null,
            stderr: ChildStdio::Null,
        }
    }
}

/// Spawn without a shell, preserving every OS-string argument boundary.
///
/// # Errors
///
/// Returns an error for an empty program, relative working directory, invalid
/// environment name, or operating-system spawn failure.
pub fn spawn_process(specification: &ProcessLaunch) -> Result<Child, String> {
    if specification.program.as_os_str().is_empty() {
        return Err("process program must not be empty".to_owned());
    }
    if specification
        .current_directory
        .as_ref()
        .is_some_and(|directory| !directory.is_absolute())
    {
        return Err("process working directory must be absolute".to_owned());
    }
    if specification
        .environment
        .iter()
        .any(|(name, _)| name.is_empty() || name.as_encoded_bytes().contains(&b'='))
    {
        return Err("process environment contains an invalid name".to_owned());
    }

    let mut command = Command::new(&specification.program);
    command.args(&specification.arguments);
    if let Some(directory) = specification.current_directory.as_ref() {
        command.current_dir(directory);
    }
    for (name, value) in &specification.environment {
        match value {
            Some(value) => {
                command.env(name, value);
            }
            None => {
                command.env_remove(name);
            }
        }
    }
    command
        .stdin(stdio(specification.stdin))
        .stdout(stdio(specification.stdout))
        .stderr(stdio(specification.stderr))
        .spawn()
        .map_err(|error| {
            format!(
                "could not launch {}: {error}",
                specification.program.display()
            )
        })
}

/// Spawn and asynchronously reap a detached desktop helper.
///
/// # Errors
///
/// Returns every validation or operating-system error from [`spawn_process`].
pub fn spawn_detached(specification: &ProcessLaunch) -> Result<u32, String> {
    let mut child = spawn_process(specification)?;
    let process_id = child.id();
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(process_id)
}

fn stdio(mode: ChildStdio) -> Stdio {
    match mode {
        ChildStdio::Inherit => Stdio::inherit(),
        ChildStdio::Null => Stdio::null(),
        ChildStdio::Piped => Stdio::piped(),
    }
}

/// Open one absolute URI through the current desktop default application.
///
/// # Errors
///
/// Returns an error when the URI has no scheme or desktop dispatch fails.
pub fn open_uri(uri: &str) -> Result<(), String> {
    let parsed = glib_uri(uri)?;
    gio::AppInfo::launch_default_for_uri(parsed.as_str(), None::<&gio::AppLaunchContext>)
        .map_err(|error| format!("could not open URI: {error}"))
}

/// Open one absolute file path through the current desktop default application.
///
/// # Errors
///
/// Returns an error when the path is relative or desktop dispatch fails.
pub fn open_file(path: &Path) -> Result<(), String> {
    let uri = file_uri(path)?;
    open_uri(uri.as_str())
}

fn file_uri(path: &Path) -> Result<gtk::glib::GString, String> {
    if !path.is_absolute() {
        return Err("file to open must be an absolute path".to_owned());
    }
    Ok(gio::File::for_path(path).uri())
}

fn glib_uri(uri: &str) -> Result<gtk::glib::GString, String> {
    let scheme =
        gtk::glib::Uri::parse_scheme(uri).ok_or_else(|| "URI must include a scheme".to_owned())?;
    if scheme.is_empty() {
        return Err("URI must include a scheme".to_owned());
    }
    Ok(uri.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdg_resolution_prefers_absolute_override_and_uses_spec_fallbacks() {
        assert_eq!(
            resolve_user_directory(
                UserDirectory::Config,
                Some(OsStr::new("/private/config")),
                Some(OsStr::new("/home/user"))
            )
            .unwrap(),
            Path::new("/private/config")
        );
        assert_eq!(
            resolve_user_directory(
                UserDirectory::Data,
                Some(OsStr::new("")),
                Some(OsStr::new("/home/user"))
            )
            .unwrap(),
            Path::new("/home/user/.local/share")
        );
        assert_eq!(
            resolve_user_directory(UserDirectory::Cache, None, Some(OsStr::new("/home/user")))
                .unwrap(),
            Path::new("/home/user/.cache")
        );
        assert_eq!(
            resolve_user_directory(UserDirectory::State, None, Some(OsStr::new("/home/user")))
                .unwrap(),
            Path::new("/home/user/.local/state")
        );
    }

    #[test]
    fn xdg_resolution_rejects_relative_roots_and_path_escape() {
        assert_eq!(
            resolve_user_directory(
                UserDirectory::Config,
                Some(OsStr::new("relative")),
                Some(OsStr::new("/home/user"))
            ),
            Err("XDG_CONFIG_HOME must be an absolute path".to_owned())
        );
        assert_eq!(
            resolve_user_directory(UserDirectory::Config, None, Some(OsStr::new("relative"))),
            Err("HOME must be an absolute path".to_owned())
        );
        assert!(
            resolve_user_path(
                UserDirectory::Config,
                None,
                Some(OsStr::new("/home/user")),
                Path::new("../escape")
            )
            .is_err()
        );
        assert!(runtime_directory(Some(OsStr::new("relative"))).is_err());
        assert_eq!(
            resolve_user_path(
                UserDirectory::Config,
                Some(OsStr::new("/config")),
                None,
                Path::new("zentty/config.toml")
            )
            .unwrap(),
            Path::new("/config/zentty/config.toml")
        );
    }

    #[test]
    fn open_contract_rejects_ambiguous_targets_before_desktop_dispatch() {
        assert!(open_uri("not-a-uri").is_err());
        assert!(open_file(Path::new("relative/file")).is_err());
        assert_eq!(
            glib_uri("https://example.com/path").unwrap(),
            "https://example.com/path"
        );
        assert_eq!(
            file_uri(Path::new("/tmp/file with spaces")).unwrap(),
            "file:///tmp/file%20with%20spaces"
        );
    }

    #[test]
    fn process_launch_preserves_arguments_environment_and_working_directory() {
        let mut arguments = ProcessLaunch::detached(
            "/usr/bin/printf",
            vec![
                OsString::from("%s|%s"),
                OsString::from("literal space"),
                OsString::from("$(not-a-shell)\nnext"),
            ],
        );
        arguments.stdout = ChildStdio::Piped;
        let output = spawn_process(&arguments)
            .unwrap()
            .wait_with_output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"literal space|$(not-a-shell)\nnext");

        let mut environment = ProcessLaunch::detached("/usr/bin/env", Vec::new());
        environment.environment.push((
            OsString::from("ZENTTY_PLATFORM_EXACT"),
            Some(OsString::from("one=two")),
        ));
        environment.stdout = ChildStdio::Piped;
        let output = spawn_process(&environment)
            .unwrap()
            .wait_with_output()
            .unwrap();
        assert!(
            String::from_utf8(output.stdout)
                .unwrap()
                .lines()
                .any(|line| line == "ZENTTY_PLATFORM_EXACT=one=two")
        );

        let root = std::env::temp_dir().join(format!("zentty-platform-cwd-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let mut working_directory = ProcessLaunch::detached("/bin/pwd", Vec::new());
        working_directory.current_directory = Some(root.clone());
        working_directory.stdout = ChildStdio::Piped;
        let output = spawn_process(&working_directory)
            .unwrap()
            .wait_with_output()
            .unwrap();
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim_end(),
            root.to_string_lossy()
        );
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn process_launch_rejects_invalid_contracts_and_reports_spawn_failure() {
        assert!(spawn_process(&ProcessLaunch::detached("", Vec::new())).is_err());
        assert!(
            spawn_process(&ProcessLaunch::detached(
                "/definitely/missing/zentty-platform-actor",
                Vec::new()
            ))
            .is_err()
        );
        assert!(
            spawn_detached(&ProcessLaunch::detached(
                "/definitely/missing/zentty-platform-actor",
                Vec::new()
            ))
            .is_err()
        );
        let mut relative = ProcessLaunch::detached("/bin/true", Vec::new());
        relative.current_directory = Some(PathBuf::from("relative"));
        assert!(spawn_process(&relative).is_err());
        let mut invalid_environment = ProcessLaunch::detached("/bin/true", Vec::new());
        invalid_environment.environment.push((
            OsString::from("INVALID=NAME"),
            Some(OsString::from("value")),
        ));
        assert!(spawn_process(&invalid_environment).is_err());
    }
}
