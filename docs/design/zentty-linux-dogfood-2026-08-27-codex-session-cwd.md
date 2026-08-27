# Zentty Linux dogfood: Codex resumed-session directory ownership

Date: 2026-08-27
Tracker: GH-108

## Discovery

The operator resumed the existing Codex **Bro** session from a directory other
than the directory recorded by Codex. Codex correctly offered and applied its
session directory, and displayed that directory in its TUI. Zentty continued to
project the parent shell directory in pane and worklane context. The defect was
not Codex resume selection; Zentty discarded Codex's authenticated `cwd` hook
field and had no explicit rule for which directory owned pane context while an
agent was active.

## Ownership design

The repair extends the existing typed state rather than introducing another
context store:

- `PaneState.working_directory` remains the durable, shell-reported directory.
- `PaneAgentStatus.working_directory` holds the authenticated active-agent
  directory.
- `WorkspaceState::effective_working_directory_for_pane` is the single selector:
  active agent first, durable shell fallback.
- a `session.end` or completed restored-agent exit removes agent ownership and
  therefore exposes the already-preserved shell directory immediately.

The Codex adapter now accepts `cwd`, `current_working_directory`, and
`currentWorkingDirectory` from source hook payloads and projects them into the
canonical agent event. The canonical protocol rejects empty, relative,
NUL-containing, missing, and non-directory paths. Accepted paths are
canonicalized before entering status state. Restored-agent seed data uses the
same validator, so a relaunch can project the saved session directory before a
new authenticated hook without overwriting the shell directory.

The effective selector now supplies the existing sidebar/worklane summary,
window and project context, project icon and Git/PR probing, task-runner and
development-server association, Open With actions, bookmark fallback, CLI list
JSON, and restore metadata. There is no second Linux-only cwd map.

## Focused automated evidence

Core coverage proves the complete ownership transition with real canonical
directories: shell `/tmp` -> authenticated Codex `/usr` -> shell `/tmp`.
Sidebar projection follows the effective owner while the durable pane remains
`/tmp`. Adapter coverage proves a Codex `SessionStart.cwd` reaches
`PaneAgentStatus`. Status coverage proves relative, missing, and NUL-containing
updates cannot replace the last valid authenticated directory. Restore
snapshot assertions prove agent B and shell A remain distinct across a clean
close.

The established `linux/tests/installed-codex-integration` journey was extended;
no parallel integration harness was created. It uses reviewed Codex CLI
0.147.0, the staged Zentty binary, real Ghostty surfaces and PTYs, real Codex
hooks/notify/IPC, a private nested X11 server, and a controlled loopback model
endpoint. Only the model response is controlled.

The real journey now:

1. creates a Codex session in directory B while the parent shell remains in A;
2. verifies the persisted restore draft owns B while durable pane state owns A;
3. relaunches Zentty through its desktop-PATH path from A;
4. requires the restored TUI and window title to project B;
5. submits a real resumed turn and requires authenticated session events to
   retain B;
6. enters `/exit`, requires the completed-restore shell replacement and its real
   PTY root attachment, and physically runs `pwd` in the same pane;
7. compares that receipt with A and requires project context to return to A;
8. forbids the failed-restore recovery UI throughout the successful path.

Passing receipt:

`Installed Codex integration passed: x11, version=0.147.0,
model-endpoint=controlled-loopback-completed-turn, wrapper=staged, pty=real,
hook=real, notify=real, ipc=real, relaunch=real-resume-tui,
resume-cwd=session, sidebar=actual-gtk-label,
context-ownership=agent-to-shell, shell-cwd=physical-receipt,
lifecycle=physical`

## Dogfood-visible projection repair

After the first installed build, an operator screenshot proved that the
effective directory was driving Git/project routing but was not visible in the
sidebar. The worklane subtitle and pane title both rendered the animated Codex
activity (`Working Bro`), while `SidebarPaneSummary.working_directory` was
discarded by the GTK renderer. GH-108 was reopened; the prior close was
premature because internal routing did not satisfy the visible pane/worklane
acceptance criteria.

The repaired sidebar keeps activity, directory, and agent state as independent
fields: the worklane subtitle shows the focused pane's compact directory, and
the pane row shows activity, compact directory, then agent status. Home paths
render with `~`; the full path remains in the directory tooltip and accessible
pane label. Codex activity animation now updates only the activity title and no
longer overwrites the directory-bearing worklane context.

The pre-existing installed-Codex journey had asserted model ownership, window
title, project routing, persistence, and relaunch, but not the GTK sidebar
widget. That coverage gap allowed the defect. The same journey now requires a
receipt emitted by the pane-directory label setter for the restored session's
exact directory. The controlled-display GTK test independently inspects the
rendered worklane and pane labels. No second sidebar or Codex harness was added.

## Failures and repairs during construction

Nothing below was converted into a pass:

1. The first resumed launch did not enable project-local behavior because the
   isolated Codex home trusted A but not B. The harness now trusts the exact
   controlled B directory; product trust policy was not weakened.
2. The original title assertion expected `Ready | zentty`. Correct Codex
   session-directory selection produced `Ready | tmp`; the assertion now checks
   B-derived output.
3. A test expected `result=authenticated` after physical Return had already
   confirmed the restored process. Restore confirmation intentionally supports
   either authenticated hooks or proven physical interaction. The corrected
   test requires the authenticated `session.start` event and authenticated cwd
   ownership rather than demanding a second impossible state transition.
4. The controlled Responses endpoint serves exactly one turn. Reusing its dead
   port for the resumed turn made hook delivery race connection failure. The
   harness now starts a fresh single-turn endpoint and rewrites only the
   isolated test config before relaunch.
5. Per-character XTEST typing intermittently failed to enter the Codex editor
   even though Return reached Zentty. A diagnostic screenshot proved the prompt
   remained empty. The journey now publishes exact text through the real X11
   clipboard, reads it back, and exercises Zentty/Ghostty's real physical paste
   shortcut before Return.
6. The first shell receipt was typed after state ownership returned to A but
   before the replacement Ghostty shell surface attached. The journey now gates
   on the completed-restore result and real PTY root attachment, not a delay.
7. A screenshot exposed an unrelated Codex warning about `/tmp/.codex` trust in
   the temporary environment. The exact controlled session directory remained
   correct and authenticated; no broader `/tmp` trust exception was added.
8. The first post-repair real journey reached the resumed Codex TUI and
   authenticated directory ownership, but physical `/exit` did not produce a
   `session.end`; the existing shell-return assertion failed. Nothing was
   waived. An unchanged lifecycle path passed on the immediate controlled
   rerun, including the new exact GTK-label receipt and physical shell return.
   The isolated non-reproduction remains uncertainty rather than proof of a
   product repair.
9. Focused closeout reran `linux/tests/test-orchestration-contract`. It retained
   its known unrelated failure because `linux/tests/staged-shell-integration`
   generates an inline agent program; this slice did not touch or waive that
   file. The same finding is already recorded by the upstream-parity dogfood
   report and remains outside GH-108.

The final real journey completes in approximately six seconds in the private
X11 environment. This is focused GH-108 evidence, not a claim of full Linux
qualification and not a deployment receipt.
