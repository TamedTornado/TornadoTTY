# Close-time process inspection — 2026-09-07

Issue: [#163](https://github.com/TamedTornado/TornadoTTY/issues/163), parent #160.
Previous: [task responsiveness](tornadotty-linux-dogfood-2026-09-07-task-responsiveness.md).

## Confirmed failure

Pane/worklane close gathered `/proc/<foreground-pid>/comm` and `cmdline` on GTK,
including a second synchronous read when accepting confirmation. Window close
and application quit used the same collection path. On `73f77702`, delaying a
real fixture pane's `comm` read blocks physical input in a sibling window.
This reproduces the blocking boundary, not a historical production incident.

The prior acceptance comparison contained only target IDs and risk booleans.
Executing a different command in the same PID could remain "running" and pass
that check despite changing the workload behind the dialog.

## Repair and ownership

- GTK captures a short-lived CloseSnapshot: target, pane evidence, window ID,
  and the existing PaneContext identity including foreground PID/topology.
  Agent evidence uses direct state lookup instead of repeatedly constructing
  all sidebar rows. No filesystem inspection occurs in this snapshot step.
- A single process-wide Gio inspection worker, with no backlog, reads process
  evidence. A permit follows actual worker execution even if its window closes.
  The existing per-window pending-close slot also covers inspection, preventing
  repeated requests from accumulating callbacks/dialogs. Busy requests leave
  the target intact and display a retry warning.
- Results require an unchanged target snapshot. Existing core close policy
  decides whether to close or ask. Disabled confirmation skips classification;
  untouched idle shells remain non-nagging; unreadable process information
  remains conservative, never classified as an idle shell.
- Accepting a dialog triggers another background inspection. Both snapshot and
  process details must still match. The final GTK commit checks the snapshot
  again after modal teardown. Replaced/moved targets, changed membership, and
  changed command arguments/name invalidate the request rather than terminating
  the new workload. For pane/worklane actions, focus returns after modal
  teardown even while validation is pending; surviving panes regain focus
  after a successful close. Accepted window/quit actions do not re-present
  the window that is being closed.
- Process arguments are held only for the lifetime of this confirmation and
  never logged. No persistent process cache or duplicate close policy exists.
  Snapshot propagation through window/application handlers is necessary because
  those callers share the same inspection boundary; it is not a new quit policy.

This is not atomic control over arbitrary exec between the last process read
and termination, nor native-crash isolation. Kernel I/O can occupy its bounded
worker indefinitely. Existing coordinator shutdown/persistence paths and the
broader #160/#163 acceptance are not claimed complete.

## Regression and focused validation

The existing `rust-multi-window` journey gains a close-inspection-only mode.
It uses private X11/Openbox and real PTYs. A narrowly targeted `sudo -n strace`
attachment works around Yama's sibling-ptrace restriction; it traces only that
private product and delays reads of the controlled pane's exact `comm` path.
It does not change syscall results, system ptrace policy, or product behavior
through a test hook. Its tracer is explicitly detached during cleanup.

```sh
ZENTTY_BUILD_OUTPUT_DIR=build/gh163-close linux/scripts/build-local
ZENTTY_MULTI_WINDOW_CLOSE_INSPECTION_ONLY=true \
  ZENTTY_LINUX_BINARY="$PWD/build/gh163-close/bin/zentty-linux" \
  linux/tests/nested-x11 linux/tests/rust-multi-window
ZENTTY_MULTI_WINDOW_LIFECYCLE_ONLY=true \
  ZENTTY_LINUX_BINARY="$PWD/build/gh163-close/bin/zentty-linux" \
  linux/tests/nested-x11 linux/tests/rust-multi-window
```

- Slow inspection: PASS sibling PTY input, changed worklane membership rejected,
  actual same-PID exec rejected after accepting the dialog, fresh confirmed
  close of precisely the two-pane target, surviving window input.
- Normal lifecycle: PASS confirmation cancel/accept, real window/quit cleanup,
  focus restoration, final-worklane replacement, untouched idle-shell closes.
  Two obsolete `Quit Zentty?` expectations were corrected to the already-shipped
  `Quit Tornado TTY?` branding; confirmation requirements were not relaxed.
  A later focus-restoration change failed survivor input after accepted window
  close. It was corrected to restore early only for cancelled dialogs and
  pane/worklane actions, not re-present a closing window. This was a product
  regression discovered by the lifecycle test, not a test retry workaround.
- Four focused close-runtime tests PASS, including real child-process `/proc`
  inspection, idle/running/missing classifications, and permit lifetime/unwind.
  The removed equality-only helper test exercised a deleted helper; real
  inspection and GUI stale-workload tests replace that limited evidence.
- Existing closed-pane-restore journey PASS: all eight failed creation actions,
  disabled-confirmation flow, failed Undo and successful real recovery.
- Integrated build, package notices, publish-age audit (91 packages, zero
  exceptions), touched-file formatting, shell syntax and diff checks PASS.
- Scoped inspection mutations: **10 caught, 0 missed, 2 unviable** in three
  minutes. Existing resource-isolated `linux/tests/mutate-rust` with safe
  gitignore/copy-target settings, four workers; no whole-suite strength claim.
- No new Clippy suppressions or unrelated lint repairs. Six existing GTK
  findings remain outside this batch.

Mutation invocation:

```sh
linux/tests/mutate-rust -p zentty-linux \
  -f crates/zentty-linux/src/application_shell/close_runtime/inspection.rs \
  -j4 --cargo-test-arg=--bin=zentty-linux \
  --cargo-test-arg=close_runtime --timeout 15
```

No full qualification, new matrix totals, or Wayland claim. Build staged only;
Jason's live terminal was not installed over, restarted, or terminated. User QA
remains in the agreed later batch. #163 stays open for remaining bookmark,
catalog/monitor I/O and broader PTY/IPC backpressure work.
