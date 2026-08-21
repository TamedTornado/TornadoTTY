# Linux runtime keymap qualification plan

Issue: GH-71, child of GH-8.

## Product outcome

Prove that Zentty accepts physical-key-position input after a live keyboard
layout change and after an explicit remap on native X11 and Wayland. The product
must remain running throughout each transition. Success is exact UTF-8 read by
the real terminal PTY, not successful event injection.

## Controlled systems

- X11 uses the existing private Xvfb environment. `setxkbmap` changes the
  server keymap after Zentty connects, `xkbcomp` records the resulting server
  map, and XTEST sends the same numeric keycode for every stage.
- Wayland uses the existing private Cage environment and its
  `zwp_virtual_keyboard_manager_v1` protocol. A focused Rust test driver sends
  reviewed XKB maps and the same Linux evdev keycode. Each new virtual-keyboard
  object changes its map while the same Zentty process and terminal remain
  alive.
- The Rust driver is test infrastructure, not a Zentty or Ghostty API. It lives
  behind an opt-in feature of `zentty-test-support`, so ordinary product builds
  do not compile or ship its Wayland dependencies.

## Test-first order

1. Add pure tests for the driver's built-in maps: the same physical key maps to
   `y` under US, `z` under German, and `ü` under the explicit test remap; map
   identities and the transmitted evdev keycode are stable.
2. Add harness contract tests that reject an unsupported backend, text/symbol
   injection in place of a numeric keycode, missing controlled-session
   identity, missing server/map evidence, and inexact PTY output.
3. Implement the Rust Wayland virtual-keyboard driver and its opt-in build.
4. Implement one canonical product journey shared by X11 and Wayland.
5. Add explicit X11 and Wayland matrix cells only after their exact commands
   pass in the controlled environments.

## Canonical journey

The fixture starts two real pane children with separate receipts. With pane 1
focused, the same physical key position must produce `y` under the initial US
map and `z` after the runtime German change. The explicit remap must then
produce `ü`. Focus moves to pane 2 without restarting Zentty; a final physical
key must appear only in pane 2, while pane 1's receipt remains exact. Both
children exit normally and the product completes its real lifecycle.

## Acceptance and non-goals

- Both backends preserve exact server/compositor map evidence and PTY bytes.
- The Wayland driver sends protocol keycodes; it has no text-input operation.
- Environment variables alone are never map evidence.
- No Ghostty change is planned. A Ghostty patch is justified only by a
  reproduced product failure owned by its GTK input path.
- Hardware device discovery and desktop-wide user configuration are outside
  this deterministic qualification cell and are not claimed.
- Discoveries, failed approaches, repairs, commands, and limitations go in a
  new dogfood record rather than extending the fcitx report further.
