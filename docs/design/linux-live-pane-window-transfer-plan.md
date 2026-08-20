# Linux live pane-to-window transfer plan

Tracking: [GH-33](https://github.com/TamedTornado/zentty/issues/33)

## Outcome and source contract

Port Zentty's existing **Move Pane to New Window** command while preserving the
same live Ghostty surface and PTY. The source transaction is authoritative:

1. Reject the only pane in the only worklane.
2. Allocate a stable destination window identity.
3. Detach the pane's runtime from the source owner.
4. Extract either the pane or its complete single-pane worklane from the source
   model, preserving source metadata and auxiliary state.
5. Retarget host-owned routing identity for the destination without pretending
   that the already-running child's environment can change.
6. Construct the destination shell and adopt the detached runtime before
   projections observe the destination model.
7. Present/focus the destination, close the source only if empty, and publish
   one aggregate snapshot.
8. Roll back model, runtime, widget, callbacks, routing, and focus on any failure.

Primary source is `AppDelegate.movePaneToNewWindow`,
`WorklaneStore.splitOutPaneToNewWindow`, and
`MainWindowController.splitOutPaneForNewWindow`. Exact source tests distinguish
moving one pane from transferring an entire single-pane worklane.

## Ownership design

- `WorkspaceState` owns pure extraction/rollback data and source fallback.
- `ApplicationCoordinator` owns the cross-window transaction and destination ID.
- Each `ApplicationShell` continues to own exactly one workspace and one
  `PaneRuntimeCoordinator`; it exposes narrow detach/adopt transaction methods.
- `PaneRuntimeCoordinator` owns the live surface/frame/callback connection set.
  A detached value must be linear: source or destination, never both/neither at
  return from the coordinator transaction.
- `ApplicationCoordinator` owns the single process-wide authenticated agent
  and tmux transport plus its capability registry. Window-local
  `AgentEventCoordinator`s retain enrichment and UI projection only. A running
  PTY's inherited source IDs are historical data, not authorization for
  post-move host commands; the process registry resolves its stable pane token
  to the pane's current `(window, worklane)` owner.
- Existing aggregate persistence is the only snapshot path.
- Ghostty receives no pane/worklane/window policy and no API change unless real
  reparenting proves a terminal-owned missing contract.

## Test construction order

1. **Red pure-model tests:** forbidden final pane; multi-pane extraction;
   single-pane worklane transfer; geometry normalization; exact identity/title/
   color/CWD/command/draft preservation; source focus fallback; rollback.
2. **Red runtime-owner tests:** detached surface/frame/callback bundle is absent
   from source, accepted once by destination, duplicate/stale adoption rejects,
   and rollback restores source ownership.
3. **Red coordinator tests:** unique destination ID, insertion/order/active
   state, destination-construction failure, adoption failure, source-close
   decision, and exact aggregate snapshot ownership.
4. **Production wiring:** one action and exact command title, contextual
   availability, narrow shell/runtime transfer methods, then coordinator
   transaction. Do not create a parallel shell or scenario mode.
5. **Real X11/Wayland journey:** extend the existing multi-window actor. Through
   a real GTK action, capture the PTY PID and pre-move scrollback marker, move
   the pane, prove the same PID/widget state survives, send physical input,
   verify source/destination UI and routing, snapshot, clean relaunch, and
   confirmed-live crash relaunch.
6. **Failure journey:** deterministic destination construction/adoption fault at
   the existing construction boundary, followed by real input to the restored
   source pane. No test-only product layout or alternate application actor.
7. **Quality gates:** format, strict Clippy, workspace tests, architecture and
   inventory validators, diff-scoped mutation where pure policy justifies it,
   affected controlled journeys, then every presently executable matrix cell.

## Evidence and claim limits

- Model tests cannot prove live PTY continuity.
- Widget identity alone cannot prove child continuity; record and compare the
  actual PTY child PID and exercise post-move physical input.
- A recreated terminal is a failure even if its command/CWD match.
- X11 evidence does not establish Wayland; both controlled environments are
  required for the supported command.
- Moving into an existing foreign worklane now reuses the same linear live-
  runtime ownership boundary and passes separate controlled X11/Wayland
  journeys. Cross-window drag animation remains explicit GH-16 scope until its
  separately stated pointer, visual, and accessibility contract passes.
- No release or full-Linux qualification claim is permitted while the
  authoritative matrix retains non-PASS required cells.

## Commit boundaries

Prefer independently reviewable commits only when each boundary is usable and
tested: (1) pure extraction transaction, (2) live runtime adoption and product
action, (3) persistence/relaunch and real-system evidence. Do not commit a
model-only product claim or a disposable alternate host.
