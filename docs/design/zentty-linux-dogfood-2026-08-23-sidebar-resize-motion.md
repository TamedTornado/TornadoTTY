# Zentty Linux sidebar resize and motion dogfood — 2026-08-23

Issue: GH-85  
Parent: GH-82

## Source and existing-system audit

- The source policy is explicit in
  `Zentty/UI/Sidebar/SidebarWidthPreference.swift`: default 280px, minimum
  180px, maximum 420px, maximum 33% of available width, and at least 200px of
  remaining content. Linux already has the same single policy in
  `zentty_core::SidebarWidthPreference`; this work must not add another width
  model.
- Linux already owns the live divider through the main `gtk::Paned` and writes
  its clamped position through the authoritative `ConfigStore` after a bounded
  350ms debounce. `reconcile_sidebar_width` and `apply_sidebar_visibility`
  already consume the same preferred width during window clamp, hide/show, and
  hover/pinned transitions.
- Existing X11 source-UX coverage physically drags the GTK divider, asserts the
  live allocation, restores it, waits for persistence, and proves the Ghostty
  OpenGL surface and PTY child were not recreated. The missing X11 matrix state
  is visual evidence, not a missing product control.
- Wayland must use the existing input-capable nested compositor path and real
  pointer delivery. Keyboard-only Cage cannot be relabeled as pointer-resize
  evidence. The existing `rust-pane-search` actor remains the owner; a new
  product actor or width state machine is prohibited.

## Execution order

1. Publish and review the already-real X11 drag state inside
   `rust-source-ux-x11`, then retain its complete existing journey.
2. Extend the existing Wayland search/sidebar journey under its controlled
   outer-pointer compositor profile to drag, clamp, persist, hide/show, and
   restore the same exact width without disrupting its live PTYs.
3. Exercise clean restart and crash-style restore through existing session or
   config-reload infrastructure rather than building a second restore harness.
4. Cover normal and reduced-motion cancellation around the current GTK/product
   transitions, including topology, focus, PTY continuity, and rendered end
   states.
5. Promote evidence only after repeated exact pixels and the complete owning
   actor pass. Keep any remaining gap explicit in the visual map and GH-85.

## X11 evidence insertion

- The existing actor now publishes an unmasked 1200x700 image immediately
  after the real divider reaches its asserted 350–365px allocation and before
  the same pointer returns it to the source default. Terminal output and cursor
  are already deterministic in this actor, so extending the terminal mask
  would be unjustified.
- The first full run captured successfully but then failed to restore 280px.
  Image capture crossed the real 350ms persistence debounce; the 352px write
  and asynchronous live reload raced the immediate return drag and reapplied
  352px after the actor briefly observed 280px. The actor now treats this as a
  real persistence transition: after capture it requires both the fresh resize
  persistence receipt and its live window projection before physically
  dragging back. The failed run is not accepted as qualification evidence.
- Two corrected complete source-UX runs passed and their unmasked expanded-
  sidebar images matched at AE=0. The second 1200x700 image is the reviewed
  baseline. X11 is promoted to `PASS`; Wayland remains the sole explicit
  `NOT_IMPLEMENTED` resize evidence cell until its real pointer journey lands.
- The first post-promotion whole-actor run enforced the new resize baseline but
  later differed by 120 pixels in an existing Peek card's auto-hiding scrollbar.
  Waiting for the expanded-width persistence changed unrelated long-journey
  timing. Resize evidence now uses an explicit early-exit profile of the same
  actor: it performs the real drag, capture, persistence, live projection, and
  product-owned confirmed quit. The ordinary full source-UX journey retains its
  established immediate return drag and timing. The existing focused visual
  profiles share one clean-quit helper rather than duplicating lifecycle code.
- The focused profile passed against the reviewed AE=0 resize baseline and
  exited cleanly through the real confirmation. A fresh unchanged complete
  source-UX run then passed every existing screenshot and lifecycle assertion,
  confirming that X11 resize evidence no longer perturbs unrelated Peek timing.

## Controlled Wayland pointer profile

- `rust-pane-search` now has a focused resize profile that refuses keyboard-
  only Wayland. It requires the existing outer-pointer compositor contract,
  sends input through the real PTY before the drag, physically drags the GTK
  divider, requires the exact live allocation and ConfigStore value, captures
  the compositor output, hides and re-pins the sidebar, and verifies the exact
  width and same PTY afterward.
- Renderer and child-process counts reject Ghostty recreation. The profile
  quits through the ordinary product shortcut and requires a clean process
  exit. It reuses the actor, `SidebarWidthPreference`, `ConfigStore`, and nested
  compositor; no second width model, product API, or actor was added.
- The first labwc run failed to move the divider because coordinates copied
  from undecorated X11 ignored the controlled Wayland compositor's outer frame:
  the outer output was 1024x768 while Zentty reported a centered 1000x700
  client. The profile now performs a bounded 2px scan only across the expected
  divider edge in the blank sidebar region and accepts no candidate until the
  product emits the source-bounded 350–365px allocation. It does not infer a
  pass from pointer delivery alone.
- A diagnostic physical output capture showed the exact decorated geometry:
  the divider is at outer x=293, with the client centered under labwc's frame.
  Rapid scan drags still failed because they did not dwell long enough for the
  nested compositor/GTK grab transition. The maintained route now targets that
  observed edge exactly, allows bounded 200ms enter/grab/presentation steps,
  and then validates the allocation. The one-off diagnostic image remains in
  `/tmp` only and is not a project artifact or accepted evidence.
- The exact-edge run did resize: intermediate receipts were 274px and 330px.
  The old 350–365px expectation was wrong for this 1000px client because the
  source maximum is 33% of available width. Linux correctly clamped at 330px.
  The profile now requires 325–335px, preserving a small compositor rounding
  allowance while proving the source maximum rather than demanding an invalid
  X11-sized target.
- Two complete corrected labwc runs passed at the exact 330px source clamp and
  their unmasked 1024x768 images matched at AE=0. The second image is the
  reviewed baseline and Wayland is promoted to `PASS`. Dedicated X11 and
  Wayland release-tier matrix cells now run the two focused real-pointer
  profiles under their existing controlled compositor environments; resize
  evidence cannot disappear behind a larger journey.
- Promotion also made the missing-baseline negative fixture stale. It now
  falsely promotes the explicit GH-87 `wide-light-x11` failure, which has a
  successful defect capture but deliberately no accepted baseline. The runner
  must continue rejecting that false PASS.
- A fresh promoted Wayland run passed the complete focused journey and enforced
  the reviewed baseline at AE=0. The visual map now has 22 PASS, 2 explicit
  GH-87 FAIL, 0 EVIDENCE_PENDING, 0 NOT_IMPLEMENTED, and 0 BLOCKED scenarios;
  full parity remains unclaimed because the two FAIL states are real defects.
