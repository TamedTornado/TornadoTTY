# Zentty Linux visual inventory reconciliation — 2026-08-23

Issues: GH-87, GH-82

## Discovery

- All 24 governed visual scenarios were passing, but the element inventory
  still reported 36 `PARTIAL` entries and one `MISSING` entry. This was not an
  honest description of the reviewed evidence after GH-85 and the responsive
  wide-layout repair.
- The `MISSING` sidebar resize handle was specifically stale: GH-85 shipped and
  passed real pointer resize, persisted bounded width, and controlled X11 and
  Wayland receipts.
- A second governance gap allowed an element to be marked `MATCH` even when
  none of its associated scenarios named the element's semantic assertion, or
  when an associated scenario was non-passing. A green scenario total alone
  therefore could not justify full element parity.

## Reconciliation

- Twenty-five elements now report `MATCH`. Each has source and Linux ownership,
  reviewed screenshots, associated semantic assertion coverage, and only
  passing scenarios. This includes the completed sidebar resize handle rather
  than preserving a stale issue state.
- The validator now rejects a `MATCH` element whose semantic assertion is not
  named by one of its associated scenarios, or whose associated scenario is not
  `PASS`. Focused negative tests prove both false-claim paths.
- Twelve elements remain explicitly `PARTIAL`: pane layout, Forward history,
  project proxy icon, focused context label, branch context, server primary and
  menu, Open With primary and menu, Global Find, bookmarks, and the pane-search
  HUD. Their product implementations exist, but the current scenario metadata
  does not yet establish the complete visual/semantic claim. They were not
  bulk-promoted from screenshots that happen to contain nearby chrome.
- Update availability remains `DEFERRED` to GH-75, and real hardware
  Back/Forward remains `DEFERRED` under GH-87's explicit acceptance boundary.
  Native window controls remain a documented Linux platform alternative.

## Current evidence state

- Element totals: **25 MATCH, 12 PARTIAL, 2 DEFERRED, 1
  PLATFORM_ALTERNATIVE**.
- Scenario totals: **24 PASS, 0 FAIL, 0 EVIDENCE_PENDING, 0 NOT_IMPLEMENTED, 0
  BLOCKED**.
- The visual runner and its negative suite pass. The claim remains
  `IMPLEMENTED_LOCAL_SUITE_ONLY`; GH-87 and GH-82 remain open.

## Focused chrome-state expansion

- The first reconciliation left twelve elements partial rather than pretending
  that a nearby screenshot proved their state. The next focused slice reuses
  the existing X11 source-UX journey at two interactions it already performs:
  Back leaving Forward enabled, and the Arrange Panes icon opening its real
  popover. It adds no product actor or alternate input route.
- The first candidate run captured both states successfully, then failed later
  when pane traversal reached the arranged offscreen terminal in model state
  but the actor reported that physical GTK focus had not transferred. The
  candidate images are retained
  only for repeatability comparison; the incomplete journey is not accepted as
  passing evidence. This failure occurred after capture and remains subject to
  a clean complete rerun rather than being converted into a pass.
- The repeat failed at the same check even though the log showed the model
  route and GTK focus callback both occurred. Ghostty's concurrent diagnostic
  write split the `focus-pane pane=pane-5` stderr record across two lines, so an
  exact-line grep rejected successful behavior. The actor now keeps the exact
  route receipt and uses the immediately following typed OSC title from pane 5
  as the stronger physical-focus proof. This removes a duplicate fragile log
  assertion; it does not replace the real pointer, keyboard, Ghostty, or PTY.
- The first run after removing that duplicate check exposed the real ordering
  it had accidentally provided: the model route can be logged just before GTK
  finishes transferring focus, so immediate typing delivered only the tail of
  the probe. The actor now gives GTK one bounded 200 ms focus-settlement
  interval after the exact route receipt, then still requires the complete OSC
  title from pane 5. It does not accept the delay itself as evidence.
- The repaired complete journey passed. The first and final history-enabled
  candidates compared at AE=0, as did the first and final Arrange Panes menu
  candidates. Both were visually reviewed and promoted without masks or pixel
  tolerance. Pane layout, Forward history, and the already-reviewed focused
  context label now report `MATCH`.
- Current totals are **28 MATCH, 9 PARTIAL, 2 DEFERRED, 1
  PLATFORM_ALTERNATIVE** across elements and **26 PASS, 0 non-passing** across
  scenarios. The remaining partials are project/branch context, server/Open
  With compound controls, Global Find, bookmarks, and the pane-search HUD.
