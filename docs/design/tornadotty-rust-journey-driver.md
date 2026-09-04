# TornadoTTY Rust journey driver

Issue [GH-149](https://github.com/TamedTornado/TornadoTTY/issues/149) owns
this migration. It addresses one specific weakness in the Linux test pyramid:
large Bash files had simultaneously owned product processes, compositor input,
timeouts, typed product evidence, assertions, cleanup, and reporting.

## One authority

`tornadotty-journey-driver` is the only journey executable. Its modules divide
responsibilities without creating independently runnable harnesses:

- `session` owns a product process group, exclusive resource leases, deadlines,
  stop escalation, descendant cleanup, bounded state, and an append-only
  machine journal;
- `input` rejects stale product identities and foreign X11 windows before
  sending real `xdotool` or `wtype` input; and
- `scenario` owns named end-to-end workflows as they migrate from Bash.

The existing nested-X11 and nested-Wayland wrappers remain authoritative for
the compositor and isolated desktop namespace. A scenario's Bash entry point
may check prerequisites, establish that namespace, resolve the staged bundle,
and invoke the Rust driver. It may not own product lifetime, parse receipts,
drive input, or reinterpret a failed run.

## Evidence and failure behavior

Every supervised run creates an owner-only session directory containing:

- `state.json`, atomically replaced at phase changes and bound to PID plus
  Linux process start ticks;
- `journey.ndjson`, a bounded, sequenced, typed lifecycle/failure journal; and
- `product.log`, the product's human diagnostics, retained for investigation
  rather than treated as the primary assertion API.

Normal teardown validates the journal through the same Rust definition that
writes it. Partial records, unknown fields or events, wrong versions, sequence
gaps, duplicate lifecycle events, and invalid lifecycle order fail closed.
Stop requests are written by the invoking process, but only the long-lived
supervisor signals the process group. Unexpected product failure and leaked
children remain failures even when cleanup succeeds.

## Real boundaries

The driver does not mock Ghostty, GTK, the PTY, compositor input, X11/Wayland
protocol state, D-Bus, clipboard, or the filesystem. `xdotool`, `xprop`, and
`wtype` are real system actors. Rust verifies that input targets the live
supervised product (or the attested nested compositor) before invoking them.

## Migration state

`desktop-window-identity` is the first fully migrated scenario: its Bash file
only validates its controlled backend and invokes `scenario window-identity`.
The Rust scenario checks packaged `StartupWMClass`, launches the staged product
and real PTY child, verifies the live X11 `WM_CLASS` or Wayland `set_app_id`,
causes the PTY child to exit, and validates both evidence streams. It passes in
private Xvfb and private headless Weston.

The divider/layout, session-restore, and notification/settings journeys have
already adopted Rust process ownership, typed product receipts, verified input,
deadlines, resource leases, and cleanup, but their remaining scenario logic is
still Bash. They remain `PARTIAL`; GH-149 must not close until those named
representative workflows are Rust-owned and their old assertions are accounted
for rather than silently deleted.

## Focused verification

Driver self-tests cover clean exit, deadline expiry, stale PID identity,
foreign-window rejection, malformed/partial/out-of-order evidence, cleanup
after requested stop, cleanup and failure after an unexpected kill, child
leakage, and conflicting resource claims. Build the driver outside the nested
desktop; only the staged executable and product run inside it.

This driver is focused integration infrastructure, not another qualification
aggregate. It runs a requested scenario exactly once and makes no release or
full-Linux qualification claim.
