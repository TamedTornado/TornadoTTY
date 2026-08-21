# Linux Wayland fractional and mixed-output scaling qualification plan

Issue: GH-68, child of GH-8.

## Outcome

Replace the blocked Wayland fractional-scale matrix cell only after a staged
Debug and ReleaseSafe Zentty window has crossed deterministic 1x, 1.5x, and 2x
outputs in an owned compositor and real Ghostty PTYs have acknowledged coherent
geometry, cell metrics, resize signals, and pointer coordinates.

## Test-first order

1. Add negative wrapper tests for an opt-in scaling profile: it must require
   labwc, exactly two owned nested outputs, `zwlr_output_manager_v1`,
   `wp_fractional_scale_manager_v1`, and `wp_viewporter`.
2. Add focused harness contracts for compositor-observed scale receipts,
   Debug/ReleaseSafe and single/multi-pane coverage, output movement, exact PTY
   acknowledgements, resize signals, and terminal mouse coordinates.
3. Extend the existing controlled Wayland-input wrapper rather than creating a
   second compositor stack. The new matrix environment profile pins labwc's X11
   backend to two outputs and records their identities and protocol inventory.
4. Implement one canonical real-product journey, then run it directly before
   changing the authoritative matrix from BLOCKED to PASS.
5. Run affected wrapper, schema, runner, inventory, ShellCheck, and real-system
   tests; document every failure and repair; commit only the complete feature.

## Controlled environment

- The outer transport remains private Xvfb and software rendered.
- labwc 0.7.1 owns two wlroots X11 outputs. `wlr-randr` 0.3.0 is the reviewed
  output-management client.
- Output state is read back from `zwlr_output_manager_v1`; `GDK_SCALE` and
  `GDK_DPI_SCALE` are forbidden.
- The journey observes baseline 1x, fractional 1.5x, and integer 2x states.
  Dedicated labwc key bindings move the active real Zentty window between the
  named outputs without application-side geometry calls.

## Product assertions

- A real PTY actor records `stty size`, CSI 16 cell-pixel reports, SIGWINCH,
  and SGR mouse reports from compositor-delivered pointer clicks.
- A second real Ghostty pane is created for the multi-pane stage; neither pane
  may disappear or restart during scale transitions.
- Fixed logical window sizing plus compositor output movement must yield
  coherent rows, columns, and cell metrics without double scaling or clipping.
- Wayland protocol evidence must identify the compositor's preferred
  fractional scale and output enter/leave transitions for the running product;
  injector success alone cannot pass.

## Boundaries

- This issue owns native Wayland scaling. Xwayland remains GH-70.
- No Ghostty change is planned unless the real journey isolates a Ghostty-owned
  defect.
- A missing protocol, output, or observable semantic receipt remains a failed
  prerequisite, never a pass or environmental skip.
