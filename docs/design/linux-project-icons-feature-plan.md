# Linux project icons feature plan

Date: 2026-08-11
Issue: GH-18 (`project icons` subset)

## Outcome

Zentty discovers a source-compatible icon for each local project without
following a candidate outside that project, performs bounded work off the GTK
main loop, caches positive and negative results, and projects the icon into the
sidebar, Worklane Peek, and focused window context when enabled.

## Source contract

- Preserve the ordered favicon/logo candidates, Xcode AppIcon selection, and
  HTML/TSX `rel=icon` fallback from `ProjectIconResolver.swift`.
- Resolve symlinks only when their canonical target remains under the canonical
  project root. Remote panes and missing/invalid images have no project icon.
- Positive results remain cached. Negative results expire after five minutes;
  explicit invalidation is supported.
- `[panes].show_project_icons` defaults to true and hides all project-icon
  projections without disabling discovery policy tests.
- Candidate metadata and source-file reads are bounded. Discovery never scans
  arbitrary directory trees and never interprets network URLs.

## Ownership and anti-accretion

- `zentty-core::project_icon` owns deterministic path policy and cache state.
- One Linux coordinator owns background discovery and generation-safe delivery.
- Existing sidebar, Worklane Peek, and window chrome remain the only GTK
  projection owners. No second project-context or filesystem watcher exists.

## Test-first order

1. Port candidate-order, AppIcon, markup, symlink-containment, size-bound, cache,
   invalidation, and configuration tests.
2. Add the single background coordinator and stale-result rejection.
3. Project real decoded SVG/PNG icons into all three source-owned GTK surfaces.
4. Run controlled X11 and Wayland journeys against temporary real projects,
   then focused mutation and the complete local qualification matrix.

## Acceptance criteria

- [x] Pure resolver policy and cache tests pass.
- [x] Linux configuration defaults and opt-out match source behavior.
- [x] Discovery is bounded, asynchronous, and generation-safe.
- [x] Sidebar, Worklane Peek, and window chrome render the chosen real icon.
- [x] X11 and Wayland staged-product journeys prove candidate priority,
      symlink rejection, negative-cache expiry/invalidation, and opt-out.
- [x] Focused mutation and every presently executable matrix cell pass.

Passing this slice is implemented-local evidence, not release or full Linux
qualification while the authoritative matrix retains non-PASS entries.
