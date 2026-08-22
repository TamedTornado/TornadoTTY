# Linux worklane shell closeout plan

Status: ratified execution plan for GH-4, with overlapping GH-16 acceptance

## Audit result

GH-4 cannot be closed by checking its stale acceptance boxes. Five inventory
entries owned by GH-4 remain `PARTIAL`, and both authoritative
`product-worklanes-{x11,wayland}` cells remain `NOT_IMPLEMENTED`.

The cell defects are partly stale:

- physical horizontal and vertical divider drags and double-click equalization
  already pass on controlled X11 and Wayland in `rust-session-restore`;
- the source contextual **Move Pane to Worklane** catalog already passes on
  controlled X11 and Wayland through the existing sidebar/multi-window actors;
- real Ghostty PTY survival, stable pane identity, and exact persisted geometry
  already pass in those same maintained actors.

The genuine remaining gap is a complete real accessibility and dynamic-state
qualification of the compound worklane/pane UI. Existing code assigns GTK
roles, labels, selected states, tooltips, and sensitivity, while existing agent
journeys exercise running, progress, needs-input, failed, and completed events.
No current test proves those two boundaries meet in the accessibility tree of
the running staged product.

## Feature outcome

Complete the real worklane shell contract as one feature:

1. Launch the staged ReleaseSafe product inside the existing controlled X11 and
   Wayland environments and retain the real pointer, keyboard, PTY, and dynamic
   agent journeys.
2. Exercise the actual sidebar, pane-control, and divider GTK widgets with
   GTK's official `GTK_A11Y=test` backend in a single-threaded controlled-display
   test. Do not add a test-only model API or infer accessibility from source
   calls alone.
3. Prove worklane and pane rows, action buttons, contextual controls, and pane
   dividers expose the intended roles, nonempty source-derived names, selected
   state, action availability, and enabled/disabled state.
4. Drive authenticated agent lifecycle events through the existing real Unix
   socket and verify progress, needs-input, failure, completion, and clearing
   update the visible and accessible pane/worklane summaries.
5. Keep worklane creation, rename, reorder, color, contextual transfer, pane
   split/move/close, divider pointer gestures, focus, PTY survival, and restore
   in the existing actors. The qualification cells compose those maintained
   journeys; no second workspace actor, fixture protocol, or model mutation path
   may be created.
6. Promote only the five GH-4 inventory entries and two matrix cells after every
   required compositor journey passes. Then reconcile GH-4 and overlapping
   GH-16 text without claiming the broader visual-polish work of GH-16 complete.

## Test construction order

1. Add focused assertions for missing named widgets, wrong roles, missing label
   and selected state, action identity, and disabled contextual actions.
2. Run the GTK accessibility contract against actual widgets and record the
   initial failure.
3. Repair only missing GTK accessibility projection in the owning sidebar,
   pane-control, or divider component.
4. Extend `rust-agent-ipc` for the real dynamic accessibility assertions; do not
   add another agent-status harness.
5. Define each `product-worklanes-*` matrix command as a composition of existing
   maintained actors plus the enhanced agent/accessibility journey.
6. Run focused Rust tests, architecture/inventory/matrix validators, governed
   mutation testing for any changed pure projection policy, and both affected
   matrix cells. Do not run unrelated app-wide qualification merely to close
   this feature.

## Claim limits

- Passing GTK accessibility metadata is not visual screenshot parity. GH-16
  remains open for its broader screenshot, motion, focus-halo, overflow, and
  polish acceptance unless separately demonstrated.
- AT-SPI absence or failure to start is a failure or explicit prerequisite, not
  a pass.
- ReleaseSafe Valgrind remains XFAIL. No full Linux qualification claim is
  permitted while any matrix cell is XFAIL or NOT_IMPLEMENTED.
