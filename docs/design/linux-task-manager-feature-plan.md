# Linux Task Manager feature plan

Date: 2026-08-11
Tracking: GH-19

## Product outcome

The command palette opens one application-level Task Manager window that lists
every live pane across every Zentty window. Each pane row reports its current
process-tree identity, CPU, resident memory, hottest process, root PID, and an
explicit availability state. Expanding a pane shows its real descendant
processes. Search, stable refresh ordering, focus-pane, copy-PID, and end-task
actions follow source Zentty behavior.

## Source authority

The port follows:

- `Zentty/TaskManager/TaskManagerModels.swift`;
- `Zentty/TaskManager/TaskManagerProcessSampler.swift`;
- `Zentty/TaskManager/TaskManagerWindowController.swift`;
- the Task Manager paths in `AppDelegate`, `PaneCommandExecutor`,
  `MainWindowController`, `AppMenuBuilder`, and `KeyboardShortcutResolver`;
- `TaskManagerPresentationTests`, `TaskManagerProcessSamplerTests`,
  `TaskManagerRootPIDTests`, and `TaskManagerWindowControllerTests`.

The source currently models network throughput but its Darwin sampler always
returns it as unavailable. Linux must preserve that honest state in this slice;
it must not relabel disk I/O, network-namespace totals, or socket queue depth as
per-pane network throughput. Real attributable network accounting remains a
named GH-19 limitation rather than guessed telemetry.

## Architecture constraints

1. Process sampling and row projection live in one focused Linux Task Manager
   module. There is no daemon, second workspace model, or host process
   supervisor.
2. `WorkspaceState` and `PaneRuntimeCoordinator` remain the authorities for pane
   identity and live Ghostty surfaces. The Task Manager receives read-only pane
   snapshots and never owns terminal lifecycle.
3. `ApplicationCoordinator` owns the one application-level diagnostics window,
   matching source Zentty's one controller across multiple windows. Per-window
   shells only expose snapshot/focus/close callbacks.
4. Sampling runs off the GTK thread, is bounded by process count and ancestry
   depth, reads only `/proc`, and verifies `(pid, start_time)` identity on every
   refresh to reject PID reuse.
5. CPU is computed from cumulative `/proc/<pid>/stat` ticks over monotonic wall
   time and `_SC_CLK_TCK`; memory is resident pages times `_SC_PAGESIZE`.
   Missing, exited, inaccessible, and remote processes remain visible with an
   explicit unavailable reason.
6. Refresh replaces values without replacing stable pane/process identities.
   It preserves selection, expansion, filter text, and hysteretic ordering.
7. “End Task” closes the selected Zentty pane through the existing lifecycle
   transaction; it does not independently signal an arbitrary process.
8. Product orchestration remains in Zentty. No Ghostty change is needed: the
   existing minimal foreground-process query supplies the live root signal.

## Test-first construction order

### A. Core red tests

- Parse realistic `/proc/<pid>/stat` records including spaces/parentheses in
  process names and reject malformed/truncated records.
- Build bounded descendant trees with cycles, missing parents, exited children,
  inaccessible entries, and PID reuse.
- Compute first-sample zero, proportional CPU, sibling history retention,
  counter rollback, zero elapsed time, memory aggregation, hottest-process
  ordering, peaks, unavailable states, filtering, and sort hysteresis.

### B. Product integration

- Register exactly one `show-task-manager` action and palette item.
- Collect every window/worklane/pane through the existing coordinator and real
  Ghostty foreground PID query.
- Open/reuse one themed GTK window; refresh asynchronously; expand processes;
  preserve selection and expansion; filter; copy PID; focus the owning pane;
  and close the selected pane through existing product actions.
- Stop scheduling and discard late worker results when the window closes or the
  application shuts down.

### C. Real-system journeys

- In staged ReleaseSafe Zentty under controlled X11 and Wayland, launch distinct
  real CPU and memory fixture trees in real Ghostty PTYs.
- Open the real palette with physical key events, invoke Task Manager, and prove
  the real diagnostics window contains the pane/root/child identities and
  nonzero bounded samples derived from kernel `/proc` data.
- Exercise search and expansion through physical events, copy a real child PID
  through the compositor clipboard, focus a pane, and close it through End Task.
- Include rapid child exit and a second real application window. Environmental
  absence cannot become a pass.

### D. Qualification

- Add X11 and Wayland Task Manager cells to the authoritative matrix and the
  existing qualification runner; do not add another orchestration layer.
- Run formatting, strict Clippy, full workspace tests, controlled compositor
  cells, copy-safe mutation tests, and every presently executable matrix cell.
- Update the feature inventory and dogfood report with every discovery, failure,
  repair, receipt, and the explicit network-accounting limitation.

## Completion boundary

This slice is complete when the source-visible Task Manager behavior above is
implemented and its real CPU/memory/process journeys pass. GH-19 remains open
and `utilities.task-manager` remains `PARTIAL` until trustworthy per-pane
network throughput and the remaining container/cgroup qualification cases are
implemented. The UI must say “Unavailable”; qualification must not count that
gap as passing network telemetry.
