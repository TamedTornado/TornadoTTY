# Zentty Linux CLI topology journeys plan

- **Tracking:** GH-44 (child of GH-22)
- **Source authority:** `ZenttyCLI/ZenttyCLI.swift`,
  `Zentty/AppState/Agent/PaneIPCHandler.swift`, `WorklaneStore`, and `docs/cli.md`
- **Status:** implemented and locally qualified; release/full qualification remain gated by the authoritative non-PASS cells

## Outcome

Complete `grid`, `split`/`hsplit`/`vsplit`, `layout`, zoom, and exact pane
resize as real user journeys. The public Rust CLI must mutate the same
`WorkspaceState`, GTK widgets, Ghostty surfaces, PTYs, persistence snapshots,
and application windows used by the GUI. Parser-only success and log-only
geometry claims are insufficient.

## Source semantics to preserve

1. `split right|left` creates a neighboring column; `split down|up` creates a
   neighboring pane in the selected column. These verbs never invoke Zentty's
   separate add-pane/offscreen navigation policy.
2. `--equal`, `--golden`, and `--ratio N` describe the selected/source pane's
   share after insertion. `hsplit` means `split right`; `vsplit` means
   `split down`.
3. `grid RxC` retains one source pane and creates exactly `R*C-1` panes in
   column-major topology. The optional command runs in all panes unless
   `--new-only` is present. Focus is `source`, `first`, or `last`.
4. `--worklane-id new` creates the grid in a new worklane in the source window;
   `--window-id new` creates it in a new application window. CWD and local
   terminal configuration inherit from the authenticated source context.
5. Layout presets operate on the selected worklane. Percentage resize sets an
   exact bounded column share; directional resize moves by a Ghostty cell.
6. Mutating replies identify the affected window, worklane, source pane,
   created panes, final focus, and resulting topology. Capability tokens and
   environment secrets never appear.

## One-system boundaries

- `WorkspaceState` remains the sole topology model and persistence input.
- `ApplicationShell` remains the sole owner of window-local GTK/Ghostty pane
  creation and layout mutation.
- `ApplicationCoordinator` owns only cross-window creation/presentation and
  rollback. Ghostty receives no Zentty CLI policy.
- Extend `rust-tmux-compat-product`; do not add another product runner. Add a
  small pure topology/result contract only where deterministic failure and
  mutation coverage cannot be obtained from the real-product actor.

## Test-first construction order

1. **Contract reds:** lock result JSON, exact topology ordering, selector
   authentication, dimensions, ratio bounds, aliases, and no-secret output.
2. **Pure topology reds:** 1x1, 1xN, Nx1, NxM construction; source/first/last
   focus; left/up ordering; layout arithmetic; exact percentage/cell resize;
   rollback from every insertion boundary.
3. **Real staged CLI:** invoke the staged CLI inside real Ghostty PTYs; assert
   discovery topology, unique IDs, CWD/environment/command receipts, terminal
   dimensions, focus, titles, process continuity, close, and persistence.
4. **Destinations and recovery:** existing/new worklane and window, hidden
   source, closed source, injected mid-grid surface failure, concurrent
   topology mutation, and restart. Failed construction must restore the exact
   pre-command topology and destroy every partial surface/PTY/capability.
5. **Controlled displays:** repeat representative NxM, focus, exact resize,
   externally driven window resize, and restart under nested X11 and Wayland.
   Environmental absence is never a pass.
6. **Mutation:** row/column arithmetic, target/destination selection, focus,
   inclusion policy, exact resize, and rollback. Always use the repository
   `gitignore = true`, `copy_target = false` mutation policy.
7. Review the complete diff, run every presently executable matrix cell, and
   only then update GH-44 and commit/push.

## Closure criteria

- Every GH-44 acceptance criterion maps to named automated evidence.
- Real PTY receipts prove all four topology shapes and command/CWD/env
  inheritance, not merely model counts.
- X11 and Wayland prove representative physical geometry and focus.
- Partial failures and concurrent changes leave no extra pane, surface, child,
  token, worklane, or window.
- Persistence/restart reproduces topology and sizes without restarting the
  wrong command or leaking a credential.
- Qualification totals are reported exactly. Full Linux qualification is not
  claimed while any required cell remains non-PASS.

## Completed evidence map

- Parser/result/no-secret contracts: `zentty-agent-ipc/tests/product_cli.rs`
  and the staged actor's human/JSON receipts.
- Shape, ordering, isolation, and geometry model: `zentty-core` workspace-state
  integration tests, with 14/14 targeted mutants caught.
- Real surfaces, PTYs, CWD/environment/commands, destinations, concurrency,
  rollback, persistence, and restart: `linux/tests/rust-tmux-compat-product`.
- Controlled display behavior: comprehensive nested X11 plus focused Cage and
  labwc Wayland profiles; focused X11/labwc profiles drive the outer window.
- Qualification declarations: 141 PASS, 0 FAIL, 7 BLOCKED, 1 XFAIL, 15
  NOT_IMPLEMENTED. A bounded aggregate run passed before the final rollback
  mutation contract; post-contract aggregate reruns pass every GH-44 cell but
  each exposed a different unrelated load-sensitive physical-UI startup
  failure that passed immediately in isolation. The latest machine receipt
  therefore correctly leaves the implemented local suite failed and
  release/full qualification not passed.
