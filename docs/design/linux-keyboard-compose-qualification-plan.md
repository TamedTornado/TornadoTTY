# Linux Dead-Key and Compose Qualification Plan

Issue: GH-66 (child of GH-8)

## Outcome

Prove that X11 and Wayland physical key sequences perform native dead-key and
Compose processing and deliver exact non-ASCII UTF-8 to a real Ghostty PTY.
This feature is separate from runtime layout/remap work (GH-71) and
repeat/modifier/focus work (GH-72).

## Design

1. Reuse `nested-x11-v1`, `nested-wayland-input-v1`, and the established
   `product-input` focus barrier. Add no display wrapper or alternate actor.
2. Launch the staged ReleaseSafe product with one real Ghostty pane and a child
   that reads exactly two terminal lines.
3. On X11, load `us(intl)+compose(ralt)` only after the product keeps Xvfb
   alive, then inspect the compiled map before injecting events. This prevents
   X server reset from silently restoring the default map.
4. Send dead-acute/e/Return and Compose/a/e/Return through XTEST on X11 and the
   compositor virtual-keyboard protocol on Wayland.
5. Require exact NFC `é` and `æ` values and their exact UTF-8 byte sequences in
   the PTY receipt. Event-tool success or visible rendering alone cannot pass.

## Failure requirements

- Missing keymap, XTEST, virtual-keyboard, locale, staged product, or exact PTY
  evidence fails; none becomes a skip/pass.
- Extra text, decomposed output, duplicate commits, or wrong line count fails.
- The product must complete its real child and surface lifecycle cleanly.
- Matrix cells remain distinct so one compositor cannot cover the other.

## Verification order

1. Shell syntax and ShellCheck.
2. Focused X11 journey.
3. Focused Wayland journey.
4. Matrix schema and runner regressions.
5. Diff review; no unrelated app-wide qualification run.

