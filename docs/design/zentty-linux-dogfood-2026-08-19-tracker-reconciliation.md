# Zentty Linux dogfood — tracker reconciliation

- **Date:** 2026-08-19
- **Starting branch:** `linux/port`
- **Starting product commit:** `914065b463f8514c86d661c4a582248b028d695e`
- **Scope:** Reconcile the public issue tracker with the feature inventory,
  qualification matrix, delivered child issues, and operator-approved deferrals.

## Why this audit was necessary

The tracker had **20 open issues** even though 39 implementation issues had
already closed. Most open issues were the original parent issues. Their bodies
still contained unchecked construction-era acceptance lists, including work
subsequently delivered by focused child issues and product commits. Counting
those parents as 20 equivalent remaining features overstated the work, while
counting only closed children understated the remaining source-parity gaps.

This reconciliation does not create another implementation-status authority:

- `docs/design/zentty-linux-feature-inventory.json` remains authoritative for
  source-derived feature implementation status;
- `linux/qualification-matrix.json` remains authoritative for qualification
  cells and their explicit limitations; and
- GitHub issues remain the execution and acceptance tracker.

## Evidence at audit start

`linux/tests/feature-inventory` reported 60 source-derived feature groups:

- 17 `IMPLEMENTED`;
- 28 `PARTIAL`;
- 1 `MODEL_ONLY`; and
- 14 `NOT_IMPLEMENTED`.

The latest committed qualification matrix contained:

- 161 `PASS`;
- 0 `FAIL`;
- 3 `BLOCKED`;
- 1 `XFAIL`; and
- 14 `NOT_IMPLEMENTED`.

These are different measures. A broad feature group can remain `PARTIAL` after
many of its behaviors pass, while one matrix cell can cover a narrow executable
contract. Neither count may be substituted for the other.

## Issue disposition

### Close as delivered foundation or completed initial scope

- **#2 — product architecture and workspace contract.** The Rust/GTK/Ghostty
  boundaries and source-derived workspace contract are established. Remaining
  product implementation belongs to #3, #4, and #5; keeping the architecture
  definition open made it look like a second implementation feature.
- **#6 — restart and crash recovery.** The feature inventory classifies the
  lifecycle-marker restore behavior as `IMPLEMENTED`, and the real crash
  recovery delivery is recorded at `9ca3219760fdc1619a4f0c98174a2ff960d31414`.
- **#12 — test and qualification architecture.** The machine matrix, false-pass
  runner tests, controlled compositor policy, Valgrind governance, focused
  mutation policy, and proportional orchestration contract are established.
  Ongoing use of that policy is a definition-of-done constraint, not an open
  feature.
- **#13 — Rust/Ghostty foundation.** The workspace, confined raw/safe ABI,
  real GTK/Ghostty surfaces, staged executable, ABI mismatch behavior, and
  lifecycle tests are delivered. Final Ghostty API pruning/upstream disposition
  remains explicitly owned by #11.
- **#19 — initial project utilities.** Development-server discovery/control,
  task-runner discovery/launch, and the initial per-pane/multi-window Task
  Manager are delivered. Jason explicitly deferred network accounting. The UI
  correctly removed the unavailable column rather than presenting fake zeroes.
  Optional network and container/cgroup telemetry moved to deferred issue #65.

Closing a foundation issue does not convert its regression constraints into
optional behavior. Existing tests, architecture validators, inventory entries,
and owning feature issues continue to enforce those contracts.

### Remain active for the initial Linux port

- **#1** — release-level parent epic.
- **#3** — workspace recipe and restore completion.
- **#4** — worklane/pane product-shell completion.
- **#5** — terminal lifecycle, close/restore, and remaining lifecycle evidence.
- **#7** — remaining source-parity and coding-agent integrations.
- **#8** — explicit physical input, IME, and scaling qualification gaps.
- **#11** — final downstream/upstream Ghostty decision and patch audit.
- **#14** — Claude team tmux compatibility completion.
- **#16** — remaining source UX, navigation, accessibility, and visual parity.
- **#17** — remaining terminal interaction and clipboard behavior.
- **#20** — remaining cross-page settings/appearance contract.
- **#22** — CLI epic completion after #14 and #32 plus shell-environment parity.
- **#23** — updates, trust, diagnostics, About, and licenses.
- **#32** — multi-window completion, immediately including live transfer into
  an existing worklane in another existing window.
- **#35** — remaining Clean Copy/selection/configuration lifecycle behavior.

This is **15 active issues**, but #1, #20, and #22 are epics rather than three
additional leaf features. Issue #65 is a separately labelled deferred future
enhancement and is excluded from the initial-port count.

The Task Manager's `PARTIAL` inventory entry and both related
`NOT_IMPLEMENTED` matrix cells now point to #65 rather than the closed #19, so
the remaining limitation cannot disappear behind a completed parent issue.

The first narrow inventory edit matched the earlier `linux_owner_issue: 19`
field for the already implemented development-server entry rather than the
later Task Manager entry. An immediate owner-summary check caught the mistake
before validation or commit. Development servers remain historical scope of
#19; only the partial Task Manager entry moved to #65.

The evidence ledger enforces an exact feature-to-owner mapping, so it was also
updated with current closure states for the affected reviewed issues and the
new #65 Task Manager owner. This does not change the 2026-08-04 source audit or
its evidence coverage; it reconciles the mutable execution-tracker projection.

## Reconciliation receipts

- Public GitHub query after closure: **16 open total**, comprising exactly
  **15 `port: active`** and **1 `deferred`** issue; **44 issues closed**.
- Closed with explicit evidence comments: #2, #6, #12, #13, and #19.
- Created deferred issue #65 and retargeted its two matrix cells plus the
  partial Task Manager inventory entry.
- Live GitHub state versus every issue state stored in the evidence ledger:
  zero mismatches.
- `linux/tests/feature-inventory`: PASS, 60 entries; status totals unchanged at
  17 implemented, 28 partial, 1 model-only, and 14 not implemented.
- `linux/tests/qualification-matrix-test`: PASS; matrix status totals unchanged.
- Architecture contract and negative self-tests: PASS.
- ApplicationShell ownership contract and negative self-tests: PASS after the
  two documented drift repairs.
- Focused `zentty-core` workspace recipe/session store tests: 11 passed.
- `zentty-ghostty`: 5 unit tests and 3 compile-fail documentation tests passed.

No product behavior was promoted, no unavailable environment became a pass,
and no full/release qualification claim was made during this tracker-only
reconciliation.

## Execution order after reconciliation

1. Finish and close **#32** as one feature issue, including cross-window live
   pane transfer and its remaining accessibility/visual acceptance.
2. Close the core state/shell chain: **#3**, **#5**, then **#4/#16** where their
   acceptance overlaps.
3. Complete **#14**, then reconcile and close CLI epic **#22**.
4. Complete terminal/settings parity in **#17**, **#35**, and **#20**.
5. Complete remaining agent parity in **#7** and lifecycle/trust work in **#23**.
6. Close supported platform gaps in **#8** and make the final Ghostty fork or
   upstream decision in **#11**.
7. Reconcile and close release epic **#1** only when its actual initial-port
   acceptance and support envelope pass.

Dependencies can change the exact order, but work must close a bounded feature
issue rather than report another tiny slice as the milestone.

## Explicit remaining uncertainty

- The 14 `NOT_IMPLEMENTED` inventory entries are not equivalent in size. Nine
  are individual later agent adapters; three are lifecycle/trust capabilities;
  the broad Ghostty appearance and shell-environment entries contain already
  delivered subsets but remain incomplete as catalogued.
- The qualification matrix deliberately retains blocked compositor/input cells
  and unimplemented workspace/worklane product journeys. Reconciliation cannot
  turn environmental absence or deferred implementation into `PASS`.
- Some parent issue checklists predate their focused children. Closure comments
  must state the superseding evidence rather than mechanically checking claims
  that were narrowed, deferred, or moved.

## Discovery and repair during reconciliation

The focused architecture validator initially failed even though the
authoritative qualification-matrix tests passed. The architecture document's
deliberately non-authoritative mirror still described the X11 worklane cell as
lacking the source contextual move affordance. Commit `914065b4` had delivered
that same-window affordance and updated the authoritative matrix, but had not
updated the mirror's exact defect text. The mirror is now reconciled to the
authoritative cell. This was documentation drift, not a product or matrix
status change; the cell remains `NOT_IMPLEMENTED` for its stated remaining
work.

The ownership validator then found a second drift from the same delivered
feature: `ApplicationShell` exported the new `move-pane-to-new-worklane`
action, but the architecture action inventory did not name it. The action is
now recorded under the existing `action_router`; no second router or ownership
path was introduced. This validator failure was repaired before any issue was
closed.

Adding the missing action exposed the validator's intentional exact count for
parameterless actions. That count was updated from 86 to 87. The named-action
equality check remains the substantive guard; the count continues to reject
duplicates or an unreviewed contract-shape change.

After the reconciliation commit was pushed, the first GH-1 comment expanded
the correct short hash `ca3f5a11` with an incorrect suffix instead of reading
the object ID from Git. A corrective public comment records the actual commit
`ca3f5a11a944babdbb27a15501ece8b783613ea2`. Exact hashes must be copied from
`git rev-parse`, never reconstructed from a short prefix.
