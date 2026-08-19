# Zentty Linux dogfood: pane runtime coordinator closeout

Date: 2026-08-19

Issues: GH-28 and parent GH-25

## Discovery

GH-28 appeared next in the roadmap, but the production extraction was already
delivered by `3541a48c`. The current product has exactly one
`PaneRuntimeCoordinator` per `ApplicationShell`; it exclusively owns the
Ghostty runtime lease, live surface/frame/focus-controller maps, deferred
launches, callbacks, and teardown. ADR 0002 and the machine ownership contract
already describe and enforce this boundary.

The issue remained open solely because the pinned Ghostty API offers no
supported deterministic way to force a valid native surface constructor to
return null. Earlier work correctly refused to add product fault injection,
resource exhaustion, a fake terminal, or a Ghostty test hook merely to turn
that branch green. Keeping the completed coordinator issue open indefinitely
did not make that impossible stimulus more real; it made the roadmap lie about
the implementation state.

## Decision

The acceptance text now distinguishes the real product construction-rollback
journey from the safe adapter's native-null mapping. Configuration rejection
after one real surface proves transactional application rollback. Native null
is mapped to `SurfaceConstructorFailed` and structurally reviewed at the safe
adapter boundary, but is not claimed as a forced-null end-to-end test. ADR 0002
retains this explicit limitation.

This closes GH-28 without adding a second lifecycle system or weakening a
failure into PASS. With every ordered child complete, GH-25 can also close;
future pane behavior extends the existing coordinator rather than reopening
the decomposition epic.

## Current verification

- ApplicationShell ownership contract: PASS.
- Ownership validator negative suite: PASS.
- Pane runtime decision tests: 9 PASS.
- Workspace state tests: 56 PASS.
- Controlled X11 partial-construction rollback session
  `f9dcad3dd305a69dc7cb36eda2a9f3f83866e25c04361b638acbedd1e4441ddc`:
  PASS with first window real and second configuration rejected.
- Controlled X11 close/restore session
  `90854a68e7a7ec523822afeb225b3bc64a0ab94b6aea7f49d776069ef17f31e2`:
  PASS with real CWD, prefill, ownership, quiescent callbacks, and physical
  input.
- Controlled Wayland close/restore session
  `bbfb1a705ed60573f4978f60948f81e2e34200f4c610aa290c7762b344bc5253`:
  PASS with the same real boundaries.
- Qualification matrix validation: PASS; declarations remain `161 PASS`,
  `3 BLOCKED`, `1 XFAIL`, and `14 NOT_IMPLEMENTED`.

The last coherent complete local run after the extraction passed all then-
executable cells. This closeout reruns the affected ownership, state, and real
product paths; it does not manufacture another aggregate run for an unchanged
coordinator.
