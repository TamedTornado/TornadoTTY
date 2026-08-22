# Linux Navigation Recents Plan

Issue: GH-79  
Parent: GH-16

## Objective

Port Zentty's session-scoped Recent Actions and grouped empty command-palette
presentation without creating another command router, pane-history store, or
multi-window authority. Requalify the already delivered Back/Forward and
cross-window exact-pane routes as part of the same user workflow.

## Source contract

The implementation authority is:

- `Zentty/UI/CommandPalette/RecentCommandsTracker.swift`;
- `Zentty/UI/CommandPalette/CommandPaletteController.swift`;
- `Zentty/UI/CommandPalette/CommandPaletteResultsResolver.swift`;
- `Zentty/AppState/PaneFocusHistory.swift` and its controller; and
- `Zentty/UI/AppActionRouter.swift`.

Recent Actions are not persisted. The source records command, Open With,
server, worklane-color, and settings identities, but not pane destinations,
restored commands, or task runners. History is most-recent-first, unique, and
capped at eight.

## Construction order

1. Add failing core tests for stable recent identities, eligibility, ordering,
   capacity, stale pruning, and grouped empty results.
2. Implement a focused core recent-action tracker and a structured palette
   result model. Preserve the existing flat resolver as a compatibility wrapper
   only if current callers/tests still need it.
3. Render semantic section headings and rows in the existing GTK palette.
   Record only successful clicks on source-eligible items; do not add a second
   action dispatcher.
4. Extend the existing source-UX real-product scenario to execute eligible and
   ineligible palette entries, reopen the palette, inspect grouping, and prove
   exact PTY focus. Extend—not duplicate—the existing multi-window scenario for
   cross-window routing regression evidence.
5. Run focused tests first, then formatter, pedantic Clippy, workspace tests,
   relevant controlled X11/Wayland cells, mutation tests, inventory/matrix
   validators, and only the presently executable qualification cells affected
   by the feature.
6. Review the diff for duplicated state/router/test authorities, update the
   inventory and dogfood record, commit, push, and close GH-79 only when its
   acceptance criteria are satisfied.

## Architecture constraints

- One `RecentActions` state object owned by the existing `CommandPaletteView`.
- Stable identity derives from `CommandPaletteTarget`; display strings never
  identify history entries.
- The existing `WorkspaceState` remains the sole per-window pane-focus history.
- The existing application/window-set callbacks remain the sole cross-window
  routing authority.
- No new integration harness; extend `rust-source-ux-x11`, a current controlled
  Wayland palette journey, and `rust-multi-window` only where necessary.

## Completion claims

GH-79 may close when the feature works and all affected executable cells pass.
GH-16 remains open for its unrelated screenshot, accessibility, animation,
hardware-button, and broader chrome-polish scope.

## Completion

Completed 2026-08-22. The implementation uses one core recent-target tracker,
the existing palette view/router, the existing per-window pane history, and the
existing application/window-set cross-window authority. No additional product
or integration-test system was introduced.
