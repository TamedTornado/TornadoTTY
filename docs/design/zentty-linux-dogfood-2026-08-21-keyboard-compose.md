# Zentty Linux dogfood: dead-key and Compose input

Date: 2026-08-21  
Issue: GH-66, under epic GH-8

## Scope correction

GH-66 originally combined dead keys, Compose, layouts, remaps, repeat,
modifiers, focus transfer, X11, and Wayland. That was another epic-sized child.
It is now the cohesive dead-key/Compose feature; GH-71 owns layouts/remaps and
GH-72 owns repeat/modifier/focus behavior.

## Discovery record

- Controlled Wayland/Cage plus `wtype` produced exact `é` and `æ` PTY lines
  through the advertised virtual-keyboard protocol.
- Holding a Wayland `wtype -P` event did not produce repeat in this environment,
  and `wtype -M shift -k a` produced lowercase `a`. Those results are not
  discarded or worked around with text injection; they motivated the explicit
  GH-72 feature and its required physical repeat/modifier harness.
- The first X11 experiment loaded `setxkbmap` before any persistent X client.
  Xvfb reset when the keymap client disconnected, silently returning to `us`.
  Loading the map after Zentty owns its X connection and verifying
  `us(intl)+compose(ralt)` fixed the harness design.
- Symbolic `Multi_key` injection against the reset/default map produced `ae`,
  correctly failing to prove Compose. With the live verified map, the same real
  physical event path produced `æ`.

## Final receipts

The maintained actor now launches one staged ReleaseSafe product and accepts
only two exact lines in its real Ghostty PTY. It checks both Unicode values and
their bytes (`c3a9`, `c3a6`), rejects extra receipt lines, and requires clean
child and application lifecycle completion. The X11 journey captures the live
compiled keymap; the Wayland environment receipt independently proves Cage's
virtual-keyboard protocol.

Focused X11 command:

```text
ZENTTY_LINUX_BINARY=build/linux-profiles/release-safe/bin/zentty-linux \
  GDK_BACKEND=x11 linux/tests/nested-x11 linux/tests/rust-keyboard-compose
```

Result:

```text
rust-keyboard-compose-x11: PASS dead-key=é compose=æ utf8=c3a9,c3a6 real-physical-input real-pty
```

Focused Wayland command:

```text
ZENTTY_LINUX_BINARY=build/linux-profiles/release-safe/bin/zentty-linux \
  GDK_BACKEND=wayland linux/tests/nested-wayland-input linux/tests/rust-keyboard-compose
```

Result:

```text
rust-keyboard-compose-wayland: PASS dead-key=é compose=æ utf8=c3a9,c3a6 real-physical-input real-pty
```

Also passed:

- `bash -n linux/tests/rust-keyboard-compose`
- `shellcheck linux/tests/rust-keyboard-compose`
- `linux/tests/qualification-matrix --validate-only linux/qualification-matrix.json`
- `linux/tests/qualification-matrix-test`
- `git diff --check`

The two formerly prose-only cells are now explicit PASS entries. Matrix totals
are 177 PASS, 0 FAIL, 1 BLOCKED, 3 XFAIL, and 5 NOT_IMPLEMENTED. Full Linux
qualification remains unclaimed.
