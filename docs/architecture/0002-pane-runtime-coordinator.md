# ADR 0002: One pane runtime coordinator per Linux window

- Status: Accepted
- Date: 2026-08-08
- Tracking: GH-28

## Context

`ApplicationShell` previously held separate maps for Ghostty surfaces, GTK pane
frames, and focus controllers, plus the runtime lease, child count, command
policy, and pending restore input. That made the composition root the implicit
owner of terminal construction, callbacks, disposal, and UI projection. It
also made it possible for a later extraction to accidentally introduce a
second pane registry.

`WorkspaceState` is the durable topology authority. Ghostty is the terminal
engine, and `zentty-ghostty` is the only safe native adapter. The Linux shell
needs exactly one transient projection between those boundaries.

## Decision

Each `ApplicationShell` owns exactly one `PaneRuntimeCoordinator`. The
coordinator exclusively owns:

- the Ghostty runtime lease and launch-command policy;
- pane ID to `GhosttySurface`, `PaneFrame`, and focus-controller projections;
- live-child accounting and pending restore prefill text;
- surface construction, callback registration, focus-controller creation,
  native disposal, and callback-driven child-exit cleanup.

The shell may ask for a surface/frame by stable pane ID to render or deliver a
product command, but it cannot access or maintain the underlying maps. tmux
compatibility uses those same lookups and never creates a parallel registry.

Construction is ordered and fail-closed: validate absence, derive the safe
configuration and agent environment, invoke the real native constructor,
attach callbacks/controllers/frame, then commit all live projections together.
Any configuration or native-constructor error unregisters the agent target and
leaves no coordinator entry. Product topology changes roll back through
`WorkspaceState`; a failed Undo Close Pane attempt is explicitly recaptured so
the user can retry rather than silently losing history.

Disposal disconnects focus and native callbacks, detaches GTK children, closes
each Ghostty surface once, and adjusts child accounting once. A stale callback
or repeated close is an explicit no-op. A child exit during shutdown disposes
the registered surface without applying another durable workspace transition.
The runtime remains leased by every surface until GObject finalization.

## Testing boundary

Pure admission, stale-removal, and shutdown child-exit decisions are unit and
mutation tested. The actual product is then exercised with real Ghostty
surfaces and PTYs in private Xvfb and nested Wayland sessions. Those journeys
cover construction rollback, physical input, focus, close/restore/prefill,
callback quiescence, and teardown. The safe adapter's compile-fail tests prove
that runtime and surface handles cannot cross threads and that a surface keeps
its runtime lease.

No test-only surface implementation, mock GTK widget, alternate product, or
second lifecycle harness is introduced.

## Known boundary

The deterministic partial-construction journey creates its first native
surface and rejects the second surface at the safe string-encoding boundary.
The pinned native API has no supported way to force an otherwise valid surface
constructor to return null without fault injection or resource exhaustion. We
do not add a product hook or broaden the upstream Ghostty API merely to fake
that condition. Native null handling remains implemented in `zentty-ghostty`
and structurally reviewed; it is not described as a real forced-null product
test.

## Consequences

- `ApplicationShell` no longer contains a shadow pane/surface registry.
- Pane runtime policy remains downstream; no Ghostty patch is required.
- GTK projection access is narrow but deliberately synchronous on the GLib
  owner thread.
- Future lifecycle state must extend this coordinator rather than accrete a
  sibling registry or scenario-specific product path.
