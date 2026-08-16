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
