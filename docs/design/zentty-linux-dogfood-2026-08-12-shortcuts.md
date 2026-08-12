# Zentty Linux Dogfood — Shortcuts and Presets

Date: 2026-08-12
Issue: #20

## Initial audit

- **Discovery:** Linux currently has several capture-phase GTK key controllers with
  hard-coded predicates in `application_shell.rs`, `pane_search.rs`, and
  `bookmarks_view.rs`. The action callbacks themselves are already centralized in
  `application_shell/action_router.rs`.
- **Decision:** do not accrete a parallel shortcut command system. The configurable
  dispatcher will activate the existing `workspace.*` actions, and migrated
  hard-coded predicates will be removed.
- **Discovery:** Control+Tab is not a normal one-shot shortcut. It owns key-down,
  repeat suppression, a hold timer, key-up, and peek commit/cancel behavior.
- **Decision:** preserve that tested state machine outside the configurable registry;
  representing it as a simple binding would regress the source-derived interaction.
- **Discovery:** the source presets resolve physical key positions through the live
  keyboard layout. Linux must likewise use GDK keycode/keyval translation rather
  than assuming US characters in the recorder.
- **Decision:** source Command defaults map to Linux Control; source Option maps to
  Alt. Super remains compositor-facing rather than becoming Zentty's default app
  modifier.
- **Risk:** application shortcuts sit ahead of a real terminal widget. A unit test
  that merely returns `handled=true` cannot prove that unbound bytes still reach the
  PTY or that bound keys do not leak.
- **Required evidence:** controlled X11 and Wayland staged-app journeys must observe
  both the GTK action effect and child-process input, including restart persistence.

Further discoveries, failures, repairs, receipts, and limitations are appended here
as the slice proceeds.

## Implementation discoveries and repairs

- **Failure:** the first `toml_edit` implementation indexed through an absent
  `shortcuts` table and serialized `shortcuts = {}` instead of the requested
  bindings.
- **Repair:** create the table explicitly before replacing its `bindings` array of
  tables. The config-store regression test also proves comment, unknown-key, and
  symlink preservation through the atomic write.
- **Failure:** the first settings-window implementation returned only `()` from its
  constructor. GTK mapped the transient window, but Rust immediately dropped both
  the strong `gtk::Window` handle and its `Rc` view state. The controlled X11
  journey exposed the resulting non-interactive window.
- **Repair:** return and retain the window in `ApplicationShell`, retain the view
  state from its close lifecycle, hide rather than destroy on close, and reuse the
  same instance.
- **Receipt:** the controlled X11 journey now passes against the staged application,
  a real Ghostty surface, and a real PTY: recording changes a binding, the bound
  chord invokes the existing action without leaking to the child, the displaced
  chord reaches the child byte-for-byte, and the binding survives process restart.
- **Failure:** the controlled nested-Wayland journey mapped and activated the
  settings transient but did not focus its search entry; environmental absence was
  correctly reported as failure rather than pass.
- **Repair:** match the already-proven Task Manager secondary-window lifecycle
  (retained, non-modal, hide-on-close) and provide a local Control+F focus action
  in addition to map/activation focus requests.
- **Repair and discovery:** Cage correctly activated the settings toplevel, but its
  activation notification arrived after the harness began typing. The harness now
  gates physical input on observed toplevel activation rather than assuming a
  timing delay. Opening from the real command palette also matches the proven Task
  Manager secondary-window path; that audit exposed that Settings had not yet been
  listed in the Linux palette, which was repaired.
- **Harness defect:** the shared shortcut journey's `main_key` parser matched
  `ctrl+alt+z` as the broader `ctrl+*` case and passed `alt+z` as a key name. This
  was a test-input defect, not product evidence. The more-specific modifier case
  now precedes the broad case.
- **Repair:** Escape now closes the settings window and explicitly presents its
  parent. This is both Linux-native dialog behavior and deterministic focus
  restoration for subsequent terminal input.
- **Receipts:** both controlled compositor journeys pass with real physical input,
  real staged GTK/Ghostty product, and real child PTY boundaries:
  `rust-shortcuts-settings-x11: PASS` and
  `rust-shortcuts-settings-wayland: PASS`. The authoritative matrix names these
  cells `shortcut_binding_runtime`: this proven shortcut subset does not imply
  that import/export, every source command, or the wider settings/appearance issue
  is complete.

## Remaining shortcut-slice scope

- The source registry audit is mechanically reconciled. The initial comparison
  found eleven missing source IDs. Focused rename actions now reuse the existing
  rename dialog and parameterized action, path copy uses the focused pane's real
  working directory, and duplicate pane opens a new column with that directory as
  the source specifies. Six IDs still lack an honest parameterless Linux action
  and are not falsely advertised as bindable: `open_with.selected_app` and
  `server.open_selected`. The adjacent issue #20 work now provides
  `app.reload_config` plus the four source appearance-mode commands through the
  real Ghostty runtime rather than recreating terminal surfaces.
- The remainder depends on adjacent issue #20 work (appearance state and safe live config
  reload); selected-app/server commands require a durable selection owner rather
  than guessing from a transient popover.
- The settings UI implements import/export and both source presets, with pure
  validation and round-trip coverage. Their native file-chooser and header-button
  journeys are not yet qualified through both compositors, so the matrix cell is
  deliberately scoped to binding/runtime persistence rather than claiming the
  entire settings feature.

## Mutation audit

- The first targeted run correctly used the repository-enforced
  `gitignore=true`/`copy_target=false` policy, so it did not recopy the large
  ignored Ghostty dependency tree. Its unmutated baseline nevertheless failed in
  the sandbox because an unrelated existing `open_with` test intentionally creates
  a special filesystem node. The same full workspace baseline passes elevated.
- Restricting the mutation command to `-- --lib shortcut` removes that unrelated
  product surface and avoids rebuilding/running the entire core integration suite
  for each local pure-model mutation. Four workers exposed a cargo-mutants 27.1.0
  scratch collision (`File exists`); two workers are stable and remain materially
  faster without abusing disk.
- Five initial survivors identified missing display/hash assertions; six later
  survivors identified unasserted public accessors and the preservation branch in
  `updated_bindings`. Tests were added for the behavior rather than suppressing
  mutants.
- **Final receipt:** 53 mutants tested in 78 seconds: 43 caught, 10 unviable,
  0 missed, 0 timeout. The completed machine-readable receipt is
  `mutants.out/outcomes.json`; interrupted diagnostic receipts are not
  qualification evidence.

## Regression qualification discoveries

- **Failure:** the first full local qualification after installing the source
  registry was not a pass. Several older product journeys still injected former
  Linux-only chords (`Ctrl+N`, `Ctrl+PageUp/Down`, and `F3`) after the product had
  deliberately adopted source-compatible defaults. This was integration-harness
  drift, not evidence that the new bindings worked.
- **Repair:** source UX, sidebar management, pane search, and session restore now
  drive the actual source-derived Linux chords (`Ctrl+T`, quick `Ctrl+Tab` /
  `Ctrl+Shift+Tab`, `Ctrl+F`, `Ctrl+G`, and `Ctrl+Shift+G`). Assertions observe
  the resulting focus/topology/search projection rather than requiring obsolete
  action log wording from the retired shortcut path.
- **Product defect exposed:** dismissing the command palette directly hid its GTK
  window but bypassed the existing workspace dismiss action, leaving physical
  input focus unreliable for the subsequent real PTY step. Escape now routes
  through `workspace.dismiss-command-palette`, restoring the terminal focus
  contract.
- **Restore-harness discovery:** quick `Ctrl+Tab` follows the source's global pane
  traversal, crossing worklane boundaries only as that traversal reaches them; it
  is not a direct next-worklane command. The restore journey now traverses from
  the last pane to the first (and back) with one wrapping forward/backward chord
  rather than treating two chords as worklane hops.
- **Focused receipts after repair:** source UX, overflowing-sidebar navigation and
  drag reorder, pane search on X11 and Wayland, session restore on X11 and
  Wayland, multi-window restore on X11 and Wayland, and Wayland bookmark
  import/export all pass against the real staged product in controlled nested
  compositor environments.
- **Governance repair:** the ApplicationShell ownership contract now assigns the
  shortcut manager, capture controller, retained settings window, newly routed
  methods, and action names to explicit owners. It also pins the three new
  shortcut implementation sources. The architecture validator passes.
- **Qualification boundary:** these focused reruns repair every failure observed in
  the first full run, but they do not convert that failed receipt into a passing
  receipt. A fresh complete `qualify-local` run is required before this slice is
  committed or described as locally qualified.
- **Second full-run failures:** a fresh matrix run passed 112 executable cells and
  failed three. Pane Search still injected obsolete `Ctrl+Shift+E` for Use
  Selection for Find; the source registry specifies `Ctrl+E`. Both agent cells
  exposed a subtler restore assertion error: quick `Ctrl+Tab` is global pane
  traversal, so two presses moved within the three-pane worklane instead of back
  to the formerly active worklane. The resulting snapshot honestly retained the
  wrong active lane and failed qualification.
- **Second repair and focused receipts:** Pane Search now drives `Ctrl+E`.
  Restore uses one wrapping `Ctrl+Tab` from the last global pane to the first and
  one wrapping `Ctrl+Shift+Tab` to return. Pane Search X11 and Session Restore on
  both controlled compositors pass after this repair. The failed full receipt is
  retained in `build/linux/qualification-summary.json` only until the required
  final rerun replaces it; it is not a passing claim.
- **Final local receipt:** the complete `linux/tests/qualify-local` rerun passed
  every presently executable support and matrix cell in 446.11 seconds. Declared
  totals are `PASS=115`, `FAIL=0`, `BLOCKED=7`, `XFAIL=1`, and
  `NOT_IMPLEMENTED=23`; therefore the implemented local suite passed, while
  release qualification and full Linux qualification correctly remain not passed.
  Suppression review was accepted, and Valgrind results remain **PASS with reviewed
  suppressions**, never an unsuppressed-clean claim. Machine evidence is
  `build/linux/qualification-summary.json` (generated and intentionally ignored).
