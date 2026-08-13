use std::path::{Path, PathBuf};

use gtk::gio;
use gtk::prelude::*;
use zentty_core::AppConfig;

use crate::config_store::{ConfigSnapshot, ConfigStore};

const RELOAD_QUIET_PERIOD_MILLIS: u64 = 150;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ReloadDecision {
    Unchanged,
    Apply(AppConfig),
    RetainLastGood(String),
}

pub(crate) struct ConfigReloadAuthority {
    path: PathBuf,
    last_good: AppConfig,
}

impl ConfigReloadAuthority {
    pub(crate) fn new(snapshot: &ConfigSnapshot) -> Self {
        Self {
            path: snapshot.path.clone(),
            last_good: snapshot.config.clone(),
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn observe_disk(&mut self) -> ReloadDecision {
        if let Err(error) = std::fs::symlink_metadata(&self.path) {
            return ReloadDecision::RetainLastGood(
                if error.kind() == std::io::ErrorKind::NotFound {
                    format!(
                        "configuration disappeared; retaining last-good state for {}",
                        self.path.display()
                    )
                } else {
                    format!(
                        "could not inspect configuration {}; retaining last-good state: {error}",
                        self.path.display()
                    )
                },
            );
        }
        match ConfigStore::load_path(&self.path) {
            Ok(snapshot) => self.observe_snapshot(snapshot),
            Err(error) => ReloadDecision::RetainLastGood(format!(
                "configuration reload failed; retaining last-good state: {error}"
            )),
        }
    }

    fn observe_snapshot(&mut self, snapshot: ConfigSnapshot) -> ReloadDecision {
        if let Some(warning) = snapshot.warning {
            return ReloadDecision::RetainLastGood(format!(
                "{warning}; retaining last-good runtime state"
            ));
        }
        if snapshot.config == self.last_good {
            return ReloadDecision::Unchanged;
        }
        ReloadDecision::Apply(snapshot.config)
    }

    pub(crate) fn accept(&mut self, config: &AppConfig) {
        self.last_good.clone_from(config);
    }
}

pub(crate) struct ConfigDirectoryWatch {
    _monitor: gio::FileMonitor,
}

impl ConfigDirectoryWatch {
    pub(crate) fn install(
        path: &Path,
        on_quiet_change: impl Fn() + 'static,
    ) -> Result<Self, String> {
        let parent = path.parent().ok_or_else(|| {
            format!(
                "configuration path has no parent directory: {}",
                path.display()
            )
        })?;
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "could not create configuration directory {}: {error}",
                parent.display()
            )
        })?;
        let monitor = gio::File::for_path(parent)
            .monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
            .map_err(|error| {
                format!(
                    "could not watch configuration directory {}: {error}",
                    parent.display()
                )
            })?;
        let watched_name = path.file_name().map(PathBuf::from);
        let debounce = std::rc::Rc::new(std::cell::RefCell::new(None::<gtk::glib::SourceId>));
        let callback = std::rc::Rc::new(on_quiet_change);
        monitor.connect_changed(move |_, file, other_file, _| {
            let relevant = watched_name.as_ref().is_some_and(|name| {
                file.basename().as_ref() == Some(name)
                    || other_file.and_then(gio::File::basename).as_ref() == Some(name)
            });
            if !relevant {
                return;
            }
            if let Some(source) = debounce.borrow_mut().take() {
                source.remove();
            }
            let callback = std::rc::Rc::clone(&callback);
            let debounce_after = std::rc::Rc::clone(&debounce);
            let source = gtk::glib::timeout_add_local_once(
                std::time::Duration::from_millis(RELOAD_QUIET_PERIOD_MILLIS),
                move || {
                    debounce_after.borrow_mut().take();
                    callback();
                },
            );
            *debounce.borrow_mut() = Some(source);
        });
        Ok(Self { _monitor: monitor })
    }
}

#[cfg(test)]
mod tests {
    use super::{ConfigDirectoryWatch, ConfigReloadAuthority, ReloadDecision};
    use crate::config_store::ConfigSnapshot;
    use std::cell::Cell;
    use std::fs;
    use std::rc::Rc;
    use std::time::Duration;
    use std::time::{SystemTime, UNIX_EPOCH};
    use zentty_core::AppConfig;

    fn private_path(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!(
                "zentty-config-reload-{name}-{}-{nonce}",
                std::process::id()
            ))
            .join("config.toml")
    }

    fn authority(path: std::path::PathBuf) -> ConfigReloadAuthority {
        ConfigReloadAuthority::new(&ConfigSnapshot {
            config: AppConfig::default(),
            path,
            warning: None,
        })
    }

    #[test]
    fn valid_change_applies_once_and_self_write_is_unchanged() {
        let path = private_path("valid");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "[clipboard]\nalways_clean_copies = true\n").unwrap();
        let mut authority = authority(path.clone());
        let ReloadDecision::Apply(config) = authority.observe_disk() else {
            panic!("valid change was not applied");
        };
        assert!(config.clipboard.always_clean_copies);
        authority.accept(&config);
        assert_eq!(authority.observe_disk(), ReloadDecision::Unchanged);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn invalid_and_missing_files_retain_last_good() {
        let path = private_path("invalid");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "[clipboard\nsecret-token = 'do-not-log'\n").unwrap();
        let mut authority = authority(path.clone());
        let ReloadDecision::RetainLastGood(diagnostic) = authority.observe_disk() else {
            panic!("invalid input did not retain last good");
        };
        assert!(diagnostic.contains("retaining last-good"));
        assert!(!diagnostic.contains("secret-token"));
        fs::remove_file(&path).unwrap();
        assert!(matches!(
            authority.observe_disk(),
            ReloadDecision::RetainLastGood(message) if message.contains("disappeared")
        ));
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn directory_watch_ignores_other_files_and_observes_atomic_replacement() {
        let path = private_path("watch");
        let parent = path.parent().unwrap();
        fs::create_dir_all(parent).unwrap();
        let notifications = Rc::new(Cell::new(0_u32));
        let callback_notifications = Rc::clone(&notifications);
        let _watch = ConfigDirectoryWatch::install(&path, move || {
            callback_notifications.set(callback_notifications.get() + 1);
        })
        .unwrap();

        let unrelated = parent.join("unrelated.tmp");
        fs::write(&unrelated, "unrelated").unwrap();
        fs::rename(&unrelated, parent.join("unrelated.done")).unwrap();
        iterate_default_context_for(Duration::from_millis(250));
        assert_eq!(notifications.get(), 0);

        let replacement = parent.join("config.external");
        fs::write(&replacement, "[clipboard]\nalways_clean_copies = true\n").unwrap();
        fs::rename(replacement, &path).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while notifications.get() == 0 && std::time::Instant::now() < deadline {
            iterate_default_context_for(Duration::from_millis(10));
        }
        assert_eq!(notifications.get(), 1);
        fs::remove_dir_all(parent).unwrap();
    }

    fn iterate_default_context_for(duration: Duration) {
        let context = gtk::glib::MainContext::default();
        let deadline = std::time::Instant::now() + duration;
        while std::time::Instant::now() < deadline {
            while context.pending() {
                context.iteration(false);
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
