# Droid parity dogfood record — 2026-08-27

## Scope and operating constraint

- Issue: GH-129, child of GH-7.
- Complete Factory Droid hooks, tasks, interactions, status, and safe restore
  parity using the existing Linux abstractions.
- Jason is actively using the installed Zentty. All builds and tests for this
  issue are staged or controlled; do not install, deploy, launch, stop, or
  replace the active application.
- Do not run full Linux qualification for this feature slice.

## Source audit

The behavior contract comes from:

- `Zentty/AppState/Agent/DroidHooksInstaller.swift`
- `Zentty/AppState/Agent/DroidTaskStore.swift`
- `Zentty/AppState/Agent/EventAdapters/DroidEventAdapter.swift`
- `Zentty/AppState/Agent/AgentLaunchBootstrap.swift`
- `Zentty/Restore/SessionRestoreStore.swift`
- the corresponding macOS logic and wrapper tests.

## Existing Linux state

- The bounded atomic persistent installer already owns all eight source hook
  groups and preserves nested user entries.
- The wrapper already recognizes Droid, tracks its PID, and forwards hook
  payloads through authenticated IPC.
- The adapter already covers basic lifecycle, notifications, manual approvals,
  TodoWrite aggregate counts, and identity-bearing task events.
- The authoritative inventory nevertheless correctly remains NOT_IMPLEMENTED:
  there is no Droid restore command or restore-draft projection, CWD aliases are
  dropped, several source interaction forms are reduced incorrectly, TodoWrite
  string/empty/authority semantics are incomplete, and the product journey uses
  a synthetic adapter payload rather than the installed hook command.

## Design decision

The macOS implementation uses a short-lived file-backed `DroidTaskStore`
because every hook bridge is a separate process. Linux already converges all
authenticated events in the long-lived canonical `AgentStatusStore`. Port the
observable task semantics into that existing authority; do not add a Droid-only
file, lock, timer, transcript poller, or reducer.

## Discoveries, failures, and repairs

This section is updated as red tests expose behavior and implementation repairs.

- The first focused adapter run failed immediately: AskUser payloads with
  choices were projected as `Question` and discarded every option, while the
  source presents a `Decision` containing the question and choices. Because the
  focused commands were intentionally chained with `&&`, the restore tests did
  not run after this known failure; they will run after the adapter reaches its
  asserted contract.
- The next run reached CWD projection but the test used a nonexistent
  `/tmp/droid-project`. The shared protocol intentionally canonicalizes only an
  existing absolute directory, so the fixture—not the security boundary—was
  repaired to use real `/tmp`.
- The complete adapter suite then found a stale Linux-only expectation that
  treated Droid `cancelled` tasks as complete. The source helper recognizes
  `completed`, `complete`, and `done`; the fixture now exercises the documented
  `complete` alias while retaining its independent 2/3 progress assertion.
- Auditing the existing product actor before execution found that generic
  `run_adapter` inherited Jason's real `HOME`. Persistent-agent launches could
  therefore modify real hook configuration during qualification. Every adapter
  run now owns and removes a private HOME/XDG config tree. The Droid actor reads
  its installed hook command only from that private tree.
- The first controlled nested-X11 journey executed all eight installed hook
  groups, rejected the bad token, rendered final Idle 1/3, and queued a Droid
  restore draft, but timed out at window close. The newly isolated XDG tree did
  not contain the harness's confirmation-off policy, so the real application
  correctly displayed `Quit Zentty?`. Each private run now writes the same
  explicit test-only confirmation policy into its own config before launch.
- All affected crate tests passed. The first strict Clippy run then rejected the
  114-line Droid dispatcher; notification classification was extracted into a
  focused helper rather than adding a lint suppression.
- Final diff review found that the real Droid actor had supplied synthetic task
  IDs even though the source `Task` and `SubagentStop` hooks increment counters
  when no identity exists. That would have hidden a source-parity gap. The actor
  now sends the anonymous source shape, and the shared protocol/reducer owns a
  session-scoped `task.delta`; no Droid-only counter store was introduced.
- Two initially requested focused restore test names were stale and selected
  zero tests. Their output was not accepted as evidence; the actual test names
  were located and both exact cells were rerun successfully.
- The first combined crate run was sandbox-denied when six IPC discovery tests
  attempted their private Unix-socket operations. The same bounded command was
  rerun with socket permissions and all assertions passed; environmental denial
  was not converted into a pass.
- Final production review found that restored aggregate counters were seeded as
  identity-owned because the existing persistence flag only distinguishes an
  explicit snapshot. A draft with progress and no task identities now restores
  as counter-owned; a restart regression proves a late identity event cannot
  replace it and a later delta can still complete it. The first version of that
  test incorrectly referenced a closure local to a neighboring test and failed
  to compile; the fixture now owns its authenticated-event closure explicitly.
- The final strict Clippy pass rejected a 112-line combined adapter regression.
  It was split into interaction/CWD and anonymous-counter/Todo-authority tests;
  no lint allowance was added, and both exact tests plus strict Clippy passed.

## Evidence

- Focused source-semantics tests: PASS for Decision/options, spec approval text,
  canonical CWD, TodoWrite string/empty behavior, Todo authority over later
  subagent events, strict opaque restore validation, and restore-draft capture.
- Complete affected crate suites: PASS. `zentty-core` and
  `zentty-agent-ipc` completed without failures.
- ReleaseSafe staged build: PASS. Cargo publication-age audit covered 91
  packages with 0 exceptions; package notice catalog covered 104 entries.
- Controlled nested-X11 Droid product journey: PASS, session receipt
  `402638c7d33b268264743acb4950214a5665c064833358291519fa392a2539a1`
  against the final rebuilt staged binary after correcting the actor to use
  anonymous source task events and preserving their authority across restore.
  It executed the privately installed nested hook commands through `/bin/sh`,
  the staged wrapper, real PTY child, helper, authenticated Unix socket, Linux
  product, Ghostty surface, and sidebar; rejected the bad-token control; showed
  final Idle 1/3; and persisted the live opaque session/CWD on real window
  close.
- Strict Clippy across all targets in `zentty-core`, `zentty-agent-ipc`, and
  `zentty-linux`: PASS with `-D warnings`. Both the production dispatcher and
  restore test were decomposed instead of suppressing line-count lints.
- ShellCheck, JSON parsing, authoritative inventory runner, and diff whitespace
  checks: PASS.
- Authoritative inventory after reconciliation: 55 IMPLEMENTED, 0 PARTIAL,
  8 NOT_IMPLEMENTED.

## Remaining limitations

- A live Droid account/model request may remain the controlled external-agent
  boundary. The staged wrapper, hook configuration and command, PTY child,
  authenticated socket, Linux product, Ghostty surface, sidebar, and durable
  restore path must remain real.
