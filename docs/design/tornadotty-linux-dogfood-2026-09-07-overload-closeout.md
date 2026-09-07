# Bounded workload closeout — 2026-09-07

Issue #163, parent #160. Baseline for this last batch: `f23acd2e`.
Staged only: Jason is working inside the live client. No installation/restart.

## Repairs in this batch

- Agent liveness no longer stats `/proc` from the GTK dispatch callback.
  The existing 500 ms sweep admits at most one job per window, two globally.
  Lifecycle deadlines still advance without waiting for I/O. Observations must
  match pane topology/context and session/PID/status timestamp; missing, stale,
  or unreadable process observations are not evidence of death.
- Project and passive-server scans retain their existing owners and gain two
  process-wide worker slots each. Slots follow actual worker exit, not window
  lifetime. Server results check topology plus each captured pane context;
  Task Manager also checks topology generation, including move-away-and-back.
- Moved the existing enrichment permit implementation to a shared module with
  a const capacity. Enrichment remains at four workers. Task Manager already
  has one application-wide controller and one in-flight sampler.
- Native project file-monitor construction/cancellation also leaves GTK. One
  GLib context thread owns the existing native monitors, at most 128 pane watch
  registrations (seven metadata targets each). Pending creation/cancellation
  holds capacity until processed, bounding churn even if a filesystem blocks.
  Changes coalesce into one bit per registration, consumed by the existing
  two-second project cycle. Excess registrations use existing periodic probing;
  there is no extra product timer, filesystem parser, or workspace registry.
- Large Copy Raw exposed a GTK/X11 hang: the private product's main thread was
  waiting in `g_output_stream_close` through a converter stream while xclip
  waited for selection transfer. Direct UTF-8 byte publication avoids redundant
  GValue text conversion. Large capture and existing ordinary/clean/raw/Markdown,
  primary-selection, cross-window and real-PTY paste checks then passed.

## Load and correctness evidence

The new `overload` scenario extends **rust-agent-ipc**, using its private display,
input, teardown and state ownership. A small Rust test-support actor uses real
PTYs and the real IPC client; it is staged separately under `test-support/`, not
installed as a public executable. This is a release-tier matrix cell, not an
extra test required for every edit.

- Four concurrent clients send roughly 60 KiB agent events, with the actual
  producer PID, throughout output. The scenario requires both accepted requests
  and explicit `ingress_full` rejections: merely generating traffic is not a pass.
- The PTY first writes **64 MiB** through an alternate screen, in 256 KiB writes
  with bounded pacing. Alternate-screen history is intentionally not retained.
  It then returns to the normal screen and writes 200 ordered records, including
  a 64 KiB line, Unicode, color, carriage-return and erase-line sequences.
- Four physical input/real CLI round trips in the quiet pane run during load,
  alongside four actual window resizes. Each must finish within two seconds.
- A real VT title marks output drain. Select All/Copy Raw and the external X11
  clipboard must match all **884,039 retained bytes**, not a producer receipt.
  The expected text excludes the final empty row, per selection semantics; no
  capture-side stripping, sorting or lossy normalization is performed.
- Measured run: **8,068 accepted, 84 rejected, 95 ms maximum input round trip,
  29,612 KiB peak anonymous-RSS growth**, exact capture PASS. The 128 MiB growth
  ceiling includes configured 64 MiB scrollback, terminal storage and clipboard.
  It does not claim total GPU/driver allocations or arbitrary workspaces are fixed.
- Final monitor-build rerun alongside the other private journeys: **8,084
  accepted, 64 rejected, 96 ms maximum input, 33,840 KiB growth**, exact capture
  PASS. Slow-Git/project-icons and development-server journeys also PASS on
  that build. Fresh transport subset: 11 library and nine Unix transport tests.
- Reviewed unchanged Ghostty `bab8c088`: four 64 KiB read buffers per PTY,
  producer backpressure when the ring fills, 64 KiB parser batches; scrollback
  is separately configured/bounded. No new Ghostty patch or parser was needed.

## Failed attempts retained

- Initial overload setup confused CLI selection/discovery with focus, then
  exposed inherited live-pane environment in the driver. That attempted live
  selection was rejected. The scenario now explicitly clears inherited endpoint
  capabilities and obtains private-instance control for actual pane focus.
- The load phase types into the already-focused private window without repeated
  X refocusing. Initially this appeared to fix the intermittent quiet-pane failure,
  but the clean-commit rerun failed again. That hypothesis was insufficient.
  Logs identified startup's delayed `on_initialized` callback reapplying pane 1
  **after** an acknowledged CLI focus on pane 2. Later host focus requests now
  consume that pending startup intent, so it cannot overwrite newer selection.
  Native initial mapping retains its unchecked path to preserve saved selection.
  After the repair, three independent overload trials ran concurrently (all
  required to pass, not retries): each observed one superseded startup intent,
  preserved all 884,039 bytes and rejected actual saturation. Worst input was
  178/210/224 ms; RSS growth was 35,900/30,856/34,272 KiB. The existing eager
  restore journey also passed with two agents and two ordinary shells, preserved
  active focus/topology, and recoverable hidden-pane failure.
- The initial sub-MiB capture run was insufficient to claim high-output PTY
  stress; the final actor adds the bounded 64 MiB alternate-screen phase.
- Clipboard capture originally had no deadline. The failed private process was
  inspected and terminated through its owning journey; transfer now has a five
  second failure deadline. We did not shrink the payload to evade the hang.
- The existing clipboard journey expected five palette results for “Copy”; the
  product has seven. Updated that discovery count, retaining the exact dispatched
  Copy action and external clipboard/PTY assertions.
- An over-broad `cargo test --bin zentty-linux` in one process produced 387 PASS,
  seven FAIL, four IGNORE: GTK initialized on multiple Rust test threads and
  shared default-context config-watch tests interfered. This is **not a green
  suite run**. Focused process-isolated checks below are the evidence used here.

## Focused checks and mutation scope

- 39 core agent-status tests; 16 Task Manager tests in private X11; 15 enrichment
  tests plus the relocated worker test; nine project-context/native-watch tests.
  Native tests hold the watch owner busy during registration/cancellation, then
  exercise actual filesystem events and removal. Task Manager's real FIFO `/proc`
  fixture holds sampling while GTK responds and ownership changes.
- Existing real development-server and slow-Git/project-icon journeys PASS.
  The latter holds a real Git subprocess and changes/refreshes project state
  while GTK input continues. Existing transport regressions cover malformed,
  oversized, saturated, disconnected, stale and concurrent-pane traffic.
- Shared worker budget mutations: **seven caught, four unviable, zero missed**.
  Lifecycle sweep: **three caught, one unviable**. That tool invocation also
  selected two unrelated restored-task bookkeeping field-deletion mutants;
  both survived the agent-status-only filter. They are not a sweep kill-rate
  success or a whole-core autonomy certification. Earlier #163 queue, dispatch,
  generation, cancellation, persistence/coalescing mutation receipts still apply.
- Strict GTK Clippy retains five existing product findings and the existing
  catalog test-module placement finding. No new suppressions or unrelated lint
  cleanup. Workspace-wide formatting has pre-existing differences; changed
  Rust files are formatted individually. No full qualification claim.

## Whole-issue acceptance map and limits

- Ingress admission/fairness/diagnostics and persistence: [bounded ingress](tornadotty-linux-dogfood-2026-09-05-ingress-backpressure.md).
- Reader/reply isolation, enrichment and freshness: [I/O boundaries](tornadotty-linux-dogfood-2026-09-07-io-boundaries.md).
- GTK dispatch: [cooperative budget](tornadotty-linux-dogfood-2026-09-07-ipc-dispatch-budget.md).
- Task launch, close and catalog I/O: the [task](tornadotty-linux-dogfood-2026-09-07-task-responsiveness.md),
  [close](tornadotty-linux-dogfood-2026-09-07-close-responsiveness.md) and
  [catalog](tornadotty-linux-dogfood-2026-09-07-catalog-responsiveness.md) reports.
- Task Manager: [blocked sampling and stale ownership](tornadotty-linux-dogfood-2026-09-07-task-monitor.md).
- Remaining monitor ownership and real PTY overload/capture: this report.

Bounds apply to admitted work, queues, caches and retained snapshots, not to a
fixed total byte size for user-created workspaces. The four-millisecond dispatch
budget is cooperative between handlers, not preemption of GTK/Ghostty calls or
  a hard frame deadline. A blocked syscall can occupy its bounded worker; it is
not forcibly cancelled. Full subprocess crash/OOM containment remains #162,
not a guarantee provided by #163. Live user QA remains the agreed later batch.

Reproduction from the repository root (no live-client installation):

```sh
ZENTTY_BUILD_OUTPUT_DIR=build/gh163-complete linux/scripts/build-local
ZENTTY_LINUX_BINARY="$PWD/build/gh163-complete/bin/zentty-linux" \
  ZENTTY_AGENT_IPC_SCENARIO=overload linux/tests/nested-x11 linux/tests/rust-agent-ipc
ZENTTY_LINUX_BINARY="$PWD/build/gh163-complete/bin/zentty-linux" \
  linux/tests/nested-x11 linux/tests/rust-project-icons
ZENTTY_LINUX_BINARY="$PWD/build/gh163-complete/bin/zentty-linux" \
  ZENTTY_CLIPBOARD_ONLY=true linux/tests/nested-x11 linux/tests/rust-pane-search
```
