# Zentty Linux dogfood — terminal Clean Copy context menu

Date: 2026-08-28
Tracking: GH-134
Ghostty: `eb1eb6281466ee7e85f629012ac77de7d9cac712`

## Discovery

Daily use exposed that selecting text and right-clicking inside the real
Ghostty surface showed only `Copy`. Linux had already implemented and qualified
`Clean Copy`, `Copy Raw`, automatic-clean policy, and `Copy as Markdown`, but
only exposed those actions through the pane three-dot menu, shortcuts, and the
command palette. That made the named feature effectively undiscoverable at the
selection itself.

The checked-in macOS source is explicit. `PaneContainerView.makeContextMenu`
places `Copy` beside `Clean Copy` when automatic cleaning is disabled. When it
is enabled, ordinary `Copy` already cleans, so the companion becomes `Copy
Raw`. `Copy as Markdown` remains a separately configurable command and is not
part of the ordinary terminal context menu.

## Design

Ghostty still owns the right-click gesture, terminal mouse-reporting decision,
word selection, popover anchor, and popup lifecycle. Its new generic GTK
embedding operation accepts a host-provided `GMenuModel`; it contains no Zentty
labels, actions, settings, or policy. The independently reviewable Ghostty diff
is one surface method, one C ABI operation, its ELF allowlist entry, and its
public declaration.

Zentty owns a focused `terminal_context_menu` module. It reproduces Ghostty's
existing menu sections while inserting exactly one source-derived companion
after `Copy`. Immediately before Ghostty opens the menu, the existing callback
refreshes that mutable model from the live `ClipboardConfig` and synchronizes
selection availability across Copy, Clean Copy, Copy Raw, and Copy as Markdown.
No second popover, clipboard implementation, transformer, or action router was
created.

## Failures and repairs

- The first shared-library build compiled the new Zig export but ELF inspection
  showed it remained local. Ghostty's explicit version script was correctly
  doing its job; the new operation was added to the reviewed
  `GHOSTTY_GTK_EMBED_1.0` allowlist and the rebuilt `.so` exposed exactly the
  expected symbol.
- The first `abi-surface` ledger edit omitted a shell continuation after the
  preceding symbol. The test rejected the edit by trying to execute the new
  symbol as a command. The allowlist was repaired and ShellCheck plus the
  actual ABI comparison passed.
- The first controlled X11 API run could not create its private server because
  `/tmp/.X11-unix` had nonstandard ownership. Restoring the standard
  `root:root`, mode `1777` host directory allowed the isolated Xvfb journey to
  run. Environmental absence or corruption was not converted into a pass.

## Focused evidence

- Ghostty `gtk-embed-lib-test`: PASS.
- ReleaseSafe Ghostty shared library: PASS; the new symbol is globally exported
  under `GHOSTTY_GTK_EMBED_1.0`.
- `zentty-ghostty-sys`: 4 PASS.
- `zentty-ghostty`: 5 unit PASS and 3 compile-fail documentation PASS.
- `zentty-linux terminal_context_menu`: 2 PASS, checking the actual first GTK
  menu-section labels for both automatic-clean states.
- Ghostty API audit and negative self-tests: PASS, 19 reviewed files and 14
  product-used exports.
- ABI surface and ABI version-node/self-test: PASS, exactly 14 exports.
- Private-X11 C API contract: PASS with a real GTK surface and `GMenuModel`.
- Private-X11 `rust-pane-search` context-only journey: PASS with a real PTY,
  real Ghostty selection, native popover, physical pointer activation of both
  `Copy` and the automatic-clean `Copy Raw` row, and external clipboard byte
  assertions.

No full qualification run was used for this focused dogfood defect. Live GNOME
visual acceptance remains required after installing the tested artifact.

## Live dogfood result

The additive Ghostty library was installed atomically before the Zentty
executable, so both the old and new executable remained launch-compatible
throughout deployment. Installed files matched their focused-tested staged
artifacts:

- `libghostty-gtk-embed.so`:
  `c6b233c8364fb154714f8b96d0f3db3d8f4e50a7087d5e7230838d48452a314d`
- `zentty-linux`:
  `d8a46c8288bd55db8ce182f43cc6abee8a6bc0f56925225a3d7353edd46541b4`

The operator explicitly quit and relaunched Zentty, exercised Clean Copy from
the native terminal selection menu, confirmed its intended transformation, and
reported “Works nicely then.” No automatic application restart or substitute
test UI was used for this acceptance.
