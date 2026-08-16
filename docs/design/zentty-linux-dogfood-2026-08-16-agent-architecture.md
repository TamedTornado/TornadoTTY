# Zentty Linux dogfood — agent architecture refactor (GH-50)

This record covers the structure-only refactor between completed agent-event
parity (GH-46) and the next managed-launch feature (GH-47). It is intentionally
separate from the already long GH-46 dogfood record.

## Guardrails

- Preserve the single path: source adapter -> canonical `AgentEvent` ->
  `AgentStatusStore` reducer -> existing workspace persistence.
- Add no adapter framework, state authority, IPC transport, persistence file,
  or integration harness.
- Move one source family at a time and rerun the existing source-contract and
  adapter characterization tests.
- Consolidate state only after the adapter surface is structurally stable.

## Baseline characterization

Before moving implementation, the focused baseline passed:

- adapter characterization: 21/21;
- reducer characterization: 27/27;
- session restore store: 6/6;
- workspace state: 54/54.

These tests already cover source aliases and no-ops, missing identity,
duplicate/reordered task events, explicit-progress precedence, cross-pane
isolation, late events, explicit identity reuse, pane transfer, and durable
restore. No second characterization harness was added.

## Discovery: the adapter file was physically monolithic

`agent_adapters.rs` was 1,727 lines. Source entry points were separate
functions, but source ownership was not visible in the module tree. Adding
GH-47 there would have increased review and regression risk.

### Repair

The implementation is now a module tree with focused source files for Codex
and Small Harness, Claude, Cursor, Droid, Gemini, Vibe, Kimi, Grok,
Antigravity, and Hermes. The existing public function names are re-exported;
callers and the machine-readable source contract did not change.

Every source-family move was followed by the existing 21-case adapter suite.
The suite remained green after each move. Strict focused Clippy then rejected
wildcard imports in the new modules; those imports were replaced with explicit
dependency lists rather than suppressed.

## Discovery: session lifecycle state used nine parallel collections

`AgentStatusStore` stored task identities, explicit-progress authority,
ended-session tombstones, Codex title ownership, observed-running state, and
three visibility deadlines in separate maps/sets with the same pane/session
key. Pane removal, pane transfer, session cleanup, restore, and reducer
transitions had to update them independently.

### Repair

A typed `SessionKey` and one `SessionBookkeeping` value now own the complete
per-session lifecycle metadata. Pane removal uses one retain, pane transfer
moves one map entry set, and session cleanup uses one remove. Task authority,
session lifecycle, and title ownership are enums rather than ambiguous booleans.
`PaneAgentStatus` remains the only visible agent projection; the workspace
recipe remains the only durable authority.

Strict Clippy initially rejected a boolean-heavy draft. The draft was repaired
with typed two-state enums rather than a lint allowance. The existing
task/lifecycle/transfer tests and the full core all-target suite then passed.

## Environmental test failure

The first sandboxed full-core run failed only when the unrelated Open With
special-file test attempted to create its Unix FIFO and received
`EPERM`. The same unmodified suite passed outside the filesystem sandbox:
58 library tests plus every core integration target. This was an execution
environment restriction, not converted into a product pass or hidden by
changing the test.

## Current evidence

- `cargo test -p zentty-core --all-targets`: PASS outside the restricted
  filesystem sandbox required by the FIFO characterization.
- `cargo clippy -p zentty-core --lib -- -D warnings`: PASS.
- `linux/tests/cli-source-contract-test`: PASS.
- `linux/tests/qualification-matrix --validate-only`: PASS.

Mutation, staged real journeys, and the complete presently executable Linux
qualification rerun remain required before GH-50 can be committed, pushed, or
closed.

## Mutation findings and repairs

The first focused session-bookkeeping mutation run could not establish a
baseline inside the restricted filesystem sandbox because the copied suite's
unchanged FIFO test received `EPERM`. It was rerun outside that restriction
with one worker and `--gitignore=true`; no environmental failure was treated as
a pass.

That run caught 11 of 13 mutants and exposed two real characterization gaps:

1. restored explicit-progress authority was not independently asserted; and
2. session identity reuse proved the tombstone transition, but did not prove
   that prior task authority was cleared with it.

The existing workspace/reducer tests were strengthened at those exact
boundaries. The corrected rerun caught **13/13** bookkeeping mutants.

A bounded post-move adapter mutation sample covered every public source entry
point and every Codex positional alias: **34 tested, 22 caught, 12 unviable,
0 missed**. It ran with one worker, `--gitignore=true`, and no copied target,
preserving the mutation disk-safety policy.

The first physical decomposition left several private, source-only parsing
helpers in the shared module. That passed behavior tests but did not meet the
ownership boundary: a future Codex, Droid, Gemini, Kimi, or Vibe change would
still have required editing the shared file. Those helpers were moved beside
their sole adapter consumers. The final shared module is 395 lines and owns
only the public exports, common protocol construction, and helpers used by
multiple source families; each source family owns its adapter and its private
exceptions. The move was mechanical and the 21-case adapter characterization
suite plus strict focused Clippy passed again afterward.

## Real staged journeys after consolidation

The ReleaseSafe product was rebuilt from the refactored sources and exercised
through the existing controlled X11 environment. No new harness was added.

- Agent launch/IPC: PASS with the real staged application, real wrapper and
  helper processes, real PTY child, authenticated Unix socket, every supported
  adapter, task idempotency, reordered completion, failure, and cleanup.
- Multi-window: PASS with two real windows, live pane transfer, construction
  rollback, clean restore, intentional `SIGKILL` crash restore, size restore,
  and non-final-window close survival.
- Consolidated session restore: PASS with real loopback SSH, physical input and
  file drop, two background agents, exact scrollback, clean relaunch,
  intentional `SIGKILL`, corrupt-state recovery, and durable task completion.

The shell's `Killed` diagnostics in the latter two receipts are the explicit
crash-recovery phases, followed by their PASS receipts; they are not unexpected
process loss.

## Final qualification rerun

`linux/tests/qualify-local` completed successfully after the refactor:

- 152 PASS;
- 0 FAIL;
- 5 BLOCKED;
- 1 XFAIL;
- 15 NOT_IMPLEMENTED.

The implemented local suite and product-boundary qualification passed. Release
qualification and full Linux qualification did **not** pass because the
declared BLOCKED, XFAIL, and NOT_IMPLEMENTED cells remain. Debug Valgrind is
**PASS with reviewed suppressions**; it is not described as an unsuppressed
clean result, and ReleaseSafe Valgrind remains XFAIL.

The Debug Valgrind receipt preserved both views: raw 427 errors/contexts,
6,160 definite bytes, and 41,428 indirect bytes; after the reviewed effective
suppression set, 0 errors/contexts and 0 definite/indirect bytes. The
suppression review outcome was ACCEPTED.

The complete agent IPC and Linux all-target suites also passed outside the
restricted sandbox required by their real Unix-socket tests (IPC: all targets;
Linux: 9 library tests, 259 application tests, and 3 executable remote-transfer
tests with the 2 qualification-owned OpenSSH cases explicitly ignored there).

## Final ownership-only move rerun

After the last source-private helpers were physically moved, the complete core
all-target suite passed again. A four-worker qualification rerun then exposed
four timing failures in unrelated GUI journeys: X11 fleet window discovery,
the Wayland bookmark dialog, and both concurrently scheduled CLI journeys. The
receipts showed UI/socket deadlines rather than assertion mismatches. Each of
the four identical staged commands passed immediately when rerun sequentially
in its declared nested X11 or Wayland environment. This is retained as evidence
of qualification-runner contention; it is not relabeled as a product pass and
the failed aggregate receipt remains on disk. The final aggregate rerun is
therefore performed with two qualification workers, preserving real components
while reducing concurrent nested desktops.

That rerun cleared the four prior cells but exposed a deterministic receipt
race in `workspace-pane-settings-x11`. The harness changed the watched config
and only then sampled the number of prior reload receipts. If GIO and GTK
processed the atomic rename before the following shell command, the new receipt
was included in the baseline and the harness waited for a nonexistent second
reload. This explains both the failure at the first threshold and the immediate
standalone pass without a product change.

The repair samples `reload_before` before `sed -i`, so the receipt boundary now
precedes the stimulus as it does elsewhere in the real-input journeys. A static
orchestration contract rejects reversing that order, and the repaired staged
X11 cell passes its complete real-worklane, real-Ghostty-surface, two-window,
physical-control, live-reload, threshold-boundary, restart, and exact-config
journey. The last aggregate receipt remains an honest failed historical receipt;
the failed cell's newer focused receipt is PASS after the harness repair. No
product behavior was weakened or mocked to obtain the pass.
