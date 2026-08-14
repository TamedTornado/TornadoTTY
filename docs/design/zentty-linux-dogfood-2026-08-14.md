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
  Save Bookmark chord at the same boundary. The three isolated Wayland bookmark
  journeys now share only a `bookmarks-wayland-modal-input` scheduler resource.
  They remain parallel with unrelated work and the X11 bookmark journeys; only
  concurrent ownership of this proven-fragile modal virtual-keyboard gesture is
  prohibited. A matrix contract requires the resource on all three cells.
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

## AI disclosure

Initial repository analysis, implementation assistance, and this report were
prepared with OpenAI Codex under Jason Maskell's direction. Any proposed
upstream Ghostty communication or contribution must be reviewed, understood,
edited, and submitted by the human contributor in accordance with Ghostty's
contribution policy.
