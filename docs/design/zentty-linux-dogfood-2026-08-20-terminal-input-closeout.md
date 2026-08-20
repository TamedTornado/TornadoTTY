# Terminal input closeout dogfood — 2026-08-20

## Scope frozen before implementation

This pass is the remaining `terminal.input-selection-scrolling` work owned by
GH17. It does not reopen the already-qualified search, clipboard-transform, or
remote-transfer implementations, and it does not create another terminal or
test authority.

The source behavior to preserve is:

- Ghostty owns VT processing, PTYs, rendering, key translation, IME, terminal
  mouse reporting, selection, clipboard ownership, scrolling, and local drops;
- Zentty owns only the surrounding pane-switch gesture and remote-file policy;
- ordinary vertical scrolling must continue to reach Ghostty, while the source
  horizontal/shift-wheel gesture switches panes once per gesture;
- local file and URL drops insert Ghostty's escaped local representation, while
  an identified SSH pane diverts the same desktop offer through Zentty's safe
  remote-transfer policy;
- primary selection paste, selection drag/autoscroll, smooth scrolling, and a
  background pane's viewport must survive Zentty embedding unchanged.

The implementation may extend existing real-product actors and matrix cells.
It must not add a second input abstraction, a test-only product API, a duplicate
GUI journey, or an application-wide qualification run.

## Baseline audit

The macOS source implements its own AppKit scroll host and selection-autoscroll
controller because AppKit is the host. Linux embeds Ghostty's native GTK
surface instead. The vendored official GTK surface already owns:

- `GtkIMContext` key filtering, preedit, commit, and cursor placement;
- GTK primary-selection middle-click policy and terminal mouse precedence;
- `GdkFileList`, `GFile`, and string drop targets;
- `GtkScrollable` adjustment synchronization;
- precision-scroll state and ordinary terminal scroll callbacks.

Zentty's Linux shell adds one capture-phase controller only for pane switching.
Its existing policy leaves unmodified vertical scrolling unhandled so the event
continues to the Ghostty surface. Existing focused tests cover its thresholds,
one-shot behavior, cooldown, and vertical fallthrough. Existing real X11 input
coverage drives the horizontal-wheel path through the staged product.

Existing real-product evidence also covers real PTY scrollback search,
Ghostty-owned selection, standard and primary clipboard ownership, two-window
clipboard isolation, local-file desktop offers diverted into a real disposable
SSH transfer on X11 and nested Wayland, hostile transfer cases, and pane/session
scrollback continuity. What remains is to make the local terminal interactions
explicit rather than inferring them from source ownership: local file drop,
middle-click primary paste, ordinary vertical scroll routing, and selection
autoscroll/background-viewport stability.

## Discoveries and decisions

- The source AppKit autoscroll controller must **not** be ported beside the GTK
  surface. Doing so would create two scroll authorities. The native Ghostty GTK
  implementation is the Linux behavior owner and should be tested in situ.
- IME has explicit `NOT_IMPLEMENTED` matrix cells on both compositor axes.
  This pass must not silently convert those cells to a pass or call GH17 fully
  complete until real composed-input qualification exists.
- The existing `file_drag_source` is a separate real GTK desktop application,
  not a fake Zentty component. Reuse it for local-drop evidence rather than
  inventing a second DnD harness.
- The existing `rust-pane-search` journey already owns physical selection and
  platform clipboard behavior, so primary-paste and local-drop assertions
  belong there. Scroll-routing evidence belongs in the existing source-UX
  journey and focused pane-switch policy tests.

## Work log

### Focused implementation

- Extended the existing `rust-pane-search` product actor rather than adding a
  new journey. `ZENTTY_TERMINAL_INPUT_ONLY=true` selects a bounded branch of
  the same staged product, real Ghostty surface, real PTY child, and compositor
  input helpers.
- Added explicit matrix cells for X11 terminal interactions, Wayland primary
  paste/scroll behavior under labwc, and physical local-file drop under Weston.
  The Wayland split is deliberate: this Weston build does not advertise the
  primary-selection protocol, while labwc does; environmental absence was not
  converted into a pass.
- Extended the existing physical DnD helper with an optional explicit nested
  Wayland destination. The old adaptive route remains the default for the
  established remote-transfer journey. Focused helper tests pin the explicit
  target and reject malformed coordinates.
- The local file fixture contains a space, dollar sign, and parentheses. The
  independent GTK source advertises `text/uri-list`; the PTY receives Ghostty's
  exact escaped path; and no Zentty remote-transfer receipt is allowed.
- The test disables Ghostty's unsafe-paste confirmation only inside the
  insertion/routing scenario. A file-list drop necessarily includes a newline,
  so the default confirmation would otherwise make compositor automation test
  the dialog instead of local-drop routing. This does not change product
  defaults or claim that the upstream confirmation policy was retested here.

### Failures and repairs

1. The first X11 local drop correctly reached Ghostty but stopped at its unsafe
   paste confirmation. The scenario now disables only that upstream gate and
   asserts the eventual PTY bytes; product defaults remain unchanged.
2. The first labwc cross-process drop landed outside the terminal. The existing
   adaptive drag destination was designed for the larger session-restore
   topology, not this compact window. The shared helper now accepts an explicit
   validated terminal target while retaining its original default.
3. Weston rejected primary-selection publication because it lacks the protocol.
   Qualification was split: labwc owns primary paste, vertical scrolling,
   autoscroll, and viewport stability; Weston owns physical Wayland DnD.
4. A Wayland split-pane assertion typed before the new surface finished focus
   acquisition and lost the first four characters. Waiting for the real
   `terminal-ready-pane` and `focus-pane` receipts fixed the actor race without
   adding a product hook.

### Passing receipts

- X11: middle-click primary paste, physical local file drop, ordinary vertical
  scrollback, exact background viewport continuity, and selection edge-drag
  autoscroll passed through the staged ReleaseSafe product.
- Wayland/labwc: middle-click primary paste, ordinary vertical scrollback,
  exact background viewport continuity, and selection edge-drag autoscroll
  passed through the staged ReleaseSafe product.
- Wayland/Weston: physical cross-process GTK local file drop passed through the
  staged ReleaseSafe product and real PTY.
- Focused matrix, orchestration, shell syntax, and DnD helper contract tests
  passed after the repairs.

### Remaining limitation

GH17 is **not closed** by this pass. The authoritative matrix still has
`ime-x11` and `ime-wayland` as `NOT_IMPLEMENTED`, owned by GH8, and the source
feature inventory remains `PARTIAL`. The controlled IBus focus reproducer is
memory-suppression evidence, not a composed-input product test. No exhaustive
terminal-input or full-Linux qualification claim is made.
