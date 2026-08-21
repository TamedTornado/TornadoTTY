# Linux X11 Scaling Qualification Plan

Issue: GH-69 (child of GH-8)

## Outcome

Replace the prose-only X11 scaling prerequisite with a deterministic real-system
qualification cell. The proof must use the standard XSETTINGS DPI channel on a
private X server, the staged Zentty product, real Ghostty terminals, and PTY
responses. `GDK_SCALE` is explicitly not acceptable as the scaling authority.

## Design

1. Run one focused product journey inside the existing `nested-x11-v1`
   environment. Do not create another display wrapper or product actor layer.
2. Start an owned `xsettingsd` with pinned 96-DPI and 192-DPI profiles in turn.
   Capture the selected XSETTINGS value before launching each product process.
3. Restore two real panes, externally resize the real X window to a fixed pixel
   geometry, and release both PTY actors only after GTK reports both terminals.
4. Have each PTY request its cell and text-area pixel dimensions using the
   standard terminal `CSI 16 t` cell report, alongside `stty size`.
5. Require the scaled run to retain the same externally observed X window size,
   increase every Ghostty cell dimension, and reduce terminal rows/columns.
   This distinguishes desktop DPI scaling from merely resizing the window.
6. Promote only `scale-x11` after the exact focused command passes. Xwayland is
   not silently covered by Xvfb: it has its own `scale-xwayland` cell and GH-70.

## Failure requirements

- Missing `xsettingsd`, XSETTINGS ownership, X11 geometry, terminal replies, or
  staged product prerequisites fails; none becomes an environmental pass.
- The actor rejects malformed, duplicated, missing, or cross-profile receipts.
- Every child pane must acknowledge the profile independently.
- Owned product and settings-daemon processes are terminated on every exit path.
- The test must fail if `GDK_SCALE` or `GDK_DPI_SCALE` is inherited.

## Verification order

1. Shell syntax and static matrix validation.
2. Focused `scale-x11` journey in the controlled X11 profile.
3. Matrix-runner regression tests.
4. Presently executable matrix cells affected by the changed execution graph;
   do not rerun unrelated app-wide journeys merely for ceremony.
