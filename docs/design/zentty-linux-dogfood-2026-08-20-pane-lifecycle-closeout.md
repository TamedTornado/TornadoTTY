# Zentty Linux dogfood — GH5 terminal lifecycle closeout

- **Date:** 2026-08-20
- **Starting commit:** `eaf18ad0a05c`
- **Scope:** Close GH5 as one terminal-lifecycle feature. Extend the existing
  `WorkspaceState`, `PaneRuntimeCoordinator`, `rust-closed-pane-restore`, and
  memory-safety systems; do not add parallel lifecycle or integration layers.

## Acceptance audit

The current product already owns one model record, one Ghostty surface/frame,
one PTY child, one callback owner, and an idempotent removal path per pane.
Controlled Debug and ReleaseSafe lifecycle cells cover default, epoll, and
io_uring on X11 and Wayland; focused adapter tests cover invalid construction;
real actors already prove CWD/title/command restoration, focus routing,
running-pane close, child-exit non-capture, rollback after construction failure,
cross-worklane identity, and quiescent callbacks after disposal.

The source audit found the remaining parity gaps behind the single PARTIAL
inventory entry:

1. Linux closed-pane capture does not retain the source agent-resume draft.
2. It does not read and archive real terminal output for the source
   `Previous output: file://...` restore affordance.
3. It does not walk a missing CWD to the nearest existing ancestor/home or show
   the source missing-directory warning.
4. The four locally feasible Debug Valgrind cells remain NOT_IMPLEMENTED even
   though the governed raw/suppressed runner and scenario manifest already
   exist. The runner is incorrectly hard-wired to `build/linux` instead of the
   staged Debug profile.
5. The two ReleaseSafe Valgrind cells are incorrectly NOT_IMPLEMENTED despite
   the operator decision that they remain executable XFAILs; they must run and
   retain receipts without broadening suppressions.

GH5's dependency on GH4 is behavioral, not a reason to duplicate worklane
code: existing cross-worklane transfer evidence is reused, while incomplete
direct manipulation remains explicit under GH4/GH16.

## Test-first order

1. Add source-derived focused failures for agent-resume precedence, missing-CWD
   ancestor/home resolution, scrollback composition, LIFO/expiry, active and
   inactive focus fallback, child-exit non-capture, and rollback/retry.
2. Extend `rust-closed-pane-restore` rather than creating another actor. Use a
   real Ghostty surface to capture terminal text, close through physical GTK
   input, inspect the cache archive and visible warning, restore a controlled
   agent command through the existing agent boundary, and prove fresh PTY,
   identity, CWD, focus, persistence, and callback quiescence on X11/Wayland.
3. Put platform-neutral close/restore policy in focused core types/functions.
   Put XDG cache file ownership and Ghostty text reading in focused Linux files;
   do not grow an unrelated catch-all component.
4. Parameterize the one existing memory-safety runner to consume an explicit
   staged bundle. Run Debug single/interaction on controlled X11/Wayland with
   preserved unsuppressed and suppressed receipts. Update the manifest only
   from observed exact scenarios; never turn environmental absence into PASS.
5. Execute ReleaseSafe interaction on both compositors as XFAIL. Preserve raw
   and suppressed evidence and the tracked finding; do not add suppressions to
   make either green.
6. Promote only cells whose complete commands have the declared outcome,
   update mirrors/inventory, mutation-test changed pure restore policy with the
   repository's safe cargo-mutants configuration, and run every presently
   executable cell before commit/push.

## Claim limits

- Debug memory results are described only as **PASS with reviewed
  suppressions**, with raw and post-suppression totals.
- ReleaseSafe Valgrind remains XFAIL.
- No exhaustive/release/full-Linux claim is permitted while any matrix cell is
  BLOCKED, XFAIL, or NOT_IMPLEMENTED.

## Dogfood record

### Source restore composition and real product proof

- Test-first core cases failed because Linux exposed neither scrollback capture
  nor deterministic home-aware CWD restore and discarded live agent status at
  close. Added those contracts to the existing closed-pane entry/result rather
  than introducing another store. Agent resume is derived through the existing
  validated `PaneRestoreDraft::resume_command`; it takes source precedence over
  the previous command.
- Runtime-construction rollback now consumes the returned restore transaction,
  removes the failed pane without recapturing it, and puts the complete original
  entry back on the LIFO stack. This preserves agent and scrollback context as
  well as the prior geometry/CWD/command context without a parallel pending map.
- Added one focused Linux cache owner. It resolves the real XDG cache root,
  validates generated pane identities, atomically publishes non-empty terminal
  text, purges files older than 24 hours, percent-encodes the file URI, and
  composes the source link before the resume/replay line. It does not own pane
  lifecycle or model state.
- The first real X11 run reached the archive, warning, restored PTY, and prefill
  but the controlled agent assertion expected bare `codex resume <id>`. The
  delivered Zentty Codex wrapper correctly inserted its reviewed hook flags
  before invoking the controlled agent boundary. Raw shell-line evidence proved
  the source command arrived unchanged; the assertion was repaired to require
  both the wrapper's session-start hook and final session ID rather than bypass
  the wrapper.
- Corrected controlled X11 and Wayland journeys now pass with real Ghostty text
  in a regular private XDG archive, an actually deleted source directory walked
  to its real ancestor, visible warning receipt, source agent-resume precedence,
  delivered hook wrapper, controlled agent call, fresh PTY/identity/focus,
  persisted fallback CWD, and no callbacks after disposal. Only the final agent
  executable is controlled; terminal, shell, IPC, wrapper, cache, GTK input,
  restore, and process lifecycle are real.

### Memory-safety actor repair and lifecycle starvation

- The old memory runner attempted to trigger removed, environment-variable
  integration hooks. The product therefore opened normally and the runner
  waited for a marker that could never appear until its 300-second timeout.
  Replaced that dead path with the existing `rust-product-smoke` actor. No new
  harness or test-only product behavior was added: Valgrind is now the direct
  product command inside the same controlled compositor session used by the
  established actor.
- Valgrind's default inline-DWARF processing made large Ghostty builds spend
  most of startup in symbol ingestion rather than exercising the application.
  `--read-inline-info=no` retained actionable allocation stacks while reducing
  paired raw/suppressed single- and multi-terminal runs to roughly 3.5 minutes.
- The first real two-terminal Debug run observed both child-exit callbacks but
  never observed either removal. Continuous software-rendering work could
  starve GLib idle-priority teardown indefinitely. Child exit owns terminal
  lifecycle, so its zero-delay handoff now uses the regular timeout source
  rather than idle priority. The repaired real multi-terminal scenario reaches
  both removals, disposes both surfaces, and becomes quiescent on X11 and
  Wayland.
- A temporary Mesa JIT-worker suppression was introduced while diagnosing the
  starvation run. Once the lifecycle fault was repaired, the rule matched no
  stable scenario and suppression governance identified it as stale. It was
  removed rather than retained as defensive noise.

### Reviewed suppression evidence

- Every Debug product cell preserves an unsuppressed receipt and a separately
  suppressed receipt. The machine-readable report binds the exact binary,
  compositor, scenario, effective suppression-set hash, raw errors/leaks, and
  post-suppression errors/leaks. A passing Debug result is only **PASS with
  reviewed suppressions**.
- The stable Debug findings required two additional narrow project rules: a
  Mesa llvmpipe conditional-read path whose consumer is Ghostty's instanced
  draw call, and GTK mnemonic accessibility list allocation reproduced by the
  existing minimal non-Ghostty GTK program. Both rules have tracking IDs,
  affected environments, exact finding types, expected per-scenario contexts
  and bytes, justification, and reproducer/upstream references in
  `linux/tests/valgrind-suppressions.json`.
- The inherited Ghostty files remain part of the hashed effective suppression
  set and governance audit. ReleaseSafe intentionally excludes Zentty's project
  suppression file: an early trial accidentally made the profile green by
  applying Debug project policy. The corrected runner executes the real
  ReleaseSafe product with inherited suppressions only and preserves status 99
  evidence as the required XFAIL rather than broadening rules.
- Governance self-tests cover missing and unknown rules, count growth, stale
  rules, scenario escape, untracked additions, suppression-set mismatch, and
  incorrect publication claims. Matrix tests cover missing cells, unknown
  statuses, unexpected skips, stale XFAILs, command failures, and false full
  qualification claims.

### Remaining qualification limits

- The authoritative matrix, not older prose receipts, determines current
  qualification. Historical dogfood records describe the state at their
  recorded commit and must not be read as overriding current cell status.
- ReleaseSafe Valgrind is deliberately XFAIL. Controlled public environments
  and the remaining environment/product-dependent cells remain explicit in the
  matrix. This work cannot claim release or full Linux qualification while any
  required cell is BLOCKED, XFAIL, or NOT_IMPLEMENTED.

### Mutation-test repairs

- The first focused restore-policy mutation run found 8 surviving mutants. The
  survivors exposed real missing boundaries rather than being waived: direct
  product-clock close/restore, the short Copilot agent name, inactive-worklane
  restore routing, exact closed-pane capacity on capture, and exact capacity
  before/after rollback. Focused tests now exercise each boundary.
- Two comparison mutants were equivalent under existing model invariants. A
  one-pane column is removed on close, so its saved height can never be reused;
  capture now expresses multi-pane presence with `get(1)` instead of an
  arithmetical length comparison. Imported and interactively resized pane
  heights are already sanitized positive values, so restore no longer repeats
  an unreachable non-positive fallback check.
- One unmutated mutation baseline failed in the unrelated concurrent bookmark
  store test after its lock deadline. The focused rerun correctly selected the
  `workspace_state` integration target rather than hiding or retrying that
  failure in a full-package baseline. Final result: 33 mutants, 27 caught, 6
  compiler-rejected/unviable, 0 missed, under the repository's resource scope
  and safe `gitignore = true`, `copy_target = false` copy policy.

### First authoritative rerun failures and repairs

- The first full local rerun executed every currently declared cell and did
  not pass. It correctly rejected `rust-closed-pane-restore` because that actor
  created a new inline `codex` shim, even though the shim was only the final
  controlled boundary. The actor now symlinks the repository's single reviewed
  `fixtures/controlled-agent` and uses its existing `codex-restore` profile.
  Both corrected X11 and Wayland journeys pass without adding another actor.
- Promoting `pane.close-restore` changed the inventory totals, but the focused
  inventory runner still asserted the previous totals and used GH5 as its
  negative closed-owner fixture. The runner now expects 21 implemented/25
  partial entries and uses still-partial GH4 for that negative; its complete
  positive and negative suite passes.
- Fresh multi-terminal Valgrind receipts contain one `used_suppression` line
  per real product process. They also explicitly report Valgrind's
  `/usr/libexec/valgrind/default.supp`, which is enabled and separately pinned
  by version but is not a project or Ghostty file. Governance previously
  treated that reviewed built-in file as unrecognized. It now counts only that
  exact built-in path in addition to the hashed explicit files, so any fourth
  source still fails as unrecognized.
- Fresh product receipts invalidated several earlier exact Fontconfig/Pango
  observations. The manifest was reconciled to the newly captured per-process
  counts and bytes for each compositor/scenario; the suppression rules were not
  broadened. This is evidence of the intended count-growth/stale-evidence gate
  working. The complete suppression audit then passed over all seven current
  raw/suppressed report pairs.
- The batched Wayland development-server cell logged a successful product
  browser action but its browser receipt assertion raced and failed. An
  immediate isolated rerun of the same real listener/product/browser journey
  passed. This is retained as a flaky test discovery rather than relabeled as
  product success; the final authoritative rerun must decide the cell.
- Two support negative tests intentionally invoke production preflight/package
  builders and therefore see the feature worktree as dirty before reaching
  their requested negative. This is not an environmental pass. The operator's
  no-commit-before-rerun condition is now satisfied because every executable
  cell ran; a local reviewed feature commit is required before the final clean
  checkout qualification can make those support contracts meaningful.

### Clean-checkout rerun: merged diagnostic interleaving

- The clean-checkout rerun cleared every earlier support failure and the
  development-server race, but Debug X11 multi-terminal Valgrind failed its
  semantic actor. Both real terminals became ready and exited, and the product
  completed lifecycle. Under 1.45 million raw Mesa diagnostics, Ghostty stderr
  and the first `terminal-ready` stdout marker were merged onto one physical
  line. The actor incorrectly required that generic marker to occupy an entire
  line, so it counted one terminal instead of two.
- The existing per-pane readiness marker remained intact for both surfaces and
  is the stronger lifecycle oracle. The actor now counts occurrences of
  `terminal-ready-pane=` rather than relying on cross-stream line atomicity. It
  does not relax the title, child-exit, or lifecycle completion assertions.
- A final focused actor rerun initially failed before product launch because
  the command sandbox denied Xvfb's private Unix-domain display socket. The
  identical controlled nested-X11 command was rerun with the required local
  socket permission and passed with two real terminals. Environmental socket
  denial was not converted into a product pass.

### Independent suppression-count reconciliation

- The corrected Debug X11 multi-terminal cell passed in isolation with its
  unsuppressed and suppressed receipts intact. Governance then rejected the
  complete report set because the previous manifest encoded several
  Fontconfig/Pango observations as exact per-process values. The first failure
  was a one-context metrics cache record where the earlier clean run had
  observed exactly three; a single fail-fast repair would have hidden the same
  problem in the remaining scenarios.
- All project-rule observations from the five current Debug reports were
  compared against the manifest in one pass. The comparison found no
  untracked rule or scenario escape. It found process-local Fontconfig/Pango
  cache partitions varying within the same controlled product journeys: for
  example, the three Wayland interaction processes independently reported 1,
  2, and 4 metrics-cache contexts, and the X11 interaction processes reported
  2, 2, and 4. These are separate process caches, not a growing aggregate leak.
- Scenario ranges now form the narrow union of the earlier reviewed
  observation and the independent clean-checkout observation. The global
  envelopes were reconciled to contain those scenario ranges. No suppression
  stack, rule pattern, allowed scenario, or required-use policy was broadened;
  a future count or byte value outside the two-run envelope still fails.
- The complete current report set passes suppression governance. Its focused
  tests still reject count growth, stale rules, scenario escape, untracked
  suppressions, identity mismatches, and false publication claims. The
  qualification matrix and boundary-contract self-tests also pass.
