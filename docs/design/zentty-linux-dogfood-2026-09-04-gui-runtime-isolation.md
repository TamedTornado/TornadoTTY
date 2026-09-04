# GUI runtime isolation dogfood record

Date: 2026-09-04

Issues: GH-160, GH-161, GH-162, GH-163, GH-164

## Objective

Prevent terminal workloads and high-rate agent integration signals from making
the GTK client unresponsive or consuming unbounded GUI memory. Preserve real
PTYs and real agent integrations; do not hide environmental failures or replace
the system with mocks.

The work is ordered deliberately:

1. GH-161: remove duplicate canonical mutations and unbounded GUI work.
2. GH-162: place pane process trees in explicit per-pane process groups and
   systemd user cgroups outside the GUI scope.
3. GH-163: bound transport queues and main-loop work.
4. GH-164: consider an out-of-process broker only if reproducible native
   crashes remain after the preceding controls.

## Field evidence

### Long-lived process inspection

The installed TornadoTTY process inspected before the second failure had:

- PID `3677559`;
- approximately 18 hours 46 minutes of uptime;
- `VmRSS` 8,452,860 KiB;
- anonymous PSS 8,327,316 KiB;
- approximately 7,890,348 KiB resident in the main heap mapping;
- 50 threads;
- approximately 37 GiB current and 41.9 GiB peak memory in the application
  cgroup, which also contained pane descendants;
- no cgroup OOM or kernel OOM record.

The application log repeatedly showed an `agent.running` event causing all of
the following work even when its projected status had not changed:

- pane context ownership logging;
- project-context refresh requests;
- whole-sidebar reconciliation;
- fleet snapshot refresh and popover rendering;
- active-worklane reveal scheduling.

`AgentStatusStore::apply_for_target_with_signal` also refreshed `updated_at` for
identical running events, so downstream equality checks saw a new fleet
snapshot every time.

### 08:35 GUI wait/force-quit incident

At approximately 08:35 CEST the user received GTK/GNOME's wait-or-force-quit
dialog again and the terminal disappeared. No source edits, build, or test run
had occurred before this incident; only bounded source inspection commands were
running in a pane.

The affected process was PID `1441610`. Initial inspection incorrectly treated
the last record in the queried journal window as process termination. Later
records from the same PID prove that it survived or recovered from this first
dialog and continued running. There were 4,193 application records in the
roughly 87 seconds ending at `2026-09-04T08:35:56.765+02:00`.

The final records show two overlapping sources of high-rate work:

1. authenticated `agent.running` events repeatedly triggered project context,
   sidebar, and fleet refreshes;
2. both running Codex panes emitted changing Braille spinner frames through the
   terminal-title callback while TornadoTTY's own frame-clock animation also
   rendered those titles.

The second observation is **not** evidence of multiple installed GTK tick
callbacks. `ensure_codex_title_animation_tick` stores one `TickCallbackId` and
rejects a second registration. The defect is that
`CodexTitleAnimation::reconcile` compares the full incoming title, including
the spinner glyph. Each source-generated frame is therefore treated as a new
semantic title, resets `last_frame`, and causes redundant derived presentation
work. This distinction must remain explicit in the repair and tests.

The force-quit action itself prevents post-mortem heap attribution. These
records strongly identify unbounded application-owned main-loop work and heap
retention, but the exact retained allocation path still requires controlled
measurement.

### 09:17 GUI force-quit incident

The same installed PID `1441610` produced a second wait/force-quit dialog and
the TornadoTTY window died at `2026-09-04T09:17:41+02:00`. The PID no longer
existed immediately afterward. Again, there was no Rust panic, clean shutdown,
kernel OOM, or cgroup OOM record.

The two minutes before termination contained:

- 9,840 terminal-title records;
- 4,902 Codex title-animation records;
- 190 authenticated agent-event records;
- 904 sidebar-agent-status renders;
- 204 fleet refreshes;
- 305 project-context records.

That is approximately 82 title records per second before counting the UI and
project work caused by duplicate `agent.running` events. This incident occurred
against the installed pre-repair build while the focused repaired build was
being tested separately. It is direct field confirmation of the reproduced
event-amplification path, not a failure of the repaired test binary.

## Retained allocation path

Source review and a red-before/green-after GTK ownership test identified one
specific application-owned cycle:

1. a pane context popover owns its descendant rename and move controls;
2. the rename button's signal closure strongly owned that same parent popover;
3. the move control and manually parented submenu also used strong parent and
   submenu references in descendant signal closures;
4. sidebar reconciliation replaced the context popover, but the detached
   widget tree retained itself through those closures;
5. repeated full sidebar reconciliation therefore accumulated detached GTK
   trees in the main heap.

Before repair, a weak reference to a detached context popover still upgraded
after the owner replaced it. After repair, all descendant-to-ancestor
references are `glib::WeakRef`, and the move control is a supported
`GtkMenuButton` using `set_popover` rather than a `GtkButton` with a manually
parented popover. The controlled test performs 128 real replacements, detaches
the final popover, drains the GTK main context, and proves that all 128 weak
references are dead. Converting the move control also removed GTK's
`Finalizing GtkButton but still has GtkPopover child` warning.

## Repair

### Canonical event reduction

`AgentStatusStore` and `WorkspaceState` now return whether an accepted event
made a semantic state or lifecycle change. Identical idempotent events preserve
the original status timestamp and lifecycle deadlines. Duplicate
`session.end`, task snapshots, task identities, phase events, and metadata no
longer manufacture changes. The explicitly cumulative `task.delta` event
remains non-idempotent by design.

The GTK event coordinator now performs pane-context projection, project
refresh, transcript enrichment scheduling, persistence, fleet refresh, and
sidebar metadata refresh only for changed events. Multiple changed events in
one runtime drain produce one metadata refresh, and project pane IDs are
deduplicated within the drain. Unchanged-event diagnostics are aggregated and
rate-limited to one record per five-second reporting window.

### Terminal-title frames

Each real Ghostty surface now owns a `TerminalTitleEventGate` before the title
log and GTK idle boundary. The gate canonicalizes Codex Braille spinner frames
to the stable semantic title. Frame-only changes therefore produce no log, no
idle callback, and no second animation reconciliation. Meaningful task counts,
titles, and ordinary shell-title changes still cross the boundary exactly
once. `CodexTitleAnimation` also retains a canonical frame-zero template so a
source-generated spinner frame cannot restart TornadoTTY's own animation.

### GTK widget ownership

Pane context menus use weak child-to-popover references, and nested move menus
use GTK's managed `MenuButton` ownership. Full sidebar replacement is no
longer required for agent status changes; the coordinator uses the existing
metadata-only refresh path.

## Repair constraints

- Canonical event reduction must report whether observable state changed.
- Identical idempotent events must not update timestamps, request project
  probes, rebuild the sidebar/fleet, persist state, or append history.
- Non-idempotent protocol events such as `task.delta` must retain their defined
  semantics; they cannot be globally deduplicated by payload.
- Spinner-frame-only title changes must not be treated as semantic title
  changes, and TornadoTTY must have only one animation owner for projected
  sidebar/window titles.
- Legitimate phase, interaction, task, working-directory, and title changes
  must propagate exactly once.
- No sleeps or timing-only assertions may substitute for state/effect
  assertions.
- Focused unit and integration tests run before any broader qualification.
- Do not restart or replace the user's installed client without explicit
  coordination.

## Test-first sequence

1. Core reducer test: a second identical `agent.running` event returns an
   unchanged outcome and preserves the first event's timestamp.
2. Core reducer test: meaningful running metadata changes return changed and
   update the projection once.
3. Core reducer tests for idempotent snapshots/task identities, without
   changing `task.delta` semantics.
4. Linux title-animation test: different Braille frames with otherwise
   identical text reconcile as one semantic title.
5. Coordinator effect test: unchanged events request no project/sidebar/fleet
   work; a real transition requests only the affected work.
6. Controlled real-IPC burst test and bounded memory/queue soak receipt.

## Current status

### Focused tests

- `zentty-core` agent-status, workspace-state, and app-config integration
  binaries: 143 passed, 0 failed.
- Linux title-gate and event-coalescing unit tests: 9 passed, 0 failed.
- Real Unix transport tests: 6 passed, 0 failed. These cover fresh client
  reconnection, canonical routing, token revocation, wrong tokens, malformed
  and oversized frames, socket ownership, and a stalled client.
- Controlled GTK ownership test: 1 passed, 0 failed under private Xvfb and
  llvmpipe. All 128 detached context menus were released.
- Bash syntax and ShellCheck: passed for both changed journey scripts.
- Focused Clippy: changed core and GTK code passed after allowing only five
  pre-existing lints in unrelated files. The changed event logger initially
  exposed one `uninlined_format_args` warning and was repaired before rerun.

An intentionally broader `zentty-agent-ipc` package run found three unrelated
launch tests that inherit the active dogfood pane's `ZENTTY_AGENT_TOOL=codex`
environment while claiming to model execution outside a managed pane. The
directly affected `unix_transport` binary is green. The ambient-environment
test defect is recorded rather than being relabeled as a product result or
silently ignored; no pane token value was printed. GH-165 tracks making that
launch harness hermetic inside managed panes.

### Mutation strength

The first reducer mutation run established a real green baseline only after it
found a stale pre-rebrand configuration-error expectation. That test now uses
the canonical product name and still proves that invalid input cannot expose a
secret. The first substantive reducer run caught 27 of 35 mutations and left
eight redundant-condition survivors. The implementation and behavioral
assertions were simplified rather than waiving those survivors. The rerun
killed 23 of 23 reducer mutants. A separate title-animation/gate run killed 6
of 6 mutants. The changed autonomy-critical logic therefore killed 29 of 29
final mutants. Mutation runs used the repository's governed wrapper with
`gitignore = true`, `copy_target = false`, and its existing memory/process
limits.

### Real-system receipts

The existing `linux/tests/rust-agent-ipc` journey gained an
`event-coalescing` scenario; no parallel harness was introduced. It launches
the real ReleaseSafe application, GTK, Ghostty surface, PTY child, CLI helper
processes, and authenticated Unix socket. It uses explicit producer and GTK
presentation receipts rather than sleep-as-success.

The normal 500-event/500-title-frame run passed with 304 KiB anonymous RSS
growth, 320 KiB PSS growth, zero main-heap growth, zero descriptor growth, a
four-millisecond GTK drain, zero duplicate state changes, and exactly one
meaningful task/UI/persistence transition.

The bounded soak sent 5,000 duplicate authenticated events and 5,000 changing
Braille terminal-title frames over 26.903 seconds. It passed with:

- 724 KiB anonymous RSS growth;
- 693 KiB total PSS growth;
- 1,040 KiB main-heap PSS growth;
- zero descriptor growth;
- minus one thread and zero child-process growth;
- zero duplicate state changes, project refreshes, full sidebar rebuilds,
  attention changes, desktop notifications, semantic title events, or
  animation restarts;
- exactly one meaningful fleet/status/persistence transition;
- a four-millisecond GTK response after the producer's completion;
- six aggregate unchanged-event log records against a duration-derived maximum
  of seven.

The first soak invocation failed because the journey assumed the short run
could emit at most two aggregate diagnostics. A 26-second producer correctly
crosses multiple five-second reporting windows and emitted six. The assertion
was repaired to enforce the documented reporting cadence plus one boundary
allowance; no product behavior or resource threshold was weakened. The rerun
passed. The private display became unreachable, its run root was removed, and
no controlled process remained.

The memory ceiling is 16 MiB for RSS, PSS, anonymous PSS, and main-heap PSS.
That is deliberately conservative relative to the repeated observed baseline
of approximately 0.3--1.0 MiB growth while still failing growth remotely close
to the installed process's multi-gigabyte defect.

### Remaining uncertainty and deployment

No repaired product has been installed or launched during this work. The
09:17 incident occurred in the older installed build. The focused evidence is
green, but GH-161's field acceptance remains open until the repaired installed
application survives real dogfooding without renewed monotonic heap growth or
event storms. Raw transport channel bounding and pane process/cgroup isolation
remain explicitly owned by GH-163 and GH-162 respectively; they were not
smuggled into this repair.
