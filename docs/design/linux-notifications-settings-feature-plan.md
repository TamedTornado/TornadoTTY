# Linux Notifications settings feature plan

Status: active implementation plan for GitHub issue #20
Date: 2026-08-12

## Source authority

The feature authority is
`Zentty/UI/Settings/NotificationsSettingsSectionViewController.swift`,
`Zentty/Config/AppConfig.swift`, `Zentty/Config/AppConfigTOML.swift`, and
`Zentty/AppState/WorklaneAttentionNotificationCoordinator.swift`. Linux must
preserve the two source rows:

1. Desktop Notifications: report platform availability, open the desktop's
   notification settings when supported, and send a real test notification.
2. Notification Sound: choose the desktop default or a named sound and preview
   the selected sound.

Linux does not have macOS's per-application `UNAuthorizationStatus` or its fixed
system-sound catalog. The explicit Linux alternative is the freedesktop desktop
notification service and freedesktop sound-theme names. “Available” means a
notification service owns `org.freedesktop.Notifications`; absence is reported
as unavailable and is never treated as success.

## Ownership and implementation order

1. Add source-compatible notification configuration to the existing
   `zentty_core::AppConfig`; do not create a page-local settings model.
2. Extend the one `ConfigStore` transaction boundary so notification writes
   preserve comments, unknown keys, unrelated tables, symlinks, permissions,
   and concurrent writers.
3. Add one focused Linux notification service module. It owns freedesktop
   availability, notification delivery, settings-launch alternatives, and sound
   preview. GTK widgets may not spawn platform commands directly.
4. Add one focused GTK Notifications page and insert it into the existing
   settings shell. Durable state flows through `ConfigStore` and
   `ApplicationShell` only.
5. Do not deliver raw Ghostty OSC events directly. The source reduces those
   events into pane attention and suppresses alerts for actively viewed panes.
   This settings slice provides the shared service boundary; issue #21 owns the
   later source-parity attention/inbox policy so there is never a second path.

## Test-first acceptance

1. Core tests pin defaults, exact TOML names, named-sound normalization, and
   invalid known values.
2. ConfigStore tests cover comments/unknowns/unrelated tables, symlink and mode
   preservation, malformed and oversize input, and serialized concurrent
   distinct updates.
3. Service policy tests pin command selection and reject unsafe/unavailable
   alternatives without spawning a shell.
4. A controlled private D-Bus journey drives the staged app through the real
   settings action path, invokes Send Test, and observes the freedesktop
   `Notify` call at the session-bus boundary. It also exercises unavailable
   service behavior; environmental absence is not a pass.
5. Existing real Ghostty terminal-notification integration must continue to
   prove sidebar agent reconciliation. The issue #21 journey must later add
   service delivery only after exercising the source attention reducer and
   active-pane suppression, rather than asserting delivery for every raw event.
6. Drive representative persistence and reload through private X11 and nested
   Wayland, then run strict format/lint/architecture gates and all presently
   executable qualification cells before commit.

## Scope boundary

This slice replaces only the Notifications placeholder. Custom audio import and
conversion require a separately specified freedesktop file-format/install
policy and remain visibly unavailable rather than being silently simulated.
The remaining Open With, Dev Servers, Pane Layout, Updates & Privacy, and Agents
pages remain later source-backed issue #20 slices.
