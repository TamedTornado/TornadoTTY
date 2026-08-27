# Dogfood record: embedded terminal context menu

Date: 2026-08-27

## Discovery

Dogfooding exposed a native terminal context menu whose **Copy** item was
disabled while text was visibly selected. The screenshot showed a real Ghostty
selection, so this was not a selection-rendering problem.

Ghostty's GTK surface owns the menu model, but that model addresses actions in
the containing window as `win.copy`, `win.paste`, `win.clear`, `win.reset`,
`win.new-window`, and `win.close`. Standalone Ghostty installs those actions on
its own window. Zentty embeds the surface in a Zentty-owned window and had only
installed its `workspace` action group. GTK therefore correctly rendered the
unresolved Ghostty menu actions as unavailable.

## Repair

- The Zentty window now supplies the small native `win` action group required
  by the embedded Ghostty menu.
- `win.copy` enters Zentty's existing default-copy pipeline; it does not create
  a second clipboard implementation. Consequently the user's automatic clean
  copy preference remains authoritative.
- Immediately before Ghostty opens the menu, the host reads the live Ghostty
  selection and synchronizes the Copy action's enabled state. This uses the
  surface's native `menu` signal and contains no timer or guessed delay.
- Paste, clear, and reset route to the focused Ghostty surface's existing
  binding actions. New Window and Close route to Zentty's existing window
  lifecycle actions.
- Uninstalling a window's action router removes both action groups, preserving
  the existing lifecycle boundary.

The Ghostty-side change is limited to exposing the already-owned `menu` signal
through the Rust embedding wrapper. Menu policy and orchestration remain in
Zentty.

## Regression coverage

The existing real-product `rust-pane-search` journey now has a focused context
menu mode. Inside the controlled private X11 environment it:

1. launches the staged ReleaseSafe Zentty binary;
2. creates output through a real PTY and Ghostty surface;
3. creates a real Ghostty selection through the product's Select All action;
4. opens Ghostty's real GTK menu with a physical right click;
5. activates the visible Copy row with a physical pointer click; and
6. reads the compositor clipboard from an independent client and compares the
   expected cleaned bytes.

Receipt:

```text
Rust terminal context menu passed: x11, real-pty=true, physical-pointer=true
```

The isolated command completed in 3.6 seconds. An initial unelevated attempt
could not create `/tmp/.X11-unix` under the sandbox and was correctly treated as
an environmental failure, not a pass. A first keyboard-driven attempt also
failed because GTK does not transfer focus into a pointer-opened popover; the
test was repaired to exercise the actual pointer interaction rather than
claiming success from the action's enabled state alone.

The broader clipboard journey subsequently exposed an unrelated stale command
palette assertion that still expects five matches for `Copy` although the
current catalog visibly returns seven legitimate matches. That assertion was
not weakened or bundled into this product repair.

## URL interaction

Ghostty's Linux default is Ctrl+click for URL activation; plain click remains
available for terminal focus and selection. This record does not claim that URL
opening has been newly qualified in Zentty. It remains a separate interaction
to validate against the embedded client and desktop opener if dogfooding shows
that Ctrl+click does not work.

