use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};

use gtk::gio;
use gtk::gio::prelude::{DBusProxyExt, DBusProxyExtManual};
use gtk::glib;
use gtk::glib::variant::ToVariant;
use zentty_core::{AttentionItem, AttentionTarget, NotificationsConfig};

use crate::custom_sound_store::{APLAY, CustomSoundStore};

const SERVICE: &str = "org.freedesktop.Notifications";
const OBJECT_PATH: &str = "/org/freedesktop/Notifications";
const INTERFACE: &str = "org.freedesktop.Notifications";
const CALL_TIMEOUT_MS: i32 = 2_000;

pub(crate) const SOUND_CHOICES: &[(&str, &str)] = &[
    ("", "Default"),
    ("message-new-instant", "Message"),
    ("complete", "Complete"),
    ("dialog-warning", "Warning"),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SettingsLauncher {
    Gnome,
    Kde,
    Unsupported,
}

pub(crate) struct NotificationService;

enum DesktopSignal {
    ActionInvoked { id: u32, action: String },
    Closed { id: u32 },
}

pub(crate) struct AttentionNotificationService {
    proxy: Option<gio::DBusProxy>,
    targets: HashMap<u32, AttentionTarget>,
    signals: Receiver<DesktopSignal>,
}

impl AttentionNotificationService {
    pub(crate) fn new() -> Self {
        let proxy = notification_proxy().ok();
        let (sender, signals) = mpsc::channel();
        if let Some(proxy) = &proxy {
            proxy.connect_g_signal(move |_, _, signal_name, parameters| {
                let signal = match signal_name {
                    "ActionInvoked" => parameters
                        .get::<(u32, String)>()
                        .map(|(id, action)| DesktopSignal::ActionInvoked { id, action }),
                    "NotificationClosed" => parameters
                        .get::<(u32, u32)>()
                        .map(|(id, _)| DesktopSignal::Closed { id }),
                    _ => None,
                };
                if let Some(signal) = signal {
                    let _ = sender.send(signal);
                }
            });
        }
        Self {
            proxy,
            targets: HashMap::new(),
            signals,
        }
    }

    pub(crate) fn send_attention(
        &mut self,
        item: &AttentionItem,
        config: &NotificationsConfig,
    ) -> Result<u32, String> {
        let proxy = self
            .proxy
            .as_ref()
            .ok_or_else(|| "could not connect to desktop notifications".to_owned())?;
        let title = format!(
            "{} {}",
            item.agent_name,
            item.status_text.to_ascii_lowercase()
        );
        let body = match item.location_text.as_deref() {
            Some(location) => format!("{location} — {}", item.primary_text),
            None => item.primary_text.clone(),
        };
        let actions = vec!["default".to_owned(), "Jump to Pane".to_owned()];
        let id = send_with_proxy(proxy, &title, &body, &actions, config, false)?;
        self.targets.insert(id, item.target.clone());
        Ok(id)
    }

    pub(crate) fn drain_activations(&mut self) -> Vec<AttentionTarget> {
        let mut targets = Vec::new();
        while let Ok(signal) = self.signals.try_recv() {
            match signal {
                DesktopSignal::ActionInvoked { id, action } => {
                    if (action == "default" || action == "jump")
                        && let Some(target) = self.targets.get(&id).cloned()
                    {
                        targets.push(target);
                    }
                }
                DesktopSignal::Closed { id } => {
                    self.targets.remove(&id);
                }
            }
        }
        targets
    }
}

impl NotificationService {
    pub(crate) fn is_available() -> bool {
        notification_proxy().is_ok_and(|proxy| proxy.name_owner().is_some())
    }

    pub(crate) fn send(
        title: &str,
        body: &str,
        config: &NotificationsConfig,
    ) -> Result<u32, String> {
        let proxy = notification_proxy()?;
        send_with_proxy(&proxy, title, body, &[], config, false)
    }

    pub(crate) fn send_pane(
        title: &str,
        body: &str,
        config: &NotificationsConfig,
        silent: bool,
    ) -> Result<u32, String> {
        let proxy = notification_proxy()?;
        send_with_proxy(&proxy, title, body, &[], config, silent)
    }

    pub(crate) fn preview_sound(config: &NotificationsConfig) -> Result<(), String> {
        if CustomSoundStore::is_custom_name(&config.sound_name) {
            let path = CustomSoundStore::path_for_name(&config.sound_name)?;
            let device = std::env::var("ZENTTY_AUDIO_DEVICE").unwrap_or_else(|_| "default".into());
            let mut child = Command::new(APLAY)
                .args(["-q", "-D", &device])
                .arg(&path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|error| format!("could not start custom sound preview: {error}"))?;
            let status = child
                .wait()
                .map_err(|error| format!("could not finish custom sound preview: {error}"))?;
            return if status.success() {
                eprintln!(
                    "zentty-linux: custom-sound playback=aplay result=played file={} device={device:?}",
                    path.display()
                );
                Ok(())
            } else {
                Err(format!("custom sound preview exited with {status}"))
            };
        }
        let sound = if config.sound_name.is_empty() {
            "message-new-instant"
        } else {
            &config.sound_name
        };
        let mut child = Command::new("canberra-gtk-play")
            .args(["-i", sound, "-d", "Zentty notification preview"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("could not start sound preview: {error}"))?;
        let status = child
            .wait()
            .map_err(|error| format!("could not finish sound preview: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("sound preview exited with {status}"))
        }
    }

    pub(crate) fn open_settings() -> Result<(), String> {
        let launcher = settings_launcher(std::env::var("XDG_CURRENT_DESKTOP").ok().as_deref());
        let (program, arguments): (&str, &[&str]) = match launcher {
            SettingsLauncher::Gnome => ("gnome-control-center", &["notifications"]),
            SettingsLauncher::Kde => ("systemsettings", &["kcm_notifications"]),
            SettingsLauncher::Unsupported => {
                return Err(
                    "this desktop does not expose a known notification settings page".into(),
                );
            }
        };
        Command::new(program)
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("could not open notification settings: {error}"))
    }
}

fn send_with_proxy(
    proxy: &gio::DBusProxy,
    title: &str,
    body: &str,
    actions: &[String],
    config: &NotificationsConfig,
    silent: bool,
) -> Result<u32, String> {
    if proxy.name_owner().is_none() {
        return Err("no freedesktop notification service is available".into());
    }
    let mut hints = HashMap::<String, glib::Variant>::new();
    if silent {
        hints.insert("suppress-sound".into(), true.to_variant());
    } else if CustomSoundStore::is_custom_name(&config.sound_name) {
        let path = CustomSoundStore::path_for_name(&config.sound_name)?;
        let path = path
            .to_str()
            .ok_or_else(|| "custom sound path is not valid UTF-8".to_owned())?;
        hints.insert("sound-file".into(), path.to_variant());
    } else if !config.sound_name.is_empty() {
        hints.insert("sound-name".into(), config.sound_name.to_variant());
    }
    let parameters = (
        "Zentty",
        0_u32,
        "",
        title,
        body,
        actions.to_vec(),
        hints,
        -1_i32,
    )
        .to_variant();
    let response = proxy
        .call_sync(
            "Notify",
            Some(&parameters),
            gio::DBusCallFlags::NONE,
            CALL_TIMEOUT_MS,
            gio::Cancellable::NONE,
        )
        .map_err(|error| format!("desktop notification failed: {error}"))?;
    response
        .get::<(u32,)>()
        .map(|(id,)| id)
        .ok_or_else(|| "desktop notification service returned an invalid reply".into())
}

fn notification_proxy() -> Result<gio::DBusProxy, String> {
    gio::DBusProxy::for_bus_sync(
        gio::BusType::Session,
        gio::DBusProxyFlags::DO_NOT_AUTO_START,
        None,
        SERVICE,
        OBJECT_PATH,
        INTERFACE,
        gio::Cancellable::NONE,
    )
    .map_err(|error| format!("could not connect to desktop notifications: {error}"))
}

fn settings_launcher(desktop: Option<&str>) -> SettingsLauncher {
    let desktop = desktop.unwrap_or_default().to_ascii_lowercase();
    if desktop.split(':').any(|part| part.contains("gnome")) {
        SettingsLauncher::Gnome
    } else if desktop
        .split(':')
        .any(|part| part.contains("kde") || part.contains("plasma"))
    {
        SettingsLauncher::Kde
    } else {
        SettingsLauncher::Unsupported
    }
}

#[cfg(test)]
mod tests {
    use super::{SettingsLauncher, settings_launcher};

    #[test]
    fn settings_launcher_is_desktop_specific_and_never_guesses() {
        assert_eq!(
            settings_launcher(Some("ubuntu:GNOME")),
            SettingsLauncher::Gnome
        );
        assert_eq!(settings_launcher(Some("KDE")), SettingsLauncher::Kde);
        assert_eq!(settings_launcher(Some("plasma")), SettingsLauncher::Kde);
        assert_eq!(
            settings_launcher(Some("sway")),
            SettingsLauncher::Unsupported
        );
        assert_eq!(settings_launcher(None), SettingsLauncher::Unsupported);
    }
}
