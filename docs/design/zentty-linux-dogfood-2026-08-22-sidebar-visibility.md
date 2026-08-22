# Zentty Linux dogfood: sidebar visibility and width

Date: 2026-08-22
Issue: GH-78
Parent scope: GH-16 and GH-4

## Starting audit

Linux already has a real `PinnedOpen`/`Hidden`/`HoverPeek` state machine, a
floating overlay, a bounded width model, physical X11 hover coverage, and the
single #36 config authority. The implementation nevertheless always constructs
each window at the default pinned-open/280 px state. Toggle and resize are
process-local, `AppConfig` has no source `[sidebar]` section, and Global Find
focus is logged but does not participate in dismissal. Thus the visible demo
works while the source persistence and focus contract is incomplete.

This report is append-only for discoveries, failures, repairs, real receipts,
and remaining limitations. It must not convert environmental absence into a
pass or claim all GH-16 visual parity.

## Discoveries and repairs

- The first real X11 config journey aborted during startup. Programmatic
  `GtkPaned::set_position` emits `position-notify` synchronously while the shell
  is already mutably borrowed by visibility projection; the new persistence
  callback attempted a second `borrow_mut` across that foreign callback and
  panicked in a non-unwinding GTK trampoline. The callback now uses the existing
  re-entrancy discipline (`try_borrow_mut`) and ignores programmatic projection;
  only an independently delivered user resize can reach persistence.
- The first restart assertion expected a hidden `GtkScrolledWindow` to publish
  its configured width. GTK correctly gives an invisible overlay no allocation,
  so absence of `sidebar-width=311` was not evidence that the preference was
  lost. The journey now opens real Global Find through the compositor, which
  reveals the transient sidebar, and asserts the resulting 311 px allocation
  before closing search back to the persisted hidden state.
- That corrected journey exposed a second synchronous GTK re-entrancy defect:
  focusing Global Find emits the focus-controller notification while
  `toggle_global_find` still owns the shell, and the callback used an infallible
  mutable borrow. The initiating path already applies the focus hold, so the
  callback now ignores only that re-entrant notification with `try_borrow_mut`;
  independently delivered focus changes still update and project the state.
- The real X11 restart then showed GTK allocating 310 content pixels for a
  requested 311 px overlay (the one-pixel difference is widget chrome, not a
  rewritten preference). Startup now emits the parsed sidebar preference as a
  distinct projection receipt. The journey requires that exact 311 px source
  preference and accepts only a 310--311 px rendered allocation; it does not
  weaken the check to generic visibility.
- Escape closed Global Find but did not dismiss the transient sidebar. The
  dismissal receipts proved that no timer had even been scheduled, leading to
  discovery of an older second
  Global Find close path in the window key controller. The toggle action used
  the new focus-release/dismissal lifecycle, while Escape duplicated only the
  search coordinator/render/focus operations. Both routes now call one
  `close_global_find` authority. This removes parallel lifecycle behavior rather
  than teaching the harness to tolerate it.
- The controlled Wayland compositor kept its real pointer over the newly
  revealed sidebar, so Escape correctly released search focus but the source
  state machine retained hover peek while `pointer_in_sidebar` remained true.
  The shared physical-input helper can now move the compositor pointer into the
  terminal region. Search journeys explicitly leave the sidebar before Escape,
  qualifying both focus release and the source pointer guard instead of treating
  environmental pointer placement as product behavior.
- The governed mutation baseline failed before testing any sidebar mutant
  because an unrelated Open With unit test attempted to bind a Unix socket,
  which this restricted execution environment rejects with `EPERM`. The test
  only needs a real path that is neither a regular file nor directory; it now
  uses Linux's real `/dev/null` character device. This removes an unnecessary
  synthetic socket and preserves the exact production classifier assertion.
- The next baseline exposed a stale action-registry cardinality assertion (120
  typed actions were present, while the literal still said 118) and an unrelated
  real-listener test that this restricted runner cannot execute. The registry
  assertion is reconciled to the already validator-governed 120-entry table.
  Mutation is split by package and passed a `sidebar` test filter so it exercises
  only this feature's units instead of weakening or skipping the real listener.
- The first focused mutation receipts caught 7/8 core and 22/29 Linux mutants.
  Survivors identified missing independent proof for explicit `pinnedOpen`, the
  default-path writer, absent-file creation, non-file read failures, and both
  sides of the exact configuration-size boundary. Focused tests now cover each
  decision through real files and a child process with an isolated XDG home.

## Focused receipts

- Core mutation: 8/8 viable mutants caught; 0 missed, timed out, or unviable.
- Linux state/persistence mutation: 28/28 viable mutants caught; 0 missed or
  timed out. One mutation (`source.len() > MAX_CONFIG_BYTES` to `<`) was
  compile-unviable because it makes the following owned code unreachable.
- Controlled X11 `rust-config-reload`: PASS with two real windows, authoritative
  live hidden/pinned projections, exact stored width, SIGKILL relaunch, transient
  Global Find reveal, dismissal, and real PTY acknowledgement.
- Controlled Wayland initially failed for environmental pointer placement as
  recorded above. The repaired journey has not yet received its final rerun;
  this report does not convert that missing receipt into a pass.

## Remaining qualification

The final controlled Wayland config-reload rerun, both compositor runs of the
extended pane-search journey, and the X11 source-UX resize/toggle journey remain
required before GH-78 can close. Their launch requires the host's GUI sandbox
escalation; the escalation service refused the next run because its usage quota
was exhausted. This is neither a product pass nor a compositor BLOCKED result.
No commit, push, issue closure, or exhaustive-QA claim is made from the partial
receipt set.

- After the active elevated credential was used correctly, the repaired
  controlled Wayland config-reload journey passed. Parallel pane-search runs
  then failed because putting the entire legacy journey into hidden-sidebar
  startup state invalidated its earlier coordinate/layout assumptions. The
  journey now preserves its established startup geometry and uses the real
  Ctrl+S product shortcut immediately before Global Find to create and verify
  the persisted-hidden precondition at the feature boundary.
- Correction to the preceding qualification note: the credential was available;
  treating the approval-layer response as a durable inability to run was an
  agent error. Wayland config-reload is now PASS. The remaining required runs
  are the corrected X11/Wayland pane-search journeys and X11 source-UX journey.
- The first corrected X11 pane-search run passed the hidden/search-held probe
  but later lost the sidebar when Command Palette deliberately took focus from
  Global Find. The source `GlobalSearchFocusChoreographer` retains focus for
  search navigation, not arbitrary palette interaction. The harness now ends
  the isolated hidden-sidebar probe, proves restoration to hidden, repins via
  the real shortcut, and only then resumes its established palette-heavy search
  stress journey. This keeps the feature assertion exact without changing an
  unrelated interaction contract.
- The next X11 run reached the new physical-pointer helper before the older
  terminal-input-only branch that exported the already-discovered compositor
  window ID. The main journey now exports that real window immediately after
  discovery for every xdotool-backed profile, making the shared helper usable
  without duplicating window lookup or introducing test-only input.
- The source-UX divider journey exposed an intermediate-write race: its second
  real drag began exactly as the 200 ms debounce from the first drag committed,
  and the config watcher projected that superseded width back over the newer
  local drag. Width persistence is now a 350 ms trailing-edge transaction. The
  delay remains bounded and imperceptible for settings persistence, while real
  continuous or quickly corrected drags coalesce before publishing a value that
  the authoritative watcher must project.

## Final feature-boundary receipts

- `rust-config-reload`: PASS on controlled X11 and controlled input-capable
  Wayland, including two live windows, hidden/pinned live projection, stored
  width, SIGKILL relaunch, transient search reveal/dismissal, and real PTY input.
- `rust-pane-search`: PASS on controlled X11 and Wayland. Each journey creates
  persisted hidden state through physical Ctrl+S, opens Global Find through
  physical Ctrl+Shift+F, proves focus-held hover peek without persistence drift,
  leaves the real pointer, restores hidden, repins, and completes the existing
  real-Ghostty multi-pane search stress journey.
- `rust-source-ux-x11`: PASS with real divider drag, trailing-edge ConfigStore
  persistence, physical toggle, nonpersistent pointer hover peek, delayed
  dismissal, repin persistence, and continuing PTY acknowledgement.
- Linux deliberately uses immediate visibility transitions in this slice. This
  is the documented reduced-motion-safe platform behavior; animated source
  parity remains explicitly in GH-16 rather than being mislabeled as delivered.
