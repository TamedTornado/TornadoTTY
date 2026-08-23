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
