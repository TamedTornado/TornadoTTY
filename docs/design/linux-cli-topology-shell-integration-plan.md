# Linux Zentty CLI, topology, and shell-integration closure plan

- **Tracking:** GH-22
- **Status:** Complete. GH-42 through GH-49 and cross-epic dependencies GH-9,
  GH-10, GH-14, and GH-32 are closed; the two GH-22-owned inventory features
  are implemented and backed by existing controlled real-product journeys.
- **Source authority:** `ZenttyCLI/`, `docs/cli.md`, `PaneIPCHandler`, and
  `DiscoveryIPCHandler` in the retained macOS source

## Outcome

Ship the source Zentty command-line companion as a Rust executable. A process
inside a real Zentty terminal can discover and control the running product's
windows, worklanes, panes, layouts, notifications, development servers, and
agent integration through the one existing private authenticated Unix socket.
The CLI is not another workspace model, PTY multiplexer, GUI automation layer,
or test facade.

## One-system design

1. `AgentRuntime` remains the only instance/socket/token authority and continues
   to inject pane identity into real Ghostty child environments.
2. `WorkspaceState` remains the only topology aggregate. CLI mutations invoke
   the same `ApplicationCoordinator`/`ApplicationShell` operations as GTK
   actions and the already-delivered tmux compatibility path.
3. The existing `zentty-agent-ipc` transport gains bounded `discover` and
   `pane` product routes. It does not gain another listener or background
   runtime.
4. Discovery is read-only and redacts capability tokens by default. An explicit
   token request is authenticated and bounded to the caller's product instance.
5. Cross-pane mutation requires the selected pane capability. A caller's token
   may always address its authenticated pane; a distinct target must present
   that target's capability rather than relying on active-window state.
6. Application-level operations (all-window discovery and new-window grids)
   stay in `ApplicationCoordinator`; window-local mutations delegate to the
   owning shell. No product policy moves into Ghostty.
7. Shell integration is staged data sourced by the real shell process. It
   injects context and prompt-time CWD/activity reporting without editing user
   dotfiles or printing control noise into the terminal.

## Source-compatible command surface

- `version`
- `list`, `list windows`, `list worklanes`, `list panes`
- aliases `window list`, `worklane list`, and `pane list`
- `select pane`
- `split`, `hsplit`, `vsplit`
- `grid` with exact dimensions, destination, focus, and optional command
- `pane focus|rename|close|zoom|resize`
- `worklane rename|color`
- `layout`
- `theme`
- the already-owned `server`, `notify`, agent-hook, launch, and internal tmux
  commands remain on the same executable

Linux-specific unsupported source commands must fail explicitly; they may not
silently succeed or disappear from help and qualification.

## Test construction order

1. Add parser golden tests from the Swift vocabulary and negative tests for
   ambiguity, bounds, malformed dimensions, conflicting selectors, unknown
   commands, and output/exit-code stability.
2. Add transport tests before the server route: version mismatch, oversized
   request/reply, missing/forged/stale token, response timeout, shutdown, socket
   replacement, and redacted versus authorized discovery.
3. Add pure target-resolution and topology-plan tests before GTK wiring,
   including multi-window ambiguity and transactional grid rollback.
4. Extend the existing staged-product actors. Invoke the actual `zentty`
   subprocess from separate real Ghostty PTYs and prove resulting GTK topology,
   exact focus, terminal input, CWD/command launch, persistence, and cleanup.
5. Repeat representative mutating flows under controlled X11 and Wayland. Use
   physical input for the resulting focus assertion rather than log-only proof.
6. Source staged Bash, Zsh, Fish, and Nushell integrations in real processes.
   Missing local shells remain explicit BLOCKED cells, never passes.
7. Mutation-test parsing, authorization, selector resolution, grid planning,
   and output dispatch with the repository's copy-safe mutation policy.
8. Run focused checks continuously and full local qualification once after the
   final implementation diff review. The child-issue qualification already
   satisfied this gate; the later ledger-only epic closeout does not rerun the
   entire application matrix.

## Closure criteria

- Every GH-22 acceptance criterion is backed by a named unit, contract, or
  real-product matrix cell.
- The staged bundle contains the GUI, `zentty` CLI, tmux shim, shell files,
  themes, terminfo, wrappers, and no source-tree dependency.
- The effective socket permissions, request bounds, tokens, selectors, and
  response errors are machine-tested.
- No duplicate topology, terminal, socket, persistence, or test runner exists.
- Dogfood records every source discovery, red failure, repair, receipt, and
  remaining environmental limitation.
- GH-22 closes only after every presently executable cell passes. Full Linux
  qualification is not claimed while unrelated non-PASS cells remain.

## Closure evidence

The machine-readable source contract contains all 40 source CLI commands: 39
are `IMPLEMENTED` and the macOS-only blur command has the documented Linux
`PLATFORM_ALTERNATIVE`. Hidden `ipc` accepts and transports every inventoried
source form; the separate agent-event contract remains authoritative for its
explicit downstream launch-context persistence boundary.

The authoritative matrix contains PASS cells for the full real staged CLI on
controlled X11 and Wayland, four standalone staged real-shell contracts, and
real Ghostty-pane Bash, Zsh, Fish, and Nushell journeys on both compositors.
All eight GH-22 children ran their focused unit, negative, mutation, transport,
and real-product evidence before closure. The 2026-08-20 epic reconciliation
uses those existing actors and does not add another socket, topology model,
shell layer, or test harness.
