use std::collections::HashMap;
use std::process::{Command, Stdio};

use gtk::gio;
use gtk::gio::prelude::DBusProxyExt;
use gtk::glib;
use gtk::glib::variant::ToVariant;
use zentty_core::NotificationsConfig;

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
        if proxy.name_owner().is_none() {
            return Err("no freedesktop notification service is available".into());
        }
        let actions = Vec::<String>::new();
        let mut hints = HashMap::<String, glib::Variant>::new();
        if !config.sound_name.is_empty() {
            hints.insert("sound-name".into(), config.sound_name.to_variant());
        }
        let parameters = ("Zentty", 0_u32, "", title, body, actions, hints, -1_i32).to_variant();
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

    pub(crate) fn preview_sound(config: &NotificationsConfig) -> Result<(), String> {
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
