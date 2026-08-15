# Zentty Linux dogfood — 2026-08-15

This record starts a new dogfood volume rather than extending the already large
August 14 report. It records the completion audit for issue 21's attention,
notification, fleet, and sleep-inhibition feature.

## Source contract recovered

- The macOS `NotificationStore` is a process-wide history, not a direct mirror
  of the current sidebar badge. It retains ready and unresolved-stop history,
  caps history at 50 items, supports dismiss/clear/stale cleanup, and orders the
  newest item first.
- Human-input requests use a three-second debounce. If the request resolves or
  the pane becomes actively viewed before that deadline, no stale notification
  may escape.
- Ready and unresolved-stop entries are immediate. The active pane still gets
  local history, but desktop delivery is suppressed. Running/starting states do
  not create inbox entries.
- A desktop or inbox action routes to the exact window, worklane, and pane. A
  successful jump does not itself claim that unresolved agent attention has
  been answered.

## Implementation and ownership decisions

- `zentty-core::AttentionInbox` is the only attention policy reducer. It owns
  signatures, debounce, ordering, history, resolution, stale pruning, and a
  delivery queue. GTK presentation and freedesktop delivery consume its typed
  output; they do not implement competing policy.
- The existing application action channel remains the sole exact-target routing
  authority. Desktop actions retain an `AttentionTarget` rather than guessing
  from labels or the currently active window.
- `AttentionNotificationService` reuses the existing freedesktop notification
  service boundary. The new persistent instance listens to real
  `ActionInvoked` and `NotificationClosed` signals; no second notification
  subsystem was introduced.
- Product-specific orchestration and the controlled D-Bus fixture remain in
  Zentty. No Ghostty change was required for this slice.

## Discoveries, failures, and repairs

1. The old Linux inbox accepted only immediate needs-input events. It had no
   source debounce, ready/unresolved-stop history, active-pane desktop policy,
   real agent desktop delivery, or desktop action callback. Those omissions are
   now implemented through the shared reducer and service.
2. The first controlled X11 invocation had no private session bus. This was a
   harness invocation defect, not environmental success. Both matrix commands
   now explicitly use `dbus-run-session`, matching the existing notification
   settings cells.
3. Xvfb intentionally runs without a window manager. GTK selected the exact
   cross-window target, but `GtkWindow.present` could not raise it for subsequent
   pointer input. The X11 harness first proves the typed application route, then
   explicitly raises/focuses the selected test window before a real terminal
   click and keystroke. This is documented harness accommodation, not product
   behavior.
4. An initial 0.8-second controlled-agent delay emitted attention before the
   second Wayland window's focus settled. The correct source focus transition
   then canceled the pending item. The fixture delay is now four seconds so the
   test controls its precondition; product policy was not weakened.
5. Early expectations used guessed identities and labels. The controlled second
   worklane is `worklane-window-2`; a prompt with choices is an approval; and
   its desktop title is `Codex needs approval`. Assertions now use the actual
   protocol and product identities.
6. A generic log substring accidentally matched `desktop-attention-activate`
   when the test intended the in-app `attention-activate` route. The assertion
   now includes the complete log prefix and cannot pass on the wrong action.
7. Ready history can legitimately leave two unresolved history entries on
   Wayland: the focused pane's prior request is resolved, then an immediate ready
   history item is added while desktop delivery is suppressed. The test had
   incorrectly required `unresolved=1`, even though the product logged the
   source-compatible third item. It now asserts creation of the ready history
   without inventing a resolution policy.
8. Final reducer review found a narrow queue race: clear, dismiss, resolution,
   or stale pruning could occur after an item was queued but before the 10ms
   coordinator drain. Those operations now remove matching queued deliveries,
   preventing a stale desktop alert after the user or lifecycle already removed
   it.
9. The first mutation run exposed 22 missed cases, principally cross-window
   stale scoping, exact retained identities, interaction fallbacks, and boolean
   aggregation equivalents. Tests were strengthened and the aggregation code
   simplified. The final targeted result is 83 mutants: 76 caught, 7 unviable,
   0 missed, and 0 timed out.
10. The first full qualification run correctly rejected the feature promotion
    because two governance receipts were stale: the inventory runner still
    expected 13 implemented/26 partial entries, and the shell-ownership contract
    did not list the notification service's new persistent constructor, action
    drain, and shared send helper. The product matrix itself also caught that
    ownership drift. Both contracts now encode the reviewed architecture and the
    inventory test explicitly pins the new attention evidence.

## Real-system evidence

- Focused reducer suite: 11 passed, 0 failed.
- Controlled X11 journey: two real Zentty windows, two real Ghostty PTYs, real
  authenticated Codex events, real GTK inbox row, private freedesktop D-Bus
  service, cross-window desktop action, and physical input at the exact target:
  PASS.
- Controlled input-capable Wayland journey: the same real windows, PTYs, IPC,
  private D-Bus service, source shortcut, cross-window action, focused desktop
  suppression, completion history, and physical target input: PASS.
- Desktop dismissal is verified by closing the real fixture notification,
  emitting a second action for its service ID, and proving no application route
  occurs.
- The existing notification-settings journey continues to own real sound-theme,
  custom-audio, silent/disabled/invalid, and desktop-settings coverage. The
  existing fleet and inhibitor journeys continue to own their lifecycle states.

## Environmental limitation

The nested Wayland run repeatedly activates Ubuntu's
`xdg-desktop-portal-gnome`, which exits with SIGSEGV in this synthetic nested
session. `xdg-desktop-portal-gtk` supplies the fallback and the controlled
product journey passes. This is external nested-environment evidence, not an
unsuppressed product-clean claim and not a reason to convert absence into PASS.
The authoritative matrix still owns compositor and controlled-public-CI gaps.

## Privacy

Agent question text and context stay on the local machine and are delivered to
the user's session notification service over D-Bus. This slice adds no telemetry
or remote approval path. Whether a desktop exposes notification contents on a
lock screen or through remote-desktop software remains desktop policy; Zentty
does not bypass or override it.

## Final qualification receipt

- Full `linux/tests/qualify-local` rerun after the governance repair: all
  presently executable support and matrix cells passed.
- Declared matrix totals: 131 PASS, 0 FAIL, 7 BLOCKED, 2 XFAIL, and 22
  NOT_IMPLEMENTED.
- This is an **implemented local suite pass**, not release qualification and not
  full Linux qualification. The repository correctly refuses those stronger
  claims while BLOCKED, XFAIL, or NOT_IMPLEMENTED cells remain.
- Valgrind status remains **PASS with reviewed suppressions**. ReleaseSafe
  Valgrind is not broadened or relabeled; the authoritative matrix retains its
  reviewed non-PASS boundary.
