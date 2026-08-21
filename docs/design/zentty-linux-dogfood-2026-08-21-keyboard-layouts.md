# Linux runtime keyboard-layout dogfood record — 2026-08-21

Issue: GH-71, child of GH-8. Plan:
`docs/design/linux-runtime-keymap-qualification-plan.md`.

## Scope and authority

This slice qualifies live US-to-German layout changes and one deterministic
physical-key remap on native X11 and Wayland. It reuses the existing nested
Xvfb/Cage environments, staged ReleaseSafe product, product-input helpers, and
real PTY actor. It does not create a second terminal harness or alter Ghostty.

The invariant is one physical position throughout: X11 keycode 29 and Wayland
evdev keycode 21 both identify `<AD06>`. Expected output is `y` under the US
map, `z` under the German map, and `ü` under the explicit remap. A later event
must reach only the newly focused second pane.

## Discoveries and rejected approaches

- `wtype -k` resolves a requested keysym back to whichever keycode produces
  that symbol. It is useful for ordinary product interaction but cannot prove
  that one physical position changed meaning. Reusing it here would have been
  text/symbol intent disguised as physical qualification.
- Host `ydotool` injects through `/dev/uinput` into the developer desktop; it
  neither owns nor targets the nested compositor and was rejected.
- The official `wayland-client` and `wayland-protocols-misc` crates expose the
  compositor's virtual-keyboard protocol and raw key request. Pinned versions
  0.31.13 and 0.3.11 are opt-in dependencies of `zentty-test-support`; ordinary
  Zentty builds neither compile nor ship the driver.
- The first dependency selection paired `wayland-client` 0.31.11 with
  `wayland-protocols-misc` 0.3.11, whose declared minimum is 0.31.13. Cargo
  rejected the graph before compilation. The direct dependency was corrected
  to the compatible exact version rather than loosening both constraints.
- The first X11 evidence matcher assumed an XKB symbol declaration occupied
  one line. `xkbcomp` emits `<AD06>` and `symbols[Group1]` on separate lines.
  The matcher now scopes the expected symbol to the complete `<AD06>` block.
- `xdotool key keycode 29` warned that `keycode` was an unknown symbol even
  though the following numeric token happened to work. The retained command is
  the unambiguous numeric form `xdotool key 29`.
- `cargo test --bin` creates a hashed test executable, not the stable
  `target/debug/wayland-keycode-driver` used by the real journey. Qualification
  now owns both the unit-test and explicit binary-build steps.
- The first multi-pane Wayland attempt reused the first German map receipt.
  The driver's deliberate absent-path policy rejected it. Each protocol
  transaction now has a unique evidence path; repeated maps retain independent
  receipts instead of overwriting evidence.

## Test-first contracts

The Rust driver unit tests prove the three reviewed maps retain `<AD06>` and
`<RTRN>`, fix the transmitted physical keycode at 21, and reject symbol-oriented
or unreviewed-keycode arguments. The shell harness contract rejects unsupported
backends, ambient X11/Wayland sessions, missing numeric-keycode use, and missing
exact PTY expectations.

The Wayland XKB file includes its required terminating NUL in the advertised
protocol size. The driver flushes and round-trips after the raw press/release
events, and its public interface has no text-input operation.

## Focused real-system results

Native X11 passed in private Xvfb session
`3572c671a06aaaadef8a41289275aed2d14bf71f8f1c69bde7cfdff43151bdff`.
The server-side `xkbcomp` receipts recorded distinct map hashes and the exact
real PTY result was:

```text
pane-1:y:79
pane-1:z:7a
pane-1:ü:c3bc
pane-2:z:7a
```

Wayland passed in private Cage session
`26d3e0105e67adc4192be2588b4d262ea6419293ccbcdbac1b2a39fa4fa25b38`
over private Xvfb transport session
`2b331a7524540816b7664b3026951fe25f39d32979390f0be37977a553488830`.
The four raw-key transactions recorded these reviewed map receipt hashes:

```text
us      11ba7d22aa5047d5a088649f9dd1deea93258feee282c3bdab0ea1e86925c095
de      7907a0ea18a674c682587222129206adc93a1bd77a46f263cdeef2ecef8d6d5c
remap   1eaee604ce8805f0beb43f1fc6dc677590796d98c3ba0a725f7954efe1738537
de      7907a0ea18a674c682587222129206adc93a1bd77a46f263cdeef2ecef8d6d5c
```

The same exact four-line PTY receipt passed. Zentty remained one running process
through all map transitions, created a second real Ghostty pane, transferred
focus, and exited cleanly. No product or Ghostty defect was found.

The authoritative matrix now has explicit `keyboard-layouts-x11`,
`keyboard-layouts-wayland`, and opt-in driver-build cells. Static matrix/schema,
harness, feature-inventory, ShellCheck, formatting, and Rust unit tests pass.
This is focused GH-71 evidence, not a claim that the entire Linux matrix or
GH-8 epic passes.

## GH-72 driver-extension regression

GH-72 extended the same opt-in raw-key driver with explicit modifier, hold,
and Enter actions. That changed the reviewed XKB payload and receipt shape, so
the Wayland layout journey was rerun rather than treating the earlier hashes as
current. It passed in private Cage session
`90ec7d67501f05b04d5cb6df490e355fef330227823ed5d4227b90266c475d84`
over private Xvfb transport session
`3529edfbac25f4b53fdabc80bd2568cca4c27df873372fac472ad133b5c11203`.
The current receipt hashes are:

```text
us      525523d3481a8a32297937df912d9f13736c05677ffa1c9dd37d76f6609054f7
de      6b30065dea0dfcd6278c528dfc02e56774c244a35dd822f8c5fd0424f3abdcc3
remap   6d330a350b4452c016b3cfca333972703abc157fca1d48900e0e64e4003a3b17
de      6b30065dea0dfcd6278c528dfc02e56774c244a35dd822f8c5fd0424f3abdcc3
```

The exact four-line PTY result remained unchanged.

## AI disclosure

Investigation and implementation assistance were provided by OpenAI Codex
under Jason Maskell's direction.
