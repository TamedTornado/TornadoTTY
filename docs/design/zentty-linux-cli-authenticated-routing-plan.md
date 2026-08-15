# Zentty Linux authenticated CLI routing plan

Date: 2026-08-15
Owner: GH-43
Parent: GH-22

## Outcome

Qualify the source CLI's authenticated discovery and control path against the
real staged Linux product. A command must identify one running instance by its
private socket and one canonical pane capability; client-supplied topology
claims must never redirect that capability. GUI and CLI actions must converge
on the existing `ApplicationCoordinator`, `ApplicationShell`, workspace store,
notification service, and development-server runtime rather than introduce a
parallel topology or focus authority.

## Existing authorities to preserve

- `AgentRuntime` owns one random private runtime directory, instance socket,
  instance identifier, and pane-token registry per process.
- `PaneTokenRegistry` maps a capability to the canonical
  window/worklane/pane target and rejects missing, unknown, and retired tokens.
- `AgentIpcServer` owns bounded framing, authentication, and delivery to the
  application thread.
- `ApplicationCoordinator` owns cross-window discovery and routing.
- `ApplicationShell` and `WorkspaceState` own pane/worklane mutation and focus.
- Existing notification and development-server services remain the only
  authorities for their respective commands.

No new daemon, discovery database, topology cache, persistence writer, or
focus model may be added for this issue.

## Test construction order

1. Extend the existing transport tests, not a new harness layer, to cover
   split/partial writes, concurrent authorized and unauthorized clients,
   canonical routing despite forged claims, stale tokens, request bounds, and
   independent sockets.
2. Extend the existing staged CLI journeys to prove token omission by default,
   authenticated explicit disclosure, environment/resource correctness in
   real child panes, inactive-worklane and cross-window targeting, stale
   selector/token failure, restart/token rotation, and visible GUI/PTY effects.
3. Run the same representative real-product command path under controlled X11
   and Wayland. Environmental absence is never a pass.
4. Add or change product code only when a failing test demonstrates a concrete
   source-contract or safety gap. Preserve the authorities above.
5. Run focused unit/integration tests, mutation testing for changed pure Rust
   routing code, both controlled-display cells, and every presently executable
   qualification-matrix cell before commit.

## Required assertions

### Endpoint and environment

- runtime directory mode `0700`, socket mode `0600`, socket type verified;
- distinct instance socket and 256-bit pane capability after restart;
- every real pane receives the staged CLI path, live socket, instance/window/
  worklane/pane IDs, capability, hook command, shell integration resources,
  and source-compatible tmux trace variables when enabled;
- closing a pane retires its token and process shutdown removes its socket.

### Discovery and mutation

- discovery never mutates focus and omits capabilities unless
  `--include-control-token` is explicit;
- only an authenticated request reaches product dispatch;
- the server derives the target from the token, ignoring forged environment
  claims;
- explicit selectors must agree with that authenticated target;
- stale IDs, retired tokens, wrong-instance tokens, and missing windows fail
  without changing the currently focused pane/worklane/window;
- pane focus/close/zoom/resize/title and worklane rename/color/select visibly
  traverse the same methods used by GUI actions;
- notification and server commands enter their existing shared services.

### Recovery and hostile input

- malformed JSON, unsupported versions/routes, missing tokens, oversized
  frames/arguments, partial writes, stalled writers, and handler timeouts have
  deterministic bounded failures;
- authorized traffic remains usable after hostile or stalled clients;
- concurrent clients cannot cross instances or redirect another capability;
- stale discovery from a stopped instance cannot reach a replacement process.

## Completion gate

GH-43 closes only after the protocol, staged CLI, real Ghostty/PTY, X11,
Wayland, matrix, and dogfood receipts are attached to the issue. No claim of
full Linux qualification is permitted while the authoritative matrix contains
BLOCKED, XFAIL, or NOT_IMPLEMENTED cells.
