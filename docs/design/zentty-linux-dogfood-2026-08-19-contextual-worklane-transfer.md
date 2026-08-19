# Dogfood: contextual worklane transfer

Date: 2026-08-19  
Owners: GH-16 (source UX), GH-4 (worklane product), GH-32 (multi-window follow-up)

## Source finding

The checked-in macOS source does not present only directional pane moves. Its
pane context menu includes **Move Pane to Worklane**, whose ordered destination
rows use the first pane's displayed identity plus `+N more`, carry the worklane
color, exclude the source worklane, and append **New Worklane in This Window**
only when the source worklane contains more than one pane.

Linux already owned the authoritative `WorkspaceState` mutations for moving a
focused pane to an existing worklane and isolating it in a new worklane. It also
had a typed `move-pane-to-worklane` GTK action. The missing product behavior was
the source contextual catalog and a GTK action for the distinct new-worklane
operation. No second workspace model or transfer mechanism was needed.

## Test-first findings and repairs

1. Extending the typed action registry correctly failed its sensitivity census
   until `move-pane-to-new-worklane` was declared as requiring multiple
   workspace panes. The repaired registry has 116 unique typed actions.
2. A first GTK design placed a `MenuButton` with its own popover inside the
   existing pane popover. A disposable controlled-X11 probe rendered the row
   but did not establish a reliable activation route. The product now drills
   into the existing popover. The child swap is deferred to the GTK idle queue
   so the clicked widget is not destroyed during its own signal emission.
3. The first full Rust unit run failed because the sandbox denied the
   pre-existing real `/proc` listener test permission. The identical suite
   passed with required host permissions; this was not converted into a pass.
4. The first X11 journey sent `xdotool click --window` to the product toplevel
   while the pointer was over a native GTK popover. GTK correctly did not treat
   that as a physical popover click. The repaired journey uses XTEST at the
   current pointer location.
5. Pointer-enter receipts can arrive after a fast scan has advanced. The final
   locator waits at each candidate and searches both axes, then clicks only
   while the matching rendered GTK button is current.

## Delivered behavior

- Every pane context menu derives ordered destinations from the current
  window's live sidebar summaries.
- Empty destinations disappear; the source worklane is never offered.
- Destination labels and colors follow the source catalog semantics.
- Moving a single-pane worklane into another worklane removes only the emptied
  source lane and keeps the exact Ghostty surface and PTY alive.
- **New Worklane in This Window** appears only for a multi-pane source and
  isolates the exact live pane using configured worklane placement.
- Menu catalogs refresh during normal sidebar rendering without rebuilding
  stable worklane cards.

## Evidence

- `cargo test -p zentty-linux --bin zentty-linux --locked`: 259 PASS after the
  sandbox-only listener failure was rerun with required host permissions.
- `cargo test -p zentty-core --locked`: all suites PASS, including 56 workspace
  state tests.
- `linux/tests/nested-x11 linux/tests/rust-source-ux-x11`: PASS, session
  `bf31c6b6516048e79f9b008358f61b4657a8d966d8d6237c847006c334e62c73`.
- `linux/tests/nested-x11 linux/tests/rust-sidebar-management-x11`: PASS,
  session `eecc79087bc2e30964e1e24f37c105f1cf5c7f4a77b554a5990594ab91906aa1`.
  The journey physically opens the pane menu, drills into the contextual
  catalog, moves pane 1 to worklane 2, then isolates it in worklane 10. It
  asserts exactly nine terminal-ready receipts throughout and sends distinct
  physical input through the same real PTY after both moves.

## Remaining limitations

- The shared GTK implementation is compiled for Wayland, but the contextual
  pointer journey has not yet been added to the controlled Wayland input cell.
  The Wayland worklane cell therefore remains `NOT_IMPLEMENTED`; environmental
  absence is not reported as a pass.
- Zentty's source catalog also groups destinations from other existing windows.
  Linux currently catalogs only worklanes in the pane's current window. Moving
  a live pane to a *new* window is already real and qualified, but moving it
  directly into an existing worklane in another existing window remains GH-32
  scope.
- The X11 worklane cell remains `NOT_IMPLEMENTED` for its separately stated
  double-click equalization, progress/failure presentation, and full
  accessibility requirements. This slice removes only the now-executed
  contextual-affordance gap.
