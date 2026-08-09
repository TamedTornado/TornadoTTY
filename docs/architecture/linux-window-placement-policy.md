# Linux window frame and placement policy

Zentty persists the actual client width and height of every mapped GTK window
in the source-compatible `WindowRecipe.frame`. On restore, finite dimensions
of at least 320×240 are requested before the window is presented. Imported
`x`, `y`, and screen metadata are preserved when the size is updated.

GTK4 does not expose a supported cross-backend API for application-directed
toplevel coordinates. Wayland deliberately assigns placement to the
compositor; GTK4 also removed the legacy `gtk_window_move` API rather than
offering an X11-only product path. Consequently:

- X11 qualification proves exact externally driven size persistence and exact
  restored client geometry, but does not claim coordinate placement.
- Wayland qualification proves the exact per-window client-size request and
  records the compositor-negotiated allocation, but does not claim final
  coordinates or that the compositor must honor a requested size.
- Zentty never converts missing coordinate authority into PASS, never uses an
  X11-only move implementation in product code, and never fabricates observed
  coordinates. Fresh Linux frames use neutral `0,0` schema values because the
  source schema requires numeric coordinates; those values are not a placement
  request.

If GTK gains an appropriate portable placement contract, or a supported
Wayland activation/placement protocol becomes available to ordinary
applications, this policy must be reviewed before product behavior changes.
