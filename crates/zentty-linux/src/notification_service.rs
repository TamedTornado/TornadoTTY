use std::collections::{HashMap, VecDeque};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};

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
const MAX_ACTIVATION_TOKEN_BYTES: usize = 4 * 1024;
const MAX_TRACKED_DESKTOP_NOTIFICATIONS: usize = 128;
const MAX_PENDING_DESKTOP_SIGNALS: usize = 256;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DesktopAttentionDecision {
    Deliver,
    SuppressActivelyViewed,
    SuppressVisiblePane,
}

pub(crate) fn desktop_attention_decision(
    desktop_allowed: bool,
    notify_when_pane_visible: bool,
    pane_visible: bool,
) -> DesktopAttentionDecision {
    if !desktop_allowed {
        DesktopAttentionDecision::SuppressActivelyViewed
    } else if pane_visible && !notify_when_pane_visible {
        DesktopAttentionDecision::SuppressVisiblePane
    } else {
        DesktopAttentionDecision::Deliver
    }
}

enum DesktopSignal {
    ActivationToken { id: u32, token: String },
    ActionInvoked { id: u32, action: String },
    Closed { id: u32 },
}

struct DesktopNotificationEntry {
    id: u32,
    target: AttentionTarget,
    activation_token: Option<String>,
}

#[derive(Default)]
struct DesktopNotificationRegistry {
    entries: VecDeque<DesktopNotificationEntry>,
    evicted_total: u64,
}

impl DesktopNotificationRegistry {
    fn track(&mut self, id: u32, target: AttentionTarget) -> Option<(u32, u64)> {
        if let Some(position) = self.entries.iter().position(|entry| entry.id == id) {
            self.entries.remove(position);
        }
        self.entries.push_back(DesktopNotificationEntry {
            id,
            target,
            activation_token: None,
        });
        if self.entries.len() <= MAX_TRACKED_DESKTOP_NOTIFICATIONS {
            return None;
        }
        let evicted = self
            .entries
            .pop_front()
            .expect("capacity overflow requires an oldest notification");
        self.evicted_total = self.evicted_total.saturating_add(1);
        Some((evicted.id, self.evicted_total))
    }

    #[cfg(test)]
    fn contains(&self, id: u32) -> bool {
        self.entries.iter().any(|entry| entry.id == id)
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    fn activation_token(&self, id: u32) -> Option<&str> {
        self.entries
            .iter()
            .find(|entry| entry.id == id)?
            .activation_token
            .as_deref()
    }

    fn store_activation_token(&mut self, id: u32, token: String) -> bool {
        let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == id) else {
            return false;
        };
        entry.activation_token = Some(token);
        true
    }

    fn take(&mut self, id: u32) -> Option<DesktopNotificationEntry> {
        let position = self.entries.iter().position(|entry| entry.id == id)?;
        self.entries.remove(position)
    }

    fn remove(&mut self, id: u32) -> bool {
        self.take(id).is_some()
    }
}

fn enqueue_desktop_signal(
    sender: &SyncSender<DesktopSignal>,
    dropped_total: &AtomicU64,
    signal: DesktopSignal,
) -> bool {
    match sender.try_send(signal) {
        Ok(()) => true,
        Err(TrySendError::Full(_)) => {
            dropped_total.fetch_add(1, Ordering::Relaxed);
            false
        }
        Err(TrySendError::Disconnected(_)) => false,
    }
}

pub(crate) struct DesktopAttentionActivation {
    pub(crate) target: AttentionTarget,
    pub(crate) service_id: u32,
    pub(crate) action: String,
    pub(crate) activation_token: Option<String>,
}

pub(crate) struct AttentionNotificationService {
    proxy: Option<gio::DBusProxy>,
    registry: DesktopNotificationRegistry,
    signals: Receiver<DesktopSignal>,
    dropped_signals: Arc<AtomicU64>,
    next_drop_report_at: u64,
}

impl AttentionNotificationService {
    pub(crate) fn new() -> Self {
        let proxy = notification_proxy().ok();
        let (sender, signals) = mpsc::sync_channel(MAX_PENDING_DESKTOP_SIGNALS);
        let dropped_signals = Arc::new(AtomicU64::new(0));
        if let Some(proxy) = &proxy {
            let dropped_signals = Arc::clone(&dropped_signals);
            proxy.connect_g_signal(move |_, _, signal_name, parameters| {
                let signal = match signal_name {
                    "ActivationToken" => parameters
                        .get::<(u32, String)>()
                        .map(|(id, token)| DesktopSignal::ActivationToken { id, token }),
                    "ActionInvoked" => parameters
                        .get::<(u32, String)>()
                        .map(|(id, action)| DesktopSignal::ActionInvoked { id, action }),
                    "NotificationClosed" => parameters
                        .get::<(u32, u32)>()
                        .map(|(id, _)| DesktopSignal::Closed { id }),
                    _ => None,
                };
                if let Some(signal) = signal {
                    enqueue_desktop_signal(&sender, &dropped_signals, signal);
                }
            });
        }
        Self {
            proxy,
            registry: DesktopNotificationRegistry::default(),
            signals,
            dropped_signals,
            next_drop_report_at: 1,
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
        if let Some((evicted_id, evicted_total)) = self.registry.track(id, item.target.clone())
            && evicted_total.is_power_of_two()
        {
            eprintln!(
                "zentty-linux: desktop-attention-routing result=evicted service-id={evicted_id} capacity={MAX_TRACKED_DESKTOP_NOTIFICATIONS} evicted-total={evicted_total}"
            );
        }
        Ok(id)
    }

    pub(crate) fn drain_activations(&mut self) -> Vec<DesktopAttentionActivation> {
        let mut activations = Vec::new();
        while let Ok(signal) = self.signals.try_recv() {
            if let Some(activation) = apply_desktop_signal(&mut self.registry, signal) {
                activations.push(activation);
            }
        }
        let dropped_total = self.dropped_signals.load(Ordering::Relaxed);
        if dropped_total >= self.next_drop_report_at {
            eprintln!(
                "zentty-linux: desktop-attention-signals result=dropped capacity={MAX_PENDING_DESKTOP_SIGNALS} dropped-total={dropped_total}"
            );
            self.next_drop_report_at = dropped_total
                .checked_next_power_of_two()
                .unwrap_or(u64::MAX)
                .max(dropped_total.saturating_add(1));
        }
        activations
    }
}

fn apply_desktop_signal(
    registry: &mut DesktopNotificationRegistry,
    signal: DesktopSignal,
) -> Option<DesktopAttentionActivation> {
    match signal {
        DesktopSignal::ActivationToken { id, token } => {
            if !token.is_empty()
                && token.len() <= MAX_ACTIVATION_TOKEN_BYTES
                && !token.contains('\0')
                && registry.store_activation_token(id, token)
            {
                eprintln!(
                    "zentty-linux: desktop-attention-signal service-id={id} kind=activation-token result=stored"
                );
            } else {
                eprintln!(
                    "zentty-linux: desktop-attention-signal service-id={id} kind=activation-token result=ignored"
                );
            }
            None
        }
        DesktopSignal::ActionInvoked { id, action } => {
            let entry = registry.take(id);
            let action = match action.as_str() {
                "default" => "default",
                "jump" => "jump",
                _ => "unsupported",
            };
            eprintln!(
                "zentty-linux: desktop-attention-signal service-id={id} kind=action action={action} target={} credential={}",
                if entry.is_some() { "found" } else { "stale" },
                if entry
                    .as_ref()
                    .is_some_and(|entry| entry.activation_token.is_some())
                {
                    "token"
                } else {
                    "none"
                },
            );
            let entry = entry?;
            (action != "unsupported").then_some(DesktopAttentionActivation {
                target: entry.target,
                service_id: id,
                action: action.to_owned(),
                activation_token: entry.activation_token,
            })
        }
        DesktopSignal::Closed { id } => {
            registry.remove(id);
            eprintln!("zentty-linux: desktop-attention-signal service-id={id} kind=closed");
            None
        }
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
            .args(["-i", sound, "-d", "Tornado TTY notification preview"])
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
        zentty_core::PRODUCT_NAME,
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
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::mpsc;

    use zentty_core::AttentionTarget;

    use super::{
        DesktopAttentionDecision, DesktopNotificationRegistry, DesktopSignal,
        MAX_TRACKED_DESKTOP_NOTIFICATIONS, SettingsLauncher, apply_desktop_signal,
        desktop_attention_decision, enqueue_desktop_signal, settings_launcher,
    };

    #[test]
    fn visible_pane_delivery_policy_preserves_defaults_and_focused_suppression() {
        assert_eq!(
            desktop_attention_decision(false, true, true),
            DesktopAttentionDecision::SuppressActivelyViewed
        );
        assert_eq!(
            desktop_attention_decision(true, true, true),
            DesktopAttentionDecision::Deliver
        );
        assert_eq!(
            desktop_attention_decision(true, false, true),
            DesktopAttentionDecision::SuppressVisiblePane
        );
        assert_eq!(
            desktop_attention_decision(true, false, false),
            DesktopAttentionDecision::Deliver
        );
    }

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

    #[test]
    fn activation_token_is_single_use_and_bound_to_the_exact_notification() {
        let target = AttentionTarget::new("window-1", "worklane-1", "pane-1");
        let mut registry = DesktopNotificationRegistry::default();
        registry.track(7, target.clone());
        assert!(
            apply_desktop_signal(
                &mut registry,
                DesktopSignal::ActivationToken {
                    id: 7,
                    token: "valid-token".to_owned(),
                },
            )
            .is_none()
        );
        let activation = apply_desktop_signal(
            &mut registry,
            DesktopSignal::ActionInvoked {
                id: 7,
                action: "default".to_owned(),
            },
        )
        .expect("known default action must activate");
        assert_eq!(activation.target, target);
        assert_eq!(activation.activation_token.as_deref(), Some("valid-token"));
        assert_eq!(registry.len(), 0);
        assert!(
            apply_desktop_signal(
                &mut registry,
                DesktopSignal::ActionInvoked {
                    id: 7,
                    action: "default".to_owned(),
                },
            )
            .is_none()
        );
    }

    #[test]
    fn closed_stale_and_invalid_notification_signals_cannot_route() {
        let target = AttentionTarget::new("window-1", "worklane-1", "pane-1");
        let mut registry = DesktopNotificationRegistry::default();
        registry.track(9, target.clone());
        registry.track(10, target);
        assert!(
            apply_desktop_signal(
                &mut registry,
                DesktopSignal::ActivationToken {
                    id: 8,
                    token: "stale".to_owned(),
                },
            )
            .is_none()
        );
        assert!(registry.activation_token(8).is_none());
        assert!(
            apply_desktop_signal(
                &mut registry,
                DesktopSignal::ActionInvoked {
                    id: 10,
                    action: "unsupported".to_owned(),
                },
            )
            .is_none()
        );
        assert!(!registry.contains(10));
        assert!(
            apply_desktop_signal(
                &mut registry,
                DesktopSignal::ActivationToken {
                    id: 9,
                    token: "bad\0token".to_owned(),
                },
            )
            .is_none()
        );
        assert!(registry.activation_token(9).is_none());
        assert!(
            apply_desktop_signal(
                &mut registry,
                DesktopSignal::ActivationToken {
                    id: 9,
                    token: "x".repeat(super::MAX_ACTIVATION_TOKEN_BYTES + 1),
                },
            )
            .is_none()
        );
        assert!(registry.activation_token(9).is_none());
        assert!(apply_desktop_signal(&mut registry, DesktopSignal::Closed { id: 9 }).is_none());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn notification_routing_is_bounded_and_evicts_the_oldest_complete_record() {
        let mut registry = DesktopNotificationRegistry::default();
        for id in 1..=u32::try_from(MAX_TRACKED_DESKTOP_NOTIFICATIONS + 1).unwrap() {
            registry.track(
                id,
                AttentionTarget::new("window-1", "worklane-1", format!("pane-{id}")),
            );
        }

        assert_eq!(registry.len(), MAX_TRACKED_DESKTOP_NOTIFICATIONS);
        assert!(!registry.contains(1));
        assert!(registry.contains(2));
        assert!(
            apply_desktop_signal(
                &mut registry,
                DesktopSignal::ActionInvoked {
                    id: 1,
                    action: "default".to_owned(),
                },
            )
            .is_none()
        );
        let newest = apply_desktop_signal(
            &mut registry,
            DesktopSignal::ActionInvoked {
                id: u32::try_from(MAX_TRACKED_DESKTOP_NOTIFICATIONS + 1).unwrap(),
                action: "default".to_owned(),
            },
        )
        .expect("newest retained notification must route");
        assert_eq!(
            newest.target.pane_id,
            format!("pane-{}", MAX_TRACKED_DESKTOP_NOTIFICATIONS + 1)
        );
    }

    #[test]
    fn reused_service_id_replaces_target_and_discards_the_old_token() {
        let mut registry = DesktopNotificationRegistry::default();
        registry.track(
            7,
            AttentionTarget::new("window-1", "worklane-1", "old-pane"),
        );
        apply_desktop_signal(
            &mut registry,
            DesktopSignal::ActivationToken {
                id: 7,
                token: "old-token".to_owned(),
            },
        );

        registry.track(
            7,
            AttentionTarget::new("window-1", "worklane-2", "new-pane"),
        );
        let activation = apply_desktop_signal(
            &mut registry,
            DesktopSignal::ActionInvoked {
                id: 7,
                action: "jump".to_owned(),
            },
        )
        .expect("reused notification identity must route to its replacement");
        assert_eq!(activation.target.pane_id, "new-pane");
        assert_eq!(activation.activation_token, None);
    }

    #[test]
    fn desktop_signal_ingress_is_nonblocking_and_bounded() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let dropped = AtomicU64::new(0);
        assert!(enqueue_desktop_signal(
            &sender,
            &dropped,
            DesktopSignal::Closed { id: 1 },
        ));
        assert!(!enqueue_desktop_signal(
            &sender,
            &dropped,
            DesktopSignal::Closed { id: 2 },
        ));
        assert_eq!(dropped.load(Ordering::Relaxed), 1);
        assert!(matches!(
            receiver.try_recv().unwrap(),
            DesktopSignal::Closed { id: 1 }
        ));
    }
}
