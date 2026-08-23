# Linux visual parity evidence plan

Issue: GH-84 (child of GH-82)

## Objective

Create one authoritative, machine-readable map between the source Zentty shell
and the staged Linux shell, then make existing real-product actors produce
reviewable screenshots and semantic receipts for every required state. The map
is evidence policy, not another product model or qualification runner.

## Authority and boundaries

- Source meaning comes from the checked-in Swift/AppKit implementation plus
  `assets/screenshot.png`; Git already versions both.
- Linux ownership comes from the existing Rust/GTK components. A parity entry
  must name both sides and cannot be satisfied by a screenshot alone.
- `linux/tests/visual-parity` validates the map and collected receipts. It is a
  support contract consumed by `qualify-local`; it does not schedule product
  journeys.
- Existing `rust-source-ux-x11`, `rust-wayland-scaling`, `rust-multi-window`,
  sidebar, and accessibility journeys remain the only product actors. They may
  emit additional screenshot/semantic receipts but must not gain alternate
  topology or input protocols.
- Live terminal pixels are nondeterministic. Masks may cover only terminal
  content rectangles derived from real GTK geometry receipts. Chrome, borders,
  controls, labels, badges, menus, and focus indicators may never be masked.

## Construction order

1. Pin the source revision and enumerate every visible shell element, state,
   tooltip, hit target, focus-order position, and accessible name.
2. Define every required scenario and its owning existing actor before adding
   capture code.
3. Implement strict schema, coverage, source-path, scenario, mask, and receipt
   validation with negative tests for missing/stale baselines, unexpected
   masks/skips, and false full-parity claims.
4. Extend the owning actors to capture controlled screenshots after their
   semantic assertions settle. Write a receipt containing environment identity,
   image/baseline hashes, exact masks, and assertion IDs.
5. Review mismatches against the source map. Mark remaining implementation work
   explicitly for GH-85, GH-86, or GH-87; never update a baseline merely to
   erase an unexplained difference.
6. Run every presently executable scenario once, review the complete diff and
   evidence, then commit and reconcile GH-84.

## Acceptance gates

- Every required element and scenario appears exactly once.
- `MATCH` requires source and Linux owners, at least one scenario, semantic
  assertions, and a current reviewed baseline.
- `PARTIAL`, `MISSING`, `BLOCKED`, `DEFERRED`, and `PLATFORM_ALTERNATIVE` require
  a tracked issue or explicit prerequisite and cannot contribute to a full
  parity claim.
- Baselines are tied structurally to one scenario specification: display
  backend, theme, scale, dimensions, actor, and semantic assertions.
- Git reviews baseline changes. The harness validates files and pixels; it does
  not maintain a second set of source, image, binary, or mask hashes.
- Environmental absence is FAIL or BLOCKED, never PASS.
- Hardware Back/Forward remains DEFERRED until real mapped hardware evidence is
  available.

## Dogfood

Use `docs/design/zentty-linux-dogfood-2026-08-23-visual-parity.md`. Record every
source mismatch, capture instability, compositor difference, mask decision,
semantic failure, repair, and remaining limitation before changing status.
