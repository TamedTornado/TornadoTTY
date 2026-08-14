# Linux agent fleet lifecycle completion plan

Issue: GH-21

Source authorities:

- `Zentty/UI/MenuBar/MenuBarFleetState.swift`
- `Zentty/UI/MenuBar/MenuBarFleetSummary.swift`
- `Zentty/UI/MenuBar/MenuBarPaneSnapshotBuilder.swift`
- `Zentty/UI/MenuBar/MenuBarStatusMenuBuilder.swift`
- `Zentty/AppState/Agent/PaneAgentReducer.swift`

## Discovered gap

The Linux fleet model already contains Waiting, Stopped, Compacting, Active,
Idle, incomplete task progress, grouped rows, exact typed targets, Settings,
and Quit. Existing real-product qualification proves only two live states
(Waiting and Running) and one cross-window route. More importantly,
`AgentPhase::UnresolvedStop` cannot currently be produced by the Linux reducer:
the canonical protocol parses `state.stopCandidate` and tracked agent PIDs, but
the reducer never consumes the former or sweeps the latter. Stopped fleet rows
therefore exist only in unit fixtures.

## Source behavior to preserve

- Fleet priority is Waiting, Stopped, Compacting, Active, then Idle.
- Waiting and Stopped share the Waiting section; Compacting and Active share
  Running; Idle is separate.
- Incomplete progress is visible and completed progress is hidden.
- A stop-candidate event keeps a previously observed live session Running for
  a two-second grace period. New Running activity cancels that candidate; an
  expired candidate becomes Idle.
- A tracked agent process that dies while Starting, Running, or needing input
  becomes Unresolved Stop for ten minutes. An already Idle dead process is
  removed immediately.
- Idle without a live tracked process expires after two minutes; non-attention
  stale sessions expire after thirty minutes.
- Closing or moving a pane removes or transfers every lifecycle deadline and
  status together. Rebuilding from the current window registry removes closed
  windows and prevents stale rows from becoming routing authority.
- Settings and Quit remain standard product actions, not fleet-owned alternate
  implementations.

## Implementation boundary

1. Add deterministic core lifecycle tests first: stop-candidate grace,
   cancellation, PID-death unresolved stop, exact visibility expiry, pane
   removal, and pane transfer.
2. Add lifecycle clocks to the existing `AgentStatusStore`; do not add another
   status reducer, timer, or pane registry.
3. Add one bounded window-local sweep through the existing
   `AgentEventCoordinator`. Linux process liveness is observed from numeric
   `/proc/<pid>` entries; the core accepts a closure and remains platform-free.
4. Improve fleet receipts/accessibility only where necessary to prove the
   already source-shaped grouped UI. Do not add a second fleet surface.
5. Expand the existing real multi-window journey with authenticated IPC and
   real PTYs to prove progress, completion, compacting, stop-candidate grace,
   tracked-process death, state reprioritization, pane/window cleanup, stale
   target rejection, Settings, and Quit. X11 proves exact cross-window input;
   Wayland retains its precise token-activation XFAIL only at that final edge.
6. Update the feature inventory and dogfood record, run mutation targets where
   practical, then run every presently executable qualification cell before
   commit and push.

## Acceptance boundary

This slice completes the always-available in-window fleet lifecycle. It does
not implement a StatusNotifierItem or disguise the existing Wayland
cross-toplevel activation-token defect. No exhaustive, release, or full-Linux
qualification claim is permitted while the authoritative matrix has non-PASS
cells.
