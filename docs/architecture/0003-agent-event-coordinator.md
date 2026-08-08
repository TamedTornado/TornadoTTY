# ADR 0003: One agent-event projection coordinator

- Status: Accepted
- Date: 2026-08-08
- Tracking: GH-29, parent GH-25

## Context

`AgentRuntime` owned the authenticated Unix socket and pane-token registry, but
also owned transcript-enrichment workers. `ApplicationShell` separately drained
agent events, tmux requests, and enrichment results and applied them to the
workspace. That split obscured cancellation and current-pane validation even
though there was only one physical transport.

## Decision

Each application window owns exactly one `AgentEventCoordinator`. It owns the
existing `AgentRuntime` and existing `CodexTranscriptEnricher`:

- `AgentRuntime` remains the sole authenticated transport, pane-token registry,
  helper environment, and typed event/tmux receiver owner.
- `zentty-core::WorkspaceState` remains the sole agent-status reducer and the
  sole authority for whether enrichment still matches a current session.
- The coordinator drains only on the GLib thread. It drops events for removed
  panes and retargets an already authenticated event when its stable pane moved
  to another worklane before the UI drain.
- Transcript workers receive immutable paths/candidates only. They never own or
  access GTK, Ghostty, `WorkspaceState`, credentials, or pane tokens.
- Every enrichment request has a monotonically wrapping generation and a
  cancellation flag. Completion requires the current pane generation and exact
  candidate; core performs a second session/status validation before applying
  the question.
- Pane removal cancels its enrichment. Window shutdown cancels every pending
  generation and permanently rejects new work. Workers use a fixed retry
  schedule and observe cancellation around every sleep, so teardown does not
  block and remaining worker lifetime is bounded by the current delay (at most
  600 ms).

No additional socket, event queue, status reducer, async runtime, or test
harness is introduced.

## Receipts and privacy

Receipts contain pane/worklane identity, event kind, and session identity needed
to diagnose routing. They must not contain pane tokens, credentials, prompt
bodies, transcript paths, or transcript contents. Persisted workspace state is
still produced solely through core's bounded public projection.

## Consequences

Transport shutdown remains `AgentRuntime`'s `Drop` responsibility. Enrichment
cancellation is explicit before widget/surface teardown, and `Drop` repeats it
idempotently. Installed-agent, real Unix-socket, real PTY, tmux, and restore
journeys remain the integration boundary; coordinator decision tests do not
replace them.
