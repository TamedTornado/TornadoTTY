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
The authoritative matrix still owns compositor gaps. ADR 0005 removed CI-host
status from product qualification because CI is an advisory check.

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

## GH-21 cross-window Wayland activation closure

### Discovery and failed approaches

1. The remaining `agent-fleet-wayland` XFAIL selected the correct typed fleet
   target and updated the destination model, but subsequent physical input
   stayed in the source toplevel. The exact pre-repair Cage journey exited 98.
2. Preserving `EventController.current_event_time()` and calling GTK's
   timestamped presentation API did not repair Cage. Instrumentation then
   established that every `wtype`/virtual-keyboard event in this environment
   carried `GDK_CURRENT_TIME` (`0`), so the harness could not supply the
   compositor authorization precondition it claimed to test.
3. The existing nested Weston/X11-seat mode produced real nonzero event times,
   but `wayland-info` proved that the installed Weston package, like Cage,
   does not advertise `xdg_activation_v1`. A timestamp alone cannot authorize
   cross-toplevel focus on Wayland, and protocol absence was not converted into
   a pass.
4. GTK 4.14's deprecated `present_with_time` path was removed from the repair.
   Direct `GdkToplevel::focus(timestamp)` is retained only as the portable
   timestamp fallback; it did not fabricate success on either compositor.
5. GDK's app-launch context initially returned no startup identifier when
   called without `GAppInfo`. That nullable form is newer than this machine's
   GLib 2.80 baseline. Supplying a bounded startup-notification-capable Zentty
   `GAppInfo` makes the same standard GDK path return the compositor credential
   where the protocol exists.

### Repair

- A capture-phase tracker is installed before action and shortcut controllers.
  It observes real key and pointer events without consuming them and exposes a
  single-dispatch, single-use event time; zero is explicitly ignored, the idle
  boundary expires an unused event, and a consumed credential cannot leak into
  a later non-user action.
- The source fleet action synchronously asks GDK for a startup notification ID
  using that event time. On Wayland this is an XDG activation token. The typed
  application action carries the opaque credential to the already-authoritative
  destination shell, which sets it before presenting and focusing the exact
  pane. No Ghostty change and no second routing system were added.
- The controlled input harness now supports labwc 0.7.1 on its existing private
  Xvfb transport with the Pixman renderer. The profile refuses startup unless
  `xdg_activation_v1` is actually advertised, records the real compositor and
  protocol inventory, and sends timestamped input through the nested X11 seat.
- A focused mode was added to the existing `rust-multi-window` actor rather than
  creating a second product actor. It constructs two real Zentty windows and
  Ghostty PTYs, moves a live pane, publishes two authenticated agent states,
  invokes the typed fleet route from the real command palette, requires both a
  nonzero event time and generated startup ID, and types into the exact remote
  PTY after the compositor transfer.

### Focused evidence

- Pre-repair Cage reproduction: exact exit 98; target selection passed and
  compositor focus transfer failed.
- Cage protocol/timestamp characterization: no `xdg_activation_v1`; virtual
  keyboard GTK event times were all zero.
- Weston characterization: real nonzero timestamps, but no
  `xdg_activation_v1`; exact destination input still failed.
- Private labwc characterization: real `wl_seat`, `xdg_activation_v1`, Pixman,
  isolated Xvfb transport, and machine-readable cleanup receipt: PASS.
- Private labwc two-window/PTY fleet activation: **PASS Wayland
  event-authorized fleet activation**.
- Full controlled X11 fleet and StatusNotifierItem lifecycle after the routing
  change: PASS.
- `UserActivationClock` single-use/zero rejection, product-input explicit
  nested-window targeting, all three compositor harness modes, matrix runner
  self-tests, and the ApplicationShell ownership contract: PASS.
- Copy-safe targeted mutation run (`gitignore=true`, `copy_target=false`): all
  8 activation-clock mutants caught; 0 missed, timed out, or unviable. An
  earlier broad-baseline attempt was rejected because the unrelated real
  `/proc` listener test could not bind inside one mutation sandbox; it was not
  counted as mutation evidence.

The matrix cell is now PASS under `nested-wayland-activation-v1`. Cage remains
useful for virtual-keyboard behavior and Weston remains useful for nested
multi-window drag behavior; neither is misrepresented as activation-capable.
Final aggregate totals and receipt hashes are recorded below after the required
full qualification rerun.

### Final GH-21 qualification receipt

- `linux/tests/qualify-local`: all presently executable support and matrix
  cells passed in 562,510ms.
- Declared totals: **132 PASS, 0 FAIL, 7 BLOCKED, 1 XFAIL, and 22
  NOT_IMPLEMENTED**. The GH-21 Wayland fleet row accounts for the PASS increase
  and stale-XFAIL removal.
- Machine summary SHA-256:
  `33d62849162a241b223fc7b437d25970a2985e0f33a6127dbb8f935b8e847825`.
- Implemented local suite and product-boundary qualification: PASSED. Release
  and full Linux qualification: NOT PASSED because the unrelated declared
  BLOCKED, XFAIL, and NOT_IMPLEMENTED cells remain.
- Debug Valgrind is **PASS with reviewed suppressions**, never an unsuppressed
  clean claim. The preserved raw receipt reports 427 errors/contexts, 6,080
  definite bytes, and 41,363 indirect bytes. Post-suppression totals are zero
  errors/contexts and zero definite/indirect bytes, with all 427 reviewed
  contexts explicitly accounted for. Suppression governance passed and
  ReleaseSafe Valgrind remains unchanged.
