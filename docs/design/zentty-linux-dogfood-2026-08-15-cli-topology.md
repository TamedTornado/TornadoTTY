# Zentty Linux dogfood — CLI topology journeys

This report records GH-44 discoveries, red failures, repairs, receipts, and
remaining uncertainty as they occur. The acceptance and test order are fixed
in `zentty-linux-cli-topology-journeys-plan.md`.

## Initial source/current-system audit

- The macOS source distinguishes horizontal neighbor columns from vertical
  panes in a column. Its CLI verbs are `split right|left|up|down`, aliases
  `hsplit`/`vsplit`, grid destinations, nine layout presets, zoom, directional
  cell resize, and percentage column resize.
- Linux already routes those commands through the one authenticated product
  socket into the real `ApplicationShell`; it does not need another CLI server,
  topology model, or Ghostty API.
- Existing controlled X11/Wayland coverage proves a 2x2 current-window grid,
  a 2x2 new-window grid, equal/ratio splits, percentage resize, and cross-window
  focus. That is useful smoke coverage but does not satisfy GH-44: it does not
  prove 1xN/Nx1 topology, new-worklane behavior, command/CWD/environment
  inheritance, exact terminal geometry, restart, concurrent changes, or
  transactional rollback.
- Current successful mutation replies are empty. GH-44 explicitly requires
  human/JSON identification of all affected objects, so discovery performed by
  a later command cannot substitute for a command receipt.
- `apply_grid` creates real panes incrementally and returns immediately on a
  later failure without removing already-created surfaces. New-window setup
  rolls the whole destination window back, but current-window and new-worklane
  grid construction are not transactional. This is a product defect relative
  to the issue contract and will receive a deterministic red failure before
  repair.
- The current grid command is sent to newly created PTYs immediately after
  surface creation. The existing split path may not yet prove readiness or
  source CWD/environment inheritance, so real child receipts—not sleeps or log
  assertions—will decide whether that behavior is correct.

## Qualification baseline

The clean pre-GH-44 receipt at commit `fbdc09a1044c1c040380e7bc25dc4d0a895a7b87`
reported **140 PASS, 0 FAIL, 7 BLOCKED, 1 XFAIL, 16 NOT_IMPLEMENTED**. Implemented
local qualification passed; release and full Linux qualification did not.
Debug Valgrind was **PASS with reviewed suppressions**, not unsuppressed-clean;
ReleaseSafe Valgrind remained XFAIL.

## First parser reds

The first two GH-44 contract tests failed for the intended reasons. Every
topology mutation rejected `--json` as an unexpected argument, so the CLI had
no machine-result request despite the issue's human/JSON acceptance criterion.
Separately, `grid 1x2 -- printf 'line one\nline two'` was accepted. The macOS
`GridLaunchCommandBuilder` rejects either LF or CR inside any command token;
Linux instead shell-quoted the line break and would have injected it into a
real terminal. The repair must happen in the public parser before socket
contact, with a defensive product-side check retained for hostile wire clients.

The parser repairs passed their focused ten-test suite. `--json` is now carried
only as an explicit topology-result flag, duplicate-option validation remains
active, and both public parsing and hostile wire handling reject LF/CR command
tokens.

## First real topology expansion

The existing staged tmux/product actor—not a new harness—was expanded with
machine receipts for split, 1x1, 1x3, 3x1, 2x2, new-worklane, and new-window
grids. The first controlled X11 run reached the new shapes and showed that
ordinary CLI-created panes did not explicitly inherit the source pane's CWD:
the split helper inserted a blank `PaneState`, leaving Ghostty to fall back to
process context. The sole pane-creation helper now copies the source pane's
recorded working directory before constructing the real surface. This repairs
GUI and CLI splits together rather than adding grid-only configuration.

The run then exposed a real command-launch race. A 1x2 grid registered the new
Ghostty surface and immediately called `send_text` before its initialized
callback. Ghostty rejected the write; the new transaction removed the partial
surface and restored the exact pre-grid state, producing
`cli-grid-rollback removed=1 failures=0`. The harness correctly failed rather
than treating rollback as command success. Grid command text for newly-created
panes is now queued on the existing pending-prefill lifecycle and delivered by
the real terminal-ready callback. The already-ready source pane still receives
the text synchronously, so a source delivery failure can roll topology back
before success is reported.

The transaction snapshots the canonical `WorkspaceState`, discovers every
surface created after that snapshot, disposes those surfaces in reverse order,
restores the state, resynchronizes the single capability registry, rerenders,
and restores focus. New-window failure continues to use coordinator teardown
because the entire destination window is its transaction.

## Source-semantic correction and result contract

Reading `WorklaneStore.applyGrid` exposed a second parity defect: a grid begun
from a worklane containing multiple panes must move the selected source pane,
with its stable identity and live terminal, into a new worklane before building
the grid. Replacing the source worklane in place would destroy unrelated pane
ownership. `WorkspaceState::isolate_focused_pane_in_new_worklane` now performs
that move, and model plus staged-product assertions prove the old lane retains
its other pane while the new lane owns the exact selected source and grid.

Topology mutations now return either a concise human receipt or a version-1
JSON receipt containing ordered rendered topology, source/final focus, and
created/affected IDs. Receipts exclude control tokens and inherited environment
values. Left and up splits received explicit ordering/size assertions; grid
coverage includes 1x1, 1xN, Nx1, and NxM; layout coverage includes halves,
vertical thirds, golden-wide, and reset. Two simultaneous real CLI clients
proved serialized mutation, unique pane IDs, and a consistent final topology.

## Real command delivery and geometry

The first terminal-ready repair was still one lifecycle event too early.
Ghostty emitted initialized before the configured child was reliably reading
its PTY, so queued command text could disappear even though surface creation
had succeeded. The pending-prefill coordinator now waits for Ghostty to expose
the real foreground child PID, with a bounded retry, before consuming and
sending the queued text. Controlled X11 receipts then proved both source and
new panes received the command, inherited the source CWD and pane/window/lane
environment, and reported distinct real `stty size` dimensions.

An early external-resize attempt selected an arbitrary visible X window and
made a later legacy assertion compare against the pre-resize viewport. The
repair targets the exact Zentty toplevel on X11 and the exact compositor-owned
outer X11 window for nested Wayland. The focused profile resizes a live 2x2,
then obtains a new result through the same product CLI and proves the viewport
changed. Cage remains the representative protocol/topology profile; labwc is
used for the resize cell because its controlled outer-X11 input contract is
explicit. This is not an environmental absence converted into a pass.

Running every exhaustive shape in software-rendered Cage crossed the harness
timeout and repeated evidence already established by the comprehensive X11
journey. The one existing product runner now has a representative topology
profile: real 2x2 creation, focus, golden layout, surfaces, PTYs, and optional
external resize. It tears its created panes down immediately. The exhaustive
X11 profile remains about forty seconds including persistence and recovery;
the focused compositor profiles complete in seconds rather than carrying a
90-second sleep-shaped test architecture.

## Transaction and persistence evidence

A controlled fault boundary, enabled only when
`ZENTTY_TEST_GRID_FAILURE_AFTER_FILE` is explicitly present in the staged
product environment, injects failure after the second real grid surface. The
real actor proves the command fails, the pre/post authenticated pane discovery
(including tokens) is byte-identical, only the original worklane remains, and
the product reports `cli-grid-rollback removed=2 failures=0`. This replaced
the nondeterministic readiness race as rollback coverage.

Closing a CLI-created topology with tmux `kill-window` was not a valid
persistence action: those panes are ordinary Zentty panes, not tmux-owned team
members. Direct X window closure also reproduced the already-known GTK/X11
`BadDrawable` teardown behavior. The final persistence journey uses the real
debounced live snapshot, verifies the 2x2 golden column/height data and absence
of capability fields, then delivers SIGKILL to model an actual crash. Relaunch
from the same state directory restores four real panes in two columns; an
idempotent golden-layout receipt proves topology and focus are live through the
public CLI. The established compositor input helper then performs normal clean
shutdown. This deliberately tests crash persistence/restart rather than
inventing a second backup or session subsystem.

## Presently run receipts

- Agent IPC product parser: **11 passed, 0 failed**.
- Core workspace state: **52 passed, 0 failed**.
- Comprehensive nested X11 topology/product journey: **PASS**.
- Focused nested X11 external-resize journey: **PASS**.
- Focused nested Cage Wayland topology journey: **PASS**.
- Focused nested labwc Wayland topology plus compositor resize: **PASS**.
- Core topology mutation run: **14/14 mutants caught**.
- Public grid-parser mutation run: **26 caught, 1 unviable, 0 missed**.
- Grid rollback surface-selection/order mutation run: **4/4 mutants caught**.

The first core mutation baseline incorrectly ran every library test inside the
restricted scratch environment and hit an unrelated FIFO/device-node test with
`Operation not permitted`; no mutant ran. The corrected invocation selected
the real `workspace_state` integration target with `--cargo-test-arg`, while
retaining `.cargo/mutants.toml`'s mandatory `gitignore = true` and
`copy_target = false`. Two surviving source-column-index mutants prompted a
last-column isolation assertion and were then killed. Grid-parser survivors
prompted explicit 6x6 boundary, include-source-only, new-window, existing
destination, and mutual-exclusion contracts; index arithmetic that created
timeout mutants was replaced by a forward iterator. The rerun had no missed or
timed-out mutants.

The real mid-grid fault journey already proved runtime rollback. A focused
pure contract now also locks its cleanup inventory: only surfaces absent from
the pre-command topology are destroyed, in reverse creation order. All four
generated mutations of that production helper were caught, closing the
rollback-specific mutation criterion without creating another integration
harness.

The authoritative matrix now promotes `external-resize-wayland` from
NOT_IMPLEMENTED to PASS. This does not imply release or full Linux
qualification; all remaining BLOCKED, XFAIL, and NOT_IMPLEMENTED cells remain
visible and must keep those aggregate claims false.

## First full-qualification repair pass

The first post-change `qualify-local` run correctly failed rather than hiding
regressions. Its declared matrix was **141 PASS, 0 FAIL, 7 BLOCKED, 1 XFAIL,
15 NOT_IMPLEMENTED**, but executable cells failed and therefore the implemented
local suite did not pass. The dominant failure was contractual, not 18
independent product defects: closed-pane and bookmark journeys still searched
for the retired `restore-prefill ... text=<command>` log. The product had
actually delivered the real prefill (`pane-prefill ... process-started=true`),
but exact command text in logs would leak arbitrary user arguments. Those tests
now assert pane identity, child readiness, and exact byte count while retaining
their existing real PTY/CWD evidence. Focused X11 and Wayland reruns of both
journeys pass.

The ordinary headless-Weston Wayland aggregate also reached the new X11-only
crash-persistence subjourney and correctly rejected physical close input in a
non-input profile. Persistence/restart is now run once in the comprehensive
controlled-X11 journey; Wayland retains its real topology coverage plus the
focused Cage/labwc profiles. The architecture mirror and pane-runtime ownership
inventory were reconciled with the authoritative matrix and renamed lifecycle
function, and their complete contract suite passes.

One Wayland config-reload cell reported a self-write loop during the highly
parallel full run. Its isolated controlled-Cage rerun passed the complete
external-last-writer, partial-write, recovery, and no-loop journey, identifying
that receipt as a load-sensitive transient rather than accepting it silently.
The full qualification must still be rerun and pass before commit.

## Baseline green qualification receipt

The four-worker rerun repaired every GH-44-related failure, but three unrelated
physical UI cells failed only under simultaneous software-compositor load:
Wayland bookmark management, X11 development servers, and its Docker variant.
The first two passed immediately when rerun in isolation (the Docker path then
passed in the bounded full run). This was not converted into a pass. The entire
authoritative suite was rerun with the runner's supported bounded concurrency
set to two.

That final run passed every presently executable support and matrix cell in
**1,092,050 ms**. The authoritative totals are **141 PASS, 0 FAIL, 7 BLOCKED,
1 XFAIL, 15 NOT_IMPLEMENTED**. The **implemented local suite passed**.
Release qualification and full Linux qualification remain **NOT PASSED**, as
required by the non-PASS cells. Suppression review was **ACCEPTED**; Debug
Valgrind evidence remains **PASS with reviewed suppressions**, never described
as unsuppressed clean, and the declared ReleaseSafe/ABI case remains XFAIL.
The retained IBus-focus report records raw **427 errors/contexts, 6,160
definite bytes, and 41,428 indirect bytes**, versus post-suppression **0
errors/contexts and 0 definite/indirect bytes**, with all 427 contexts accounted
for by the reviewed effective suppression set.
That green receipt was subsequently superseded by the
post-mutation-contract reruns below; `build/linux/qualification-summary.json`
always names the most recent aggregate attempt.

The final repository-wide Rust verification also passed with
`cargo test --workspace --locked`: every workspace unit, integration,
security-contract, source-fixture, and doc test completed with zero failures.

## Post-mutation-contract qualification rerun

After adding the focused rollback mutation contract, the entire local matrix
was rerun rather than relying on the earlier green receipt. Exactly one cell,
`product-pane-lifecycle-debug-wayland-default-multi`, failed because Ghostty's
native Debug logger inserted a record between fields of the Rust
`pane-prefill` receipt on their shared stderr stream. The product log contained
the correct pane identity, `process-started=true`, and the expected byte count;
the test had accidentally restored the cross-runtime byte-atomicity assumption
that this journey had previously documented as invalid.

Splitting the assertion into two synchronized fields still failed on a focused
rerun because a native record can be inserted inside either formatted field.
The product now constructs the complete non-secret receipt in memory and emits
that short record with one stderr write. Closed-pane and bookmark journeys can
therefore require the exact pane identity, delivery state, and byte count
together without ever searching for the user command. The affected controlled
Wayland cell was rerun after this repair before the final aggregate
qualification.

The next aggregate rerun proved the repaired lifecycle and bookmark cells but
found two different failures. The architecture ownership guard correctly
rejected the new `write_prefill_receipt` helper until it was named in the pane
runtime inventory; the inventory is now reconciled. The unrelated Docker dev
server cell failed before product launch because its controlled Openbox process
did not become ready under aggregate compositor load. Its isolated controlled
X11 rerun is retained rather than treating environmental startup absence as a
pass. Both repairs/evidence were completed before the final aggregate rerun.

The third aggregate attempt passed the repaired lifecycle, bookmark restore,
architecture, and Docker cells, plus every GH-44 topology cell. One unrelated
Wayland bookmark import/export cell then failed because its real name dialog
did not appear under concurrent compositor load. An immediate isolated run in
the same controlled Cage profile passed the complete real export chooser,
physical delete, real import chooser, and persisted portable-data journey.

Accordingly, every presently executable cell has run and every observed
failure has a green isolated reproduction, but the latest aggregate machine
receipt still correctly says **implemented local suite FAILED**. Its declared
matrix totals remain **141 PASS, 0 FAIL, 7 BLOCKED, 1 XFAIL, 15
NOT_IMPLEMENTED**; those declared statuses do not erase the execution failure.
This document does **not** claim release qualification, full Linux
qualification, or an all-green latest aggregate run. The repeated pattern of a
different physical-UI startup failure under two-worker load is retained as a
qualification-runner load/isolation defect rather than hidden by retries.
