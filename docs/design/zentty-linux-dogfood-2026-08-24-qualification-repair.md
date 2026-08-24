# Zentty Linux qualification failure retirement dogfood

Date: 2026-08-24
Tracking: GH-88 and child issues GH-89 through GH-94

This append-only record starts the retirement of the non-pass outcomes from the
2026-08-23 local qualification receipt. The tracker issues own acceptance
criteria and test order. No full matrix rerun is permitted until isolated and
cluster evidence is green.

## GH-89: staged journey reproduction

- The preserved matrix logs initially suggested both staged journeys lost
  `build-metadata`. An independent X11 rerun proved that this was not an
  artifact deletion race: the staged bundle never copied that optional file.
- The actual cause of the metadata read was environment leakage introduced in
  GH-83. `qualify-local` exported the real Gemini binary/version around the
  entire matrix. Every invocation of `rust-agent-ipc`, including staged smoke,
  interpreted the mere binary variable as a request to run installed Gemini.
  That journey then read profile metadata which is not part of this deliberately
  assembled test bundle.
- Repair decision: only the two authoritative agent-integration cell commands
  select the required Gemini journey. `qualify-local` may put the reviewed pnpm
  bin directory on PATH, but it must not inject journey-selection variables
  into unrelated cells. The actor keys execution from
  `ZENTTY_REQUIRE_REAL_GEMINI=true`, never from an ambient binary variable.
- After the leaked Gemini journey no longer obscured the run, the independent
  X11 staged journey reached a second, real failure. Exact child-line mapping
  proved it was not `kill-window`: the public `zentty version` returned a full
  40-character commit while the ratified CLI contract requires 12 characters.
- This drift began when About/package provenance correctly changed
  `zentty_revision` to the full hash and the same value was accidentally reused
  as `ZENTTY_BUILD_COMMIT`. The repair keeps the full hash in build metadata and
  derives a separately validated 12-character display revision for the public
  CLI and embedded About surface.
- A current ReleaseSafe build completed in about 65 seconds. Its public receipt
  was `zentty 0.1.0 (5884df60005c)`, while `build-metadata` retained the full
  `5884df60005cec81f59bf022d203b32dc5b85035` provenance identity.
- The complete staged X11 journey then passed in its private Xvfb session:
  product smoke, the full compatibility-agent actor, tmux compatibility, and
  final staged-bundle assertion. The corresponding private Weston Wayland
  journey also passed. Neither run selected installed Gemini, and both receipts
  explicitly reported `real-gemini=prerequisite-not-requested`.
- The explicit installed-Gemini X11 scenario was then rerun against the rebuilt
  product and passed with `real-gemini=true`, proving the isolation repair did
  not disable the required journey.
- No concurrent artifact contender was involved in either reproduced failure,
  so an artifact mutex or immutable-bundle redesign would be a false repair.
  The shared environment-leak and full-vs-display provenance contracts are now
  checked by the focused orchestration contract. The authoritative full matrix
  has not been rerun.

## GH-90: controlled X11 focus reproduction

- `platform-clipboard-x11` reproduced independently and immediately with X11
  `BadMatch`, opcode 42 (`X_SetInputFocus`). A shell trace located the exact
  operation: `rust-pane-search` used a name-only `xdotool search`, accepted
  window `2097154`, and focused it without checking mapping or PID ownership.
- This actor had drifted from the shared input contract, which already binds to
  `--onlyvisible --pid "$product_pid"` and verifies the returned owner. The
  compositor warnings after the X error were unrelated cleanup noise.
- `rust-pane-search` owns three of the baseline failures, and
  `rust-task-runners` used the same stale/unmapped-window pattern. Both now
  require a mapped window belonging to their exact launched product before
  physical focus. No X error is suppressed and no alternate window is used.
- The mapped PID-owned lookup now lives once in `product-input`. Its focused
  runner rejects a missing mapped window and a visible window owned by another
  process, while retaining the exact successful XTEST sequence.
- Independent reruns passed for platform clipboard, terminal input, task
  runners, and the complete composite Ghostty API product-usage cell. The
  latter covered API audit, closed-pane restoration, full pane search, and tmux
  compatibility in one private X11 session.
- Platform clipboard and task runners were then deliberately run concurrently
  in two private Xvfb sessions. Both passed with distinct 64-hex session IDs,
  proving the repair scopes discovery by process/session rather than serializing
  unrelated X11 cells.

## GH-95: seven-day Rust crate quarantine

- An operator alert about an active Rust supply-chain attack interrupted visual
  qualification. The existing product boundary was useful but incomplete:
  `Cargo.lock` is committed and every authoritative Cargo build/test invocation
  uses `--locked`, so ordinary qualification cannot silently select a new
  publication. A deliberate `cargo update`, however, had no publication-age
  policy.
- Cargo 1.97.1 provides crates.io `pubtime` index data but stable Cargo rejects
  `-Z min-publish-age`. Adding only repository configuration would therefore
  advertise protection that the product toolchain did not enforce.
- A real disposable resolver probe selected exact `cc = "=1.4.4"`, published
  `2026-08-21T08:20:10Z`. Pinned nightly Cargo 1.100.0 rejected it as three days
  old under the seven-day policy and suggested the eligible 1.4.3 release. No
  crate was compiled and the product lockfile was not modified.
- The first attempted repair added a named update wrapper while leaving normal
  Cargo unconstrained. The operator correctly rejected that as procedural
  guidance rather than hardening, and the wrapper was removed before commit.
  The project now pins `nightly-2026-08-17` itself and enables native
  `min-publish-age` in `.cargo/config.toml`, so ordinary unwrapped Cargo commands
  inherit denial. GH-96 requires migration to the first stable Cargo release
  supporting this feature; the nightly pin is explicitly temporary.
- The independent audit intentionally does not trust resolver behavior because
  native Cargo permits young versions that are already locked. It enumerates
  every crates.io package record in `Cargo.lock`, reads the corresponding
  sparse-index records, requires canonical UTC publication times, and emits a
  machine-readable PASS/EXEMPT/FAIL receipt. Missing index records, missing or
  malformed times, and environmental absence are failures rather than passes.
- The first current-lock audit covered 77 crates.io packages: 77 PASS, 0 FAIL,
  0 exceptions. The youngest selected crate was
  `toml 1.1.4+spec-1.1.0`, published `2026-07-28T19:03:26Z` and 2,286,256
  seconds old at the receipt time. The oldest was `block-buffer 0.10.4`,
  published `2023-03-09T02:08:25Z`. No exception was needed.
- Deterministic negative tests now reject a too-new package, missing and
  malformed publication times, untracked authorization, expired and stale
  exceptions, a false PASS receipt, a resolver-policy override, and an
  unauthorized update. The valid paths prove both an old package and an exact
  Jason-authorized exception. CI consumes this as an advisory check and does
  not turn its receipt into release authority.
- **Audit correction before commit:** the initial independent audit consumed
  Cargo's active metadata graph, not every record retained in `Cargo.lock`.
  That silently omitted 14 inactive optional/target packages and made the
  preceding 77-package result incomplete. The complete lockfile contained
  `cc 1.4.4`, published only three days earlier. Native Cargo proved the gap by
  rejecting an ordinary `cargo update -p cc --precise 1.4.4`; the audit was
  repaired to parse all lockfile package records directly and then rejected the
  same crate. Cargo downgraded it to eligible `cc 1.4.3` without an override.
- The corrected complete-lock audit covers 91 crates.io packages: 91 PASS,
  0 FAIL, 0 exceptions. Its youngest package is now `wayland-backend 0.3.17`,
  published `2026-08-14T21:50:33Z` and more than nine days old. The incomplete
  77-package receipt is retained here as a discovery, not represented as final
  evidence.
- The initially selected `nightly-2026-08-24` resolved dependencies and passed
  tests, but strict all-target Clippy reproducibly hit a Rust compiler ICE while
  laying out GTK/GIO async opaque types. That toolchain is not an acceptable
  project pin. `nightly-2026-08-17` supports the same native Cargo policy and
  completes strict all-target Clippy, so the pin was moved back one week. GH-96
  still requires stable migration as soon as stable Cargo supports the policy.
- Final validation on the selected toolchain passed the deterministic policy
  suite, the 91-package complete-lock audit with zero exceptions, strict
  all-target Clippy, the complete workspace/all-target Rust suite, CI manifest
  and workflow contract tests, and a ReleaseSafe product build. The first
  sandboxed workspace run could not bind its real Unix test sockets and failed
  explicitly with `EPERM`; the required elevated rerun exercised those sockets
  and passed. This was environmental absence, not converted into a pass.
- A final ordinary, unwrapped `cargo update -p cc --precise 1.4.4` on
  `nightly-2026-08-17` was rejected because the crate was three days old versus
  the seven-day minimum. The rejected probe left `Cargo.lock` on eligible
  `cc 1.4.3`; the complete-lock audit remained 91 PASS and zero exceptions.
- Final diff review caught an orchestration defect before commit: the first
  `build-local` edit placed the audit after the build-metadata environment
  assignments, causing those assignments to apply to the audit instead of the
  following Cargo build. The audit now runs as a separate command immediately
  before the intact Cargo environment block. A structural regression test fixes
  that ordering and adjacency, and the repaired ReleaseSafe build passed.
