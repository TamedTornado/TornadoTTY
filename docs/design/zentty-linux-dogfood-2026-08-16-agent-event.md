# Zentty Linux dogfood: agent event parity

This report begins with GH-46. It is intentionally separate from the completed
shell-integration report. Every source discovery, red test, product failure,
repair, real receipt, and remaining uncertainty for agent event parity belongs
here.

## 2026-08-16: existing-system audit

The first audit disproved the assumption that GH-46 begins with an absent
agent system. Linux already owns one capability-authenticated Unix socket, one
canonical JSON event protocol, eleven adapter normalizers, one application
event coordinator, one multi-session per-pane reducer, and live consumers for
sidebar status, attention inbox, fleet, sleep inhibition, terminal progress,
Codex titles/transcripts, process death, and restore drafts. Replacing or
duplicating those authorities would be accretive programming.

The real gaps found before coding are narrower but substantive:

- the CLI has no source-compatible `agent-status` command;
- GH-45's `agent-signal` receiver intentionally accepts only shell-state,
  pane-root-PID, and pane-context, not agent lifecycle or agent PID signals;
- the published version-1 canonical protocol describes artifact and working-
  directory objects that Rust currently ignores through serde's default
  unknown-field behavior;
- Linux retains parent session identity and per-session progress, but has not
  yet reconciled every source ordering, hierarchy, visible projection, and
  restart expectation needed to call that durable subagent bookkeeping.

No product code was changed during this audit. The next step is a
machine-readable source table and red tests against these exact gaps, using the
existing actor and real-product journey rather than adding a harness.

## Canonical compaction events

The machine-readable source table immediately exposed two source canonical
events missing from Rust's enum: `agent.compacting` and `agent.compacted`.
The first focused test failed at the wire parser with an exact unknown-variant
diagnostic listing only the older seven events. Rust now accepts both source
events. Compacting keeps the session running and uses the supplied text or the
source default `Compacting`; compacted keeps it running and clears transient
compaction text. Both cancel stale stop/idle state through the existing reducer
rather than adding compaction bookkeeping elsewhere.

The focused test and the complete `zentty-core` target suite passed after the
repair. The first complete core run inside the restricted tool sandbox failed
only when the pre-existing Open With special-file test attempted its required
Unix node operation and received `EPERM`; the exact suite passed outside that
sandbox. This is an execution-environment constraint, not agent-event evidence
and not a product pass inferred from an absence.

## Compatibility lifecycle and PID path

The first real-product attempt failed before the new lifecycle route appeared.
The staged `bin/zentty` had been refreshed, but the wrapper-owned helper at
`libexec/zentty/agent-wrappers/shared/zentty` was still the previous build.
That was a staging mistake in the focused developer build, not a product
protocol failure. Refreshing the existing wrapper helper (the normal complete
build already does this) made the same journey pass; no alternate launcher or
test path was added.

The existing controlled-agent actor and `rust-agent-ipc` journey now execute a
real Codex wrapper, real Ghostty PTY child, separate staged `zentty` helper
processes, and the real private Unix socket for `agent-status` running,
needs-input, completed, and clear plus `agent-signal pid attach`. The product
receipt proves the visible sidebar and canonical fleet transitions, removal on
clear, real child PID delivery, and that hostile Unicode/option-like status
text does not escape into receipts. The isolated X11 run passed.

An exact reducer test exposed a source-semantic discrepancy: an unscoped PID
clear only looked for Linux's `pane-default` session, while the source clears
the tracked PID from every session in the pane. The reducer now clears exactly
one named session or all sessions when no session is named. The same test pins
parent identity and proves clearing a child does not clear its parent.

The initial lifecycle implementation validated `--origin` and `--confidence`
but then discarded both. That would make conflicting compatibility, inferred,
and explicit signals depend only on arrival time. The canonical store now owns
source-compatible origin/confidence priority, retains root-over-child ordering
when lifecycle state is otherwise tied, and rejects a weak inferred idle that
conflicts with an explicit running event. Canonical `agent-event` input remains
explicit-hook/explicit by definition; no second reducer was introduced.

Remaining uncertainty is explicit in the agent-event contract: artifact and
context fields parse and merge in memory but do not yet have their complete
visible/persistent product behavior, and Copilot/small-harness adapters remain
dependencies of GH-47. These gaps are not counted as implemented.

## Context persistence and secret boundary

The source uses an event-supplied working directory ahead of a pane's original
launch directory when hydrating status and exporting a restore draft. Linux
previously parsed that context but still used only the pane launch directory.
Focused restore and transcript tests now pin the source precedence: canonical
working-directory context drives transcript lookup and validated restore
drafts, with the pane directory remaining the fallback.

The same red fixture included hostile launch arguments and an `API_TOKEN` in
the event launch environment. Linux retains those fields only in volatile
status. It does not serialize them: supported resume commands are rebuilt from
validated tool/session identity, and the restore snapshot contains no event
environment. This deliberately preserves the source's user-visible resume
behavior without turning an authenticated hook into a secret-persistence
channel. Tool-specific launch argument/environment persistence remains
PARTIAL until each supported tool has a minimal allowlist justified by an
actual resume requirement.

The source artifact link currently flows into its status/attention model but
has no direct sidebar action. Linux now matches that model boundary: a complete
kind/label/URL tuple is retained on the canonical session; incomplete or
unknown tuples do not fabricate a link, receipts never print its content, and
no URL is opened without a future explicit user action.

## Every implemented adapter through the real product boundary

The existing `rust-agent-ipc` journey originally launched real staged wrappers
for only Codex, Claude, and Gemini. Unit coverage existed for the other adapter
normalizers, but that did not prove their wrapper/helper environment reached a
real pane capability or visible product state. The same journey and the same
controlled actor now launch Cursor, Droid, Kimi, Grok, Antigravity, Hermes, and
Vibe wrappers as separate processes. Each emits its native source-shaped hook
payload through the staged helper and private socket into a real Ghostty PTY,
then proves the expected typed attention state in the sidebar. Sensitive hook
message bodies remain forbidden in receipts.

The expanded isolated X11 run passed with the eleven then-implemented Linux
adapters plus the separate Codex notification form. At that checkpoint,
Copilot and Small Harness were explicit dependencies rather than silent skips.
The later source audit below implements Small Harness event normalization and
leaves only its managed launcher, plus Copilot, explicitly owned by GH-47.

## Source adapter audit corrected false generic parity

A second, case-by-case source read found that the first Rust implementation's
shared "common hook" classifier was too broad. It made superficially similar
agents behave alike where the source deliberately does not: Grok notifications
and question pre-tool hooks are no-ops because its canonical re-emitter owns
attention; Kimi only reacts to `permission_prompt`, `AskUserQuestion`, and its
three approval tools; Hermes session-finalize is idle rather than session
removal; Cursor error is an unresolved failure and aborted is a stop candidate;
Antigravity `fullyIdle=false` is an unresolved failure; Vibe tool hooks without
a tool name are no-ops. Droid manual-mode mutation approvals and
`ExitSpecMode` post-tool behavior were also missing.

Those were product bugs, not documentation gaps. The existing adapters were
narrowed instead of adding a replacement layer. `agent.failed` is now the
canonical input used by authoritative built-in source adapters to enter the
existing `UnresolvedStop` projection. Source-specific negative tests prove the
old generic guesses stay absent. The controlled Cursor and Grok real-process
profiles were corrected as well; the earlier expected receipts had encoded the
wrong behavior and therefore could not be treated as parity evidence.

The audit also corrected a source-reading error: `PlanUpdated`, subagent task
bookkeeping, and `SessionEnd` in `CodexEventAdapter.swift` belong to the
adjacent **Small Harness** adapter, not Codex. Codex's installed source hook set
remains the eight events in its launch bootstrap. Linux now exposes a real
`--adapter=small-harness` normalizer with Small Harness identity while GH-47
continues to own its managed launcher. The machine-readable contract separates
implemented event ingestion from that launcher dependency.

## Durable identity-based task bookkeeping

Claude `TaskCreated`/`TaskCompleted`, Cursor subagent, Droid task, and Small
Harness subagent hooks cannot safely be reduced as blind counters: duplicates
and completion-before-creation otherwise inflate or corrupt progress. The
canonical reducer now owns an identity-keyed task registry per pane and
session. Duplicate start/completion is idempotent, completion-before-start is a
completed task, cross-pane keys remain isolated, and an explicit
`task.progress` snapshot remains authoritative over later counter-style hooks.
Task lifecycle events require non-empty session and task identity; source hooks
without stable identity retain lifecycle state but cannot fabricate progress.

The same registry and its projected progress are serialized only in the
existing validated agent restore draft and restored through the existing
`AgentStatusStore`. No hook-side state file or second agent authority was
created. The persistence fixture proves a 1/2 task state survives application
reconstruction and the remaining task completes to 2/2 afterward. Hostile
launch arguments and environment remain excluded from that draft.

The first all-target core run inside the restricted execution sandbox again
hit `EPERM` when a pre-existing test created a real Unix socket. The unchanged
suite passed with its required real socket outside the sandbox. This was
recorded rather than converting environmental absence into a pass.

The first corrected all-adapter X11 run then failed for a useful reason: the
Grok controlled profile used `shell`, but Linux's still-shared question helper
classified shell as an approval tool for every dialect and suppressed the
event. Grok's source only suppresses its ask tool. Narrowing that one branch to
Grok's question-name predicate repaired the real wrapper -> PTY -> helper ->
socket -> sidebar journey; the rerun passed with native source behavior for all
currently staged adapters.

The existing consolidated session-restore journey was extended rather than
adding another restart harness. Its checked-in snapshot starts Codex at 1/2
with two identity-bearing tasks; the real resumed child sends the remaining
canonical completion; the sidebar reaches 2/2; clean persistence restarts the
app at 2/2; and replaying the completion remains idempotent. The first run's
expectation incorrectly said a task-only event changes `Starting` to `Running`;
the protocol intentionally says task events do not change lifecycle, so the
receipt was corrected to `Starting 2/2`. The second run then exposed a harness
assertion that demanded the original 1/2 state even after the clean snapshot
had correctly persisted 2/2. Restricting that assertion to the initial launch
made the full restart, inactive-worklane, physical-input, clean/crash/corrupt
journey pass. Both failures were evidence errors; neither was hidden or
converted to a product pass.

## Late events, exact aliases, and mutation closure

An adversarial late-event case exposed one more reducer defect: after an
authoritative `session.end`, a delayed task event could recreate that session
because the ordinary reducer creates a status on first observation. The
existing `AgentStatusStore` now retains an ended-session tombstone scoped to
the pane and session. Late events are ignored, an explicit new
`session.start` permits deliberate identity reuse, and pane removal or transfer
cleans or moves the tombstone with the rest of the single projection. No
second lifecycle registry or persistence file was introduced.

The first focused adapter mutation run was executed outside the restricted
sandbox because its unmutated core baseline creates a real Unix socket. It
tested 25 mutants and reported 15 caught, one compiler-unviable, and nine
survivors. Eight survivors showed that Codex's documented positional/kebab
aliases were implemented but not independently asserted; the ninth showed
that a zero-total Small Harness progress update did not have an exact boundary
test. Red-focused assertions now pin every alias and prove zero-total progress
retains running state without manufacturing progress. The narrowed rerun
caught all 18 selected mutants.

Two additional governed mutation runs covered the durable reducer and wire
identity rather than stopping at normalization. The task registry/restore run
caught all 11 selected mutants. The protocol run initially found that the
maximum accepted wire size and the first rejected byte were not distinguished:
an oversized whitespace payload failed as invalid JSON under either mutant,
so it was false evidence for the size guard. A valid canonical event padded to
the exact ceiling now passes and the same event one byte larger must return
`RequestTooLarge`. The corrected protocol run tested 13 mutants: 12 were
caught and one was compiler-unviable.

The adapter audit also found that accepting `event` and `type` keys globally
would silently broaden Claude, Codex, Gemini, Cursor, Droid, Kimi, and Vibe
beyond their source hook contracts. Only the source families that actually
use those keys retain them; a negative Claude regression pins that boundary.
Small Harness is now exercised through the staged helper and a real PTY/app
journey with source-shaped plan and subagent events. Its managed executable
bootstrap remains a GH-47 dependency. GH-47's issue body originally omitted
Small Harness despite the machine-readable contract assigning it there; the
issue was corrected before GH-46 completion so the remaining launcher cannot
disappear silently.

The first post-refactor adapter mutation run then found four additional
survivors. Moving Small Harness and Droid task mapping into focused helpers had
left the tests proving eventual projected progress without proving whether the
source aliases normalized to `task.started` versus `task.completed`. A new
normalization test pins both identities for both adapters. The narrowed rerun
tested nine mutants, caught seven, and rejected two as compiler-unviable. The
post-refactor task projection run caught all 16 selected mutants.

Strict Clippy review found that the first implementation had pushed three
adapter functions beyond the repository's function-size policy and had made
the compatibility lifecycle enum unnecessarily large. The adapters were
partitioned into source-specific lifecycle/notification/task helpers, task
projection was extracted from the canonical reducer without creating another
authority, restored task arguments were grouped into one value, and the
lifecycle event is now boxed at that parser boundary. Core and IPC strict
Clippy pass. The workspace-wide Clippy command still reports unrelated
pre-existing hotspot warnings in Linux application, test-support, and older
workspace-state test code; those were not hidden by adding broad allowances to
this feature.

The first complete local qualification run executed every presently runnable
cell and finished with 151 observed passes, one observed failure, one expected
failure, five blocked prerequisites, and fifteen not-implemented cells. The
single failure was `agent-fleet-x11`. Its raw log proved the new 2/5 progress
reached both sidebar and fleet projections, but the journey still expected two
agents because it tried to resurrect a deliberately ended `fleet-waiting`
session by sending `agent.needs-input` without a new `session.start`. That old
fixture assumption contradicted the new late-event contract. The existing
multi-window actor now sends an explicit start before deliberately reusing the
session identity. The exact controlled D-Bus/X11 cell then passed, including
waiting, progress, completion, compaction, idle, stopped cleanup, and exact
real-PTY routing. The failed qualification receipt remains recorded rather
than being relabeled as environmental absence or a pass.

The corrected full rerun passed every presently executable support test and
matrix cell in 827.1 seconds. The authoritative declared totals remain 152
PASS, 0 FAIL, 5 BLOCKED, 1 XFAIL, and 15 NOT_IMPLEMENTED. Therefore the exact
claim is **implemented local suite passed**; release qualification and full
Linux qualification remain **not passed**. The Debug Valgrind cell is PASS
with reviewed suppressions through the accepted suppression-governance gate;
this is not an unsuppressed-clean claim. The XFAIL and unimplemented cells were
not broadened or relabeled to close GH-46.
