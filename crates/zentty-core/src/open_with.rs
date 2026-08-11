use std::{collections::HashSet, ffi::OsString, fs, path::Path};

use crate::OpenWithConfig;

pub const SYSTEM_FILE_MANAGER_ID: &str = "system-file-manager";
pub const SYSTEM_TERMINAL_ID: &str = "system-terminal";
pub const LINUX_OPEN_WITH_BUILTIN_IDS: &[&str] = &[
    "vscode",
    "vscode-insiders",
    "cursor",
    "zed",
    "windsurf",
    "antigravity",
    "codex",
    "claude",
    "finder",
    "xcode",
    "android-studio",
    "intellij-idea",
    "rider",
    "goland",
    "rustrover",
    "pycharm",
    "webstorm",
    "phpstorm",
    "sublime-text",
    "bbedit",
    "textmate",
    SYSTEM_FILE_MANAGER_ID,
    SYSTEM_TERMINAL_ID,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenWithTargetKind {
    Editor,
    FileManager,
    Terminal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpenWithLauncher {
    DesktopApplication {
        application_id: String,
    },
    Executable {
        path: String,
        prefix_args: Vec<String>,
    },
    ExecutableDirectoryOption {
        path: String,
        option_prefix: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenWithTarget {
    pub id: String,
    pub name: String,
    pub kind: OpenWithTargetKind,
    pub launcher: OpenWithLauncher,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OpenWithCatalog {
    pub enabled: Vec<OpenWithTarget>,
    pub primary: Option<OpenWithTarget>,
    pub unavailable_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpenWithLaunchPlan {
    DesktopApplication {
        application_id: String,
        canonical_uri: String,
    },
    Executable {
        executable: String,
        arguments: Vec<OsString>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenWithLaunchError {
    MissingPath,
    NotDirectory,
    CannotCanonicalize,
}

impl OpenWithCatalog {
    #[must_use]
    pub fn resolve(config: &OpenWithConfig, available: Vec<OpenWithTarget>) -> Self {
        let enabled_ids = config
            .enabled_target_ids
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        let mut seen_available = HashSet::new();
        let enabled = available
            .into_iter()
            .filter(|target| {
                enabled_ids.contains(target.id.as_str()) && seen_available.insert(target.id.clone())
            })
            .collect::<Vec<_>>();
        let available_ids = seen_available;
        let mut seen_unavailable = HashSet::new();
        let unavailable_ids = config
            .enabled_target_ids
            .iter()
            .filter(|id| {
                !available_ids.contains(id.as_str()) && seen_unavailable.insert((*id).clone())
            })
            .cloned()
            .collect::<Vec<_>>();

        let primary = enabled
            .iter()
            .find(|target| target.id == config.primary_target_id)
            .or_else(|| enabled.first())
            .cloned();

        Self {
            enabled,
            primary,
            unavailable_ids,
        }
    }
}

impl OpenWithTarget {
    /// Builds a launch request from a real local directory without invoking a shell.
    ///
    /// # Errors
    ///
    /// Returns an error when the path is missing, is not a directory, or cannot
    /// be canonicalized.
    pub fn launch_plan(&self, directory: &Path) -> Result<OpenWithLaunchPlan, OpenWithLaunchError> {
        let metadata = fs::metadata(directory).map_err(|_| OpenWithLaunchError::MissingPath)?;
        if !metadata.is_dir() {
            return Err(OpenWithLaunchError::NotDirectory);
        }
        let canonical =
            fs::canonicalize(directory).map_err(|_| OpenWithLaunchError::CannotCanonicalize)?;
        match &self.launcher {
            OpenWithLauncher::DesktopApplication { application_id } => {
                Ok(OpenWithLaunchPlan::DesktopApplication {
                    application_id: application_id.clone(),
                    canonical_uri: path_to_file_uri(&canonical),
                })
            }
            OpenWithLauncher::Executable { path, prefix_args } => {
                let mut arguments = prefix_args.iter().map(OsString::from).collect::<Vec<_>>();
                arguments.push(canonical.into_os_string());
                Ok(OpenWithLaunchPlan::Executable {
                    executable: path.clone(),
                    arguments,
                })
            }
            OpenWithLauncher::ExecutableDirectoryOption {
                path,
                option_prefix,
            } => {
                let mut directory_option = OsString::from(option_prefix);
                directory_option.push(canonical.as_os_str());
                Ok(OpenWithLaunchPlan::Executable {
                    executable: path.clone(),
                    arguments: vec![directory_option],
                })
            }
        }
    }
}

fn path_to_file_uri(path: &Path) -> String {
    let mut uri = String::from("file://");
    for byte in path.as_os_str().as_encoded_bytes() {
        if matches!(
            byte,
            b'A'..=b'Z'
                | b'a'..=b'z'
                | b'0'..=b'9'
                | b'-'
                | b'.'
                | b'_'
                | b'~'
                | b'/'
                | b':'
        ) {
            uri.push(char::from(*byte));
        } else {
            use std::fmt::Write;
            write!(uri, "%{byte:02X}").expect("writing to a String cannot fail");
        }
    }
    uri
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        os::unix::ffi::OsStringExt,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;

    static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn target(id: &str) -> OpenWithTarget {
        OpenWithTarget {
            id: id.into(),
            name: id.into(),
            kind: OpenWithTargetKind::Editor,
            launcher: OpenWithLauncher::Executable {
                path: format!("/bin/{id}"),
                prefix_args: Vec::new(),
            },
        }
    }

    fn fixture_directory(label: &str) -> std::path::PathBuf {
        let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "zentty-open-with-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn catalog_preserves_source_catalog_order_and_falls_back_to_first_primary() {
        let config = OpenWithConfig {
            primary_target_id: "missing-primary".into(),
            enabled_target_ids: vec!["second".into(), "first".into(), "second".into()],
            custom_apps: Vec::new(),
        };
        let catalog = OpenWithCatalog::resolve(&config, vec![target("first"), target("second")]);

        assert_eq!(
            catalog
                .enabled
                .iter()
                .map(|target| target.id.as_str())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
        assert_eq!(catalog.primary.as_ref().unwrap().id, "first");
        assert!(catalog.unavailable_ids.is_empty());
    }

    #[test]
    fn catalog_reports_each_missing_enabled_id_once() {
        let config = OpenWithConfig {
            primary_target_id: "missing".into(),
            enabled_target_ids: vec!["missing".into(), "available".into(), "missing".into()],
            custom_apps: Vec::new(),
        };
        let catalog = OpenWithCatalog::resolve(&config, vec![target("available")]);

        assert_eq!(catalog.unavailable_ids, ["missing"]);
        assert_eq!(catalog.primary.as_ref().unwrap().id, "available");
    }

    #[test]
    fn catalog_uses_requested_primary_when_it_is_not_first() {
        let config = OpenWithConfig {
            primary_target_id: "second".into(),
            enabled_target_ids: vec!["first".into(), "second".into()],
            custom_apps: Vec::new(),
        };
        let catalog = OpenWithCatalog::resolve(&config, vec![target("first"), target("second")]);
        assert_eq!(catalog.primary.as_ref().unwrap().id, "second");
    }

    #[test]
    fn executable_plan_preserves_prefix_and_passes_one_canonical_path_argument() {
        let directory = fixture_directory("path with spaces");
        let target = OpenWithTarget {
            id: "custom".into(),
            name: "Custom".into(),
            kind: OpenWithTargetKind::Editor,
            launcher: OpenWithLauncher::Executable {
                path: "/opt/example editor".into(),
                prefix_args: vec!["--new-window".into()],
            },
        };

        let OpenWithLaunchPlan::Executable {
            executable,
            arguments,
        } = target.launch_plan(&directory).unwrap()
        else {
            panic!("expected executable plan");
        };
        assert_eq!(executable, "/opt/example editor");
        assert_eq!(
            arguments,
            [
                OsString::from("--new-window"),
                directory.clone().into_os_string(),
            ]
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn desktop_plan_encodes_canonical_file_uri() {
        let directory = fixture_directory("café # directory");
        let target = OpenWithTarget {
            id: SYSTEM_FILE_MANAGER_ID.into(),
            name: "Files".into(),
            kind: OpenWithTargetKind::FileManager,
            launcher: OpenWithLauncher::DesktopApplication {
                application_id: "fixture.desktop".into(),
            },
        };

        let OpenWithLaunchPlan::DesktopApplication { canonical_uri, .. } =
            target.launch_plan(&directory).unwrap()
        else {
            panic!("expected desktop application plan");
        };
        assert!(canonical_uri.starts_with("file:///"));
        assert!(canonical_uri.contains("caf%C3%A9%20%23%20directory-"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn directory_option_plan_keeps_the_path_in_one_monolithic_argument() {
        let directory = fixture_directory("terminal path");
        let target = OpenWithTarget {
            id: SYSTEM_TERMINAL_ID.into(),
            name: "System Terminal".into(),
            kind: OpenWithTargetKind::Terminal,
            launcher: OpenWithLauncher::ExecutableDirectoryOption {
                path: "/usr/bin/xdg-terminal-exec".into(),
                option_prefix: "--dir=".into(),
            },
        };

        let OpenWithLaunchPlan::Executable { arguments, .. } =
            target.launch_plan(&directory).unwrap()
        else {
            panic!("expected executable plan");
        };
        let mut expected = OsString::from("--dir=");
        expected.push(directory.as_os_str());
        assert_eq!(arguments, [expected]);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn executable_plan_preserves_non_utf8_linux_directory_bytes() {
        let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let mut name =
            format!("zentty-open-with-bytes-{}-{sequence}-", std::process::id()).into_bytes();
        name.push(0xff);
        let directory = std::env::temp_dir().join(OsString::from_vec(name));
        fs::create_dir(&directory).unwrap();
        let target = target("editor");

        let OpenWithLaunchPlan::Executable { arguments, .. } =
            target.launch_plan(&directory).unwrap()
        else {
            panic!("expected executable plan");
        };
        assert_eq!(arguments, [directory.clone().into_os_string()]);
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn launch_plan_rejects_missing_paths_and_regular_files() {
        let directory = fixture_directory("validation");
        let missing = directory.join("missing");
        let file = directory.join("file");
        fs::write(&file, "not a directory").unwrap();
        let target = target("editor");

        assert_eq!(
            target.launch_plan(&missing),
            Err(OpenWithLaunchError::MissingPath)
        );
        assert_eq!(
            target.launch_plan(&file),
            Err(OpenWithLaunchError::NotDirectory)
        );
        fs::remove_dir_all(directory).unwrap();
    }
}
