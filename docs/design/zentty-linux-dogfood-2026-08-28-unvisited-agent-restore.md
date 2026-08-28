# Unvisited agent restore intent survives clean relaunches

Date: 2026-08-28  
Tracking: GH-135

## Operator report

The operator reproduced a destructive two-launch sequence:

1. resume Codex in a worklane;
2. close Zentty cleanly;
3. relaunch Zentty without visiting that worklane, then close cleanly again;
4. relaunch and visit the worklane.

The pane opened as a bare shell instead of resuming the Codex session. The
worklane and pane topology survived, but the agent restore intent did not.

## Discovery

`PersistenceCoordinator::complete_launch` intentionally consumes the startup
snapshot after a successful launch. A later clean snapshot therefore has to be
complete from the live `WorkspaceState`; it cannot rely on merging data from
the consumed file.

`WorkspaceState::seed_restored_agent` previously projected an accepted
`PaneRestoreDraft` only into `AgentStatusStore` as a provisional `Starting`
status. `WorkspaceState::agent_restore_drafts` then attempted to reconstruct a
draft from that transient presentation state. An initial shell title or other
pre-authentication lifecycle reconciliation could legitimately remove the
provisional status. The next clean snapshot consequently retained the pane but
published no agent draft.

The deterministic failing core reproduction was:

1. import the persisted window recipe;
2. seed the exact accepted Codex restore draft;
3. reconcile the real pre-authentication `bash` title;
4. request the next snapshot's restore drafts.

Before the repair, step 4 returned an empty list.

The product also clarified an important runtime detail. Zentty constructs the
hidden Ghostty surface model at launch, but an unmounted surface does not start
its PTY command until the worklane is realized. The product journey therefore
has to visit the preserved worklane before requiring the Codex hook receipt.

## Repair

The existing `WorkspaceState` aggregate now retains accepted, pending agent
restore drafts by pane identity. This is not a second persistence system:

- live authenticated agent state remains the preferred source for a new draft;
- pending intent is only the fallback while live state cannot yet produce a
  resumable draft;
- explicit failed-restore fallback clears both agent presentation and pending
  intent;
- closing a pane clears the pending intent after first capturing it for the
  existing closed-pane undo path;
- new-window and cross-window pane transfers move pending intent with the pane.

This prevents incidental terminal presentation from changing durable launch
intent while preserving the existing semantic removal paths. No timer, stale
snapshot retention, second store, or shell-command heuristic was added.

## Test-first evidence

The new core regression failed before the repair:

```text
test unmaterialized_restore_draft_survives_the_next_clean_snapshot ... FAILED
left: []
right: [PaneRestoreDraft { ... session_id: "session-codex" ... }]
```

It passes after the repair. The complete focused workspace-state suite also
passes: 73 tests, 0 failures.

The persistence coordinator now runs two consecutive clean relaunch cycles in
one deterministic test. Each startup consumes its prior snapshot, each
unvisited model receives the initial shell title, and each following startup
must recover the exact accepted draft.

The existing `linux/tests/rust-session-restore` actor gained a focused
`ZENTTY_UNVISITED_RESTORE_ONLY=true` scenario rather than a second harness. It
uses the staged ReleaseSafe binary, real GTK/Ghostty surfaces and PTYs, physical
X11 input, clean application shutdown, persisted snapshot bytes, and controlled
Codex hooks. Its final receipt was:

```text
rust-session-restore: PASS unvisited-agent drafts=preserved relaunches=2 authentication=real
```

## Harness discoveries and corrections

Nothing was converted into a pass silently:

1. The first sandboxed X11 attempt failed before product launch because the
   sandbox view of `/tmp/.X11-unix` did not satisfy Xorg ownership rules. The
   same existing nested-X11 runner was rerun outside that sandbox; no system
   ownership was changed.
2. The first product assertion waited for `terminal-ready-pane` on a hidden
   surface. Logs proved hidden ownership with `surface-owned`; terminal-ready
   correctly occurs only after realization. The assertion was corrected to the
   actual lifecycle contract.
3. The first relaunch initially waited for hidden agents without visiting their
   worklanes. Physical pane traversal was added to match the operator's final
   reproduction step.
4. `Ctrl+Tab` traverses panes in sidebar order, not one worklane per gesture.
   The actor now crosses the intervening pane identities before requiring the
   second agent.
5. The broad background-start helper also forbids visiting an ordinary sibling
   pane. That condition is unrelated once this scenario intentionally traverses
   the lane, so the focused branch checks exact launch counts and authenticated
   events directly instead of weakening the shared helper.

## Remaining uncertainty

This repair proves draft durability and real resume execution across the
reported sequence. It does not change the deliberate policy that an explicitly
failed resume followed by the user's shell fallback removes that resume intent.
It also does not claim that an arbitrary third-party agent can always resume a
session; failures remain visible through the existing recovery UI rather than
silently erasing topology.
