# Linux Worklane Peek completion plan (GH-80)

## Scope

Finish the existing Worklane Peek implementation without creating a second
preview model or another integration-test harness. This issue owns only
`worklane.peek-live-navigation`; cross-window pane drag/drop remains GH-81 and
general chrome/motion/scaling polish remains GH-82.

The macOS implementation is the behavioral source. Its important distinction
from a static window switcher is that Peek is a zoomed view of the existing
live terminal runtimes, with neighboring worklanes available as navigation
context. Linux must preserve that meaning using GTK/Ghostty primitives rather
than copying AppKit mechanics.

## Construction order

1. Extend the existing pure Peek projection/state tests first: source traversal,
   spatial navigation, live metadata, gesture thresholds, cancellation, and
   transition policy.
2. Extend the existing real X11 actor rather than adding a Peek-only harness.
   Prove physical input, live PTY updates while Peek is open, exact-card pointer
   selection, resize cancellation, reduced-motion policy, and focus restoration.
3. Extend the existing controlled input-capable Wayland route for the subset of
   physical events the compositor permits deterministically.
4. Extend the existing multiwindow actor to prove that Peek belongs only to the
   active product window and is torn down safely when its window closes.
5. Implement the smallest product changes needed to make those tests pass.
6. Run focused Rust tests and actors, then the presently executable local
   qualification matrix once. Reconcile inventory/matrix/docs only from the
   resulting receipts.

## Design constraints

- `PaneRuntimeRegistry` remains the only owner of Ghostty surfaces.
- Peek consumes the existing workspace state and agent/project projections; it
  does not create a parallel preview status store.
- A preview must remain bound to the real terminal widget. A screenshot or
  cached title is not evidence of liveness.
- Closing shells reject opening. Ordinary physical input reaches only the
  compositor-focused window; the application coordinator cancels an in-flight
  Peek on real window deactivation. Window close and resize also cancel before
  normal lifecycle work continues. `GtkWindow::is-active` is not used as an
  open-time authorization gate because it is unreliable in WM-free controlled
  X11 sessions.
- Smooth-scroll gestures lock an axis, accumulate to the source 40-point
  threshold, and navigate at most once per gesture. Wheel events preserve their
  one-step semantics. GTK's natural-scroll-normalized deltas are used as
  delivered; the application must not invert them a second time.
- Normal motion is bounded and wraps hard-cut. When GTK animations are disabled,
  selection and dismissal update without animation.
- Accessibility is asserted against real GTK widgets: the shield is a modal
  navigation surface, cards have unique names, and the selected card exposes
  selected/current state rather than color alone.

## Exit criteria

- All GH-80 acceptance criteria have real or deterministic integration evidence.
- No new harness, preview model, surface owner, or global input system exists.
- `worklane.peek-live-navigation` is `IMPLEMENTED` only after both controlled
  display routes and multiwindow isolation pass.
- The dogfood report records every failure, repair, receipt, and remaining
  limitation. Any unproven condition remains explicit and prevents closure.
