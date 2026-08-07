# Linux pane-lifecycle matrix convergence plan

- **Status:** completed
- **Date:** 2026-08-07
- **Owners:** #5 and #12

## Problem

The authoritative matrix still declares 22 of the 24
`product_pane_terminal_lifecycle` cells `NOT_IMPLEMENTED`, although the Rust
product, safe Ghostty adapter, and real product journeys now exist. The rows
cannot honestly become PASS by pointing them at whichever build happens to be
in `build/linux`: Debug and ReleaseSafe evidence must remain distinguishable,
and the lifecycle family must exercise more than the existing terminal-smoke
family.

## Acceptance

1. `build-local` preserves independently runnable Debug and ReleaseSafe staged
   bundles, each with its exact metadata and libraries.
2. Product tests load libraries from the selected staged bundle and reject a
   profile mismatch instead of silently testing the last build.
3. Every single-terminal lifecycle cell runs three fresh real product/GTK/
   Ghostty/PTY lifecycles for its compositor, profile, and async backend.
4. Every multi-terminal lifecycle cell physically closes and restores a real
   pane, proving fresh identity, focus fallback, CWD, command prefill, PTY
   ownership, persistence, and teardown for its compositor, profile, and async
   backend.
5. Controlled Wayland cells requiring input use the input-capable nested
   compositor. X11 cells use the controlled nested X11 server.
6. Static governance rejects missing axes, wrong profiles, wrong journeys, or
   reintroduction of the retired C host.
7. All 24 commands must actually pass before the status changes are committed.
   Environmental absence remains a failure/prerequisite result, never a pass.
8. Dogfood records the red state, failures, repairs, receipts, runtime, and new
   totals. Full qualification remains false while any other required cell is
   not PASS.

## Runtime boundary

This closes a previously ratified matrix family; it does not add another
harness layer. It reuses `build-local`, `rust-product-smoke`,
`rust-closed-pane-restore`, the existing controlled compositors, and the
existing matrix runner. The single and multi commands intentionally prove
different lifecycle behavior from each other and from ordinary terminal smoke.
