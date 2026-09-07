# GH-162: workload containment

Issue: [#162](https://github.com/TamedTornado/TornadoTTY/issues/162), under #160.
Starting revision: `fc7b33df4ff28914b05e8cec00046c7ecd339f02`.

## Policy and current status

Jason approved maximum memory ceilings of **80% aggregate / 60% per pane**,
lower reclaim thresholds, and CPU/I/O weights rather than hard CPU quotas.
The implementation selects 70% aggregate / 50% pane for reclaim thresholds,
and weight 100 for both CPU and I/O. The new `[workloads]` configuration section
validates these independently through the existing strict/partial config parser.
Budgets use memory capacity, not fluctuating free memory; callers must cap the
capacity by tighter ancestor cgroup limits. That host-capacity discovery and
the real GUI launch integration are **not implemented yet**.

The parsed defaults (not yet active on real pane launches) are:

```toml
[workloads]
aggregate_memory_high_percent = 70
aggregate_memory_max_percent = 80
pane_memory_high_percent = 50
pane_memory_max_percent = 60
cpu_weight = 100
io_weight = 100
```

This is an in-progress foundation, **not completed application containment**.
No installation, restart, live pane migration, or full qualification was run.

## Lifetime ownership proof

An ordinary GNOME application scope can outlive the GUI when other processes
remain in it. Binding pane units to that scope would not establish GUI lifetime.
The implemented user-manager service instead waits on a kernel pidfd for the
actual GUI process. The descriptor is passed through the supported D-Bus
`StandardInputFileDescriptor` property. The helper is an internal mode of the
same executable, before GTK initialization; no privileged daemon or extra public
CLI copy. No PID polling timer or inherited pipe writer controls its lifetime.

A pane attaches only its calling process to its generated transient scope.
`StartTransientUnit` acknowledgement is not success: wait for `JobRemoved`, then
verify actual kernel cgroup membership before permitting a future exec.
Five-second D-Bus/job deadlines bound failed operations; they do not infer
process lifetime. Scopes bind to the pidfd observer with `BindsTo` and `After`.
Failed scopes are not immediately garbage-collected, preserving diagnostics.

On this Ubuntu 24.04 / systemd 255 host, the real disposable-process test passed:
the GUI-role process creates the observer; a separate pane-role process enters
its scope; its detached background descendant remains in that scope. Killing
only the disposable GUI stops the observer, scope, and detached descendant.
The GUI remains outside the pane aggregate.

## Focused evidence

Recreate with the staged helper binary, not Jason's installed executable:

```sh
ZENTTY_BUILD_OUTPUT_DIR=build/gh162 linux/scripts/build-local
TORNADOTTY_OWNER_BINARY="$PWD/build/gh162/bin/zentty-linux" \
  cargo test --locked -p zentty-linux --test workload_systemd -- --ignored --nocapture
cargo test --locked -p zentty-core --lib workload_policy
```

The systemd test requires the real user manager; run outside the sandbox with
permission. It creates uniquely named `tornadotty-tests-*.slice` hierarchies,
never attaches the test driver or an existing application, and stops only its
own units at teardown. It uses a synthetic 2 GiB capacity, a 1 GiB pane maximum,
and no allocation pressure; these are isolated test values, not production
defaults. Kernel memory thresholds are checked with page rounding, together
with task limits and CPU quota absence. An earlier test compile failure used a
nonexistent `Result::is_err_or` method; corrected before execution.

Observed: two policy/config tests PASS, 26 existing configuration integration
tests PASS; real lifecycle + memory/CPU/task limit test PASS in 0.11 s.
I/O enforcement is **BLOCKED on this host**: the system root has the controller,
but `user.slice` does not delegate it. An unconditional `io.weight` read failed
with ENOENT even though systemd accepted the property. The implementation now
reports controller availability explicitly; the test independently checks the
delegated controller list and verifies the actual weight only when available.
No host delegation settings were modified. Effective device fairness and actual
pressure behavior are not established by property readback.

A second manager object reproduced a real `AlreadySubscribed` failure because
GIO shares the session-bus connection. Accept only that exact named D-Bus error
(not matching log text); other subscription failures remain errors. The real
test retains two manager objects and still requires actual unit job completion.
That regression passes in 0.10 s after the repair. The focused workload identity
validation test also passes. No mutation-strength or complete #162 acceptance
claim is made for this in-progress foundation.
The integrated staged build passed (53.12 s); the final staged executable's
observer was then used in another passing 0.10 s real-system run. It has not
been installed or launched as a GUI.

## Remaining acceptance work

- Integrate pre-exec containment without replacing Ghostty's resolved shell,
  login arguments, shell integration, PTY/session setup, or job control.
- Discover effective memory capacity; initialize one workload aggregate without
  silently replacing foreign/existing unit policy; manage multiple windows.
- Classify genuinely unavailable systemd versus refusal/failure. Only genuine
  unavailability permits an explicit process-group-only fallback.
- Stop owned scopes on close/quit and expose precise resource failures through
  existing recovery UI without erasing topology or restore intent.
- Exercise actual resource pressure, sibling responsiveness, close/restore,
  SSH/tmux/job control, manager fallback, and stale identity/error paths through
  focused existing integration boundaries. No whole-matrix PASS is claimed.
