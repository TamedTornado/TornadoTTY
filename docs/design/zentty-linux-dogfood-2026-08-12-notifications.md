# Zentty Linux Notifications dogfood — 2026-08-12

This append-only record covers the Notifications settings slice of GitHub issue
#20. The implementation plan is
`docs/design/linux-notifications-settings-feature-plan.md`.

## Source audit

- The source page is not a generic enable/disable toggle. It has a desktop
  notification status row with Open Settings and Send Test, plus a notification
  sound row with system choices, preview, and custom audio import.
- macOS authorization states and its fixed named-sound catalog do not exist on
  Linux. The named alternative is the freedesktop notification service and
  freedesktop sound-theme names. The UI says Available only when
  `org.freedesktop.Notifications` has an owner.
- Custom audio import is not silently treated as complete. It remains the
  explicit `notification-custom-sound-import` NOT_IMPLEMENTED matrix cell under
  GH-20 pending a bounded conversion/install policy.

## Implementation discoveries and decisions

- Existing Ghostty OSC desktop notifications cross the embedding boundary and
  update agent/sidebar state. A first implementation also delivered every event
  immediately through the new service. Source review found that this was wrong:
  macOS first reduces terminal events into pane attention and suppresses system
  alerts for an actively viewed pane. The direct Linux delivery was removed
  before commit rather than creating a parallel notification policy. This slice
  owns settings, test delivery, and the desktop-service boundary; source-parity
  attention delivery remains separate tracked feature work.
- Direct freedesktop D-Bus delivery was selected over `notify-send`: the product
  does not depend on an optional command-line helper, and the sound-name hint is
  part of the exact method call. The proxy uses `DO_NOT_AUTO_START`, so absence
  is an observable error rather than unexpectedly launching a service.
- Sound preview invokes `canberra-gtk-play` with an argv vector and no shell.
  Missing tools and nonzero exits are errors. User-configured sound names never
  become executable text.
- GNOME and KDE settings launchers use exact argv vectors. Unknown desktops
  report that no known settings page exists rather than guessing a command.
- Notification persistence extends the one XDG `ConfigStore` transaction and
  the one `AppConfig`; no page-local store or second configuration layer was
  introduced.

## Harness discoveries, failures, and repairs

- **Failure:** the first qualification idea used a tiny project-owned D-Bus
  server. That duplicated an external component and weakened the real-system
  boundary. It was deleted before commit. Qualification installs and runs
  Ubuntu's real `notification-daemon` instead and observes its real method call
  with `dbus-monitor` on a private session bus.
- **Failure:** ordinary Tab injection initially kept targeting the main X11
  window because the shared helper deliberately pins main-window input.
  **Repair:** the focused harness uses the same explicit settings-toplevel focus
  boundary as the established settings journey, and the visible Send Test
  mnemonic provides a standard accessible physical-input path.
- **Failure:** attempts to extend the journey to the GTK sound dropdown exposed
  popover/focus timing differences in synthetic X11 input. No product-only key
  handling or fake test action was retained. Sound model and persistence remain
  covered at the core/store layers; custom sound end-to-end scope remains
  explicit rather than turning harness activity into a false pass.
- **Failure:** `notification-daemon` 3.20 crashes when run directly on the
  minimal nested Cage Wayland compositor. It is an X11 notification server.
  **Repair:** in both cells the real daemon runs on the outer wrapper-owned Xvfb
  transport while Zentty itself runs on the declared X11 or Wayland backend.
  The freedesktop D-Bus contract is display-neutral; this avoids claiming the
  daemon itself is Wayland-native.
- **Failure:** the first Wayland Send Test chord arrived before Cage activated
  the nonmodal settings toplevel and was correctly logged as an unbound terminal
  shortcut. **Repair:** the journey now waits for the compositor activation
  receipt before sending physical input.
- **Discovery:** giving a nested Wayland product a private session bus causes
  Ubuntu's desktop portal activation to emit GNOME-backend crash warnings under
  Cage. These are separate portal processes, not Zentty or the notification
  daemon, and the GTK portal backend continues. They remain visible in raw test
  output rather than being suppressed.
- **Full-matrix failure:** the first post-slice run found the architecture
  qualification mirror still named the deleted prose-only notification cell.
  **Repair:** the architecture mirror now enumerates both executable desktop
  cells plus custom-sound NOT_IMPLEMENTED, and its platform-settings defect now
  describes the settings pages that actually exist.
- **Full-matrix product failure:** the established source-UX journey exposed a
  real pre-existing divergence after an arranged offscreen pane was selected by
  Ctrl+Tab: the model selected pane-5 while physical input remained in pane-2.
  **Repair:** adjacent-pane navigation now reveals the selected column; the
  post-key-release path reasserts focus only after GTK lays out that viewport.
  Pane closure likewise reconciles the focused viewport. The journey now waits
  for real `focus-pane` and viewport receipts instead of racing an action log.
  The repaired complete source-UX journey passed. This was not reclassified as
  environmental or suppressed.
- **Runner failure:** after reporting the two genuine failures above, the
  matrix summary hit `/usr/bin/jq: Argument list too long` while assembling its
  unusually large retained receipt. This was a runner summary defect, not a
  test pass. **Repair:** the runner now passes the result collection to jq via a
  private temporary JSON file instead of the process argv, retaining the same
  machine schema without depending on `ARG_MAX`. All other executable cells in
  that run passed; the repaired failures were rerun directly before the final
  aggregate rerun.
- **Focus-repair regression caught on rerun:** the first deferred Ctrl+Tab
  repair could fire after a user immediately opened Command Palette, stealing
  focus back to the terminal. The real Wayland session-restore journey caught
  this at its `Close Worklane` cancellation step. **Repair:** deferred terminal
  focus is now conditional on no command-palette or global-search overlay being
  active. The complete Wayland restore/cancellation journey passed after the
  repair.
- **Notification X11 cleanup failure:** a successful real notification journey
  activated `xdg-document-portal`, whose private FUSE mount raced the nested
  wrapper teardown and invalidated the environment receipt. The journey does
  not exercise file portals; it now pins GIO to its local filesystem backend in
  addition to disabling GTK portal dialogs. D-Bus notification delivery remains
  real and independently monitored.
- **Aggregate repeatability:** the last aggregate rerun executed every declared
  cell but reported three runtime failures: one Debug X11 lifecycle child-exit
  deadline, one Wayland bookmark-dialog focus deadline, and the notification
  portal teardown race above. Neither unrelated deadline reproduced when its
  exact matrix command was rerun in its controlled wrapper. The repaired
  notification command also passed through the new teardown path. Therefore all
  presently executable commands have passing current evidence, but the retained
  aggregate summary remains `implemented_local_suite_passed=false`; this record
  does not claim release or full Linux qualification.

## Evidence

## 2026-08-13 correction — custom audio completed by GH-41

The earlier `notification-custom-sound-import` NOT_IMPLEMENTED statement was
accurate for the GH-20 Notifications-settings slice but is no longer current.
GH-41 extends the same page, config store, service, and real-system journey; the
authoritative matrix now marks that explicit cell PASS. Current implementation
and qualification evidence is recorded in
`zentty-linux-dogfood-2026-08-13-custom-sounds.md`.

- Core notification TOML defaults, exact keys, unknown-key compatibility, and
  invalid types pass in `zentty-core`'s `app_config` integration test.
- ConfigStore's real filesystem test proves notification writes preserve the
  final symlink, comments, unknown root/section keys, unrelated settings, and
  private `0600` mode, and remove stale optional custom metadata.
- Focused mutation runs used the repository's mandatory safe-copy policy
  (`gitignore = true`, `copy_target = false`). The notification document mutant
  was caught. The notification launcher run caught one mutant; the other was
  compiler-unviable rather than surviving the tests.
- Controlled X11 passed with a real settings deep link, physical Send Test
  activation, private session bus, real `notification-daemon`, and a monitored
  `Notify` payload.
- Controlled Wayland passed the same product path in nested Cage while the real
  X11 notification service remained isolated on the wrapper-owned outer Xvfb.
- Both journeys stop the service and require the second Send Test to report an
  explicit unavailable error; environmental absence cannot become PASS.
- Source review after the first implementation caught an architecture error:
  immediate raw OSC delivery bypassed the source attention reducer and its
  active-pane suppression. That code was removed, the feature plan was
  corrected, and the inventory now marks the broader notification/inbox feature
  PARTIAL rather than pretending settings delivery completed issue #21.

## Remaining uncertainty

- Custom audio import/conversion/install/cleanup is not implemented.
- Named sound preview depends on the host sound theme and
  `canberra-gtk-play`; audible output is not claimed by the D-Bus journey.
- GNOME portal backend crashes observed only in the deliberately minimal nested
  Cage/private-bus environment remain raw environmental evidence. No Zentty
  suppression or pass reinterpretation was added.
