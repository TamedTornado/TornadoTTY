# Linux status-notifier fleet completion plan

Tracking: GH-21, `desktop.fleet-status`

## Source contract

Zentty's macOS `MenuBarStatusController` is an optional process-wide projection
of the same fleet model used in the application. It starts only when enabled,
tracks every live window, exposes the aggregate state, opens a grouped fleet
menu, routes a pane selection through the canonical exact-pane handler, and
includes Settings and Quit. It is not a second agent registry or reducer.

Linux preserves that outcome with the freedesktop StatusNotifierItem protocol
when a real watcher/host is present. The existing in-window GTK fleet remains
the universal fallback and must not disappear when the desktop has no watcher
(including stock GNOME without an extension).

## Design boundaries

- `ApplicationCoordinator` remains the sole process-wide fleet aggregator.
- A focused `status_notifier` module owns only D-Bus capability discovery,
  object publication, watcher registration, aggregate presentation, activation,
  and teardown.
- The item receives the already-built `FleetPaneSnapshot` vector. It may not
  discover panes, consume agent events, or maintain a second status model.
- `Activate` opens the existing in-window fleet. Pane routing remains the typed
  `ApplicationAction::ActivateFleetPane` path.
- The Agents setting controls publication. Absence or loss of a watcher is an
  explicit unavailable state, never a pass and never a reason to remove the
  in-window control.
- The implementation uses GTK/GIO's existing D-Bus support; no new runtime or
  package dependency is introduced.

## Tests-first order

1. Add focused tests for capability/presentation resolution, disabled and
   unavailable behavior, exact aggregate status, watcher loss, and idempotent
   updates.
2. Add a controlled StatusNotifierWatcher/Host executable to the existing test
   support crate. It must own a private session-bus name, accept a real item
   registration, inspect the exported properties, invoke real D-Bus methods,
   observe update signals, and record item disappearance.
3. Extend the existing real multi-window fleet journey rather than creating a
   second product actor. Under controlled X11 and input-capable Wayland it must
   prove registration, aggregate state changes, D-Bus activation opening the
   real GTK fleet, Settings and Quit actions, disabled/unavailable fallback,
   and shutdown cleanup.
4. Add a direct malformed typed-target action test at the existing action
   presentation boundary and prove no panic, focus change, or stale row.
5. Mutation-test the new state transitions and update the authoritative matrix,
   inventory, architecture contract, and dogfood record.

## Acceptance

- A real compliant watcher observes exactly one Zentty item when enabled.
- Required item properties and state transitions reflect the canonical fleet.
- D-Bus activation opens the same accessible in-window fleet UI.
- Settings and Quit use standard product actions and their confirmation policy.
- Disabled setting, missing watcher, watcher loss, and app shutdown are explicit
  and leak no bus object or stale registration.
- Malformed/stale targets are rejected by the canonical action boundary.
- X11 and Wayland controlled journeys pass, except for the already-declared
  exact cross-toplevel activation-token XFAIL.
- `desktop.fleet-status` becomes `IMPLEMENTED` only after all presently
  executable qualification cells pass.
