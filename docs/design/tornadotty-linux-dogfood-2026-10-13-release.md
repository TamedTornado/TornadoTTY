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
