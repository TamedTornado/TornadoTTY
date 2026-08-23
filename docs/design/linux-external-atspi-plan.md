# Linux external AT-SPI completion plan

Date: 2026-08-23
Tracking: GH-86 (child of GH-82)

## Outcome

Make the staged Zentty product externally observable through a private real
AT-SPI session on controlled X11 and Wayland. The evidence must come from a
separate Rust inspector process. In-process GTK metadata remains supporting
coverage and cannot satisfy this issue.

## Test-first order

1. Extend the existing `rust-worklane-accessibility` actor rather than creating
   another product journey. Start a private session bus and AT-SPI registry
   inside the existing nested X11/Wayland wrappers.
2. Add one focused Rust AT-SPI inspector to `zentty-test-support`. It must walk
   the registry-owned tree and emit a machine-readable receipt identifying the
   application, window, worklanes, panes, controls, roles, states, focus, and
   live changes it actually observed.
3. Preserve the current failing Zentty receipt and a passing stock GTK control
   receipt so environmental absence can never be mistaken for product success.
4. Locate the failure at the Zentty/Ghostty/GTK boundary. Implement only the
   smallest owning-project repair. Any Ghostty patch must be independently
   tested, product-neutral, and reviewable upstream.
5. Add negative runner coverage for a missing registry, a mock or wrong
   application, stale processes, missing required nodes/states, and a false
   external-accessibility claim.
6. Rerun both controlled compositor actors, focused unit/contract tests, the
   matrix validators, and every presently executable matrix cell affected by
   the change. Do not claim full Linux qualification from this issue alone.

## Boundaries

- The staged product, its real Ghostty surfaces, PTYs, GTK widgets, D-Bus, and
  AT-SPI registry are real. Only deterministic terminal commands and private
  desktop infrastructure are controlled.
- Product accessibility policy and orchestration stay in Zentty. Ghostty owns
  only an embedding lifecycle or surface defect that also affects other hosts.
- No shadow accessibility model, direct model-call substitute, second product
  actor, or ambient developer-desktop dependency is permitted.

## Acceptance evidence

- Raw registry/application identities and inspector JSON receipts for X11 and
  Wayland.
- A real GTK control proves the private service is usable before Zentty is
  inspected.
- Live focus and state changes are observed externally, not inferred from
  widget construction.
- Missing prerequisites produce `BLOCKED` or failure, never pass or skip.
- Every failure, rejected repair, decision, result, and remaining uncertainty
  is appended to the GH-86 dogfood record.
