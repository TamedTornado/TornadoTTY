# Linux Global Find plan

Tracking: [GH-34](https://github.com/TamedTornado/zentty/issues/34)

## Feature outcome

Port Zentty's source **Global Find** as one window-local coordinator and one
sidebar HUD. It fans a query out to the real Ghostty search state of every live
pane in the current window, aggregates totals, and navigates exact matches in
frozen source order across worklanes. Ghostty remains the only scrollback search
engine; Zentty owns orchestration and presentation.

## Source contract

The authoritative behavior is in `GlobalSearchCoordinator`,
`GlobalSearchFocusChoreographer`, `SidebarGlobalSearchButton`,
`SidebarGlobalSearchRowView`, and the `window.search.find` command definition.
The port must preserve:

1. target order frozen when each non-empty query starts;
2. immediate dispatch for three or more characters and 150 ms debounce below
   that threshold;
3. explicit navigation flushing a pending short query, then waiting for the
   first total from every live target;
4. one aggregate zero-based selection internally and one-based `selected/total`
   presentation;
5. within-pane traversal before cross-pane wraparound, skipping zero matches;
6. exact worklane/pane selection before the destination Ghostty navigation
   action, with field focus retained;
7. stale-target removal without admitting panes created after query freeze;
8. clear versus close distinction and complete terminal-search cleanup; and
9. strict window scope.

## Ownership

- A new pure `zentty-core` Global Find state machine owns frozen targets,
  per-pane totals/selections, aggregate ordinal, debounce/pending-navigation
  decisions, and reconciliation. It owns no timer, GTK widget, or terminal.
- One focused Linux coordinator module owns the GLib timeout and maps pure
  decisions to existing `PaneRuntimeCoordinator` surfaces and workspace
  selection.
- One focused GTK sidebar component owns the search button/row, exact labels,
  aggregate count, accessibility, and focus lifecycle.
- `PaneRuntimeCoordinator` remains the only pane-to-Ghostty registry.
- Existing Ghostty binding actions and `GhosttySearchOverlay` GObject state are
  used first. No ABI change is allowed without a red real-system proof that the
  existing product-neutral contract is insufficient.
- The existing real-product actor is extended through its existing multi-window
  path; no new product actor or search harness is permitted.

## Test construction order

1. Red pure tests for fan-out, totals, ordinals, next/previous, wrap, pending
   totals, focus divergence, stale/frozen membership, clear, and end.
2. Red GTK component tests for vocabulary, accessibility, count formatting,
   sensitivity, and one action identity.
3. Implement the pure state machine and window-local coordinator without UI.
4. Wire the sidebar HUD, shortcut, palette action, and field-focus lifecycle.
5. Extend controlled X11 and Wayland journeys with distributed real PTY
   scrollback matches, hidden-worklane navigation, physical post-navigation
   input, debounce flush, membership changes, and two-window isolation.
6. Run focused mutation testing only over pure policy, with permanent
   `gitignore=true` and `copy_target=false`; then strict Clippy, workspace and
   architecture gates, affected journeys, and every presently executable
   authoritative matrix cell.

## Claim limits

- Model tests do not prove Ghostty scrollback search or GTK focus.
- One pane does not prove aggregation or cross-worklane navigation.
- X11 does not establish Wayland.
- Search receipts may contain only pane IDs and numeric state; queries and
  terminal text must not enter diagnostics or dogfood evidence.
- `search.window-global` may be marked IMPLEMENTED only after both compositor
  journeys and every presently executable qualification cell pass.
- Multi-window aggregate search, snippets, regex, and fuzzy search are later
  scope.
