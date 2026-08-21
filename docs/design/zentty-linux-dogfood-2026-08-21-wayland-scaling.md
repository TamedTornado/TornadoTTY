# Zentty Linux dogfood — Wayland fractional and mixed-DPI scaling

Date: 2026-08-21

Issue: GH-68 (child of GH-8)

Plan: `docs/design/linux-wayland-scaling-qualification-plan.md`

## Outcome

The authoritative `fractional-scale-wayland` cell is now PASS. One controlled
labwc journey runs the staged ReleaseSafe and Debug products through a real
single-pane 1x baseline, an equal two-pane 1x baseline, a live 1.5x fractional
transition, and movement to a 2x output. It uses real Ghostty surfaces and PTYs,
compositor-delivered pointer input, compositor output management, and Wayland
protocol receipts. It does not use `GDK_SCALE` or `GDK_DPI_SCALE`.

No Ghostty source change was required. The new orchestration and policy remain
in Zentty; the existing controlled Wayland-input wrapper gained one opt-in,
strictly validated scaling profile.

## Controlled system and evidence

- Private Xvfb transport: 3200x1800, software rendered, isolated authority.
- Nested compositor: Ubuntu `labwc` 0.7.1-1build1 with `WLR_X11_OUTPUTS=2` and
  Pixman.
- Output-management client: Ubuntu `wlr-randr` 0.3.0-1.
- Baseline outputs read back through `zwlr_output_manager_v1`:
  - `X11-1`: 1024x768, position 0,0, scale 1;
  - `X11-2`: 1024x768, position 1024,0, scale 2.
- Required protocols are observed, not assumed:
  `wp_fractional_scale_manager_v1`, `wp_viewporter`, and
  `zwlr_output_manager_v1`.
- Ambient display and D-Bus endpoints are sanitized, the controlled process
  group is reaped, its private display is unreachable afterward, and no owned
  process remains.

Final controlled session:

- Wayland session ID:
  `da29b62a3defa9dae18bb94147c56a803e2c135f3237e130189ccbe47e2aaf24`
- Outer X11 session ID:
  `c3f4ebaae0f43a953e13c3f96415b71646eb0c3d0f6948a3bd726d2d877e1928`
- Human receipt SHA-256:
  `99bdbf03dc0346ad08f7d533d1ef07907ad23465a45c8b3655b175e76df5eb5d`
- Controlled-environment receipt SHA-256:
  `62998aed4fee1bf079c5b3235fc2c05d8d0570e8cf1f61f9ed89e4b8987a14a8`
- Scaling harness SHA-256:
  `42575e558205fcf56804d201e0a85328ef33bda33a538182ea760a4e920485f6`

The exact matrix command passed:

```sh
ZENTTY_NESTED_WAYLAND_INPUT_COMPOSITOR=labwc \
ZENTTY_NESTED_WAYLAND_SCALING_PROFILE=1 \
linux/tests/nested-wayland-input \
  bash -o pipefail -c \
  'env -u GDK_SCALE -u GDK_DPI_SCALE GDK_BACKEND=wayland linux/tests/rust-wayland-scaling'
```

Final semantic summary:

```text
rust-wayland-scaling: PASS profiles=ReleaseSafe,Debug scales=1,1.5,2 single-baseline multi-baseline fractional-1.5 integer-2 real-ghostty-csi real-pty SIGWINCH mouse-coordinate
```

Both profiles produced seven exact geometry acknowledgements, four exact SGR
pointer transitions, and seven completion acknowledgements. At 1x the single
pane reported 30x71 cells in ReleaseSafe and 32x73 in Debug; after a real equal
right split, both panes in each profile reported 32x36. The 1.5x transition
retained coherent equal PTY geometry while the
compositor emitted `preferred_scale(180)` and both PTYs observed additional
SIGWINCH delivery. Moving the same window to 2x emitted
`preferred_scale(240)`; both panes agreed on 19x42 cell pixels and 38 columns.
ReleaseSafe reported 20 rows and Debug 13 rows because their staged UI builds
have different vertical allocations; equality is required within each profile,
not falsely across profiles. Every stage also produced fresh Wayland surface
commits, rejecting a frozen or stale rendering path.

## Discoveries, failed approaches, and repairs

1. **The first command used the wrong environment-variable spelling.**
   `ZENTTY_NESTED_WAYLAND_COMPOSITOR` is not the wrapper contract; the correct
   name is `ZENTTY_NESTED_WAYLAND_INPUT_COMPOSITOR`. The wrapper rejected the
   scaling profile instead of silently choosing a compositor. The final command
   and matrix dispatch use the correct variable.

2. **labwc action arguments differ from an intuitive output-name API.**
   Duplicate XML attributes were rejected by the parser, and attempted
   `output`/`output_name` arguments were rejected. The documented
   `MoveToOutput direction="left|right"` action is accepted. `Maximize`, rather
   than `ToggleMaximize`, makes the initial state deterministic.

3. **wlroots' two X11 output windows initially overlap.**
   XTEST pointer events consequently reached only the topmost output even when
   `xdotool --window` named the other top-level. The scaling profile now moves
   the two wrapper-owned 1024x768 windows to non-overlapping locations on the
   private 3200x1800 Xvfb desktop. This is test transport setup, not a product
   geometry operation. Non-scaling Cage, Weston, and labwc profiles retain
   their existing discovery and behavior; their regression test caught and
   repaired an intermediate over-broad window filter.

4. **A normal new pane is stacked, not split.**
   The initial actor used `Ctrl+T`, so the hidden pane correctly retained its
   own geometry rather than sharing the visible allocation. The retained test
   calls the public `zentty split right --equal` API against pane 1, proving a
   genuinely visible two-pane topology while keeping the same real command in
   both Ghostty PTYs.

5. **Fixed physical pointer coordinates encoded responsive-layout guesses.**
   Sidebar and split allocations occupy different physical ranges at 1x, 1.5x,
   and 2x. Several early runs armed SGR reporting but delivered one or both
   clicks to the sidebar or the adjacent pane. The retained harness scans a
   single terminal-height row in each of the two isolated output windows. Only
   the real pane consumes SGR button reports; the actor requires two ordered,
   same-row, increasing terminal coordinates. This checks the compositor → GTK
   → Ghostty coordinate mapping without treating injection success as proof.

6. **Fractional scaling does not imply a fabricated PTY shrink.**
   At 1.5x, labwc and GTK retained the window's logical allocation while the
   fractional-scale/viewport protocol changed presentation. Requiring fewer
   rows and columns would have encoded a false behavior. The test instead
   requires protocol-observed scale 180, coherent equal pane allocations,
   additional resize notification, valid cell metrics and pointer coordinates,
   and continuing surface commits. The 2x output transition does change device
   cell metrics and visible rows, and those changes are checked separately.

7. **Failure logs were initially too broad.**
   Dumping 400 lines of `WAYLAND_DEBUG` made a local failure noisy without
   improving diagnosis. Failures now retain concise terminal-ready,
   fractional-scale, viewport, surface-commit, and error excerpts plus bounded
   PTY/output receipts.

8. **A scale-string assertion was malformed for 1.5.**
   Concatenating `.000000` would have expected `1.5.000000`. Exact supported
   forms are now mapped to `1.000000` and `1.500000`; other requested scales
   fail.

9. **Runtime output identity is not inferred from numeric X11 window IDs.**
   The wlroots backend does not publish a stable mapping between named Wayland
   outputs and its outer X11 top-level IDs. The wrapper owns and separates both;
   pointer proof addresses each once. Named scale and position proof still
   comes from `wlr-randr`, while left/right movement and preferred-scale events
   prove the product's actual output transition.

## Focused validation

All presently executable affected tests passed after the repairs:

```text
linux/tests/rust-wayland-scaling-test
  PASS isolation, scales, profiles, topology, PTY, SIGWINCH, pointer contracts
linux/tests/nested-wayland-input-test
  PASS Cage/Weston/labwc regression plus mixed scaling, isolation, cleanup
linux/tests/test-orchestration-contract
  PASS controlled scaling environment ownership
linux/tests/qualification-matrix-test
  PASS runner negative and claim tests
linux/tests/qualification-matrix --validate-only
  PASS schema and coverage
linux/ci/validate-pr-subset
  PASS cells=19 claims=subset-only
linux/ci/validate-pr-subset-test
  PASS, including scaling-contract drift rejection
```

The matrix now declares 185 PASS, 0 FAIL, 0 BLOCKED, 3 XFAIL, and 5
NOT_IMPLEMENTED cells. This does **not** claim exhaustive Linux QA or full
Linux qualification: XFAIL and NOT_IMPLEMENTED cells remain, and GH-70 still
owns Xwayland compositor scaling. This journey qualifies the controlled
software-rendered labwc environment; representative hardware-GPU GNOME/KDE
coverage remains outside this cell rather than being silently inferred.
