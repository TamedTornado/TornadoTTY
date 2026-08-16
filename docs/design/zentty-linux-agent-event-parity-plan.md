# Linux agent-event parity and bookkeeping plan

- **Tracking:** GH-46, child of GH-22 and related to GH-14
- **Status:** source audit and red-contract construction
- **Source authority:** `AgentStatusCommand`, `AgentSignalCommand`,
  `AgentEventBridge`, every source event adapter, `PaneAgentReducer`,
  `AgentStatusPayload`, `AgentIPC`, and `docs/agent-status-protocol.md`

## Outcome

Complete the source-compatible agent event path from the staged CLI and
installed wrappers through the existing authenticated socket into the one
`WorkspaceState` agent reducer, visible sidebar/inbox/fleet projections, and
existing restore-draft persistence. Do not add a second agent store, transport,
launcher, product actor, or model-server layer.

## Initial audit findings

1. Linux already has one authenticated socket, pane-scoped capabilities, a
   versioned canonical event envelope, eleven adapter normalizers, one
   `AgentEventCoordinator`, one `AgentStatusStore`, and real sidebar, inbox,
   fleet, sleep-inhibition, title, progress, lifecycle-sweep, and Codex
   transcript consumers. GH-46 is completion of that system, not permission to
   replace it.
2. Canonical Linux events already cover session start/end, running, idle,
   needs-input, input-resolved, task progress, session parent identity, PID,
   stop-candidate grace, interaction type/text, transcript path, and
   concurrent sessions per pane.
3. The source CLI additionally exposes compatibility `agent-status` and the
   lifecycle/agent-PID forms of `agent-signal`. GH-45 deliberately implemented
   only shell-state, pane-root-PID, and pane-context. The remaining CLI forms
   must reduce through the canonical store rather than mutate sidebar state.
4. The published canonical protocol documents artifact and working-directory
   objects, but the current Rust wire event silently ignores them. This is a
   product gap and must receive an explicit model/reducer/UI or persistence
   decision; it may not disappear as an unknown serde field.
5. Parent session IDs and per-session progress are retained in the canonical
   store, but the source hierarchy, duplicate/reordered-event policy, and
   restart/restore behavior still need an expectation-by-expectation audit
   before the issue can claim durable subagent bookkeeping.

## Test-first order

1. Produce a machine-readable source-to-Linux table for every canonical event,
   CLI compatibility form, adapter and alias, required identity, reply, state
   transition, ordering rule, persistence effect, and visible projection.
2. Add red focused contracts for missing lifecycle `agent-signal`,
   `agent-status`, artifact/context fields, clear/attach PID semantics,
   duplicates, reordering, conflicting sessions, child sessions, stale
   targets, and restart boundaries.
3. Extend the existing canonical event types and `AgentStatusStore`; do not
   introduce another reducer. Preserve bounded input, strict version/event
   validation, capability-authenticated target canonicalization, and exact
   adapter acknowledgements.
4. Extend the existing controlled agent and `rust-agent-ipc` product journey.
   Use real staged CLI/wrapper processes, the real private socket, real app,
   real pane/PTY, and visible sidebar/inbox/fleet plus persisted restore state.
   Fake only the external model response where an installed agent requires it.
5. Exercise every currently supported adapter, duplicate/reordered/late input,
   inactive and moved panes, process death, clean restart, crash restart, and
   hostile text. Environmental absence remains BLOCKED.
6. Mutate identity correlation, event/version gates, transition/ordering rules,
   progress bounds, PID death, hierarchy, expiry, and acknowledgements inside
   the resource-isolated mutation runner.
7. Reconcile the CLI source contract, feature inventory, matrix, issue
   acceptance criteria, and dogfood evidence. Run every presently executable
   cell only after the feature-owned gates pass.

## Acceptance boundaries

- One pane can host multiple identified parent/child sessions without one
  session erasing another; the source priority rule selects presentation.
- Duplicate, reordered, late, conflicting, unknown, cross-pane and stale-token
  events have deterministic tested outcomes.
- Task progress, attention, approval/question details, PID/crash state,
  artifacts, working directory, transcript and launch/restore identity survive
  exactly as the source requires—no more and no less.
- Agent integration and wrapper config changes continue through their existing
  consent/config owner and are never triggered by unauthenticated input.
- Receipts contain hashes/counts and synthetic fixture text only; pane tokens,
  user prompts, transcripts, and ambient secrets never enter logs or commits.

## Closure

GH-46 closes only when the complete source table has no silent row, real staged
journeys prove the visible and persistent outcomes, mutation has no unreviewed
viable survivor, the full executable matrix passes, and the report remains
honest about every BLOCKED, XFAIL, and NOT_IMPLEMENTED cell.
