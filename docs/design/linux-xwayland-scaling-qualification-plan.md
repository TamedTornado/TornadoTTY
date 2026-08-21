# Linux Xwayland compositor-scaling qualification plan

Issue: GH-70, final child of GH-8.

## Outcome

Replace `scale-xwayland` NOT_IMPLEMENTED only after staged ReleaseSafe and Debug
Zentty products run with `GDK_BACKEND=x11` on a wrapper-owned Xwayland server,
while the enclosing controlled labwc compositor moves them across observed 1x,
1.5x, and 2x output states and real Ghostty PTYs acknowledge coherent geometry,
SIGWINCH, and pointer coordinates.

## Test-first order

1. Extend the existing opt-in labwc mixed-output profile with a mutually
   constrained Xwayland mode. Add negative tests for invalid profile
   combinations, missing Xwayland, missing startup evidence, and malformed
   Xwayland receipts.
2. Record Xwayland identity, version, owned process-group membership,
   executable and command hashes, display authentication, root geometry, and
   RANDR outputs in the existing controlled environment receipt. Never reuse
   or infer the developer desktop's Xwayland.
3. Add one focused real-product journey covering single and equal-split panes in
   staged ReleaseSafe and Debug builds. The journey must assert the GTK X11
   backend, Xwayland window identity, compositor output readback, X11-observed
   geometry, Ghostty CSI metrics, PTY size/SIGWINCH, SGR pointer coordinates,
   and continued compositor presentation.
4. Run the journey directly before changing the authoritative matrix. Add a
   distinct `nested-xwayland-scaling-v1` profile so the matrix runner owns all
   environment selection and evidence validation.
5. Run affected wrapper regressions, schema/runner negatives, inventory,
   ShellCheck, and exact real-system cell. Record every failure and repair,
   then commit only the complete feature.

## Boundaries

- Reuse the existing private Xvfb → labwc environment; do not create a parallel
  compositor stack.
- Product and policy orchestration stay in Zentty. No Ghostty change is planned
  unless the real journey isolates a Ghostty-owned defect.
- Native Xvfb/X11 evidence from GH-69 and native Wayland evidence from GH-68 do
  not satisfy this issue.
- Missing Xwayland, output-management, authentication, or semantic PTY evidence
  fails or skips as an explicit prerequisite; it never becomes PASS.
