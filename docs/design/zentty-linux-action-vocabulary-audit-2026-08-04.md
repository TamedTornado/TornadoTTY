# Zentty Linux action vocabulary audit — 2026-08-04

This audit covers every user-facing action label currently rendered by the
Linux shell. The checked-in macOS implementation is authoritative for product
vocabulary. GTK-only descriptions may be platform-native, but product commands
must retain Zentty's verbs because those verbs can encode different behavior.

The executable contract is `crates/zentty-linux/src/source_ui.rs`. Linux UI
surfaces consume those constants, and its tests verify the vocabulary against
the checked-in Swift source rather than merely repeating strings in a Rust
assertion.

## Current Linux actions

| Linux surface | Ratified label | Source evidence | Result |
|---|---|---|---|
| Leading chrome | `Toggle sidebar` | `SidebarToggleButton.swift` | Exact |
| Leading chrome | `Arrange panes` | `PaneNavigationButtons.swift` | Exact |
| Leading chrome | `Navigate Back` | `KeyboardShortcutResolver.swift` and `WindowChromeViewTests.swift` | Repaired: Linux said `Go back` |
| Leading chrome | `Navigate Forward` | `KeyboardShortcutResolver.swift` and `WindowChromeViewTests.swift` | Repaired: Linux said `Go forward` |
| Leading chrome | `Notifications` | `KeyboardShortcutResolver.swift` | Exact; currently disabled |
| Sidebar creation | `New worklane` | `SidebarCreateWorklaneButton.swift` | Exact |
| Worklane menu | `Rename Worklane…` | `SidebarPaneRowButton.swift` | Exact |
| Worklane menu | `Close Worklane` | `SidebarPaneRowButton.swift` | Exact |
| Worklane menu | `Move Worklane Up` / `Move Worklane Down` | `KeyboardShortcutResolver.swift` | Exact |
| Worklane menu | `Worklane Color` | `SidebarPaneRowButton.swift` | Exact |
| Pane menu | `Rename Pane…` | `SidebarPaneRowButton.swift` | Exact |
| Pane menu and Arrange | `Split Right` | `PaneRightCommandPresentation` in `PaneLayoutPreferences.swift` | Repaired: Linux invented `New Pane Right` |
| Pane menu and Arrange | `New Pane Below` | `KeyboardShortcutResolver.swift` and pane context menus | Exact |
| Pane menu | `Move Pane Left` / `Move Pane Right` / `Move Pane Up` / `Move Pane Down` | `KeyboardShortcutResolver.swift` | Exact |
| Pane menu | `Close Pane` | `KeyboardShortcutResolver.swift` and pane context menus | Exact |

`Worklane actions` and `Pane actions` are GTK accessible descriptions for the
two unlabeled overflow buttons, not product commands. They are intentionally
platform-native and must not be presented as source action names.

## Rightward layout semantics

Zentty exposes two distinct rightward commands:

- `Split Right` uses `PaneRightInsertionBehavior.visibleSplit`: create an
  adjacent full-height column and resize the layout so both columns are visible.
- `Add Pane Right` uses `PaneRightInsertionBehavior.worklaneAdd`: create the
  adjacent column without shrinking the current column, allowing horizontal
  worklane expansion.

The current Linux renderer always lays columns out homogeneously inside the
visible viewport. Its implemented behavior is therefore named `Split Right`.
`Add Pane Right` is **not implemented** and must not be offered as an alias until
Linux owns non-homogeneous column widths and horizontal worklane scrolling.

`New Pane Below` inserts immediately after the owning pane in the same column
and divides that pane's current height between the old and new panes. It never
means a full-width row below the worklane or a split of a stale globally focused
pane.

## Prevention

- All current Linux action surfaces use the source-owned constants.
- The contract test reads the checked-in Swift files and fails if the cited
  source vocabulary disappears.
- The contract separately proves that visible split and width-preserving add
  remain distinct source behaviors and rejects `New Pane Right` as their blend.
- Real UI tests refer to `Split Right`; the next pointer scenario must also prove
  exact-pane targeting before pane-local hover controls ship.
