# GH-183: opener stderr busy loop

Issue: [#183](https://github.com/TamedTornado/TornadoTTY/issues/183).

## Incident and diagnosis

Jason reported overnight sluggishness followed by the GUI disappearing on
September 8. The old GUI's final log timestamp was 06:49:09 Europe/Madrid;
the replacement started at 06:49:29. The last 10,000 sampled records were
identical empty `warning(os-open): open stderr=` warnings; the final-second
query returned 15,000 warnings. Earlier samples contained the same symptom.

This matches a confirmed stock Ghostty defect introduced in `bb375a2f7565`:
`takeDelimiterExclusive` stops before a newline, and the opener loop never
consumes it. The loop then logs empty strings indefinitely, never reaching its
normal child wait. The bug exists in our shipped pin `bab8c088f45e`.

No accessible kernel OOM/crash record or new `/var/crash` dump established the
final termination mechanism. This is **not proof of a memory leak or an OOM
kill**. Current post-restart memory usage does not reconstruct overnight RSS.
Initial broad journal searches were stopped; follow-up samples were bounded
by PID, record count, and execution deadline because the flood makes broad
searches expensive. No log deletion, host change, or live restart was performed.

## Repair and focused regression

Ghostty commit: `02821fa5813557c2dd9957a0378faf4fb9266e2f`, pushed to
`TamedTornado/ghostty`, branch `zentty/gh183-opener-stderr`. Its commit message
links #183; TornadoTTY's pin points to that exact repair.

The production opener now uses a small bounded stderr-record helper that calls
the standard `takeDelimiter`, consuming the delimiter and returning null at
EOF. The 256-byte streaming buffer and read-failure stop behavior are retained.
Long lines are emitted in bounded chunks; trailing unterminated text is kept.
Blank lines remain legitimate single records, not an excuse to hide a spin.

Before the semantic fix, the extracted original logic failed two of three
regressions. After the fix all three passed:

- Normal and blank newline-delimited records progress to EOF.
- Empty and final unterminated streams terminate correctly.
- Three real shell children each write blank lines, a 600-byte line, and an
  unterminated tail through a real 256-byte buffered pipe. Exact content,
  bounded chunks, finite consumption, and successful child reaping are checked.
  Iteration assertions make the buggy implementation fail rather than hang.

Commands in the Ghostty checkout:

```sh
../.tools/zig-x86_64-linux-0.16.0/zig test src/os/open_stderr.zig -O ReleaseSafe
# Normal Ghostty graph, filtered to this regression (standard project cache/env):
zig build test -Dtest-filter='opener stderr' -Doptimize=Debug \
  -Dcpu=baseline -fno-sys=gtk4-layer-shell -j16 --summary all
```

The filtered graph passed 77/77 tests including its supporting unnamed tests,
not the full Ghostty suite. An initial integration compile rejected `catch
break` in the while condition; corrected to `catch null`, preserving the prior
stop-on-read-failure behavior, then rerun successfully.

## Real product check

The existing `rust-product-smoke` journey has an `opener` scenario, not a new
harness. A private `xdg-open` actor checks an actual Ghostty screen export,
writes the problematic stderr shape, and records its PID. Physical F8 input
invokes `write_screen_file:open`, which enters the same `Surface.openUrl`
fallback as URL clicks without font-dependent click coordinates. Three opens
must finish their stderr, reap their children, retain exactly one blank record
each, and leave PTY input/title/exit working. A log-size bound aborts a regression
before it can flood the host. No real browser is launched.

Results: staged build PASS; private X11 opener journey PASS; private Wayland
opener journey PASS. Each backend invoked and reaped three actual opener
children, retained three legitimate blank stderr records, read every final
unterminated marker, then passed the existing terminal input/title/exit checks.
The X11 product log had 121 total lines, rather than an unbounded warning flood.
`bash -n` and `shellcheck -x -S warning` passed for the changed journey.

```sh
ZENTTY_BUILD_OUTPUT_DIR=build/gh183 linux/scripts/build-local
ZENTTY_LINUX_BINARY="$PWD/build/gh183/bin/zentty-linux" \
  ZENTTY_RUST_SCENARIO=opener \
  linux/tests/nested-x11 linux/tests/rust-product-smoke
ZENTTY_LINUX_BINARY="$PWD/build/gh183/bin/zentty-linux" \
  ZENTTY_RUST_SCENARIO=opener ZENTTY_RUST_PHYSICAL_INPUT=true \
  linux/tests/nested-wayland-input linux/tests/rust-product-smoke
```

No installation or live restart requested or performed; no whole-application
qualification run. These checks fix and exercise the confirmed opener defect,
not an assertion that the earlier GUI disappearance's exact cause is proven.
