# TornadoTTY v0.1.2 release field report

- Tracking: [#206](https://github.com/TamedTornado/TornadoTTY/issues/206).
- Jason requested a new release following the server-menu repair, then explicitly
  requested the outstanding Clippy failure be fixed. Starting source is
  `e2208141c0418c7dbbd02a7b0e1ae54429825bf9`; prior public release is `v0.1.1`.
- Continue the existing native Debian/Arch **unsigned prerelease** policy.
  Checksums and provenance are not signatures. Signing/attestation #152 is not
  silently represented as delivered. Do not install or restart the live client.
- Earlier incidents and installations remain in the
  [desktop field report](tornadotty-linux-dogfood-2026-09-08-desktop-activation.md).

## Release preflight and lint repair

- The original CLI function-length failure was real. Strict workspace/all-target
  Clippy exposed further existing failures as earlier ones were removed: workload
  error documentation, overlong dispatcher/restore/callback/test functions,
  clone/default/borrow idioms, float conversions/comparisons, checked time
  subtraction, test placement, and an infallible API returning `Result`.
- Repairs use focused helper extraction within existing owners, preserving
  callback registration order, routing, readiness waits, cleanup, and persistence
  checks. No lint allowances or lowered thresholds. Viewport extents use lossless
  integer-to-f64 conversion; pane float coordinates are widened for comparison.
  Exact assigned-width test comparisons remain exact via `total_cmp`.
- Strict `cargo clippy --locked --workspace --all-targets --profile release-safe
  --no-deps -- -D warnings` PASS after repairs (before version-only changes).
  Workspace rustfmt and diff checks PASS. Formatter corrections previously
  discarded as unrelated are now intentional release-gate cleanup, not recovered
  behavioral fixes. Initial failing checks and final receipt are retained under
  `/tmp/tornadotty-v012-clippy-*.log` and `/tmp/tornadotty-v012-fmt-first.log`.
- Focused pre-version checks PASS: CLI binary (2), identity (1), helper routing
  (16), launchers (31), product parser (11); core attention (15), notification
  capability (2), restore recipes (10), workload policy (2); receipt contracts
  (9), driver (3), and session ownership (7). These are not fresh GUI/PTY or
  release-qualification results. Receipts use `/tmp/tornadotty-v012-*-tests.log`
  and `/tmp/tornadotty-v012-cli-checks.log`.

## Release identity and publication preparation

- All nine workspace crates and their lock entries move from 0.1.1 to 0.1.2;
  external dependencies and the pinned Ghostty revision are unchanged. AppStream
  release history and release-specific notes are updated.
- The publisher previously hard-coded "first public prerelease" and old Omarchy
  test claims. It now requires nonempty release-specific notes (or an explicitly
  supplied notes file). Focused publication tests use a fake GitHub boundary and
  verify current note bytes, explicit fork targeting, prerelease/verified-tag
  flags, and rejection before publication for missing notes or corrupt bundles.
  No real GitHub publication is implied by that test.
- `omarchy-laptop.local` initially failed DNS resolution. Jason brought the
  laptop online; native Arch tools are present and the prior release checkout is
  clean at `4a02c9f3`. Its isolated Rust nightly-2026-08-17 and Zig 0.16.0 caches
  remain usable; no system packages were installed. Native builds and package
  lifecycle checks are pending, not passed.
- Full Rust checks, authoritative `qualify-local`, tag/package production,
  artifact verification, and publication remain separate steps. No previous
  matrix totals are carried forward as current evidence.

## First versioned checks

- Strict workspace/all-target Clippy also PASSes after the 0.1.2 version update.
  The first broader Rust test run FAILed in the session cleanup fixture: the test
  observed its PID file after shell redirection created it but before `printf`
  wrote the PID (`ParseIntError::Empty`). This is fixture readiness, not a failed
  product cleanup assertion. Both PID-producing fixtures now publish by atomic
  same-directory rename; no timeout, assertion, or cleanup requirement is relaxed.
- Publication notes/checksum tests, release-version tests, workflow negative
  contracts, AppStream validation, and script ShellCheck PASS where run. An initial
  ShellCheck invocation omitted `-x` and reported the existing sourced-helper
  SC1091; rerun with source following is required before claiming the aggregate.
- PID publication repair PASSes all seven session tests. The second monolithic
  workspace invocation then FAILed on existing process-global test ownership:
  multiple test threads attempted GTK initialization and interfered with default
  GLib-context file-watch delivery. No product assertion was weakened or ignored.
  Re-ran the nine config-watch tests in `isolated-session` and each of the three
  non-ignored GTK tests in a separate `nested-x11` process: all PASS. The remaining
  workspace/all-target tests PASS in their normal parallel partition, excluding
  only those twelve separately executed tests. Declared ignored tests retain
  their original prerequisites; this is not an all-ignored-tests qualification.
  Receipts: `/tmp/tornadotty-v012-{config-watch,activity_title,sidebar,task_manager}-isolated.log`
  and `/tmp/tornadotty-v012-workspace-tests-isolated.log`. ShellCheck with `-x`,
  workspace rustfmt, and diff checks PASS.
- Both prior public v0.1.1 packages were downloaded and verified against their
  published SHA256SUMS as upgrade fixtures. Native preparation initially refused
  the explicit old Ghostty checkout rather than changing it; after a clean-tree
  check, it was explicitly advanced to the unchanged release pin
  `b62d1c57fe7d265ab030fd25c96742e1b11442b8` and preparation PASSed.

## Native candidate and interrupted qualification

- Native Omarchy candidate construction and package audit PASS at `c8691074`:
  `0.1.2+gitc86910746ead-1`, SHA-256
  `802389c61657ce8466a57bd66edd077452b62fd3c768582017fc765c91cca571`.
  Real pacman install/upgrade from the verified public 0.1.1 package, installed
  help, and uninstall PASS in a disposable user namespace. The host pacman
  database fingerprint was unchanged. This is a candidate receipt, not the
  final tagged 0.1.2 artifact or a host installation.
- The first authoritative local run started at 19:23 CEST. The pinned Ghostty
  regression PASS took 1,038,770 ms (about 17 minutes). Debug and ReleaseSafe
  integrated builds, hardening, and ABI checks PASSed. The run was deliberately
  interrupted after discovering inherited parent-pane routing variables in the
  supposedly private test environments; see [#207](https://github.com/TamedTornado/TornadoTTY/issues/207).
  Preserved per-cell receipts contain 80 PASS, 16 FAIL, 2 XFAIL, one
  missing/invalid Valgrind report, and one dependency-blocked cell. This is an
  **incomplete, failed run**, not qualification of all 231 declared cells.
  Receipts, including paired raw/suppressed logs, remain under
  `build/release-v0.1.2-evidence/first-run/`.
- The architecture and engine API audit inventories had not followed existing
  changes: the typed-receipt crate (#148), bounded ingress ownership, extracted
  helpers, the five already pinned Ghostty fixes, and two private-bus command
  mirrors. Reconciled those concrete owners/edges/callsites; no new architecture
  or qualification-status changes. Architecture positive/negative checks and
  the 25-file/14-export engine audit plus negative tests PASS.
- The public invalid-option path still printed `zentty-linux` while its public
  contract expected TornadoTTY. Reproduced in the staged binary, corrected the
  top-level diagnostic using the existing branding constant, rebuilt with
  `build-local`, and the real no-desktop CLI journey PASSed. Strict all-target
  Clippy remains PASS.
- The stopped run also exposed X11 class lookup, stale visual baselines, missing
  private-bus setup, IME, installed-CLI, and Valgrind failures. These remain
  unresolved unless a later focused receipt explicitly supersedes them. In
  particular, Debug X11 multi-pane Valgrind returned 99 with unsuppressed
  llvmpipe worker undefined-value contexts; no suppression was broadened.

## Parent-pane isolation repair (#207)

- Each of the existing no-display, X11, and Wayland wrappers now discards the
  enclosing instance/socket, pane token/credential and identity, window/worklane
  identity, CLI path, and hook command/tool before child setup. A test actor may
  establish its own explicit context after that boundary; private HOME/XDG alone
  is not treated as protection against an inherited explicit socket.
- Added poisoned-parent assertions to all three existing wrapper regressions.
  All FAILed before repair (64/79/79) and all PASS after repair, including their
  existing process-group cleanup tests. The previously failing real X11
  keyboard-repeat/PTY journey now PASSes with unchanged behavior assertions.
  Receipts: `/tmp/tornadotty-v012-*-isolation.log` and
  `/tmp/tornadotty-v012-*-test-{red,green}.log`.
- ShellCheck warning/error severity PASS. Its informational SC2317 diagnostics
  on pre-existing indirectly invoked cleanup traps remain; no blanket allowance
  was added. The identified release-runner process group and owned private
  sessions were stopped and confirmed absent. No client restart or installation
  was performed. No release/tag has been published.

## Focused native identity and private-bus repairs

- [#208](https://github.com/TamedTornado/TornadoTTY/issues/208): the private X11
  scaling regression exposed zero matching windows. Its native `xprop` diagnosis
  confirmed the host instance name but Ghostty's display-wide class. The existing
  host-identity adapter now sets the X11 display class before host windows are
  created, via a private type-checked GTK FFI helper. Wayland remains a no-op;
  no new dependency, engine export, or engine revision is introduced. Existing
  X11 scaling (96/192 DPI, two real PTYs), representative multi-pane external
  resize, and Wayland GTK/Ghostty/PTY smoke all PASS after repair. The original
  exact class selector is preserved, not replaced with a looser lookup.
- The full agent IPC journey had gained a controlled notification daemon but
  not its private-bus prerequisite. It now follows the existing session-restore
  fixture pattern and re-execs under `dbus-run-session` after validating the
  private display boundary, before starting any product or daemon. The existing
  real Codex review-routing journey PASSes without an externally supplied bus.
- Focused strict all-target Clippy for the affected crates, workspace rustfmt,
  diff checks, and ShellCheck warning/error severity PASS. Receipts:
  `/tmp/tornadotty-v012-x11-class-{scaling,resize,wayland,clippy-green}.log`
  and `/tmp/tornadotty-v012-codex-private-bus.log`.
- Fcitx remains FAIL after those repairs: initial exact preedit/Backspace and
  pane-1 plain input pass, but pane 2 receives `a日` rather than `日` after focus
  returns. This is a reproducible failure, not classified away as environment
  noise. The unsuppressed Debug X11 multi-pane Valgrind finding also remains
  unresolved. Jason has been asked whether to hold this release for repairs or
  explicitly publish an unsigned prerelease with those failures disclosed;
  nothing is published while that risk decision is pending.

## Requested bug repairs: IME focus lifetime (#209)

- Jason chose to fix the bugs rather than publish with the failures. Release
  remains held; the earlier risk question is no longer pending.
- The pinned Fcitx GTK adapter can emit commits from reset/focus-out paths.
  Ghostty guarded synchronous reset only; it still accepted a commit once the
  owning surface had lost focus. The minimal engine repair rejects commits from
  an unfocused surface. No sleeps, retries, byte-assertion changes, or new engine
  API. The final patch is [Ghostty `d21021ad6`](https://github.com/TamedTornado/ghostty/commit/d21021ad6a9956f04bb53d91ece1c1ac4abc6bfa),
  pinned here with reconciled architecture/API audit identities.
- Both Fcitx and IBus exact-byte composition, focus transfer, and active-preedit
  destruction journeys PASS on X11 and Wayland with the final engine behavior.
  A concurrent Fcitx run first completed its product assertions but FAILed during
  cleanup: the extra IM wrapper session owned a document-portal FUSE mount that
  its plain directory removal could not clean. The existing display wrapper is
  now kept as the runtime-directory and process-group owner; Fcitx still gets a
  separate bus and private configuration. Added a failing ownership regression,
  then PASSed it and both real Fcitx journeys after repair. The failed cleanup
  receipt is retained, not called a successful full invocation.
- Final receipts: `/tmp/tornadotty-gh209-final-owned-{x11,wayland}.log`,
  `/tmp/tornadotty-gh209-final-ibus-{x11,wayland}.log`, and
  `/tmp/tornadotty-gh209-wrapper-{red,green}.log`. Zig format, script ShellCheck
  warning/error severity, architecture, and exact pinned API audit checks PASS.
- #210 is not fixed by these changes. The first origin-tracked paired diagnostic
  did not reproduce the llvmpipe worker contexts; its suppressed phase exceeded
  the unchanged 300-second qualification cap, so no passing report was published.
  A separate diagnostic-only poisoned-heap run with matching Ubuntu Mesa symbols
  completed with zero post-suppression errors. That does not invalidate the
  original finding or establish a repair. Original, raw, suppressed, and
  diagnostic receipts remain preserved; all four suppression-file hashes remain
  unchanged. Debug symbols/tools were extracted under `/tmp`, not installed.

## Renderer first-frame ordering (#210)

- The early origin/poisoning diagnostics did not reproduce the worker finding.
  GLX graphics replay completed with existing driver-owned contexts only; EGL
  tracing/replay had tool compatibility limitations and is not release evidence.
  Matching Mesa symbols then made the worker/producer distinction explicit.
- Source inspection identified GTK drawing before the renderer worker's first
  terminal snapshot. Extended the existing engine GTK embedding spike to draw a
  newly initialized renderer in a real GL context before any snapshot update,
  then update from real terminal state and draw again. This deterministically
  FAILed with `UnpreparedFrameWasPresented` on both surfaces. Raw origin-tracked
  Memcheck reproduced the original llvmpipe worker Cond/Value8 stack family,
  tracing undefined data to Ghostty `Renderer.init`, not a driver exemption:
  exit 99, 10,881 errors / 110 contexts including existing external findings.
- The repair defers drawing under the existing draw mutex while the cell grid
  is empty. The first snapshot schedules the normal draw. The regression now
  PASSes on X11 and Wayland and verifies both no unprepared presentation and
  successful presentation once snapshot data exists. No sleep, retry, relaxed
  assertion, or changed suppression. Engine repair and regression are pushed as
  [Ghostty `485b32104`](https://github.com/TamedTornado/ghostty/commit/485b32104b66bf52d9e7c449742d46f49a5374a3).
- Green raw origin tracking has 151 errors / 20 existing external contexts,
  with no llvmpipe worker finding or undefined origin from `Renderer.init`.
  The paired suppression-enabled first-frame regression completes with zero
  errors and zero definite/indirect leaks: **PASS with reviewed suppressions**.
  Recreate via the existing private X11/Wayland wrapper and engine
  `GHOSTTY_EMBED_SPIKE_FIRST_FRAME=1 zig build gtk-embed-spike`.
- `build-local` Debug integration succeeds. Real two-pane Wayland lifecycle and
  X11 Fcitx exact-byte composition/focus/destruction PASS with both fixes.
  Architecture/negative tests, the exact 26-file/14-export API audit and its
  negative tests, Zig format, ShellCheck warning/error severity, and diff checks
  PASS. The integrated product's original paired Debug X11 two-pane Valgrind
  journey completes under the unchanged 300-second phase limit: both real
  lifecycles PASS, with zero post-suppression errors and zero definite/indirect
  leaks (**PASS with reviewed suppressions**). The raw receipt remains visible
  with 940 errors / 852 contexts; raw cleanliness is not claimed. Aggregate
  suppression governance and the remaining release matrix are not certified by
  this focused run. This is a development evidence bundle, not final clean release provenance;
  nothing has been installed, restarted, tagged, or published as a release.
- Receipts: `/tmp/tornadotty-gh210-spike-{red,green}.log`,
  `/tmp/tornadotty-gh210-spike-wayland-green.log`, and
  `/tmp/tornadotty-gh210-first-frame-valgrind-{red,green-raw,green-suppressed}.log`.
  Copies are preserved in `build/release-v0.1.2-evidence/gh210-first-frame/`.

## Wheel tilt worklane jumps (#211)

- Jason reported that mouse-wheel scrolling in a Claude Code pane intermittently
  jumped to another worklane. The live client's journal (installed 0.1.2,
  Wayland) showed `action=previous-pane` landing in a neighboring worklane while
  he scrolled. A live `evtest` capture of his Logitech G502 confirmed the trigger:
  over 60 s it sent 6 isolated `REL_HWHEEL -1` notches. One arrived 6 ms
  into a vertical scroll stream (followed 0.3 ms later by the client's
  `previous-pane`). Five arrived from light wheel contact with no vertical
  motion within 24 s. Jason did not intend any tilt.
- Confirmed cause: the Linux horizontal/Shift-wheel gesture called the global
  `select_adjacent_pane`, which walks every pane of every worklane and wraps.
  Source `PaneStripView.settleAdjacentPane` only settles the adjacent column of
  the current strip and stops at the edges. A "vertical-stream window" filter was
  considered and rejected: it could not catch the five stray tilts that had no
  vertical motion nearby.
- Repair:
  - The scroll gesture now uses the existing in-worklane
    `apply_focus_operation(Left/Right)` (parity) and emits a bounded
    `pane-scroll-switch direction=… result=focused|edge` receipt.
  - At Jason's direction, an unshifted discrete wheel tilt never switches panes
    and stays with the terminal (`linux.wheel-tilt-no-pane-switch`).
  - Shift+wheel and precise horizontal swipes still switch columns.
  - Keyboard next/previous pane and Peek are unchanged.
- Unit regression: FAIL before the repair (`dx=-1` unshifted wheel returned
  `Previous`), PASS after. Strict release-safe Clippy for `zentty-linux` and
  ShellCheck (warning severity) PASS.
- `rust-source-ux-x11` gained Shift+wheel left/edge/right, tilt, and
  single-pane-worklane checks. Each check is ordered by a typed probe that the
  expected real PTY must answer. The full journey FAILs before the wheel
  section at HEAD for reasons independent of #211, tracked in #212:
  - the divider returns to 286 px rather than 278–282;
  - `empty-dark-x11` differs from its baseline;
  - an intermittent "pane menu did not open Rename Pane" failure, seen in a
    HEAD run and a repaired-build run;
  - one unlocalized silent abort after `pane-controls-revealed-x11`.
- Diagnostic-only evidence: an uncommitted copy that changed only the divider
  return coordinate (288 → 282) and a scratch visual map with this actor's
  baselines uncompared. One repaired-build run passed the complete two-column
  wheel section (Shift left focused, left edge without wraparound, Shift right
  focused, tilt buttons 6/7 not switching) and continued past it. The basis is
  its later `pane-controls-*` visual receipts; the journey deleted its product
  log on that run's silent exit. This is not a
  journey PASS. The journey-level red and the single-pane-worklane section have
  not been observed. Live QA on the G502 is pending.

## Source UX journey triage and menu tooltip repair (#212)

- Triage of the failures that blocked `rust-source-ux-x11`:
  - The sidebar divider return expectation predates the wide 8 px sidebar
    handle (`51bf7e73`). The narrow handle measured its grab offset from an
    extended hit area, so the drag to 288 used to land on 280. It now lands on
    286. The journey now derives the return point from the grab offset and
    keeps the 278–282 assertion.
  - The same handle moved the pane strip 8 px right, so the column-divider
    press landed inside pane-1. The journey now derives the strip origin as
    window width minus the observed viewport. Its width extraction no longer
    aborts silently under `pipefail`.
  - The forward drag read its allocation receipt after a fixed 0.2 s sleep. It
    now waits, bounded, for the allocation receipt that follows
    `sidebar-preferred-width`.
- Confirmed product defect: a sidebar row's ⋮ "Pane actions" tooltip appeared
  over the open menu's first item ("Rename Pane…") and swallowed the click.
  A real-display screenshot taken before the click showed the tooltip covering
  the target. A real user would hit the same defect. Every tooltip-bearing menu
  button now suppresses its tooltip while its menu is open
  (`menu_tooltip::suppress_while_open`). After the repair, three runs passed
  the Rename step, which had failed three of three immediately before.
- The ApplicationShell ownership validator compared jq's codepoint-sorted
  lists with locale-collated `sort`, so it failed under `en_US.UTF-8` at clean
  HEAD. It now pins `LC_ALL=C`. The contract and its negative tests PASS; it
  also registers `scroll_switch_pane`.
- Not resolved: every visual baseline reached so far differs from current
  presentation, and later journey steps still fail (palette disabled-state
  receipts, arranged-pane post-layout scroll). Jason directed that this old
  pixel-baseline journey not be resurrected. Product-cell status was not
  updated from these runs; no journey PASS is claimed.
- Jason authorized commit, push, and package installation without restarting
  the live client. Live QA of #211 and the tooltip repair is pending.

## Client abort on mouse-clicking an Agent Status row

- Field report, 2026-10-31 16:38:56: the whole client (PID 5606) aborted when
  Jason opened Agent Status from the gear icon and clicked the row for pane-54,
  the fifth worklane. Journal:
  `pane_control_bypass.rs:62 RefCell already mutably borrowed`, then
  `panic in a function that cannot unwind`. Every window in the process died.
- Confirmed cause: `activate_attention_target` holds the shell mutably and calls
  `dismiss_status_popovers`. Unmapping the popover under the pointer makes GTK
  synthesize a crossing synchronously, so the window's capture motion controller
  from #203 (`ca9f824e`) called `shell.borrow()` inside that mutable borrow. The
  stack shows the signal emitted from the `gtk_widget_unmap` cascade.
- Keyboard activation does not cross, which is why the existing journeys that
  activate rows from the keyboard missed it.
- Repair: all six Ctrl observers go through `BypassObserver`. A blocked update
  waits for one idle, which applies the newest state rather than the state it
  captured, because GDK events outrank idles and may have superseded it.
- Regression: `ZENTTY_AGENT_IPC_SCENARIO=fleet-pointer linux/tests/rust-agent-ipc`
  performs a real X11 click and then checks that the pane still takes input.
  - Before the repair: 6 of 6 runs aborted with the panic above. A separate
    scratch reproduction aborted 9 of 9.
  - After the repair: 6 of 6 PASS, scratch reproduction 3 of 3 survived, the
    full X11 `rust-agent-ipc` run PASS, and the #203 Ctrl-bypass journey
    (`rust-divider-layout-driver`, X11) PASS.
- Early versions of the regression were invalid. A sweep of clicks hid the
  crossing, and a single click 4 of 5 times activated nothing and did not abort.
  Waiting for the attention commit and settling first made the abort
  deterministic. Hypothesis, not confirmed: a click racing a popover re-render
  has no target, which would be a separate missed-click bug for real users.
- Not tested: Wayland (the scenario is X11-only) and Bell inbox rows, which
  reach the same `activate_attention_target` call.
- Not resolved, tracked in #143: the row claimed input was needed when it was
  not. pane-54 (Codex) reported `NeedsInput GenericInput` on all 12829 status
  lines from 00:01 until the abort, so the state was stuck for over 16 hours.
  Hypothesis, not confirmed because the entry predates the journal window read:
  `apply_codex_terminal_notification` maps any Codex OSC 9, including genuine
  completion, to `NeedsInput`. Those lines also log an unchanged status every
  few seconds, which the logging policy says not to do by default.

## Public CI subset reduced to what it can run

- Jason chose to drop the cells blocking advisory CI and track them as gaps
  rather than repair them now. `known_gaps` in `linux/ci/pr-subset-v1.json`
  names them, and the validator requires each to be tracked, explained, present
  in the matrix and absent from `selected_cells`.
- Dropped: `fractional-scale-wayland` and `scale-xwayland` (#213), and
  `shortcut-binding-runtime-wayland` (#214). `scale-xwayland` was not in the
  original request. It runs the same script, which unconditionally requires
  ImageMagick, so keeping it would have left CI red. CI never reached it, so its
  failure there is inferred from the script.
- Public CI therefore no longer exercises compositor scaling or runtime shortcut
  binding. The matrix cells and their local status are unchanged.
