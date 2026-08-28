# Zentty Linux dogfood — terminal pointer contrast

Date: 2026-08-28
Tracking: GH-106

## Reopened discovery

The earlier GH-106 repair correctly removed a transparent pane-drag overlay
that intercepted pointer input across Ghostty's first row. Continued daily use
showed that this was not the entire defect: GNOME Yaru's size-24 `text` I-beam
still loses contrast over ordinary terminal themes throughout the surface.

Replacing the I-beam with the system arrow had already failed operator QA
because it removed the expected text-selection affordance. The defect was
therefore reopened rather than hidden behind the earlier first-row repair.

## Design

Zentty observes the existing Ghostty GTK widget's `mouse-shape` and
`mouse-hidden` property notifications after Ghostty projects its semantic
cursor. Only the rendered named `text` cursor is replaced with a 32-by-32 RGBA
I-beam containing a white core and continuous near-black outline. The cursor's
hotspot remains at its center.

The host does not change Ghostty's terminal protocol, embedding ABI, or cursor
state. Named link, resize, drag, wait, hidden, and application-requested shapes
remain native. The callback is owned by the safe Rust surface wrapper's normal
disconnect lifecycle and is reinstalled when a live surface transfers between
windows, preventing duplicate or stale handlers.

## Focused evidence

- Pure tests check exact transparent, outline, and core pixels and reject
  substitution for pointer, hidden, resize, and unnamed cursors.
- The existing private-X11 terminal-input journey now requires a live staged
  Ghostty surface to log semantic `text` projection to `outlined-ibeam` before
  continuing its real PTY and physical-pointer interactions.
- Strict Clippy passes for the touched crates with only the two documented
  pre-existing crate lints allowed (`too_many_lines` and `unused_self`).

Visual suitability remains operator QA. The tested ReleaseSafe artifact is
installed atomically while the old process remains running; the new cursor is
visible only after an explicit relaunch.

## Live dogfood result

The focused-test-passing ReleaseSafe executable was atomically installed at
`/usr/lib/zentty/bin/zentty-linux`; its SHA-256 matched the staged artifact
(`2092953d61385598be71ba4a422f5c4c8dfdae58b82468d71070611643c98c36`).
The operator then explicitly quit and relaunched Zentty from GNOME and reported
the terminal pointer was “Much better.” No automatic restart or synthetic
desktop substitution was used for this visual acceptance check.
