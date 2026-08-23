# Linux pane drag/drop completion plan (GH-81)

## Outcome

Port the source `PaneDropResolver` behavior to GTK without introducing a
second pane-transfer authority. Dragging is an input and presentation layer;
all committed moves must use the existing `WorkspaceState` mutations and the
existing transactional cross-window detach/adopt/rollback path.

## Construction order

1. Port the source resolver and typed drag payload as pure Rust, then pin its
   precedence, invalid-input, self-drop, stale-generation, and exact-identity
   behavior with focused tests before connecting GTK.
2. Add a dedicated pane-drag GTK component. Do not grow `sidebar.rs` or
   `application_shell.rs` into a second drag coordinator.
3. Install drag sources on real pane frames and sidebar pane rows. Render a
   complete pane identity card as the drag icon and a live, fully rendered
   insertion slot at destinations.
4. Install destination zones for pane leading/trailing/top/bottom regions,
   column boundaries, worklane cards, and foreign windows. Source precedence
   remains worklane target, stack gap, pane split, column insertion, cancel.
5. Route same-window commits to exact-pane core mutations. Route foreign-window
   commits through the existing coordinator detach/adopt transaction, preserving
   the live `PaneRuntimeCoordinator` entry rather than recreating a surface.
6. Reject malformed payloads, self-drops, stale source/destination generations,
   missing panes/worklanes/windows, and teardown races before mutation. A failed
   adopt must use the existing rollback path.
7. Extend the existing `rust-multi-window` and source-UX actors for physical
   X11 and input-capable Wayland pointer journeys. No new workspace harness or
   test protocol is permitted.
8. Add mutation testing only for the pure resolver and mutation boundary. Use
   repository mutation configuration (`gitignore = true`, `copy_target = false`)
   and never copy `build/` into mutant scratch trees.

## Required receipts

- Resolver unit tests and mutation survivors: zero for the scoped resolver.
- Same-window real pointer drops: stack reorder, horizontal split/column
  insertion, worklane append, cancellation, invalid/self drop.
- Cross-window real pointer drop: exact pane ID and PTY PID preserved, source
  cleanup correct, destination focus/input functional, agent/tmux routing
  retained, and clean plus crash-style restore coherent.
- Destination destruction and generation change reject without topology or
  persistence mutation.
- GTK accessibility identifies each draggable pane and valid destination by
  action and identity, without color-only meaning.
- Controlled X11 and Wayland pass the focused journeys before one final local
  qualification attempt. The unrelated GH-83 qualification defect remains
  explicit and cannot be converted into a pass.

## Boundaries

- Zentty owns all GTK drag presentation and orchestration.
- `zentty-core` owns deterministic topology mutation only.
- Ghostty receives no change unless a terminal-owned embedding defect is
  independently demonstrated.
- The existing menu, palette, and keyboard transfer routes remain supported and
  are equivalence controls, not alternate implementations.
