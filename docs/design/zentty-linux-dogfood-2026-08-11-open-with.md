# Zentty Linux dogfood — Open With

Date: 2026-08-11
Issue: GH-18 (`project.open-with` subset)

## Slice contract

The test-first contract is
[`linux-open-with-feature-plan.md`](linux-open-with-feature-plan.md). This log is
updated during implementation rather than reconstructed at the end.

## Source and platform discoveries

- The initial plan incorrectly described `enabled_target_ids` as an ordered
  presentation list. Direct inspection of
  `OpenWithPreferencesResolver.enabledTargets` showed that the source converts
  it to a set, then emits built-ins in catalog order and custom applications in
  configured order. The core resolver and plan were corrected before Linux
  runtime integration, and a focused unit test pins this behavior.
- Source Zentty keeps target discovery separate from preference resolution.
  Enabled targets are filtered by availability, and the configured primary
  falls back to the first enabled available target.
- Source custom applications are opaque stable ID, name, and application path
  records. Linux will narrow `path` to a real executable and pass the canonical
  directory as one argv rather than interpreting a command string.
- Source uses the platform workspace API rather than spawning editor-specific
  command lines. The Linux equivalent must therefore preserve the desktop
  application boundary for `.desktop` targets and use direct argv only for
  explicitly configured executables.
- A final launcher review caught an unsafe source-to-Linux assumption before
  qualification: installed `codex`, `claude`, and `antigravity` command names
  are not evidence of desktop-editor directory-opening semantics. They were
  removed from automatic executable discovery. Their source IDs remain
  reserved, avoiding accidental interpretation of a project directory as an
  agent prompt or unsupported CLI argument.
- Final standards review caught a second launcher defect before commit:
  `xdg-terminal-exec` follows the proposed Default Terminal Execution
  Specification's monolithic `--dir=<path>` option, not the initially modeled
  `--working-directory <path>` pair. Core launch policy now represents this
  directory-option contract explicitly. The controlled real-process journeys
  discover `xdg-terminal-exec` through `PATH` and assert one exact
  `--dir=<canonical path>` argv on both compositors; this is separate from the
  direct custom-executable path contract.
- The first controlled X11 launch disproved an integration-test assumption:
  although Zentty calls `GAppInfo.launch_uris` with the unit-tested percent-
  encoded `file:` URI, GLib resolves that local URI to a canonical path before
  expanding the controlled desktop entry. This is platform-owned behavior, not
  evidence that Zentty bypassed GIO. The real desktop handler receipt now
  asserts that one exact canonical local target arrives, while the core test
  separately pins the URI passed into the GIO boundary.

## Construction record

- Planning began from GH-18 and the source `OpenWithCatalog`,
  `OpenWithPreferencesResolver`, `ProjectFileResolver`, `OpenWithService`, config
  normalization, focused-context, chrome, and command-palette code. No Linux
  target list or behavior was invented before that audit.
- Core config parsing now ports source normalization for malformed custom apps,
  duplicate IDs and paths, reserved built-in IDs, enabled-ID deduplication and
  primary fallback. The first mutation pass caught three insufficient tests;
  independent empty-field, custom-primary and requested-secondary-primary
  cases were added, after which all 21 viable core mutants were caught (two
  compiler-rejected mutants were unviable).
- Linux discovery uses the real GIO default `inode/directory` application,
  source-compatible editor IDs resolved to canonical executable files, an
  explicit system terminal target, and normalized custom executable paths.
  Runtime launch is either `GAppInfo.launch_uris` or direct `Command` argv;
  neither route evaluates shell text.
- The first sandboxed nested-X11 attempt could not create an X socket because
  this managed environment maps `/tmp/.X11-unix` ownership to `nobody` inside
  the sandbox. Running the existing isolated compositor harness with its
  required GUI escalation restored the intended boundary; no harness exception
  or environmental pass was added.
- The first controlled X11 product run exposed a bad fixture target: unknown
  target IDs are correctly removed by source normalization, so they cannot
  represent an installed application that disappeared. The fixture now enables
  the valid but Linux-unavailable `xcode` ID and verifies the explicit
  `unavailable=xcode` discovery receipt.
- The first Wayland run typed into the terminal before Cage had completed
  focus. The journey now requires the real `focus-pane` receipt and a fresh
  command-palette-shown count before typing. Both X11 and Wayland subsequently
  passed with real staged Ghostty terminals, physical compositor input, a real
  controlled desktop application, and a real custom executable.
- A targeted Linux-runtime mutation pass initially failed its unmutated
  baseline because an unrelated real kernel-listener test cannot bind inside
  the network sandbox; the required escalated rerun was used. Integration-only
  shell/GIO/focused-pane boundaries are excluded from the pure-policy mutation
  command and remain covered by both real compositor journeys. The first pure
  runtime pass caught two remaining assertion gaps around blank/reserved custom
  IDs; assertions were tightened before the final rerun.
- The second pure runtime pass reduced the survivors to duplicate-ID insertion
  polarity: rejecting the first duplicate and retaining the second still
  produced one ID, so a count-only assertion was insufficient. The test now
  pins the first app's canonical executable path. The exact remaining mutant
  was then caught; across the governed pure-runtime set all 15 viable mutants
  were caught and one compiler-rejected mutant was unviable.
- Final review also found a remote-context race: the action consulted only the
  asynchronously projected SSH identity, so an action issued before that cache
  updated could have opened the local working directory of the SSH process.
  The action now synchronously probes the focused foreground process before
  resolving `/proc/<pid>/cwd`. The real X11 and Wayland journeys enter a real
  authenticated OpenSSH session and prove that Open With is rejected without
  invoking the controlled desktop application.
- The first post-review X11 rerun exposed a harness path bug rather than a
  product failure: the matrix supplies a repository-relative staged binary,
  but the journey changes into its controlled project directory before exec.
  The harness now canonicalizes the verified staged binary first, preventing
  the working-directory fixture from changing which executable is addressed.
- The next X11 run exposed an XTest input-lifecycle edge: activating a palette
  row closes its focused widget before every synthetic modifier release is
  guaranteed to arrive, and the subsequent SSH command was interpreted as
  application shortcuts. An explicit root-window modifier release did not
  correct the application-window state.
- A root-window modifier release was insufficient on the next rerun because
  the owned application window still observed Control during XTest text entry.
  Xdotool's scoped `--clearmodifiers` also did not make a post-palette terminal
  command trustworthy. Rather than add more synthetic-input repair, the
  journey now starts a second real product whose initial command is the real
  authenticated SSH launcher, waits for the independent remote-actor title,
  then uses a fresh physical palette route to prove rejection. This removes
  the unrelated focus-transition ambiguity from the product assertion.
- The first two-process X11 run proved the remote rejection but its final local
  launch assertions accidentally read the second product log. Local and remote
  logs now have explicit identities and lifecycle scanning covers both. The
  corrected X11 journey passed with real desktop, custom-executable, OpenSSH,
  Ghostty, compositor-input, and canonical-target evidence.
- The final runtime mutation invocation initially used cargo-mutants'
  file-glob `--exclude` option for a mutation-name regular expression. Its
  reported 30 mutants made that mistake visible before substantive execution;
  the run was interrupted and corrected to `--exclude-re`. The governed pure
  set then tested 16 mutants: 15 caught and one compiler-unviable.
- Chrome review found that the primary control always displayed a folder even
  when the primary target was an editor or terminal. Primary and menu rows now
  derive their symbolic icon from the explicit target kind.

## Remaining work and uncertainty

- The first full qualification rerun found one unrelated failure in the real
  X11 sidebar journey: under the concurrent matrix load, pane 8 did not report
  terminal readiness within its six-second per-pane deadline. The exact cell
  passed immediately in isolation, identifying load sensitivity rather than an
  Open With regression. The real-terminal deadline is now 12 seconds and its
  controlled pane commands live for 30 seconds, retaining a bounded physical
  journey while matching the harness comment that all nine PTYs survive the
  concurrent run. Full qualification must be rerun; the failed receipt is not
  being reclassified as a pass.
- The second full rerun reached all sidebar assertions but then failed an
  unrelated natural-child-exit tail after 33 seconds: later-created `sleep 30`
  panes had correctly not exited within a fixed 20-second wait. Natural pane
  lifecycle already has dedicated Debug/ReleaseSafe, backend, compositor, and
  cardinality cells. The sidebar journey now ends after its real PTY, overflow,
  pointer-drag, keyboard-reorder, and row-stability assertions; its trap owns
  teardown rather than duplicating lifecycle qualification.
- That rerun also exposed a latent negative self-test bug in the nested-X11
  wrapper: production reservations deliberately span displays 2000–11999, but
  the fake `xvfb-run` accepted only four-digit values. A PID-derived reservation
  above 9999 therefore returned fixture code 105. The self-test now validates
  the wrapper's exact numeric range. This was not converted into environmental
  success.
- The third concurrent run proved the sidebar failure was not PTY startup: it
  again stopped at pane 8 after automatic reveal had scrolled the original
  fixed pointer coordinate away from the New Worklane button. Source Zentty
  owns Command+N for New Worklane; Linux now exposes the idiomatic exact Ctrl+N
  mapping through the existing action, while Ctrl+Shift+N remains New Window.
  The journey proves the visible button once, then uses the real physical
  source-owned command for the remaining overflow lanes instead of repeatedly
  clicking a coordinate whose rendered target changes.
- The first focused shortcut test did not compile because this file's test
  module uses an explicit `super` import list and the new pure predicate was
  omitted. The import was added; no wildcard or second shortcut implementation
  was introduced.
- The fourth full run reached and logged the accepted reorder, then asserted
  drag cleanup before GTK's immediately following drag-end callback wrote its
  receipt. The resulting log contained `visual=cleared` after the failure
  message, proving an observation race. The journey now waits a bounded five
  seconds for that exact lifecycle receipt before asserting it;
  no product delay or environmental pass was added.
- The corrected full run completed every presently executable support and
  matrix cell. Declared totals are `PASS=105`, `FAIL=0`, `BLOCKED=7`,
  `XFAIL=1`, and `NOT_IMPLEMENTED=21`. The implemented local suite passed;
  release and full Linux qualification did not pass because declared non-PASS
  cells remain. Debug Valgrind is **PASS with reviewed suppressions**: raw
  `427` errors/contexts with `6,080` definite and `41,395` indirect bytes;
  post-suppression `0` errors/contexts and `0` definite/indirect bytes, with
  all `427` errors/contexts tracked as suppressed. Both raw and suppressed
  receipts are preserved by the machine-readable summary.
- Correcting the system-terminal launch contract changed executable code, so
  the full matrix was rerun rather than reusing that receipt. Under the
  concurrent run, `product-pane-lifecycle-debug-x11-epoll-multi` exhausted its
  ten-second initial-surface deadline at 10.09 seconds; its captured product
  log then showed both original real PTYs becoming ready normally. This was an
  unexpected executable-cell failure, not a pass and not evidence that the
  Open With terminal repair regressed lifecycle ownership. Initial product
  startup in that journey now has a bounded 20-second deadline; its later
  interaction waits retain the shared ten-second bound. A replacement full
  receipt is required before commit.
- In that replacement run, all 105 declared-PASS matrix cells succeeded,
  including the formerly timing-sensitive lifecycle cell, but the concurrent
  nested-X11 wrapper self-test failed before its fake display probe. The
  wrapper's fake `xvfb-run` had been repaired to accept the production
  `2000..11999` reservation range, while its fake `xdpyinfo`, `glxinfo`, and
  child probe still encoded a decimal-shape regex that rejected `10000..11999`.
  A PID-derived five-digit reservation exposed the contradiction. All four
  controlled boundaries now validate the same numeric range; the support
  failure remains a failure until the complete qualification entry point
  produces a replacement receipt.
- The final replacement entry-point run completed in 402.32 seconds: every
  support test and every presently executable matrix cell passed. Declared
  totals remain `PASS=105`, `FAIL=0`, `BLOCKED=7`, `XFAIL=1`, and
  `NOT_IMPLEMENTED=21`. The machine receipt claims only implemented-local and
  product-boundary qualification; release and full Linux qualification remain
  false. Debug Valgrind is **PASS with reviewed suppressions**: the preserved
  raw receipt reports `427` errors/contexts, `6,080` definite bytes, and
  `41,395` indirect bytes; the preserved post-suppression receipt reports zero
  errors/contexts and zero definite/indirect bytes, with all `427` contexts
  tracked as suppressed.
- Final code review found that direct launch plans converted the canonical
  directory to a lossy UTF-8 `String`. That would silently change a valid Linux
  path containing non-UTF-8 bytes before handing it to an editor or terminal.
  Executable arguments now remain `OsString` values end to end, including the
  monolithic `xdg-terminal-exec --dir=` argument, and a focused regression test
  uses an actual directory containing byte `0xff`. Because this changes
  executable code after the receipt above, mutation and the full qualification
  entry point must run again before commit.
- The post-repair core mutation run tested 23 mutants: 21 were caught and two
  were compiler-unviable. The final full entry-point run completed in 405.61
  seconds with all support tests and all presently executable cells passing.
  Declared totals and honest claims are unchanged (`PASS=105`, `FAIL=0`,
  `BLOCKED=7`, `XFAIL=1`, `NOT_IMPLEMENTED=21`; implemented-local passed,
  release and full Linux qualification not passed). The latest raw and
  post-suppression Valgrind totals are unchanged from the reviewed receipt
  above.

- The source project-file resolver substitutes only Xcode workspace/project
  bundles. Xcode is deliberately unavailable on Linux, so every available
  Linux target receives the canonical focused directory. This is recorded in
  the feature inventory rather than silently inventing editor-specific project
  file formats.
- No release or full Linux qualification claim is made while the matrix retains
  non-PASS cells.
