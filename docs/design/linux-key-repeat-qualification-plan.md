# Linux key-repeat and modifier-order qualification plan

Issue: GH-72, child of GH-8.

## Outcome

Qualify compositor/server-generated key repeat, ordered modifier transitions,
and pane focus transfer through the real Zentty terminal path on X11 and
Wayland. The test must detect stuck modifiers, duplicate trailing input, and
delivery to the wrong pane.

## Test-first order

1. Extend the existing Rust Wayland raw-keycode driver with pure tests for its
   reviewed actions: tap, shifted tap, held key, and Enter-only termination.
   Reject unbounded hold times and arguments that express text or symbols.
2. Add shell harness contract tests for controlled-session ownership, exact
   pane receipts, bounded repeat counts, modifier release, and focus barriers.
3. Implement one canonical two-pane journey shared by X11 and Wayland.
4. Run the two real controlled environments before adding PASS matrix cells.
5. Run affected static, Rust, and matrix tests; document failures and repairs;
   commit only the complete feature.

## Real-system design

- X11 uses private Xvfb, pins server repeat to a 200 ms delay and 20 Hz with
  `xset`, captures the server-reported values, and sends numeric XTEST keycode
  29 plus real Shift press/release events.
- Wayland uses private Cage and the existing opt-in Rust
  `zwp_virtual_keyboard_v1` driver. The driver captures `wl_keyboard.repeat_info`
  from the owned compositor, sends evdev keycode 21, and keeps the key down
  across the externally driven focus transfer.
- Neither backend synthesizes the repeated text. The X server or compositor/
  client repeat path must generate it from one bounded key-down interval.

## Canonical journey

1. Pane 1 receives uppercase `Y` from modifier-down, key-down/up,
   modifier-up, then receives lowercase `y` from an independent physical tap.
   This proves modifier ordering and release.
2. A second real pane is created and pane 1 is restored through the public CLI.
3. The physical Y position is held. Pane 1 must receive an initial bounded
   prefix. While the key remains down, focus transfers through the public CLI
   to pane 2, which must receive a bounded suffix.
4. Key-up stops repeat. Pane 2 receives the raw Enter that closes its suffix;
   pane 1 is refocused and receives an Enter-only raw event to close its prefix.
5. Both PTY receipts must contain only the expected bytes and remain unchanged
   during a post-release settling interval.

## Boundaries

- No Ghostty change is planned unless the real journey establishes a defect in
  Ghostty's GTK key path.
- Desktop keyboard preferences and hardware enumeration are not claimed.
- Timing bounds are derived from captured controlled repeat configuration, not
  from the developer desktop.
- Findings and receipts belong in a new dogfood record.
