# Zentty Linux dogfood: Ghostty appearance and live reload

Date: 2026-08-12
Tracking: GitHub issue #20

## Initial audit

- **Discovery:** the safe Rust embedding adapter has surface construction and
  terminal actions but no runtime configuration update operation. Recreating
  surfaces would restart PTYs and violate the source behavior.
- **Ghostty boundary:** Ghostty already owns default config loading and propagation
  through `CoreApp.updateConfig`. The missing capability is only a narrow GTK
  embedding entry point; configuration parsing must not be duplicated in Zentty.
- **Upstreamability decision:** add one boolean runtime reload function to the
  experimental embedding header, version script, Zig implementation, sys binding,
  and safe adapter. Product-specific watching, persistence, settings policy, and
  theme UI stay in Zentty.
- **Source behavior:** source theme mode remembers independent dark/light choices,
  serializes automatic mode as `dark:<name>,light:<name>`, clamps opacity, preserves
  unrelated Ghostty configuration, resolves included files, and reloads live
  surfaces.
- **Qualification risk:** a log-only reload test is insufficient. The product
  journey must retain the same child PID and terminal scrollback while observing a
  real Ghostty runtime property change on multiple already-open surfaces.

Discoveries, failures, repairs, evidence, and remaining uncertainty will be
appended during implementation.

## Implementation record

- **Ghostty change:** branch `zentty/gtk-embed-reload-config`, commit
  `f4e85f032a0118eca32d2179b4f379a3769c7bb2`, adds exactly one exported
  operation. It loads Ghostty's normal configuration stack and invokes the
  existing core update path. The public boundary returns `false` for a null or
  stale runtime. No Zentty theme, shortcut, persistence, or UI policy entered the
  Ghostty fork.
- **Engine tests:** the focused `runtime reload` Zig test, the full GTK embedding
  library test target, and Zig formatting passed before the Ghostty commit was
  pushed.
- **Product boundary:** the raw sys declaration is wrapped by the existing
  main-thread-only `GhosttyRuntime`; no second runtime, surface registry, or
  configuration parser was introduced. `app.reload_config` routes through the
  existing shortcut registry and action router and has no invented default key.
- **ABI audit discovery:** the first old/new mismatch run failed with
  `fixture identity or hardening is invalid`. The new export had correctly made
  the authoritative API ledger stale. The ledger now records the 27-commit,
  16-export fork delta, the safe Rust owner, the real product caller, and the real
  journey. The normalized API audit and deliberate historical/current library
  mismatch then passed. `nm -D` reports the new operation under
  `GHOSTTY_GTK_EMBED_1.0`.
- **Qualification failure:** the first complete local qualification rerun stopped
  both ReleaseSafe and Debug build prerequisites after successful compilation.
  The lower-level ABI inventory was current, but `linux/tests/abi-surface` had a
  second explicit export allowlist and rejected the new symbol. That build gate is
  intentionally independent defense, not a parallel runtime system. Its list was
  reconciled to the reviewed 16-symbol ABI before rerunning qualification; the
  failed receipt remains in this record rather than being presented as a pass.
- **Real reload discovery:** the first controlled X11 journey changed native cell
  height from 17 to 42 logical pixels and retained the same foreground PID, but
  the following unbound `Ctrl+S` no longer reached the PTY. Reload had displaced
  keyboard focus. The product now restores focus immediately and once more after
  Ghostty's configuration propagation/layout turn; the input assertion then
  passed.
- **Multi-surface strengthening:** the consolidated shortcut/settings journey now
  creates a second real Ghostty pane before reload. It requires before/after
  native cell metrics for both surfaces, a larger cell height on both, and the
  same nonzero foreground PID for each. This is not a mock or log-only assertion:
  the metrics and PIDs are read through the native Ghostty ABI from live surfaces.
- **Controlled compositor receipts:** the strengthened journey passes in both the
  private Xvfb X11 environment and the nested Cage Wayland environment with
  virtual physical input:
  `rust-shortcuts-settings-x11: PASS real-gtk-settings physical-recorder
  real-ghostty-reload preserved-pty ...` and the equivalent Wayland receipt.
- **Appearance model and persistence:** Zentty now has one pure theme-mode/theme
  specification/background-opacity model. Its tests cover source-compatible
  automatic/dark/light selection, independent dark and light theme names,
  finite percentage clamping, Ghostty serialization, duplicate-key
  reconciliation, and newline rejection. The application config accepts both the
  Linux-neutral mode names and the source application's legacy serialized names.
- **Safe writer boundary:** `ConfigStore` remains the single persistence owner.
  Appearance updates use the existing bounded, symlink-preserving atomic replace
  path. Ghostty appearance updates additionally take a bounded adjacent lock,
  preserve comments and unknown settings, retain mode `0600`, and serialize
  concurrent theme/opacity updates without losing either value.
- **Integration failure and repair:** the first source-command journey found zero
  command-palette results even though the actions were registered. The palette
  intentionally has an explicit curated item list; adding the four source theme
  commands there repaired the product projection rather than weakening the test.
- **Persistence failure and repair:** the next real X11 run reached `Use Light
  Theme` but rejected a valid existing Zentty config that lacked an `[appearance]`
  table. The writer had indexed the missing item before its optional-field helper
  required a table. It now explicitly creates the table, rejects a conflicting
  non-table value, and the regression test starts from an unrelated table only.
- **Real source-command receipts:** controlled X11 and nested Cage Wayland runs now
  open the real command palette with physical input, resolve exactly one `Use
  Light Theme` item, execute it, and observe both `theme = GitHub Light Default`
  in Ghostty's real config and `theme_mode = "light"` in Zentty's real config.
  The receipt is `PASS ... source-theme-mode real-ghostty-reload preserved-pty`
  for both backends.
- **Mutation discovery and repair:** the first focused safe mutation run found 47
  appearance-model mutants and five survivors. They exposed missing direct
  assertions for all serialized mode values, the public opacity percentage, and
  each private comment/empty-line classification branch. Those behavioral
  assertions were added; the rerun completed in 74 seconds with 44 caught, three
  unviable, zero missed, and zero timeouts. Both runs used the repository wrapper,
  two workers, `gitignore=true`, and `copy_target=false`.

## Remaining uncertainty and scope

- The reload contract still lacks direct public-boundary tests for foreign and
  stale runtime pointers, malformed-config diagnostics, teardown races, and rapid
  successive reloads. These remain explicit API evidence gaps rather than passes.
- This slice proves process-global propagation across two existing surfaces in one
  product window. A cross-window product journey is still required before claiming
  every-window behavior exhaustively.
- Theme-mode memory and safe comment/symlink-preserving theme/opacity persistence
  are implemented. The Appearance settings projection, theme resources/gallery,
  background images, file watching, and the platform blur alternative remain in
  issue #20.
- The Ghostty and Zentty configuration files are each replaced atomically, but the
  two-file operation is not a cross-file transaction. If the second write fails,
  the on-disk Ghostty theme may be newer than Zentty's remembered mode; live reload
  is deliberately withheld on that failure. This remains an explicit recovery
  design gap, not a hidden pass.
- `Use Dark Theme` currently resolves to the source application's named dark-theme
  fallback, which has not yet been bundled for Linux. The deterministic product
  journey therefore exercises Ghostty's built-in light fallback. Shipping and
  selecting the source theme resource remains required issue #20 scope.
- Full Linux qualification remains impossible while the authoritative matrix has
  BLOCKED, XFAIL, or NOT_IMPLEMENTED cells. Passing results here are scoped to the
  presently exercised feature journeys only.

## Qualification checkpoint

After the ABI allowlist repair, the complete presently executable local run
passed in 439.61 seconds. The machine summary reports `PASS=115`, `FAIL=0`,
`BLOCKED=7`, `XFAIL=1`, and `NOT_IMPLEMENTED=23`; the implemented local suite
passed, while release and full Linux qualification correctly remain not passed.
Suppression governance was accepted; the applicable result remains **PASS with
reviewed suppressions**, not an unsuppressed-clean claim.

After the appearance model, safe writers, and source theme-mode actions were
added, the complete presently executable suite was rerun rather than relying on
the earlier reload-only receipt. It passed in 438.55 seconds with unchanged
declared totals: `PASS=115`, `FAIL=0`, `BLOCKED=7`, `XFAIL=1`, and
`NOT_IMPLEMENTED=23`. The implemented local suite passed; release and full Linux
qualification correctly remain not passed. Suppression governance again reported
accepted, so the Valgrind characterization remains **PASS with reviewed
suppressions**.

## Bundled source fallback theme

- **Source audit:** `GitHub-Dark-Personal` is not a Ghostty built-in. The macOS
  product bundles the exact file and, when a written theme specification refers
  to it, installs a standalone copy into the user's Ghostty themes directory so
  plain Ghostty can resolve the shared configuration.
- **Packaging repair:** the ReleaseSafe build now stages that exact checked-in
  resource at `share/zentty/ghostty/themes/GitHub-Dark-Personal`. Both staged
  X11 and Wayland bundle journeys compare its bytes with the source resource
  before launching the real packaged product.
- **Safe install policy:** the existing `ConfigStore` owns fallback installation;
  no second appearance persistence authority was introduced. It resolves the
  installed resource relative to the executable, follows XDG config precedence,
  publishes a private `0600` file without overwriting an existing regular file,
  rejects a final symlink or non-file target, bounds resource size, and removes
  its temporary file on every publication result.
- **Harness defect exposed:** the multi-surface journey originally waited for the
  second pane's GTK projection, then sometimes sampled reload metrics before its
  native Ghostty surface and child PID were ready. A no-change real runtime reload
  is now the readiness barrier; no timing sleep or mocked surface was added.
- **Wayland input discovery:** zero-delay `wtype` could flood a newly presented
  palette so only `Use` arrived before assertions. Matching the already-used X11
  five-millisecond physical typing cadence made the real nested-Wayland input
  deterministic rather than converting absence into a pass.
- **Real receipts:** `Use Dark Theme` is exercised through the real command
  palette after a complete product restart. Controlled X11 and nested Cage
  Wayland both verify the exact installed resource bytes, `0600` mode, persisted
  dark mode, real Ghostty config, live reload, and terminal input/process
  continuity. Both staged bundle journeys also pass.
- **Full-run failure and repair:** the first complete qualification rerun failed
  only `shortcut-binding-runtime-x11`. On the loaded host, the fresh-process
  persistence assertion toggled the sidebar immediately before opening the dark
  theme palette; its deferred focus/render work overtook the overlay, so no query
  was delivered. The journey now opens and completes the palette feature first,
  then verifies the already-persisted shortcut. The exact failed matrix command
  against `build/linux-profiles/release-safe` passes after that ordering repair.
- **Mutation receipt:** the initial focused run found eight missed policy mutants.
  Tests were strengthened for automatic-mode slots, file/symlink/directory
  distinctions, exact and over-limit sizes, inspection and publication errors,
  installed-resource resolution, and concurrent-publication diagnostics. The
  final safe two-worker run tested 23 mutants in 85 seconds: 21 caught, two
  unviable, zero missed, zero timeout.
- **Corrected full qualification receipt:** after the failed X11 ordering cell was
  repaired and rerun directly, the complete presently executable matrix passed
  in 441.84 seconds. Its 146 declared cells report `PASS=115`, `FAIL=0`,
  `BLOCKED=7`, `XFAIL=1`, and `NOT_IMPLEMENTED=23`. The implemented local and
  product-boundary suites passed; release and full Linux qualification correctly
  remain not passed. Debug Valgrind remains **PASS with reviewed suppressions**:
  the preserved raw receipt reports 427 errors/contexts, 6,080 direct bytes, and
  41,394 indirect bytes; the reviewed post-suppression receipt reports zero
  errors, contexts, or leak bytes. Suppression governance was accepted. No
  environmental absence was converted into a pass.

## Unified Appearance settings and complete theme catalog

- **Source inventory and staging:** the source application ships a complete
  Ghostty theme library rather than only the named dark fallback. The ReleaseSafe
  staging path now copies the entire checked-in library. The staged-bundle cell
  compares the source and installed directories recursively and exactly; a
  missing, additional, changed, or symlink-substituted entry fails qualification.
- **One settings system:** the retained Shortcuts page and the new Appearance page
  now live under one nonmodal `Zentty Settings` shell. The focused
  `settings_shell.rs`, `appearance_settings.rs`, and `theme_catalog.rs` modules
  project UI and bounded catalog reads only. Existing `ConfigStore` remains the
  sole writer and `ApplicationShell` remains the sole live-reload authority. The
  architecture ownership contract now hashes and inventories all three modules,
  so they cannot become an unreviewed parallel settings system.
- **Catalog policy:** theme files are limited to regular, nonempty files no larger
  than 64 KiB. Parsing accepts Ghostty background, foreground, and the first 16
  palette indices, normalizes supported three-, six-, and eight-digit colors,
  and never treats file contents as markup. Bundled themes load first and an
  identically named user theme deterministically overrides them. Search and
  dark/light classification operate on the resulting single catalog.
- **Real user-precedence journey:** the controlled product journey creates a real
  user-owned `TokyoNight Moon` theme with background `#010203` before application
  startup. Appearance search must return exactly one row, selecting it must log
  `source=user background=#010203`, and the existing writers must persist the
  selected name into both real configuration paths before a native reload. This
  proves precedence through the staged product rather than only through a unit
  fixture.
- **Input-routing failure and repair:** the first unified-shell build left the old
  Shortcuts window-level `Ctrl+F` controller installed, so it captured search even
  on Appearance. Search routing now belongs only to the settings shell and follows
  the visible page. The original Shortcuts journey remains unchanged under
  controlled X11 and Wayland input.
- **Wayland focus failure and repair:** an Appearance-originated native reload
  scheduled the main-window layout render and terminal focus restoration. Under
  Cage, that work stole focus from the still-open settings window. Settings
  reloads now preserve settings focus and avoid an unrelated main-layout render;
  closing settings explicitly restores the terminal child after compositor
  activation. Command-originated reload keeps its terminal-focus behavior.
- **Controlled X11 ordering discovery:** after a native appearance reload, Xvfb's
  direct-to-toplevel synthetic input could ambiguously target the main window
  during the already-established Shortcuts journey. Appearance is therefore
  exercised after, not inside, that journey. No missing event was accepted as a
  pass, and the existing shortcut assertions were not weakened.
- **Readiness and scale discoveries:** Wayland required the preceding dark-theme
  reload to complete before reopening settings. With the full catalog, keyboard
  traversal through hundreds of visible rows could not reasonably reach opacity;
  the journey now narrows to one real theme before traversing controls. Rapid
  opacity key changes also caused repeated synchronous reload work and dropped
  physical events. Product opacity changes are now coalesced for 120 ms, and the
  journey uses one deterministic `Home` event while still asserting the real
  writer, native reload metric, and uninterrupted PTY.
- **Mutation environment discovery:** the first focused mutation invocation was
  attempted inside the restricted sandbox. Its unmutated baseline correctly
  failed because an unrelated real kernel-listener test received `EPERM`; that is
  not a product failure and was not converted into a passing receipt. The
  campaign was rerun with the required host permissions through the repository
  wrapper. Every run retained `gitignore=true`, `copy_target=false`, two workers,
  and the bounded output directory, so the historical multi-gigabyte scratch-copy
  failure cannot recur.
- **Mutation repairs:** an early completed campaign exposed missing direct checks
  for the catalog size boundary, BT.709 channel coefficients and exact luminance
  boundary, RGB projection, ignored-line classification, palette index boundary,
  and HOME fallback. The pure eligibility and ignored-line policies are now
  directly observable, and the tests cover the maximum and over-maximum sizes
  without depending on filesystem-parser side effects. The final mutation totals
  are 95 tested in four minutes: 89 caught, six unviable, zero missed, and zero
  timeouts. The unmutated baseline passed with real kernel-listener access.
- **Focused compositor receipts:** after opacity coalescing and real user-theme
  precedence were added, the complete consolidated settings journey passed under
  both private Xvfb and nested Cage. Each receipt reports
  `PASS unified-settings theme-catalog ... real-ghostty-reload preserved-pty`;
  neither run used the developer desktop or a mocked terminal.
- **Workspace-test environment receipt:** an unelevated all-workspace run failed
  eight real Agent IPC CLI cases at Unix-socket creation with `EPERM`. This was
  the same restricted-kernel condition seen by mutation baseline, not an ignored
  test or application regression. The complete locked workspace suite was rerun
  with host kernel access and passed, followed by strict all-target Clippy, Rust
  formatting, and diff hygiene.
- **First full-matrix failure:** the first complete rerun did not pass and is not
  presented as one. Two older product journeys failed under the concurrent
  matrix: Wayland bookmark import/export did not observe focus on its name dialog,
  and X11 source UX did not observe the rendered divider move after Resize Pane
  Down. Neither path is touched by this slice. The exact controlled commands then
  passed in isolation, including real chooser/file persistence and real pointer/
  keyboard divider behavior. That establishes contention-sensitive harness
  timing, not permission to erase the failed receipt; a corrected complete rerun
  remains required below.
- **Second full-matrix failure and harness repair:** the complete rerun repaired
  the bookmark cell but again found the X11 divider assertion and additionally
  delivered only `Se` from the physical Wayland `Settings` query under load. The
  divider harness had waited for any newer layout log, so unrelated deferred
  layout work could satisfy its readiness predicate before the requested size was
  rendered. It now waits for the observed numeric allocation to cross the exact
  expected boundary, then for the reverse command to restore it. Wayland typing
  retains real `wtype` input but uses a 20 ms inter-key cadence in this journey,
  preventing load from dropping the tail of a physical query. Neither repair
  adds a mock, retry-pass, or log-only product assertion.
- **Third full-matrix failure and final pointer readiness repair:** numeric divider
  readiness worked and the Wayland settings journey passed under load. The X11
  source journey advanced to its final teardown, then clicked pane 2's Close
  coordinate before the post-layout hover controls had actually become visible.
  Each teardown click now first requires the target pane's real hover controller
  to report `state=shown`; only then does `xdotool` click the pane-local control.
  The click must still emit the exact target action and close the real surface.
  This replaces an implicit compositor-speed assumption, not a product assertion.

The remaining issue #20 scope is still explicit: additional source settings
sections, external config/include watching, background images, OpenCode theme
sync, and cross-window reload evidence are not claimed by this slice.

### Final qualification receipt

After the contention-sensitive product journeys were repaired and each exact
failed cell passed directly, the complete presently executable matrix passed in
511.16 seconds. All support-runner tests passed. The 146 declared cells remain
`PASS=115`, `FAIL=0`, `BLOCKED=7`, `XFAIL=1`, and `NOT_IMPLEMENTED=23`.
Implemented-local and product-boundary qualification passed; release and full
Linux qualification correctly remain not passed.

Debug Valgrind remains **PASS with reviewed suppressions**, not an unsuppressed
clean claim. Its preserved raw receipt reports 427 errors/contexts, 6,160 direct
bytes, and 41,428 indirect bytes. The reviewed post-suppression receipt reports
zero errors, contexts, direct bytes, or indirect bytes and accounts for all 427
suppressed contexts. Suppression governance passed and the machine summary binds
both receipt hashes. ReleaseSafe Valgrind remains XFAIL and no suppression was
broadened.
