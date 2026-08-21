# Zentty Linux dogfood — Xwayland compositor scaling

Date: 2026-08-21

Issue: GH-70 (final child of GH-8)

Plan: `docs/design/linux-xwayland-scaling-qualification-plan.md`

## Outcome

The authoritative `scale-xwayland` cell is now PASS. The existing private
Xvfb → labwc harness owns a real Xwayland 23.2.6 server, and the staged
ReleaseSafe and Debug Zentty products run on it with `GDK_BACKEND=x11`. The
same canonical scaling journey used for native Wayland now proves the distinct
X11-on-Xwayland path at compositor scales 1x, 1.5x, and 2x with real single and
equal-split Ghostty PTYs.

No Ghostty source change was required. GH-70 extended the existing controlled
compositor and canonical scaling journey instead of adding a parallel test
stack.

## Owned environment and exact receipt

- Outer transport: private 3200x1800 Xvfb with private Xauthority.
- Compositor: labwc 0.7.1, two wlroots X11 outputs, Pixman renderer.
- Xwayland: X.Org Xwayland 23.2.6 (12302006), launched by the compositor in its
  owned process group with the supported `-shm` transport.
- Initial compositor state: X11-1 1024x768 at scale 1 and X11-2 1024x768 at
  scale 2.
- Initial Xwayland RANDR state: X11-1 1024x768 and X11-2 512x384, root
  1536x768.
- Xwayland has no `_XSETTINGS_S0` owner and no X resource `Xft.dpi`; stable
  10x21 Ghostty cell metrics across all scale states prove that toolkit DPI was
  not applied a second time over the compositor's logical-output scaling.
- Ambient display/D-Bus endpoints are sanitized. The outer transport,
  compositor, Xwayland, and product are reaped; the private display is
  unreachable afterward and no owned process remains.

Final identifiers and hashes:

- Wayland session:
  `da6a032a383035cd5b193ec29bd4892d9f728f5520102bd41aec0ec4898722e4`
- Outer X11 session:
  `24939ed1042216d5102afafef7de0b27dd451aa88dbfc8ab410aa99616819160`
- Human receipt SHA-256:
  `2a57523ef9ea33727b89fa353943b2a54a3e546c7c339a0a3dbede4bd1d81b1c`
- Controlled-environment receipt SHA-256:
  `e3e07a92a56ae7ecbed4f22a21438bf1590fba52b1a52ae29528539978a78418`
- Canonical scaling harness SHA-256:
  `b67253f5524ff18f1ce43d38343436e4fa2532f6d4d5e5658def4b9468290230`
- Controlled compositor wrapper SHA-256:
  `2b015635b0828f09cbdf1ff0a36cc9524c7e570f99becbbcd046dd1f9eb09513`
- Xwayland binary SHA-256:
  `9bbd85d6cb9a763d05ad8d39b3e35baccb7542293de2d5cf03f76fdc875d0ca7`
- Xwayland command-line SHA-256:
  `4bf44a9f703e0ca32be5ead5c27167bfb90993fa5d1b987845864beb919294bb`

The exact matrix command passed:

```sh
ZENTTY_NESTED_WAYLAND_INPUT_COMPOSITOR=labwc \
ZENTTY_NESTED_WAYLAND_SCALING_PROFILE=1 \
ZENTTY_NESTED_WAYLAND_XWAYLAND_PROFILE=1 \
linux/tests/nested-wayland-input \
  bash -o pipefail -c \
  'env -u GDK_SCALE -u GDK_DPI_SCALE GDK_BACKEND=x11 ZENTTY_COMPOSITOR_SCALING_BACKEND=xwayland linux/tests/rust-wayland-scaling'
```

```text
rust-wayland-scaling: PASS backend=xwayland profiles=ReleaseSafe,Debug scales=1,1.5,2 single-baseline multi-baseline fractional-1.5 integer-2 real-ghostty-csi real-pty SIGWINCH mouse-coordinate
```

In both profiles the maximized Xwayland window changed from 1024x743 at 1x to
682x487 at 1.5x and 512x359 at 2x. The equal split changed from 32x36 terminal
cells per pane to 20x36 and then 13x36. Both panes agreed at every settled
stage, both received increasing SIGWINCH counts, and the cell size remained
10x21 rather than being double-scaled. SGR pointer reports remained ordered and
same-row at every scale. Exactly two terminal-ready markers prove neither pane
restarted.

## Discoveries, failures, and repairs

1. **Xwayland is lazy.** labwc's startup environment publishes `DISPLAY=:2`
   before the Xwayland process accepts clients. An immediate PID lookup or
   `xdpyinfo` was therefore false evidence. The wrapper polls authenticated
   `xdpyinfo`, which activates the lazy server, then identifies Xwayland inside
   labwc's owned process group.

2. **Direct-parent identity is wrong under this session manager.** The
   compositor's Xwayland process is reparented by the user-session subreaper,
   so `pgrep -P labwc` failed even though Xwayland retained labwc's exact
   process group. Process-group membership is also the boundary the wrapper
   terminates and audits. The retained receipt records PID and process group,
   not a fictitious direct parent.

3. **Pixman requires Xwayland shared-memory transport.** With labwc's Pixman
   renderer, the default rootless Xwayland glamor startup crashed before the
   window manager connection. Forcing wlroots GLES2 was not a workaround:
   nested Xvfb exposes no DRI3/DRM FD, and wlroots rejected the renderer. The
   retained `WLR_XWAYLAND` wrapper execs the system `/usr/bin/Xwayland` with
   its documented `-shm` option. This is still the real distro Xwayland server;
   it selects the transport compatible with the intentionally software-only
   compositor. The controlled receipt now pins the resolved executable and
   hashes both that binary and its exact command line. Hardware/glamor coverage
   is not inferred from this cell.

4. **A diagnostic startup helper initially leaked.** Keeping the labwc startup
   script asleep moved it outside the compositor process group on this host.
   Failed probes exposed the leak immediately. The helper now writes the
   compositor-provided display and exits; all leaked diagnostic helpers were
   terminated, and the final wrapper regression proves no private roots or
   processes remain.

5. **Rootless Xwayland reports PointerRoot for `XGetInputFocus`.** `xdotool
   getwindowfocus` therefore returns no usable focused window even after labwc
   focuses the product. The shared input helper accepts this only when all
   controlled-Xwayland identity variables are present and the discovered
   top-level's `_NET_WM_PID` matches the live product. Focus correctness is then
   proven semantically by exact CSI and SGR reports from the real PTY, not by
   treating PointerRoot as success. Native X11 retains its strict focused-PID
   assertion.

6. **Compositor output normalization changes logical positions.** When X11-1
   becomes 1.5x, labwc places it at the right of X11-2 rather than honoring an
   assumed fixed left-to-right name order. The test reads both the compositor
   and Xwayland RANDR state. It moves the owned product to the observed X11-2
   coordinate rather than inferring coordinates from output names.

7. **Resize acknowledgement needed a settlement boundary.** The first
   fractional run released its PTY actors immediately after output management;
   one pane sampled before GTK completed both allocations. A bounded 500 ms
   compositor settlement precedes semantic capture. Absence is not converted
   to success: exact equal pane geometry and increasing SIGWINCH remain
   mandatory afterward.

8. **XSETTINGS needed explicit treatment.** Merely forbidding `GDK_SCALE` did
   not prove that toolkit DPI was absent. The retained journey requires the
   canonical `dump_xsettings` no-owner result, rejects X resource `Xft.dpi`,
   and requires stable Ghostty cell metrics while Xwayland logical window and
   PTY dimensions shrink exactly with compositor scale.

## Focused validation

The affected suite passes:

```text
linux/tests/lib/product-input-test
  PASS native focus rules plus narrowly controlled rootless-Xwayland fallback
linux/tests/lib/controlled-environment-test
  PASS strict optional Xwayland receipt schema
linux/tests/nested-wayland-input-test
  PASS Cage/Weston/labwc, mixed scale, owned Xwayland, isolation and cleanup
linux/tests/rust-wayland-scaling-test
  PASS both backend contracts and ambient-session rejection
native Wayland exact journey
  PASS after canonical harness generalization
linux/tests/test-orchestration-contract
  PASS separate native-Wayland and owned-Xwayland matrix profiles
linux/tests/qualification-matrix-test
  PASS runner negatives and false-claim rejection
linux/tests/qualification-matrix --validate-only
  PASS schema and coverage
linux/ci/validate-pr-subset-test
  PASS both scaling-contract drift negatives
```

The matrix now declares 186 PASS, 0 FAIL, 0 BLOCKED, 3 XFAIL, and 4
NOT_IMPLEMENTED cells. This is not exhaustive Linux QA or full qualification:
the remaining XFAIL/NOT_IMPLEMENTED cells are unchanged, and this controlled
software-rendered Xwayland `-shm` environment does not claim hardware/glamor or
representative GNOME/KDE coverage.
