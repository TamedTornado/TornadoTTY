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

## Project proxy and branch context

- The source audit found a genuine Linux mismatch rather than an evidence-only
  gap. macOS always shows the focused working directory's workspace icon as a
  fallback when no custom project icon is resolved. Linux hid the chrome icon
  unless a custom bitmap existed, despite already owning the correct picture
  widget and custom-icon resolver.
- The focused pane summary now carries its existing working directory into the
  GTK projection. The existing project-icon view retains custom decoded assets
  and otherwise uses the platform folder icon with the full working directory
  as its tooltip. No second path resolver or persisted state was added.
- The branch control previously used a button's implicit text child. It now
  owns a bounded label with middle ellipsis, preserves the existing remote URL
  action, and explains the full branch plus remote availability in its
  tooltip. This matches the source semantics without copying AppKit widgets.
- The existing real Git/review actor now requires the fallback-icon projection
  receipt in addition to its real nested working directory, repository, `gh`
  boundary, browser-open receipts, and screenshot. It passed on controlled X11
  and input-capable Wayland.
- The intentional icon made both reviewed review-context baselines stale. Two
  independent captures on each compositor were byte-identical (AE=0), were
  visually inspected, and only then replaced the old baselines. Fresh checked-
  in-map runs passed on both compositors; no mask, tolerance, skip, or new
  product actor was added.
- The focused workspace projection test passes, formatting is clean, and the
  staged ReleaseSafe product builds. A broad Clippy invocation encountered the
  already-existing `worklane_peek::render` `too_many_lines` finding outside
  this slice; it was not hidden or repaired opportunistically in GH-87.
- Project proxy and branch context now report `MATCH`. Element totals are
  **30 MATCH, 7 PARTIAL, 2 DEFERRED, 1 PLATFORM_ALTERNATIVE**; the seven
  remaining partials are server/Open With compound controls, Global Find,
  bookmarks, and the pane-search HUD.

## Development-server chrome

- The inventory audit exposed another product gap, not merely stale metadata:
  Linux had the source-compatible server actions and sidebar rows but no
  dedicated server primary/menu segments in the trailing window chrome.
- The existing `WindowChrome` now owns a globe primary segment and an
  independent chevron menu segment before Open With. They are absent when the
  active worklane has no visible server, and are populated from the existing
  ranked-server authority when it does. The primary action uses the existing
  selected-server route; menu entries use the existing targeted server action.
  No server registry, ranker, URL launcher, or test actor was duplicated.
- The source actor's foreground server initially made the terminal capture
  dynamic. The repair did not mask or normalize that output: the same real
  listener now runs as a child of the actor shell, the shell clears its own PTY
  only after both listeners are ready, and the actor physically hides unrelated
  dynamic sidebar rows before the chrome-focused capture. Ghostty's resize
  overlay and cursor are disabled through its normal test configuration.
- The existing development-server actor physically opened the chevron popover,
  closed it, clicked the primary segment, and required the controlled browser
  launcher to receive the exact selected real-listener URL. Its prior CLI,
  shortcut, scanner, Watch, browser, ignore, stop, and restart assertions still
  pass.
- Two independent X11 candidates were byte-identical (AE=0), were visually
  reviewed, and were promoted without a mask or tolerance. A fresh checked-in-
  map X11 journey passed, and the complete existing controlled Wayland journey
  also passed.
- Server primary and server menu now report `MATCH`. Element totals are
  **32 MATCH, 5 PARTIAL, 2 DEFERRED, 1 PLATFORM_ALTERNATIVE** and scenario
  totals are **27 PASS, 0 non-passing**. Remaining partials are Open With
  primary/menu, Global Find, bookmarks, and the pane-search HUD.
