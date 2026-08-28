# Linux dogfood: worklane reorder projection

Date: 2026-08-28
Issue: [TamedTornado/zentty#133](https://github.com/TamedTornado/zentty/issues/133)

## User-visible failure

While creating a worklane for a resuming Codex session, Jason dragged the lane
from the second to the third sidebar position several times. The drag preview
looked correct, but the card returned to its original position. It appeared in
the requested position only after Codex finished another lifecycle transition.

## Raw evidence

The installed process journal for PID 5644 recorded the decisive transaction:

```text
11:16:14 worklane-drag=begin id=worklane-7
11:16:14 worklane-drag=preview-slot target=worklane-6 edge=after reflow=live
11:16:14 action=reorder-worklane id=worklane-7 insertion=2 order=worklane-4,worklane-6,worklane-7
11:16:14 worklane-drag=preview-drop id=worklane-7 placement=after:worklane-6 accepted=true
11:16:14 persistence-save ... topology=window-1[worklane-4:pane-6|worklane-6:pane-12|worklane-7:pane-13]
```

A second visually valid drop was accepted but emitted no model action because
the model already contained the requested order. Later status activity caused
a complete sidebar render, which finally projected that stored order.

## Diagnosis correction

The first investigation incorrectly attributed the failure to pointer starvation
from startup title events. Jason clarified that the drag interaction and preview
were visually correct. Re-reading the receipts with that fact established the
actual invariant violation: the drop handler updated and persisted
`WorkspaceState`, but never reordered the GTK card children. Clearing the
temporary preview therefore exposed the old view order.

The speculative title-coalescing queue and drag-handle changes were removed in
full before the final patch. They are not part of this repair.

## Repair

The reorder action now:

1. commits the stable-ID model order;
2. projects that order directly onto the existing GTK worklane card children;
3. reads the actual GTK child order and records whether it matches the model;
4. restores terminal focus.

It deliberately does not invoke the full sidebar renderer from the drop
callback. The first focused test attempt did that and caught a second defect:
full rendering removed the live preview during GTK drop dispatch, re-entering
the drop path and emitting the action twice. Direct card projection preserves
the gesture-owned preview until `drag-end`, while placing the hidden real card
in its final position underneath it.

## Integration-test isolation discovery

The existing X11 sidebar journey used ambient CLI instance discovery for a
contextual rename. With Jason running the installed dogfood application, that
CLI connected to the live instance instead of the private X11 product and
failed to find the test pane. This could also have mutated a live session if a
selector happened to match.

The journey's real pane children now publish their own private socket, instance
identity, topology IDs, and pane capability into the journey-owned temporary
directory. CLI assertions source that pane-scoped receipt. No ambient running
ZenTTY instance can be selected.

## Validation

- `cargo test -p zentty-linux worklane_drop_edge_changes_at_the_card_midpoint --no-fail-fast`: PASS.
- `shellcheck -e SC1091 linux/tests/rust-sidebar-management-x11`: PASS. `SC1091`
  remains the file's reviewed dynamic-source warning.
- `linux/scripts/build-local`: PASS; ReleaseSafe staged product, Cargo publish-age
  audit 91 packages/0 exceptions, package notices 75 Cargo + 27 Ghostty entries.
- `ZENTTY_LINUX_BINARY=build/linux/bin/zentty-linux GDK_BACKEND=x11 linux/tests/nested-x11 linux/tests/rust-sidebar-management-x11`:
  PASS with nine real Ghostty PTYs, physical GTK drag/drop, immediate actual
  child-order equality, stable identity, active selection, contextual transfer,
  and retained live PTYs.

No full qualification run was performed for this focused dogfood repair. The
staged build was not installed over Jason's active dogfood application.
