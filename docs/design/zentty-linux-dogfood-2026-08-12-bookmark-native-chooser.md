# Zentty Linux dogfood — native bookmark chooser closeout

Date: 2026-08-12
Issue: GH-18 (`workspace.bookmarks-presets` closeout)

## Contract

This record follows the closeout section in
[`linux-bookmarks-presets-feature-plan.md`](linux-bookmarks-presets-feature-plan.md).
The product keeps one bookmark store, one import/export envelope, and one GTK
action owner. Only the Linux native chooser coordination boundary may change.

## Starting evidence

- Controlled X11 maps the real portal chooser but synthetic keyboard activation
  cannot activate Save; the absence of an exported file exits 1.
- Controlled Wayland maps the portal but cannot associate it reliably with the
  Zentty parent surface; the GNOME portal backend requires a GNOME session that
  the isolated compositor deliberately does not assume.
- The final Git/review qualification rerun exposed a related cleanup fact:
  `xdg-document-portal` can retain a private FUSE mount after the D-Bus process
  group exits. The nested-X11 wrapper now deterministically unmounts and proves
  removal, but that repair does not make portal keyboard routing a product pass.
- Source uses the macOS native save/open panel for a Zentty-owned portable file.
  An in-process transient GTK chooser is the direct Linux platform analogue and
  avoids making desktop-portal implementation behavior part of the product's
  own file-format contract.
