# Native input closeout dogfood — 2026-08-20

## Scope frozen before implementation

This GH8 pass starts with the stale `physical-key-wayland` blocker only. It
will reuse the controlled nested Wayland compositor, the existing
`product-input` actor, the staged ReleaseSafe product, Ghostty's GTK event
controller, and a real PTY acknowledgement. It will not use text injection,
an in-product test hook, or the ambient developer desktop.

The existing general Wayland lifecycle cells must remain usable in their
non-input compositor. Therefore physical input is an explicit mode of the
canonical `rust-product-smoke` journey, not an unconditional new prerequisite
for every invocation.

## Baseline discovery

The matrix still describes Wayland physical keys as blocked on a virtual
keyboard harness. That prerequisite is stale: `nested-wayland-input` now pins
Cage with `zwp_virtual_keyboard_manager_v1`, verifies the protocol with
`wayland-info`, creates a private Wayland session, and uses `wtype` to submit
compositor-visible events. Numerous later product journeys already use that
path, but `rust-product-smoke` still makes its Wayland child emit a title
without accepting input. The X11 branch, by contrast, requires an exact line
from XTEST before emitting its semantic title receipt.

The repair is to give the requested Wayland physical-input mode the same
semantic contract as X11: compositor key events must pass through GDK, GTK,
Ghostty, and the real PTY child before the title acknowledgement can exist.

## Work log

### Repair

- Added an explicit `ZENTTY_RUST_PHYSICAL_INPUT=true` mode to the canonical
  `rust-product-smoke` actor. In that mode the Wayland PTY child blocks on the
  exact line `zentty-rust-input`; only after the line arrives does it emit the
  semantic `zentty-rust-smoke-wayland` terminal title and exit successfully.
- Input uses the existing `product-input` authority. Cage advertises and the
  wrapper verifies `zwp_virtual_keyboard_manager_v1`; `wtype` sends key events
  through the private compositor; GTK and Ghostty translate them; the PTY child
  supplies the only success acknowledgement.
- The mode is opt-in. Existing headless Weston lifecycle cells retain their
  non-input behavior and do not acquire a virtual-keyboard prerequisite.
- The matrix `physical-key-wayland` cell is now executable `PASS` in
  `nested-wayland-input-v1`, replacing its stale blocker text.

### Passing receipts

- Wayland physical input: private Cage session
  `c8187f50ecb8b4c5b65df433969e5b421e4a686ea9a83ffbab5f3e2554b52e23`
  passed compositor → GDK/GTK → Ghostty → real PTY acknowledgement.
- X11 regression: private Xvfb session
  `baacc05c5b989644665a9e8fe8449580ae53f3561ed4ed20fa33dee159f5f195`
  retained XTEST input, exact PTY acknowledgement, and external resize.
- Ordinary non-input Wayland lifecycle: private headless Weston session
  `092ad70962914e6befb6a538fbe8d5a947d1d1845a32327098d9d85e8071c134`
  passed, proving the new prerequisite is bounded to the named cell.
- Matrix and orchestration validators passed. The declarations are now
  172 PASS / 0 FAIL / 2 BLOCKED / 3 XFAIL / 6 NOT_IMPLEMENTED.

### Remaining limitations

This does not prove hardware key scanning and does not claim that virtual
keyboard events are physical devices. It does prove the required Linux product
boundary without bypassing the compositor or calling a text API inside Zentty.
Dead keys, layouts/remaps, repeats/modifiers, IME composition, and compositor
scaling remain GH8 work. In particular, the existing controlled IBus focus
reproducer remains suppression-governance evidence, not IME correctness.
