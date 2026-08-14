# Zentty Linux dogfood record — 2026-08-14

This is the active implementation record after the issue-21 attention-inbox
slice. It records discoveries, failures, repairs, real-system evidence, and
remaining uncertainty as the Linux agent fleet functionality is built.

## Cross-window agent fleet status (GH-21)

### Source contract audit

- The macOS source authority is `Zentty/UI/MenuBar/`. In particular,
  `MenuBarFleetState`, `MenuBarFleetSummary`, `MenuBarPaneSnapshotBuilder`, and
  `MenuBarStatusMenuBuilder` establish the contract; this slice does not invent
  a generic process monitor.
- Only panes with an explicit agent status belong in the fleet. The status
  priority is Waiting, Stopped, Compacting, Active, then Idle. The source groups
  Waiting with Stopped, Compacting with Active, and leaves Idle separate.
- Source interaction copy is semantic: approval is “Requires approval,” a
  question or decision is “Needs decision,” authentication is “Needs sign-in,”
  and otherwise the row says “Needs input.” A row targets one exact window,
  worklane, and pane.
- The macOS global presentation is a menu-bar status item. Linux must keep the
  same process-wide model and exact routing while providing an always-available
  in-window GTK control; an optional StatusNotifierItem is not a prerequisite
  for the usable product feature.

### Discovery and repair: activation is not resolution

- Audit of the immediately preceding attention-inbox implementation found an
  incorrect lifecycle shortcut: a successful jump to an attention row marked
  the row resolved before the agent emitted a completion or other resolving
  event. Focus proves routing, not resolution. This violated the issue-21
  acceptance criterion and could hide unresolved work.
- The application action channel is now explicitly shared by attention and
  fleet routes. Successful activation only focuses the exact target. A stale
  attention target may still be resolved as stale; a live target remains
  unresolved until the agent-event projection changes it. Fleet activation has
  a distinct `fleet-activate` receipt rather than being mislabeled as inbox
  activation.

### Implemented boundary

- `zentty-core::agent_fleet` owns the source-derived, GTK-independent fleet
  snapshot, priority, section totals, labels, context, progress, and aggregate
  accessibility copy. Focused tests cover cross-window ordering, all source
  state mappings, interaction labels, empty and mixed summaries, incomplete
  progress, and exclusion of non-agent shells.
- `ApplicationCoordinator` is the sole process-wide collector. It obtains
  enriched sidebar summaries from every live shell, builds one authoritative
  snapshot, and sends the same value to every window only when that value
  changes. This does not create a second agent-status store.
- The GTK projection provides an Agent Status control, grouped Waiting,
  Running, and Idle rows, an aggregate status dot, exact-pane actions,
  settings and standard quit routes, accessibility labels, and an Agent Status
  command-palette entry so the nonmodal view remains keyboard reachable on
  both Wayland and X11.

### Evidence so far

- `cargo test --locked -p zentty-core --test agent_fleet`: four focused model
  tests passed.
- `cargo check --locked -p zentty-linux`: passed after the process-wide
  projection was connected.
- `docs/architecture/tests/validate-application-shell-ownership`: passed after
  the authoritative ownership/action contract was updated for the fleet
  coordinator and typed fleet actions.

### Controlled-compositor discoveries

- The first X11 journey exposed two test/UX defects rather than being made to
  pass by coordinates. “Agent Status” was not a unique command-palette query
  because the Agents settings entry also matched, and a generic popover did not
  initially establish a keyboard focus target. The fleet now focuses its
  source-highest-priority row when opened, records that exact identity, and is
  separately searchable with descriptive command text.
- The controlled X11 journey now passes with two real Ghostty PTYs in two real
  windows: pane 1 publishes a real authenticated approval event, pane 2
  publishes a real running event after live reparenting, the process fleet
  reports two agents, the waiting row is first, selecting it targets
  `window-1/worklane-1/pane-1`, and subsequent physical input is observed by
  that exact PTY. Final focused session:
  `b7f4896f0e1ec13b731a77426fbf8f166e7b0050137d507f8a7bc9590192ad0d`.
- The equivalent controlled Wayland journey proved aggregation, priority,
  GTK rendering, the keyboard-accessible triple-parameter action, and correct
  application target selection, but Cage did not move compositor focus to the
  other toplevel. Input remained in the source window even when the activation
  was kept synchronous with the physical user gesture and focus was settled on
  `is-active`. This is not treated as a pass or as environmental absence.
  `agent-fleet-wayland` is an explicit stale-XFAIL under GH-21 and exits 98 only
  after reproducing this exact xdg-activation gap.
- The diagnostic frame also caught a useful visual fact: the compact fleet is
  genuinely rendered as grouped Waiting and Running rows with semantic status
  pills, rather than being inferred from logs. The temporary screenshot hook
  was removed; qualification relies on the real interaction receipts.
- The first full qualification rerun exposed two harness-governance defects.
  The feature-inventory oracle still expected the pre-fleet PARTIAL and
  NOT_IMPLEMENTED totals, and the final matrix report expanded both complete
  JSON documents into `jq` argv, crossing Linux's argument-size limit after the
  two new cells. The oracle now reflects the authoritative inventory, and the
  runner feeds those documents through bounded files with a regression source
  assertion. The next rerun reached the matrix and found one remaining stale UI
  receipt: `rust-source-ux-x11` still expected the five-control chrome string.
  It now requires the sixth source-derived Agent Status control rather than
  weakening the product back to the old surface.
- The next complete rerun passed that repair but one parallel X11 cell lost
  Xvfb between two consecutive `xdpyinfo` invocations during environment-proof
  setup. The first invocation succeeded and the product never started. This was
  neither a product failure nor permission to accept missing environment
  evidence. The wrapper now captures the authoritative proof in one bounded
  retry loop instead of performing a throwaway readiness call followed by a
  second race-prone capture; failure after the one-second bound remains fatal.
- That rerun also revealed a scope error in the consolidated journey: the new
  fleet segment had been added unconditionally to `rust-multi-window`, so its
  established Wayland PASS cell reached the fleet's deliberately tracked exit
  98. Fleet setup/assertions are now gated by the explicit fleet-only mode;
  the existing restoration journey remains its prior product contract. A
  separate attention X11 journey also missed its final PTY receipt under the
  concurrent consolidated load. Attention and fleet now declare the same real-
  session scheduler resource as the larger agent journeys rather than masking
  compositor/CPU contention with longer timeouts or retries.
- Correcting attention activation exposed an existing interaction dependency:
  the old path incorrectly resolved the attention item as a side effect, which
  happened to replace and dismiss its popover. Once activation correctly kept
  unresolved attention visible, the popover retained the input grab and the
  exact target PTY did not receive subsequent physical input. Activation now
  explicitly dismisses both status popovers before presenting and focusing the
  target; it does not mutate agent state. The repaired real X11 journey passed
  with authenticated agent IPC, inbox-row activation, and physical input in the
  exact PTY (session
  `48d3bf8b56a1c06836596c017d643b738836c613b76541caa7663eab9f7856e3`).
- The established non-fleet Wayland multi-window journey was rerun after the
  fleet-only gate repair and passed its live transfer, construction rollback,
  clean restore, SIGKILL restore, size restore, and non-final-close contracts
  (Wayland session
  `a31e146a454a81180f7011f239a5e56b0d3321512c3e6877bba7477097c11eaa`).
- The first otherwise-complete qualification run reported one X11 agent cell
  failure: its consolidated restore journey began a real two-file SCP rollback
  but did not publish completion or failure before the bound. The same restore
  journey then passed alone, and the exact full X11 agent cell passed on a
  focused rerun, so no timeout or retry was added and the failed receipt remains
  evidence rather than being relabelled.
- Diagnosis also found two installed Gemini executables: the qualification
  environment resolves the reviewed `0.53.0`, while the interactive FNM shell
  resolves `0.55.1`. The strict version guard rejected both attempts to pair an
  executable with the other version's expectation. The authoritative matrix
  retains the controlled qualification environment's `0.53.0` pin; a manual
  focused `0.55.1` adapter journey also passed in session
  `abbc0de016c36c29263d44970a7335d8e62b98a6639d776632b01184583ee4d0`,
  but does not replace the matrix receipt.

### Remaining before this slice is complete

- Token-backed cross-toplevel Wayland activation remains the tracked defect;
  the existing stale-XFAIL must fail if the defect disappears or changes shape.
- Stale/window-close rows, product progress and stopped-state journeys, and the
  optional controlled StatusNotifierItem mapping remain issue-21 scope.
- Every presently executable cell must now be run
  before commit or push. No exhaustive, release, or full-Linux qualification
  claim is permitted while the matrix retains non-PASS cells.

### Final local qualification receipt

- The final full `linux/tests/qualify-local` run passed every presently
  executable support and matrix cell in 558.85 seconds. Declared matrix totals
  are **129 PASS, 0 FAIL, 7 BLOCKED, 2 XFAIL, and 22 NOT_IMPLEMENTED**.
- The implemented local suite passed. Release qualification and full Linux
  qualification did **not** pass because the authoritative gaps remain explicit.
- Debug IBus-focus Valgrind is **PASS with reviewed suppressions**, not an
  unsuppressed-clean claim. The preserved raw receipt contains 427 error
  contexts, 6,000 definite bytes, and 41,362 indirect bytes; the reviewed
  post-suppression receipt contains zero error contexts and zero definite or
  indirect bytes. ReleaseSafe Valgrind remains explicitly NOT_IMPLEMENTED and
  tracked by `DOGFOOD-2026-08-02-RELEASESAFE-VALGRIND`, rather than being made
  green with broader rules.
- Machine-readable receipt:
  `build/linux/qualification-summary.json`. Both agent integration cells, the
  new X11 fleet cell, and the controlled stale-XFAIL Wayland fleet cell produced
  their expected outcomes in this same run.

## Agent sleep inhibition (GH-21)

### Source audit and design decision

- The authoritative macOS behavior is in
  `Zentty/AppState/NotificationStore.swift`, not an inferred
  `AgentStatusCenter` abstraction. It owns one application-wide assertion,
  acquires only for the explicit `running` phase, and delays release for ten
  seconds. Starting, needs-input, idle, unresolved-stop, and unrecognized panes
  are not qualifying sources.
- Linux retains those policy semantics in the pure
  `AgentSleepInhibitionState`. `ApplicationCoordinator` is the single
  process-wide aggregation and lease authority; settings pages and windows do
  not own competing timers or inhibitors. The architecture ownership contracts
  now name that authority and its shutdown edge explicitly.
- The real backend is the host's systemd-logind interface, invoked as
  `systemd-inhibit --what=sleep --mode=block`. The staged Zentty executable has
  one hidden lease-helper mode that acknowledges acquisition and then consumes
  a private stdin pipe. This uses no shell. Ordinary teardown closes the pipe
  and reaps the wrapper; SIGKILL closes the only writer in the kernel, so the
  helper exits and cannot silently retain an orphaned logind lease.
- A final backend review found that readiness had early-exit detection but no
  deadline if a launched backend stayed alive without granting a lease. The
  owner now fails and reaps an acquisition that has not acknowledged within
  five seconds. A real `/bin/false` child test proves that early exit is
  observed, cleared, and not left as an acquiring lease.
- The same final ownership review found the bounded readiness thread was not
  joined after child teardown, despite the architecture contract requiring
  platform-task join/reap. The lease now retains that join handle, kills and
  reaps the child before joining on every release/failure path, and
  transactionally kills/reaps the child if the reader thread itself cannot be
  created.
- This backend preserves display sleep, but systemd-logind does not offer the
  macOS assertion's narrower distinction between idle system sleep and an
  explicit user-requested system sleep. A block inhibitor therefore also
  requires explicit sleep operations to negotiate the lock. This known Linux
  semantic difference is documented rather than hidden behind a fake portal or
  treated as an unsuppressed parity claim.
- Host discovery found the real `/usr/bin/systemd-inhibit` from systemd 255 and
  a responsive logind inhibitor list. Backend absence or denial remains an
  explicit environment failure; the setting is insensitive when discovery
  fails and requested configuration is retained.

### Tests-first evidence and repairs

- Six focused core tests cover first acquisition, process-wide lease reuse,
  ten-second debounce and cancellation, disabled behavior, non-running input,
  backend-failure retry suppression, and idempotent forced release. Three Linux
  backend tests cover readiness-before-pipe-consumption, bounded single-line
  diagnostics, and real early process exit.
- The real-product journey uses the staged ReleaseSafe binary, Ghostty PTYs,
  physical nested-compositor keyboard input, authenticated Agent IPC through
  the injected real Zentty CLI, live config reload, and the real logind
  inhibitor list. It proves running-only acquisition; shared lease identity;
  debounce cancellation and completion; immediate setting-disable release;
  re-enable/reacquire; non-final agent-pane close while a real pane survives;
  final PTY child exit; graceful shutdown; and SIGKILL cleanup.
- An initial child-exit assertion expected the ten-second debounce. The real
  product correctly shuts down when its only PTY child exits, so the process
  releases immediately with `reason=application-shutdown`. The journey was
  corrected to require that stronger lifecycle behavior rather than weakening
  the product to satisfy the mistaken assertion.
- The expanded X11 journey passed in controlled session
  `a221f2cd237ba988fa8cdb5d90ba9ae9b518b0256e53b1e45421963062743f16`.
  The equivalent expanded Wayland journey passed in controlled session
  `83d6bf6fac3da2183992d384b96f90c50237e965b47a736573b329cc52908ca1`.
  The existing physical X11 Agents-settings journey also passed with persisted
  caffeination disabled in session
  `a65abf37114114f68e4fce14262b3c0d7084355a3d6d3a362e08160fc49d06e8`.
- A later full-matrix rerun passed both new inhibitor cells but exposed an
  unrelated nested-Wayland bookmark interaction race: focus reached the Save
  Bookmark control, yet its first physical Return chord was lost before the
  name dialog mapped. The exact import/export journey passed alone in session
  `4d0392af83d7e319f41906bcce6772b9798e71059c8ce2924a7a2aa44315edbc`.
  The harness now repeats the real Return chord at a bounded interval only
  while the dialog's required focus receipt is absent. This cannot turn absence
  into a pass: a mapped empty dialog rejects its default Save action, and the
  journey still requires focus, typed name, persisted export/import, and final
  product receipts.
- The next complete run confirmed the failure was concurrency-shaped rather
  than specific to import/export: that repaired cell passed, while the
  simultaneously scheduled Wayland management journey lost the same initial
  Save Bookmark chord at the same boundary. The initial repair serialized only
  the three Wayland bookmark journeys. Later full-run evidence below reproduced
  the identical lost modal chord on X11, so all six real bookmark journeys now
  share `bookmarks-modal-input`. They remain parallel with unrelated work; only
  concurrent ownership of this proven-fragile modal gesture is prohibited. A
  matrix contract requires the resource on all six cells.
- `desktop.agent-sleep-inhibition` is now IMPLEMENTED in the feature inventory.
  The qualification matrix adds explicit X11 and Wayland PASS cells and a
  serialized `system-sleep-inhibitor` resource, because concurrent cells must
  not make assertions against the same real host-wide logind state.
- The first full qualification attempt reached all support checks, then failed
  in the matrix scheduler before publishing product results: its serial result
  collector rebuilt an increasingly large JSON array through `jq --argjson`
  for every completed cell and crossed Linux `ARG_MAX` at 131 PASS cells. The
  earlier bounded-file repair protected only final summary composition, not
  this per-cell accumulation path. The collector now appends one compact JSON
  object per line and slurps the bounded file once after scheduling. Focused
  runner governance asserts both the JSON-lines path and the absence of the old
  accumulated `--argjson` invocation, so further matrix growth cannot silently
  reintroduce this failure shape.

### Final qualification receipt for this change

- The complete locked Rust workspace passed, including 6 new core policy tests
  and 3 new Linux backend tests. Formatting, compilation, ShellCheck, feature
  inventory, matrix negative tests, orchestration, architecture, and ownership
  contracts also passed.
- After the JSON-lines collector repair, a fresh
  `linux/tests/qualify-local` run passed every presently executable support and
  matrix cell in 657.57 seconds, including all three serialized Wayland
  bookmark journeys. Declared totals are **131 PASS, 0 FAIL,
  7 BLOCKED, 2 XFAIL, and 22 NOT_IMPLEMENTED**. The implemented local suite
  passed; release qualification and full Linux qualification did **not** pass.
- Debug IBus-focus Valgrind is **PASS with reviewed suppressions**, never an
  unsuppressed-clean claim. Its preserved raw receipt contains 427 error
  contexts, 6,160 definite bytes, and 41,428 indirect bytes. The reviewed
  post-suppression receipt contains zero error contexts and zero definite or
  indirect bytes. ReleaseSafe Valgrind remains explicit and non-PASS; no
  suppression was broadened for this slice.
- Machine-readable receipt:
  `build/linux/qualification-summary.json`. No release or full-Linux
  qualification claim is made while the authoritative matrix retains non-PASS
  cells.

## Agent fleet lifecycle completion (GH-21)

### Discovery and source reconciliation

- The grouped Linux fleet already rendered Waiting, Stopped, Compacting,
  Active, Idle, incomplete progress, Settings, and Quit. However,
  `AgentPhase::UnresolvedStop` was unreachable in the real product. The
  protocol decoded `state.stopCandidate` and stored an agent PID, while the
  sole Linux reducer consumed neither the stop candidate nor process death.
  A Stopped row could therefore pass a pure fleet fixture without ever being
  produced by a real PTY. This was a genuine product/test-coverage gap, not a
  compositor limitation.
- The source reducer establishes exact clocks: two seconds of stop grace, two
  minutes of idle visibility, ten minutes of unresolved-stop visibility, and
  thirty minutes before an inactive non-attention session is stale. A stop
  candidate remains Running during grace and becomes Idle only if Running was
  previously observed; a candidate-only ephemeral session disappears. A dead
  tracked PID removes an already-idle session immediately and otherwise
  produces Unresolved Stop after clearing prompt text and attention.
- The implementation plan is recorded in
  `docs/design/linux-agent-fleet-lifecycle-plan.md`. The repair deliberately
  extends the existing `AgentStatusStore`, `WorkspaceState`, and
  `AgentEventCoordinator`; it adds no parallel reducer, timer, pane registry,
  or fleet surface. One 500 ms bounded sweep runs through the existing
  application tick, and Linux supplies only `/proc/<numeric-pid>` liveness to
  the platform-free core closure.

### Tests-first repairs and real-system evidence

- Deterministic core tests were written before the reducer repair. They prove
  the exact 1,999/2,000 ms stop boundary, cancellation by new activity,
  candidate-only expiry, tracked-PID death, attention/text clearing, exact
  unresolved/idle/stale deadlines, dead-idle removal, pane removal, and
  transfer of an in-flight deadline to a new-window workspace. The existing
  agent-status suite remains the single reducer suite.
- A first attempt to match the source's unobserved-idle removal changed an
  existing intentional Linux interrupt-reconciliation contract: a late Codex
  idle event at the exact suppression deadline is allowed to recreate the
  known Codex session. The failing test caught that overreach. Normal idle
  behavior was restored; only source-defined stop candidates use the
  observed-running guard introduced by this slice.
- Alternative real signals were audited against the new clocks. OSC activity,
  terminal notifications, title reconciliation, user submit, interrupt, shell
  return, `session.end`, pane removal, and pane transfer now cancel, establish,
  remove, or carry lifecycle clocks rather than allowing an expired candidate
  to overwrite newer state.
- The existing `rust-multi-window` actor was expanded rather than creating a
  second fleet harness. Its real PTY child publishes authenticated waiting,
  running, 2/5 progress, 5/5 completion, Compacting, stop-candidate, tracked
  live PID, PID death, recovery, and session-end events through the installed
  Zentty CLI. The real application rebuilds its GTK fleet after each state.
  Sanitized receipts report state counts and whether incomplete progress is
  present without logging prompt text or terminal content. The same rendered
  progress string now owns the GTK visual and accessibility label.
- Controlled X11 session
  `a0ecd647439a90d8879c8b0b7bb11a4a93921e80401a071c0030f4770a8f43cd`
  passed the final expanded real-product lifecycle, exact cross-window PTY
  route, source-window close, surviving destination, and zero-stale-row
  aggregate rebuild.
  Controlled Wayland session
  `d519852ffcde4686d670fe077c5e75d239bb3e4ec9397bfbf325b3d11aec7f5c`
  passed the same lifecycle sequence and reached only the already-tracked
  xdg-activation boundary, exiting 98 as declared. Environmental absence was
  not treated as a pass.

### Remaining scope and uncertainty

- The X11 journey additionally requires the routed source window to close, the
  process to survive in its destination window, and the aggregate fleet to
  rebuild with zero stale rows. The first lifecycle receipt predated that
  assertion; the final staged-binary session above reran and passed it.
- The optional StatusNotifierItem fixture, explicit footer Settings/Quit
  physical journeys, and malformed fleet-target presentation journey remain
  open under GH-21. The always-available in-window surface is never described
  as desktop-tray qualification. The Wayland cross-toplevel activation-token
  defect remains XFAIL and was not disguised by weakening exact PTY focus.
- A strict workspace Clippy run found the fleet/core changes clean after one
  local reducer decomposition, then stopped on existing Linux-bin warnings
  (two pre-existing overlong functions, needless borrows, a missing terminal
  semicolon, and a pre-existing sleep-inhibitor argument style). This slice
  does not silently repair unrelated housekeeping; the authoritative
  qualification invocation will determine the repository's declared lint
  cell and the raw failure is retained here.
- The first scoped lifecycle mutation run did not reach mutants in the
  filesystem sandbox because an unrelated existing core test creates a Unix
  special-file fixture; the unmutated baseline failed with `EPERM`. The same
  command was rerun in the normal qualification environment, where it exposed
  five real test weaknesses: four redundant/untested ephemeral-start
  predicates and one unobservable source-side deadline leak after pane
  transfer. The redundant predicates were removed because `Starting` cannot
  carry human attention or a stop-candidate deadline in the Linux model. Exact
  one-second early-exit tests and a pane-ID-reuse transfer test were added.
  The final narrowed run caught **30 of 30** lifecycle/transfer mutants in 89
  seconds with `.cargo/mutants.toml` still enforcing `gitignore = true` and
  `copy_target = false`; no ignored build tree was copied.
- The first complete qualification run after this repair finished in 578.19
  seconds with 131 declared PASS cells, but the implemented suite correctly
  failed rather than publishing a clean receipt. The architecture ownership
  contract detected three new coordinator-local functions
  (`begin_lifecycle_sweep`, `lifecycle_sweep_due`, and
  `linux_process_is_alive`) that had not been added to its explicit inventory.
  The contract now owns those functions and describes bounded lifecycle and
  Linux liveness projection; focused architecture validation passes.
- The same full run lost a real Wayland tmux compatibility transaction while
  many controlled GUI cells were concurrent: its child reached `show-buffer`
  and `kill-pane`, then the client disconnected during pane shutdown. The same
  exact real-product sub-journey passed alone in controlled Wayland session
  `041256b08cd3d8d3bbbae549c13d4f0629dc3175ebdc5a69d0532ff3f04ea9ad`.
  This is retained as concurrency-shaped harness evidence, not converted into
  a pass for the failed full run. A complete post-contract-repair run is still
  required before commit.
- The post-contract full rerun passed the repaired architecture cell and the
  formerly failing Wayland agent cell, but exposed the same lost-first-Return
  shape in the X11 bookmark management cell. The GTK receipt proved keyboard
  focus had reached `save-bookmark`; ten bounded retries sent Return through
  XSendEvent to the known toplevel, yet the dialog never mapped. The X11 actor
  now uses XTest against the already-proven real focus, matching the reliable
  multi-window input path rather than synthetic per-window events GTK may
  ignore. The full physical bookmark-management journey passed afterward in
  controlled X11 session
  `2bdf8ad438816e4de5ba8c6aec292bfea32326ff35b2791f8bea546145f4ccbd`.
  Neither failed full receipt is represented as an implemented-suite pass.
- A third full run reproduced the modal loss more decisively: both X11
  management/import-export and Wayland import-export reached the exact focused
  Save Bookmark control but failed to map the dialog, while the serialized
  Wayland management and save/restore cells passed. This rules out a
  Wayland-only resource and shows aggregate concurrent modal actors are the
  contention. All six bookmark journeys now share the single
  `bookmarks-modal-input` scheduler resource. The closed-world resource axis
  and runner negative contract were updated together; no cell disappeared or
  changed status. A final complete run must validate this scheduling repair.
- The fourth complete run validated that repair: all six bookmark journeys
  passed while sharing the declared modal-input resource. It then exposed one
  real config-watch lifecycle defect in `config-live-reload-wayland`. A single
  Settings write produced two `config-reload result=applied` receipts, so the
  cell correctly rejected the run instead of accepting the duplicate as a
  harmless event. `ConfigDirectoryWatch` cancelled monitor events through a
  local debounce source, but replacing the watch after a reload dropped only
  its `GFileMonitor` objects. A pending GLib timeout retained the old callback
  and could apply the next document before the replacement watch applied that
  same document again. The watch now owns the pending source and cancels it on
  drop. A deterministic unit test schedules a real directory event, replaces
  the watch before the 150 ms quiet period, and proves the stale callback does
  not fire. The controlled real-product Wayland config journey then passed in
  session
  `9331f0f457d3f7a5e201109a4eaa803020fb2b7a1607b159167c38f31b7d5f91`,
  including its explicit `self-write-no-loop=true` receipt. The fourth failed
  full receipt remains a failure. The matching controlled X11 journey also
  passed in session
  `96ffa616a649e390189f9a6ae0c938520de70dc52450115165defeac3e75f63a`.
  Scoped config-watch mutation testing exercised 13 generated mutants: nine
  were caught and four were compiler-unviable; none survived. A clean complete
  qualification rerun is still required.
- The next complete run passed both config-reload cells and every bookmark
  cell, but the ownership contract rejected the newly implemented
  `ConfigDirectoryWatch::drop` method because the contract's closed function
  inventory had not been updated with that lifecycle boundary. This was a
  documentation/contract omission in the repair, not a product failure, and
  the implemented suite correctly remained failed. The config-reload owner now
  explicitly inventories `drop`; the focused architecture contract and all of
  its negative tests pass in isolated session
  `0b172410c3f6b6544c4760705dcece39feea4d1118b7f726a04dee4c4dd09c8f`.
  No clean full receipt is claimed until another complete run passes.
- The final complete `linux/tests/qualify-local` run passed every presently
  executable support test and matrix cell in 575,030 ms. The authoritative
  declared totals are **131 PASS, 0 FAIL, 7 BLOCKED, 2 XFAIL, and 22
  NOT_IMPLEMENTED**. Therefore the implemented local suite passed, while
  release qualification and full Linux qualification correctly remain not
  passed. Debug Valgrind is **PASS with reviewed suppressions**, not an
  unsuppressed-clean claim: its preserved raw receipt contains 427 errors in
  427 contexts, 6,160 definitely lost bytes, and 41,428 indirectly lost bytes;
  the reviewed post-suppression receipt contains zero errors, contexts, or
  lost bytes and reports 427 suppressed errors/contexts. ReleaseSafe Valgrind
  remains non-PASS as declared; no suppression was broadened. The machine
  summary is `build/linux/qualification-summary.json`, and the accepted Debug
  report identity is
  `27367065506690b35ea7a1ec38cd8f74ece97f525ec9def620067c0605c5ef5e`.

## AI disclosure

Initial repository analysis, implementation assistance, and this report were
prepared with OpenAI Codex under Jason Maskell's direction. Any proposed
upstream Ghostty communication or contribution must be reviewed, understood,
edited, and submitted by the human contributor in accordance with Ghostty's
contribution policy.
