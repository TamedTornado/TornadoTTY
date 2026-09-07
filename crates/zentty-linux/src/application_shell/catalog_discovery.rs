//! Executable/desktop discovery has one worker and one latest request per
//! window. Only plain catalog data crosses back to GTK, never Gio app objects.
use gtk::{gio, glib};
use zentty_core::{OpenWithConfig, OpenWithTarget, ServerBrowserTarget, ServerDetectionConfig};

use super::{ApplicationShell, open_with_runtime, server_runtime};
use crate::open_with_settings::RefreshCompletion;
use crate::settings_navigation::SettingsSection;

#[derive(Clone, PartialEq)]
struct Key {
    apps: OpenWithConfig,
    browsers: ServerDetectionConfig,
}

impl Key {
    fn capture(shell: &ApplicationShell) -> Self {
        Self {
            apps: shell.config.open_with.clone(),
            browsers: shell.config.server_detection.clone(),
        }
    }
}

#[derive(Default)]
pub(super) struct Discovery {
    requested: Option<Key>,
    ready: Option<Key>,
    generation: u64,
    running: bool,
    pub(super) apps: Vec<OpenWithTarget>,
    pub(super) browsers: Vec<ServerBrowserTarget>,
    pub(super) pending_settings: Option<SettingsSection>,
    pub(super) refresh: Option<RefreshCompletion>,
}

impl Discovery {
    fn begin(&mut self, key: &Key, force: bool) -> Option<u64> {
        if force || self.requested.as_ref() != Some(key) {
            self.generation = self.generation.wrapping_add(1);
            self.requested = Some(key.clone());
            self.ready = None;
        }
        if self.running || (!force && self.ready.as_ref() == Some(key)) {
            return None;
        }
        self.running = true;
        Some(self.generation)
    }

    fn finish(&mut self, key: &Key, generation: u64) -> bool {
        self.running = false;
        self.generation == generation && self.requested.as_ref() == Some(key)
    }
}

pub(super) fn ready(shell: &ApplicationShell) -> bool {
    shell.catalog_discovery.ready.as_ref() == Some(&Key::capture(shell))
}

pub(super) fn request(shell: &mut ApplicationShell, force: bool) {
    let key = Key::capture(shell);
    let state = &mut shell.catalog_discovery;
    let previous = state.generation;
    let started = state.begin(&key, force);
    if state.generation != previous {
        // Do not leave an old custom executable or disabled browser launchable
        // while its replacement is being inspected.
        shell.open_with_runtime.catalog = zentty_core::OpenWithCatalog::default();
        shell
            .chrome
            .configure_open_with(&shell.open_with_runtime.catalog);
        shell.server_runtime.browser_catalog =
            zentty_core::ServerBrowserCatalog::resolve(&key.browsers, Vec::new());
    }
    let Some(generation) = started else { return };
    let weak = shell.self_handle.borrow().clone();
    eprintln!("zentty-linux: catalog-discovery result=started generation={generation}");
    glib::spawn_future_local(async move {
        let source = key.clone();
        let result = gio::spawn_blocking(move || {
            let path = std::env::var_os("PATH");
            (
                open_with_runtime::discover_available_targets(&source.apps, path.as_deref()),
                server_runtime::discover_browser_targets(&source.browsers, path.as_deref()),
            )
        })
        .await;
        let Some(shell) = weak.upgrade() else { return };
        let mut current = shell.borrow_mut();
        let valid = current.catalog_discovery.finish(&key, generation);
        if current.shutting_down {
            return;
        }
        if !valid || Key::capture(&current) != key {
            eprintln!("zentty-linux: catalog-discovery result=stale generation={generation}");
            // Refresh belongs to the newest requested scan, not necessarily
            // this finishing worker. Keep it pending while coalescing.
            request(&mut current, false);
            return;
        }
        let Ok((apps, browsers)) = result else {
            let refresh = current.catalog_discovery.refresh.take();
            current.catalog_discovery.pending_settings = None;
            drop(current);
            ApplicationShell::report_action_error(
                &shell,
                "catalog-discovery",
                "Application discovery worker failed. Retry opening settings or refreshing apps.",
            );
            if let Some(refresh) = refresh {
                refresh(Err("Application discovery worker failed".into()));
            }
            return;
        };
        current.catalog_discovery.apps = apps;
        current.catalog_discovery.browsers = browsers;
        current.catalog_discovery.ready = Some(key);
        project(&mut current);
        let refresh = current.catalog_discovery.refresh.take();
        let projection = refresh
            .as_ref()
            .map(|_| current.refresh_open_with_projection());
        let settings = current.catalog_discovery.pending_settings.take();
        if let Some(section) = settings {
            current.request_show_settings(section);
        }
        drop(current);
        if let Some(refresh) = refresh {
            refresh(projection.expect("refresh prepared a projection"));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_coalesce_without_overlapping_workers_or_publishing_old_preferences() {
        let a = Key {
            apps: OpenWithConfig::default(),
            browsers: ServerDetectionConfig::default(),
        };
        let mut b = a.clone();
        b.apps.primary_target_id = "custom:new".into();
        b.browsers.preferred_browser_id = "firefox".into();
        let mut state = Discovery::default();
        let first = state.begin(&a, false).unwrap();
        assert!(state.begin(&a, false).is_none());
        assert!(
            state.begin(&b, false).is_none(),
            "latest request must not spawn a concurrent worker"
        );
        assert!(!state.finish(&a, first), "old preferences must not publish");
        let latest = state.begin(&b, false).unwrap();
        assert!(state.finish(&b, latest));
        state.ready = Some(b.clone());
        assert!(
            state.begin(&b, false).is_none(),
            "ready catalog must not rescan on unrelated reload"
        );
        let refresh = state.begin(&b, true).unwrap();
        assert!(state.ready.is_none());
        assert!(
            !state.finish(&b, latest),
            "same settings do not validate an older refresh generation"
        );
        assert!(state.finish(&b, refresh));
    }

    #[test]
    fn change_back_and_force_during_scan_still_require_fresh_discovery() {
        let a = Key {
            apps: OpenWithConfig::default(),
            browsers: ServerDetectionConfig::default(),
        };
        let mut b = a.clone();
        b.apps.enabled_target_ids.clear();
        let mut state = Discovery::default();
        let first = state.begin(&a, false).unwrap();
        assert!(state.begin(&b, false).is_none());
        assert!(state.begin(&a, false).is_none());
        assert!(state.begin(&a, true).is_none());
        assert!(!state.finish(&a, first));
        let latest = state.begin(&a, false).unwrap();
        assert!(state.finish(&a, latest));
    }
}

fn project(shell: &mut ApplicationShell) {
    shell.open_with_runtime.catalog = zentty_core::OpenWithCatalog::resolve(
        &shell.config.open_with,
        shell.catalog_discovery.apps.clone(),
    );
    let catalog = &shell.open_with_runtime.catalog;
    eprintln!(
        "zentty-linux: open-with-discovery available={} primary={} unavailable={}",
        catalog.enabled.len(),
        catalog
            .primary
            .as_ref()
            .map_or("none", |target| target.id.as_str()),
        catalog.unavailable_ids.join(",")
    );
    shell.chrome.configure_open_with(catalog);
    shell
        .chrome
        .set_open_with_context_available(open_with_runtime::focused_context_is_available(
            catalog,
            open_with_runtime::focused_context(shell).as_ref(),
        ));
    shell.server_runtime.browser_catalog = zentty_core::ServerBrowserCatalog::resolve(
        &shell.config.server_detection,
        shell.catalog_discovery.browsers.clone(),
    );
    let catalog = &shell.server_runtime.browser_catalog;
    eprintln!(
        "zentty-linux: server-browser-discovery available={} preferred={} unavailable={}",
        catalog.enabled.len(),
        catalog
            .preferred
            .as_ref()
            .map_or("none", |target| target.id.as_str()),
        catalog.unavailable_ids.join(",")
    );
}
