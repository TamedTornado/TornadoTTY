# Zentty Linux dogfood: completed restored-agent exit

Date: 2026-08-27
Tracker: GH-114

## Discovery

After a reboot, Zentty correctly restored the operator's Bro Codex session. The
operator updated Codex in a separate terminal and then exited the live restored
Bro session inside Zentty. Instead of returning the existing pane to an ordinary
shell, Zentty presented **Failed to resume session**. This was a product defect,
not a failed restore: the restored TUI had already been visibly usable.

The retained journal provides the ordering:

- `06:19:02`: Zentty launched the restored Codex child.
- `06:19:19`: the pane emitted real terminal notification output from that
  session.
- `06:19:30`: the child exited and Zentty logged
  `agent-restore-launch pane=pane-6 result=failed fallback=shell`.
- `06:19:34`: the operator invoked Retry.
- `06:20:02`: the retried child finally emitted its authenticated
  `session.start` event.

The first child was therefore still classified as a pending launch when it
exited because it never emitted the optional authenticated hook. The update in
another terminal was incidental; it made the live agent prompt for exit but did
not invalidate the already running PTY.

## Root cause and lifecycle contract

The runtime represented restore ownership only as a pending-command map.
Authentication removed the map entry. Child exit then had only two relevant
outcomes: an entry still present meant failed restore and no entry meant an
ordinary pane exit. That model lost the distinction between a restore that had
never become usable and a restored agent that had completed normally. It also
meant an authenticated restored agent's later exit could remove the workspace
pane rather than return it to a shell.

Restore ownership is now an explicit one-way state machine:

1. **Pending restore** + child exit: retain topology and show recovery controls.
2. **Running restore** + child exit: retain the same pane identity and replace
   its completed agent surface with a real ordinary shell.
3. **Ordinary child** + exit: retain the existing ordinary-pane lifecycle.
4. Shutdown and stale callbacks retain their existing non-mutating behavior.

An authenticated agent event confirms Pending to Running. A direct physical
terminal submission or interrupt also confirms it before the input reaches
Ghostty, covering a visibly usable restored agent whose current Codex version
does not emit the hook. There is no timer, grace period, or inferred wall-clock
success. Confirmation is monotonic and the restore command remains owned until
the running child exits.

## Regression strategy

Focused coordinator tests pin pending failure, confirmed completion, monotonic
confirmation, shutdown disposal, stale callback rejection, tmux ownership, and
ordinary exit. The established `linux/tests/rust-session-restore` harness gains
a focused completed-restore mode rather than a parallel harness. It launches a
real controlled restored child through the real Ghostty PTY, authenticates it,
lets it exit, and requires all of the following:

- `result=completed fallback=shell` is emitted;
- the recovery UI is never shown;
- a new real shell root PID attaches to the same pane;
- worklane and pane topology are unchanged.

The existing focused immediate-failure mode remains the counterexample and must
continue to show persistent recovery controls without losing topology. Neither
environmental absence nor a missing shell attachment is converted into a pass.

Twelve focused coordinator tests and all-target `zentty-linux` Clippy with
warnings denied pass. The release-safe staged product then ran both focused
counterexamples in a private nested-X11 environment:

- completed restore: `PASS completed-agent-restore authenticated=true
  fallback=real-shell recovery=absent topology=preserved`;
- immediate failure: `PASS failed-agent-restore topology=preserved
  fallback=real-shell snapshot=clean`.

The first sandboxed nested-X11 invocation did not launch the product because
Xvfb could not bind its `/tmp/.X11-unix` listener. The same command was rerun
with host-level socket access and passed; the environmental failure was not
converted into a pass. Package installation and the operator's real Bro
reproduction remain pending and will be appended. This is focused lifecycle
evidence, not a claim of broad or full qualification.

## Acceptance-criteria closeout

Tracker reconciliation found that the original focused journey proved only
authenticated completion and immediate failure. GH-114 also required real
direct-interaction confirmation and a successful Retry transition, so the
issue remained open rather than being closed from incomplete evidence.

The established `rust-session-restore` actor now owns both missing scenarios.
Its controlled Codex child can deliberately omit authentication and wait for
physical PTY input, or fail exactly its first launch before behaving like the
ordinary authenticated restore fixture. These are modes of the existing
external-agent actor, not a new integration layer.

The direct-interaction scenario sends `confirmed-by-user` through the nested
X11 compositor, Ghostty, and the real PTY. The child receives the exact line;
Zentty confirms the pending restore from that terminal submission, observes
the child exit, and mounts a shell in the same pane without recovery UI:

`PASS completed-agent-restore authenticated=false interaction=physical fallback=real-shell recovery=absent topology=preserved`.

The Retry scenario first failed because its test called the generic terminal
input-preparation helper after the recovery panel had correctly focused
**Retry**. That helper intentionally returned focus to Ghostty, so Return did
not activate the button. The actor now establishes compositor input before the
launch-delayed child fails; the product's real recovery focus handoff then owns
Return. A second assertion incorrectly expected a `state=cleared` receipt.
Retry destroys and replaces the failed `PaneFrame`, including its recovery
widget, so no surviving widget exists to clear. The corrected invariant is
exactly one recovery presentation followed by a newly authenticated surface,
normal completion, and same-pane shell fallback:

`PASS retried-agent-restore first=failure retry=physical authenticated=true fallback=real-shell topology=preserved`.

The recovery panel now explicitly focuses **Retry** and emits a focus receipt,
improving keyboard accessibility while making the real activation boundary
observable. The two journeys, 12 coordinator tests, ShellCheck, direct
formatting, strict Linux Clippy, and the staged ReleaseSafe build pass. The
first sandboxed Xvfb start failed because it could not bind
`/tmp/.X11-unix`; the unchanged journey passed with approved host socket
access. No full qualification or installed-product deployment was run.
