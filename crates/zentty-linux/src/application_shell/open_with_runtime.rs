use std::{
    collections::HashSet,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use gtk::gio;
use gtk::gio::prelude::AppInfoExt;
use zentty_core::{
    LINUX_OPEN_WITH_BUILTIN_IDS, OpenWithCatalog, OpenWithConfig, OpenWithLaunchPlan,
    OpenWithLauncher, OpenWithTarget, OpenWithTargetKind, SYSTEM_FILE_MANAGER_ID,
    SYSTEM_TERMINAL_ID,
};

use super::ApplicationShell;

type BuiltinExecutableSpec = (
    &'static str,
    &'static str,
    OpenWithTargetKind,
    &'static [&'static str],
    &'static [&'static str],
);

const BUILTIN_EXECUTABLES: &[BuiltinExecutableSpec] = &[
    (
        "vscode",
        "VS Code",
        OpenWithTargetKind::Editor,
        &["code"],
        &[],
    ),
    (
        "vscode-insiders",
        "VS Code Insiders",
        OpenWithTargetKind::Editor,
        &["code-insiders"],
        &[],
    ),
    (
        "cursor",
        "Cursor",
        OpenWithTargetKind::Editor,
        &["cursor"],
        &[],
    ),
    ("zed", "Zed", OpenWithTargetKind::Editor, &["zed"], &[]),
    (
        "windsurf",
        "Windsurf",
        OpenWithTargetKind::Editor,
        &["windsurf"],
        &[],
    ),
    (
        "android-studio",
        "Android Studio",
        OpenWithTargetKind::Editor,
        &["studio", "android-studio"],
        &[],
    ),
    (
        "intellij-idea",
        "IntelliJ IDEA",
        OpenWithTargetKind::Editor,
        &["idea"],
        &[],
    ),
    (
        "rider",
        "Rider",
        OpenWithTargetKind::Editor,
        &["rider"],
        &[],
    ),
    (
        "goland",
        "GoLand",
        OpenWithTargetKind::Editor,
        &["goland"],
        &[],
    ),
    (
        "rustrover",
        "RustRover",
        OpenWithTargetKind::Editor,
        &["rustrover"],
        &[],
    ),
    (
        "pycharm",
        "PyCharm",
        OpenWithTargetKind::Editor,
        &["pycharm"],
        &[],
    ),
    (
        "webstorm",
        "WebStorm",
        OpenWithTargetKind::Editor,
        &["webstorm"],
        &[],
    ),
    (
        "phpstorm",
        "PhpStorm",
        OpenWithTargetKind::Editor,
        &["phpstorm"],
        &[],
    ),
    (
        "sublime-text",
        "Sublime Text",
        OpenWithTargetKind::Editor,
        &["subl"],
        &[],
    ),
];

pub(super) struct OpenWithRuntime {
    pub(super) catalog: OpenWithCatalog,
}

impl OpenWithRuntime {
    pub(super) fn discover(config: &OpenWithConfig) -> Self {
        let available = discover_available_targets(config, std::env::var_os("PATH").as_deref());
        let catalog = OpenWithCatalog::resolve(config, available);
        eprintln!(
            "zentty-linux: open-with-discovery available={} primary={} unavailable={}",
            catalog.enabled.len(),
            catalog
                .primary
                .as_ref()
                .map_or("none", |target| target.id.as_str()),
            catalog.unavailable_ids.join(",")
        );
        Self { catalog }
    }
}

pub(super) fn open_primary(shell: &ApplicationShell) {
    let Some(target) = shell.open_with_runtime.catalog.primary.clone() else {
        eprintln!("zentty-linux: action=open-with-primary unavailable=no-primary-target");
        return;
    };
    open_target(shell, &target.id);
}

pub(super) fn open_local_path_primary(shell: &ApplicationShell, path: &Path, context: &str) {
    let Some(target) = shell.open_with_runtime.catalog.primary.clone() else {
        eprintln!("zentty-linux: action=open-{context} unavailable=no-primary-target");
        return;
    };
    let plan = match target.launch_local_path_plan(path) {
        Ok(plan) => plan,
        Err(error) => {
            eprintln!(
                "zentty-linux: action=open-{context} id={} error={error:?} path={}",
                target.id,
                path.display()
            );
            return;
        }
    };
    match launch(plan) {
        Ok(()) => eprintln!(
            "zentty-linux: action=open-{context} id={} result=launched path={}",
            target.id,
            path.display()
        ),
        Err(error) => eprintln!(
            "zentty-linux: action=open-{context} id={} error={error} path={}",
            target.id,
            path.display()
        ),
    }
}

pub(super) fn open_target(shell: &ApplicationShell, target_id: &str) {
    let Some(target) = shell
        .open_with_runtime
        .catalog
        .enabled
        .iter()
        .find(|target| target.id == target_id)
    else {
        eprintln!(
            "zentty-linux: action=open-with-target id={target_id} unavailable=unknown-target"
        );
        return;
    };
    let directory = match focused_local_directory(shell) {
        Ok(directory) => directory,
        Err(error) => {
            eprintln!("zentty-linux: action=open-with-target id={target_id} unavailable={error}");
            return;
        }
    };
    let plan = match target.launch_plan(&directory) {
        Ok(plan) => plan,
        Err(error) => {
            eprintln!("zentty-linux: action=open-with-target id={target_id} error={error:?}");
            return;
        }
    };
    match launch(plan) {
        Ok(()) => eprintln!(
            "zentty-linux: action=open-with-target id={target_id} result=launched path={}",
            directory.display()
        ),
        Err(error) => eprintln!(
            "zentty-linux: action=open-with-target id={target_id} error={error} path={}",
            directory.display()
        ),
    }
}

pub(super) fn focused_context_is_available(shell: &ApplicationShell) -> bool {
    !shell.open_with_runtime.catalog.enabled.is_empty() && focused_local_directory(shell).is_ok()
}

fn focused_local_directory(shell: &ApplicationShell) -> Result<PathBuf, &'static str> {
    let pane_id = shell.state.focused_pane_id().ok_or("no-focused-pane")?;
    let foreground_process_id = shell
        .pane_runtime
        .surface(pane_id)
        .and_then(zentty_ghostty::GhosttySurface::foreground_process_id);
    if shell.remote_panes.identities.contains_key(pane_id)
        || foreground_process_id
            .and_then(super::ssh_identity::probe_ssh_destination)
            .is_some()
    {
        return Err("remote-pane");
    }
    let directory = foreground_process_id
        .and_then(|pid| fs::read_link(format!("/proc/{pid}/cwd")).ok())
        .or_else(|| {
            shell
                .state
                .pane(pane_id)
                .and_then(|pane| pane.working_directory.as_deref())
                .map(PathBuf::from)
        })
        .ok_or("missing-directory")?;
    let canonical = fs::canonicalize(directory).map_err(|_| "stale-directory")?;
    if !canonical.is_dir() {
        return Err("not-directory");
    }
    Ok(canonical)
}

pub(super) fn discover_available_targets(
    config: &OpenWithConfig,
    path: Option<&std::ffi::OsStr>,
) -> Vec<OpenWithTarget> {
    let mut targets = BUILTIN_EXECUTABLES
        .iter()
        .filter_map(|(id, name, kind, candidates, prefix_args)| {
            resolve_executable(candidates, path).map(|executable| OpenWithTarget {
                id: (*id).into(),
                name: (*name).into(),
                kind: *kind,
                launcher: OpenWithLauncher::Executable {
                    path: executable.to_string_lossy().into_owned(),
                    prefix_args: prefix_args
                        .iter()
                        .map(|argument| (*argument).into())
                        .collect(),
                },
            })
        })
        .collect::<Vec<_>>();

    if let Some(app) = gio::AppInfo::default_for_type("inode/directory", true)
        .or_else(|| gio::AppInfo::default_for_type("inode/directory", false))
        && let Some(application_id) = app.id()
    {
        targets.push(OpenWithTarget {
            id: SYSTEM_FILE_MANAGER_ID.into(),
            name: app.display_name().into(),
            kind: OpenWithTargetKind::FileManager,
            launcher: OpenWithLauncher::DesktopApplication {
                application_id: application_id.into(),
            },
        });
    }

    if let Some(executable) = resolve_executable(&["xdg-terminal-exec"], path) {
        targets.push(OpenWithTarget {
            id: SYSTEM_TERMINAL_ID.into(),
            name: "System Terminal".into(),
            kind: OpenWithTargetKind::Terminal,
            launcher: OpenWithLauncher::ExecutableDirectoryOption {
                path: executable.to_string_lossy().into_owned(),
                option_prefix: "--dir=".into(),
            },
        });
    }

    let reserved_ids = LINUX_OPEN_WITH_BUILTIN_IDS
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let mut custom_ids = HashSet::new();
    let mut custom_paths = HashSet::new();
    targets.extend(config.custom_apps.iter().filter_map(|app| {
        if app.id.trim().is_empty()
            || app.name.trim().is_empty()
            || reserved_ids.contains(app.id.as_str())
            || !custom_ids.insert(app.id.as_str())
        {
            return None;
        }
        let canonical = canonical_executable(Path::new(&app.path))?;
        if !custom_paths.insert(canonical.clone()) {
            return None;
        }
        Some(OpenWithTarget {
            id: app.id.clone(),
            name: app.name.clone(),
            kind: OpenWithTargetKind::Editor,
            launcher: OpenWithLauncher::Executable {
                path: canonical.to_string_lossy().into_owned(),
                prefix_args: Vec::new(),
            },
        })
    }));
    targets
}

pub(super) fn resolve_executable(
    candidates: &[&str],
    path: Option<&std::ffi::OsStr>,
) -> Option<PathBuf> {
    let path = path?;
    for directory in std::env::split_paths(path) {
        for candidate in candidates {
            if let Some(executable) = canonical_executable(&directory.join(candidate)) {
                return Some(executable);
            }
        }
    }
    None
}

pub(crate) fn canonical_executable(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }
    let canonical = fs::canonicalize(path).ok()?;
    let metadata = fs::metadata(&canonical).ok()?;
    (metadata.is_file() && metadata.permissions().mode() & 0o111 != 0).then_some(canonical)
}

fn launch(plan: OpenWithLaunchPlan) -> Result<(), String> {
    match plan {
        OpenWithLaunchPlan::DesktopApplication {
            application_id,
            canonical_uri,
        } => {
            let app = gio::AppInfo::all()
                .into_iter()
                .find(|app| app.id().as_deref() == Some(application_id.as_str()))
                .ok_or_else(|| "desktop application disappeared after discovery".to_owned())?;
            app.launch_uris(&[canonical_uri.as_str()], None::<&gio::AppLaunchContext>)
                .map_err(|error| error.to_string())
        }
        OpenWithLaunchPlan::Executable {
            executable,
            arguments,
        } => {
            let mut child = Command::new(&executable)
                .args(&arguments)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|error| format!("could not launch {executable}: {error}"))?;
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use zentty_core::OpenWithCustomApp;

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn fixture_root() -> PathBuf {
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "zentty-open-with-runtime-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn write_executable(path: &Path) {
        fs::write(path, "#!/bin/sh\nexit 0\n").unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    #[test]
    fn path_discovery_requires_a_real_executable_and_uses_canonical_path() {
        let root = fixture_root();
        let bin = root.join("bin");
        fs::create_dir(&bin).unwrap();
        fs::write(bin.join("code"), "not executable").unwrap();
        assert!(resolve_executable(&["code"], Some(bin.as_os_str())).is_none());
        write_executable(&bin.join("code"));
        assert_eq!(
            resolve_executable(&["code"], Some(bin.as_os_str())),
            Some(fs::canonicalize(bin.join("code")).unwrap())
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn custom_discovery_rejects_relative_duplicate_reserved_and_non_executable_targets() {
        let root = fixture_root();
        let first = root.join("first");
        let second = root.join("second");
        write_executable(&first);
        write_executable(&second);
        let custom_apps = vec![
            ("custom:first", "First", first.to_str().unwrap()),
            ("custom:first", "Duplicate ID", second.to_str().unwrap()),
            ("custom:path", "Duplicate path", first.to_str().unwrap()),
            ("vscode", "Reserved", second.to_str().unwrap()),
            (
                "codex",
                "Reserved without Linux launcher",
                second.to_str().unwrap(),
            ),
            ("custom:relative", "Relative", "relative-program"),
            ("", "Empty ID", second.to_str().unwrap()),
            ("custom:empty-name", "", second.to_str().unwrap()),
            ("custom:empty-path", "Empty Path", ""),
        ]
        .into_iter()
        .map(|(id, name, path)| OpenWithCustomApp {
            id: id.into(),
            name: name.into(),
            path: path.into(),
        })
        .collect();
        let config = OpenWithConfig {
            primary_target_id: "custom:first".into(),
            enabled_target_ids: vec!["custom:first".into()],
            custom_apps,
        };
        let targets = discover_available_targets(&config, Some("".as_ref()));
        assert_eq!(
            targets
                .iter()
                .filter(|target| target.id.starts_with("custom:"))
                .map(|target| target.id.as_str())
                .collect::<Vec<_>>(),
            ["custom:first"]
        );
        assert!(targets.iter().all(|target| !target.id.is_empty()));
        assert!(targets.iter().all(|target| target.id != "vscode"));
        assert!(targets.iter().all(|target| target.id != "codex"));
        let retained = targets
            .iter()
            .find(|target| target.id == "custom:first")
            .unwrap();
        let OpenWithLauncher::Executable { path, .. } = &retained.launcher else {
            panic!("custom executable changed launcher kind");
        };
        assert_eq!(Path::new(path), fs::canonicalize(&first).unwrap());
        fs::remove_dir_all(root).unwrap();
    }
}
