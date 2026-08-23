# Zentty Linux proportional wide layout dogfood — 2026-08-23

Issue: GH-87
Parent: GH-82

## Source audit and repair

- The explicit `wide-dark-x11` and `wide-light-x11` failures showed two live
  columns retaining their prior absolute widths after the real window grew to
  1600x900, leaving the right side of the pane viewport unused.
- The source behavior is not an equal-width guess. `WorklaneStore` computes
  `nextReadableWidth / previousReadableWidth` whenever its layout context
  changes, and `PaneStripState.scalePaneWidths` applies that factor to every
  multi-column worklane. This preserves user-adjusted column ratios while
  filling the new readable viewport. Single-column lanes continue to resolve
  directly from the current viewport.
- Linux now implements that policy once in `WorkspaceState` and projects it
  through the existing application allocation reconciliation. The live GTK
  overlays and their content-width request are updated in place; the product
  does not rebuild its topology, recreate Ghostty surfaces, or introduce a
  second layout model.

## First-run findings

- The first real-product run stopped at the previously passing 640x600 narrow
  baseline before reaching either wide capture. This was a useful failure, not
  image noise: the old narrow baseline showed one full-width column with the
  second column offscreen, while the source proportional policy correctly
  keeps both columns visible at their preserved ratio. The stale baseline was
  not bypassed or overwritten during that run.
- For candidate review only, a `/tmp` copy of the authoritative visual map
  marked the stale narrow scenario non-promoting. The checked-in map remained
  unchanged. This allowed the same existing X11 actor to capture narrow dark,
  wide dark, and wide light states without adding an actor or weakening its
  semantic assertions.
- The repaired 640x600 state showed both live columns filling the readable
  region. Both 1600x900 states filled the entire pane viewport with the same
  column ratio, retained the focus border without washing out terminal
  content, and preserved sidebar/chrome boundaries. Dark and light are real
  Ghostty theme projections selected through the command palette.

## Evidence and promotion

- Two independent controlled X11 candidate journeys produced AE=0 for all
  three unmasked images: `narrow-dark-x11`, `wide-dark-x11`, and
  `wide-light-x11`.
- The second reviewed images became the baselines. The first images still
  compared at AE=0 after promotion. Both GH-87 wide scenarios moved from
  explicit `FAIL` to `PASS`; the stale narrow baseline was updated without
  changing its already-`PASS` status.
- A fresh run using the checked-in authoritative map passed all three promoted
  gates and closed through the product-owned confirmation path. No pixel mask,
  tolerance, skip, new qualification layer, or fake terminal was introduced.

## Regression qualification

- The complete real-product X11 source UX journey passed after promotion,
  including live PTYs, focus, worklane peek, pane-local controls, arrangement,
  and the reviewed responsive captures.
- The controlled X11 scaling actor passed at 96 and 192 DPI with two real
  Ghostty panes and real PTY geometry.
- The controlled nested-Wayland scaling actor passed ReleaseSafe and Debug at
  1x, 1.5x, and 2x. Its real PTYs observed coherent geometry, SIGWINCH, and
  pointer-coordinate transitions across scale changes.
- The full Rust workspace passed: no failed tests; the four explicitly ignored
  environment-owned GTK accessibility/OpenSSH tests remain owned by their
  existing real-system actors. Focused Clippy passed with warnings denied.
- Qualification-matrix runner tests and schema/coverage validation passed.
- The authoritative visual scenario totals are now **24 PASS, 0 FAIL,
  0 EVIDENCE_PENDING, 0 NOT_IMPLEMENTED, 0 BLOCKED**. Its deliberately bounded
  claim remains `IMPLEMENTED_LOCAL_SUITE_ONLY`; this is not a claim of full
  Linux qualification.

## Remaining scope

- This repair closes the only two `FAIL` entries in the visual map, but it does
  not by itself close GH-87 or claim full Linux qualification. The issue still
  owns reconciliation of its broader semantic/scaling acceptance criteria,
  and GH-86 separately owns external AT-SPI through the Ghostty embed boundary.
- The element inventory itself still reports 36 `PARTIAL`, one `MISSING`, two
  `DEFERRED`, and one `PLATFORM_ALTERNATIVE`. At least the resize-handle entry is
  stale after completed GH-85 work, so GH-87 must reconcile those element
  states against the now-reviewed scenarios rather than treating scenario
  success as issue completion. Hardware Back/Forward and update UI remain
  explicitly deferred under their tracked policies.
