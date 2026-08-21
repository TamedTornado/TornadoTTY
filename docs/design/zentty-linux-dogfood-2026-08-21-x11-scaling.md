# Zentty Linux dogfood: X11 scaling qualification

Date: 2026-08-21  
Issue: GH-69, under epic GH-8. Xwayland follow-up: GH-70.

## Initial state

The authoritative `scale-x11` cell was BLOCKED on a controlled X11/Xwayland
server with deterministic DPI and externally observable geometry. Existing
external-resize coverage proved allocation changes but did not prove desktop
scale behavior. Several unrelated journeys set `GDK_SCALE=1` for stability;
that is not evidence for X11 DPI qualification.

## Discovery record

- A direct `xrandr --dpi` request did not change Xvfb's `xdpyinfo` resolution or
  Ghostty cell dimensions. Treating that command as proof would have been a
  false positive.
- An `Xft.dpi` resource installed with `xrdb` also did not affect GTK 4/Ghostty
  in the controlled session. GTK 4 consumes the desktop XSETTINGS channel here.
- A private `xsettingsd` publishing `Xft/DPI 98304` (96 DPI) yielded Ghostty's
  `CSI 16 t` cell report of 21x10 pixels. Publishing `196608` (192 DPI) yielded
  42x19 pixels in the same Xvfb geometry. This is the real standard desktop
  channel and not a process-local `GDK_SCALE` override.
- The sandbox-visible `/tmp/.X11-unix` ownership maps host root to `nobody`, so
  direct sandbox Xvfb startup fails its security ownership check. Controlled GUI
  journeys must run with the already-required elevated host execution; changing
  the wrapper to weaken X socket checks would be incorrect.
- The first two-pane implementation accidentally emitted `CSI 16 t` from a
  process-substitution subshell into its capture pipe rather than the terminal.
  Both actors started and then timed out without receipts. The repair writes the
  query to `/dev/tty` and reads the terminal response from that same real PTY.
- Exact reply diagnostics then showed both panes returning `CSI 6;21;10 t`.
  The remaining actor failure was its own process-substitution parser: the
  helper omitted a newline, so Bash populated both dimensions but returned EOF
  failure to `set -e`. The helper now emits a complete line.
- Review caught an overclaim in the original child decomposition: Xvfb proves
  native X11, not Xwayland hosted by a Wayland compositor. GH-69 was narrowed
  to native X11, GH-70 now owns Xwayland, and the new `scale-xwayland` matrix
  cell remains NOT_IMPLEMENTED with that exact tracked defect.
- The first `linux/ci/preflight-test` attempt reached its clean-checkout
  negative fixture while this implementation was intentionally uncommitted;
  preflight correctly rejected the dirty checkout, so that test must be rerun
  after the reviewed commit rather than weakened or misreported as a product
  failure.

## Implementation and final receipts

The focused actor reuses `nested-x11-v1`, restores two real Ghostty panes, pins
the real X window at 1200x800, and compares 96-DPI and 192-DPI XSETTINGS runs.
It rejects inherited `GDK_SCALE`/`GDK_DPI_SCALE`, missing settings ownership,
non-exact pane counts, malformed terminal reports, unchanged cell geometry, or
unchanged PTY rows/columns. The CI environment manifest, preflight, and literal
workflow package list now carry the `xsettingsd` prerequisite.

Final focused product command:

```text
linux/tests/nested-x11 env -u GDK_SCALE -u GDK_DPI_SCALE \
  ZENTTY_LINUX_BINARY=build/linux-profiles/release-safe/bin/zentty-linux \
  GDK_BACKEND=x11 linux/tests/rust-x11-scaling
```

Result:

```text
nested-x11: READY private-xvfb private-xauthority backend=x11 software-renderer=1
rust-x11-scaling: PASS xsettings-dpi=96,192 window=1200x800 panes=2 real-ghostty-csi real-pty
```

Focused policy commands also passed:

- `shellcheck linux/tests/rust-x11-scaling linux/ci/preflight`
- `bash -n linux/tests/rust-x11-scaling`
- `linux/ci/validate-environment`
- `linux/tests/qualification-matrix --validate-only linux/qualification-matrix.json`
- `linux/tests/qualification-matrix-test`
- `git diff --check`

The native `scale-x11` cell moved from BLOCKED to PASS. The explicit Xwayland
cell adds one tracked NOT_IMPLEMENTED entry rather than allowing Xvfb evidence
to erase that gap. Declared totals are now 175 PASS, 0 FAIL, 1 BLOCKED, 3 XFAIL,
and 5 NOT_IMPLEMENTED. Full Linux qualification is not claimed.
