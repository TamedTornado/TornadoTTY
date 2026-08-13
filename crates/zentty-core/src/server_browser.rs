use std::{collections::HashSet, ffi::OsString};

use crate::{ServerDetectionConfig, normalize_server_url};

pub const SYSTEM_DEFAULT_BROWSER_ID: &str = "system-default";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServerBrowserLauncher {
    SystemDefault,
    DesktopApplication { application_id: String },
    Executable { path: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerBrowserTarget {
    pub id: String,
    pub name: String,
    pub launcher: ServerBrowserLauncher,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ServerBrowserCatalog {
    pub enabled: Vec<ServerBrowserTarget>,
    pub preferred: Option<ServerBrowserTarget>,
    pub unavailable_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServerBrowserLaunchPlan {
    SystemDefault {
        url: String,
    },
    DesktopApplication {
        application_id: String,
        url: String,
    },
    Executable {
        executable: String,
        arguments: Vec<OsString>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerBrowserLaunchError {
    InvalidUrl,
}

impl ServerBrowserCatalog {
    #[must_use]
    pub fn resolve(config: &ServerDetectionConfig, available: Vec<ServerBrowserTarget>) -> Self {
        let requested_ids = config
            .enabled_browser_target_ids
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        let enable_all = requested_ids.is_empty();
        let mut available_ids = HashSet::new();
        let mut enabled = Vec::new();
        for target in available {
            if !available_ids.insert(target.id.clone()) {
                continue;
            }
            if target.id == SYSTEM_DEFAULT_BROWSER_ID
                || enable_all
                || requested_ids.contains(target.id.as_str())
            {
                enabled.push(target);
            }
        }

        let enabled_ids = enabled
            .iter()
            .map(|target| target.id.as_str())
            .collect::<HashSet<_>>();
        let preferred = enabled
            .iter()
            .find(|target| target.id == config.preferred_browser_id)
            .or_else(|| {
                enabled
                    .iter()
                    .find(|target| target.id == SYSTEM_DEFAULT_BROWSER_ID)
            })
            .cloned();
        let mut seen_unavailable = HashSet::new();
        let unavailable_ids = config
            .enabled_browser_target_ids
            .iter()
            .chain(std::iter::once(&config.preferred_browser_id))
            .filter(|id| {
                id.as_str() != SYSTEM_DEFAULT_BROWSER_ID
                    && !enabled_ids.contains(id.as_str())
                    && seen_unavailable.insert((*id).clone())
            })
            .cloned()
            .collect();

        Self {
            enabled,
            preferred,
            unavailable_ids,
        }
    }

    #[must_use]
    pub fn target(&self, id: &str) -> Option<&ServerBrowserTarget> {
        self.enabled.iter().find(|target| target.id == id)
    }
}

impl ServerBrowserTarget {
    /// Creates a shell-free launch plan for a normalized local HTTP(S) server.
    ///
    /// # Errors
    ///
    /// Returns an error when `raw_url` is not accepted by Zentty's development
    /// server URL policy.
    pub fn launch_plan(
        &self,
        raw_url: &str,
    ) -> Result<ServerBrowserLaunchPlan, ServerBrowserLaunchError> {
        let url = normalize_server_url(raw_url)
            .map_err(|_| ServerBrowserLaunchError::InvalidUrl)?
            .url;
        Ok(match &self.launcher {
            ServerBrowserLauncher::SystemDefault => ServerBrowserLaunchPlan::SystemDefault { url },
            ServerBrowserLauncher::DesktopApplication { application_id } => {
                ServerBrowserLaunchPlan::DesktopApplication {
                    application_id: application_id.clone(),
                    url,
                }
            }
            ServerBrowserLauncher::Executable { path } => ServerBrowserLaunchPlan::Executable {
                executable: path.clone(),
                arguments: vec![OsString::from(url)],
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ServerBrowserCustomApp, ServerDetectionConfig};

    fn config(preferred: &str, enabled: &[&str]) -> ServerDetectionConfig {
        ServerDetectionConfig {
            preferred_browser_id: preferred.into(),
            enabled_browser_target_ids: enabled.iter().map(|id| (*id).into()).collect(),
            custom_browsers: Vec::<ServerBrowserCustomApp>::new(),
            ..ServerDetectionConfig::default()
        }
    }

    fn target(id: &str) -> ServerBrowserTarget {
        ServerBrowserTarget {
            id: id.into(),
            name: id.into(),
            launcher: if id == SYSTEM_DEFAULT_BROWSER_ID {
                ServerBrowserLauncher::SystemDefault
            } else {
                ServerBrowserLauncher::Executable {
                    path: format!("/bin/{id}"),
                }
            },
        }
    }

    #[test]
    fn catalog_keeps_system_default_and_enabled_available_targets() {
        let catalog = ServerBrowserCatalog::resolve(
            &config("firefox", &["firefox", "missing"]),
            vec![
                target(SYSTEM_DEFAULT_BROWSER_ID),
                target("firefox"),
                target("chromium"),
                target("firefox"),
            ],
        );
        assert_eq!(
            catalog
                .enabled
                .iter()
                .map(|target| target.id.as_str())
                .collect::<Vec<_>>(),
            [SYSTEM_DEFAULT_BROWSER_ID, "firefox"]
        );
        assert_eq!(catalog.preferred.as_ref().unwrap().id, "firefox");
        assert_eq!(catalog.unavailable_ids, ["missing"]);
    }

    #[test]
    fn empty_enabled_list_exposes_all_and_missing_preference_falls_back() {
        let catalog = ServerBrowserCatalog::resolve(
            &config("missing", &[]),
            vec![target(SYSTEM_DEFAULT_BROWSER_ID), target("firefox")],
        );
        assert_eq!(catalog.enabled.len(), 2);
        assert_eq!(
            catalog.preferred.as_ref().unwrap().id,
            SYSTEM_DEFAULT_BROWSER_ID
        );
        assert_eq!(catalog.unavailable_ids, ["missing"]);
    }

    #[test]
    fn explicit_system_default_selection_disables_every_optional_browser() {
        let catalog = ServerBrowserCatalog::resolve(
            &config(SYSTEM_DEFAULT_BROWSER_ID, &[SYSTEM_DEFAULT_BROWSER_ID]),
            vec![target(SYSTEM_DEFAULT_BROWSER_ID), target("firefox")],
        );
        assert_eq!(
            catalog
                .enabled
                .iter()
                .map(|target| target.id.as_str())
                .collect::<Vec<_>>(),
            [SYSTEM_DEFAULT_BROWSER_ID]
        );
        assert_eq!(
            catalog.preferred.as_ref().map(|target| target.id.as_str()),
            Some(SYSTEM_DEFAULT_BROWSER_ID)
        );
    }

    #[test]
    fn target_lookup_returns_only_the_exact_enabled_target() {
        let catalog = ServerBrowserCatalog::resolve(
            &config("firefox", &[]),
            vec![target(SYSTEM_DEFAULT_BROWSER_ID), target("firefox")],
        );

        assert_eq!(catalog.target("firefox").unwrap().name, "firefox");
        assert!(catalog.target("chromium").is_none());
    }

    #[test]
    fn launch_plans_normalize_safe_urls_and_reject_hostile_schemes() {
        assert_eq!(
            target("firefox").launch_plan("localhost:5173/app").unwrap(),
            ServerBrowserLaunchPlan::Executable {
                executable: "/bin/firefox".into(),
                arguments: vec![OsString::from("http://localhost:5173/app")],
            }
        );
        assert_eq!(
            target("firefox").launch_plan("file:///etc/passwd"),
            Err(ServerBrowserLaunchError::InvalidUrl)
        );
    }
}
