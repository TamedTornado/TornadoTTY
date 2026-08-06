# Linux tmux-compatibility facade port plan

- **Status:** Phases 0–1 complete; Phase 2 authenticated IPC in progress; Phase
  3 discovery/select/send, capture/buffers, team split, kill/dissolve, and
  layout/resize vertical slices complete; remaining command classifications
  are in progress
- **Date:** 2026-08-05
- **Owner:** [#14 — Linux tmux compatibility and agent-team IPC](https://github.com/TamedTornado/zentty/issues/14)
- **Parent:** [#1 — production-quality Zentty Linux port](https://github.com/TamedTornado/zentty/issues/1)
- **Field record:**
  [`zentty-linux-dogfood-2026-08-01.md`](zentty-linux-dogfood-2026-08-01.md)

## 1. Product boundary

Zentty does not ship a second terminal multiplexer. Ghostty surfaces remain the
only PTY and terminal owners. The product ships a narrowly compatible `tmux`
executable for Claude Code agent teams:

```text
Claude Code child
  -> staged/installed tmux shim
  -> staged/installed zentty __tmux-compat CLI
  -> authenticated private Unix socket
  -> running Zentty instance
  -> existing worklane/pane/layout/input/capture operations
  -> existing Ghostty surface and PTY
```

The Linux implementation is Rust. It does not add a Rust-specific Ghostty API,
a daemon, another pane model, another session store, or an alternate PTY path.

## 2. Source authority

The machine-readable source contract is
`docs/design/zentty-tmux-compat-source-contract-v1.json`. It is derived from:

- `ZenttyResources/bin/tmux-shim/tmux`;
- `ZenttyCLI/TmuxCompatCommand.swift`;
- `Zentty/AppState/Agent/TmuxCompatArguments.swift`;
- `Zentty/AppState/Agent/TmuxCompatIPCHandler.swift`;
- `Zentty/AppState/Agent/TmuxFormatRenderer.swift`;
- `Zentty/AppState/Agent/TmuxCompatStore.swift`;
- `Zentty/AppState/Agent/AgentIPCProtocol.swift` and `AgentIPC.swift`;
- `Zentty/AppState/WorklaneSessionEnvironment.swift`; and
- every `TmuxCompat*` and shell-integration Swift test.

The contract names aliases, global options, subcommand options, output,
mutations, intentional no-ops, explicit failures, and currently silent unknown
commands. Linux may improve unsafe or misleading failure behavior, but any
departure must be classified as a Linux security/diagnostic correction rather
than silently presented as exact parity.

## 3. Ordered implementation

### Phase 0 — Freeze the source behavior before porting

1. Add the machine-readable source contract and a validator that rejects
   duplicate commands/aliases, unknown dispositions, missing source evidence,
   alias collisions, undocumented options, and untracked fixture IDs.
2. Record golden invocation/format/target/store fixtures before implementing
   their Rust equivalents.
3. Add the validator to the focused architecture/support gate.

**Exit:** the source vocabulary and every explicit no-op/failure are reviewable
without reading the Rust implementation. The validator must fail when a source
command disappears.

### Phase 1 — Pure Rust compatibility core, tests first

**Completed 2026-08-05.**

Create a focused `zentty-tmux-compat` library crate. It owns only pure logic:

- global and subcommand argument parsing;
- typed command representation;
- pane/window target selectors;
- send-key translation;
- tmux-format rendering;
- compatibility-store transitions and schema validation; and
- versioned request/result payloads.

Tests are table-driven from the frozen fixtures. Decision-heavy parser,
renderer, target, and transition code receives mutation testing. Unknown,
missing, duplicate, malformed, oversized, invalid UTF-8, and cross-scope input
must fail explicitly where the Linux security contract requires it.

All Rust mutation runs use `linux/tests/mutate-rust`. The checked-in
`.cargo/mutants.toml` and wrapper both require `gitignore = true` and
`copy_target = false`; this prevents cargo-mutants from copying ignored staged
Ghostty/build artifacts into every worker and avoids duplicating target trees.
Direct `cargo mutants` invocations are not a supported project workflow.

**Exit:** every frozen pure contract passes in Rust; deliberately changed
source behavior is documented; mutation survivors are either killed or
explicitly justified.

### Phase 2 — Extend the existing authenticated Agent IPC transport

Extend `zentty-agent-ipc`; do not create a second socket server. Add:

- versioned tmux-compat request and response frames;
- bounded stdin/stdout and argument counts/sizes;
- server-canonical routing from the pane capability token;
- response payloads and source-compatible exit mapping;
- peer/socket/path/symlink/stale-endpoint checks; and
- clean concurrent-client and shutdown behavior.

Start with negative transport tests, then add a real separate-process CLI to
socket round trip. Client-forwarded window/worklane/pane IDs are claims, never
authority.

**Exit:** malformed or incorrectly targeted requests cannot mutate a pane, and
the ordinary agent-event protocol remains green.

### Phase 3 — Product command handlers

Map typed commands onto the existing Linux workspace and `ApplicationShell`:

1. discovery/list/display and format output;
2. pane selection and source-compatible named/literal `send-keys`;
3. capture and buffers;
4. first team split as a golden-ratio right column;
5. later splits stacked vertically in that column while leader focus remains;
6. kill/dissolve with leader-width restoration;
7. layout/resize and worklane/window targeting; and
8. the inventoried intentional no-ops and explicit unsupported results.

Product actions remain ordinary product actions. No application-embedded test
scenario or test-only CLI option is allowed.

**Exit:** focused model tests and physical product journeys prove topology,
focus, terminal input, capture, teardown, and scope isolation.

### Phase 4 — Staging, discovery, and shell integration

Stage the `zentty` CLI and executable `tmux` shim without source-tree paths.
Expose the shim only when agent-team integration is enabled and no real tmux
session is already active. Port Bash, Zsh, Fish, and Nushell behavior using
XDG paths and without modifying user configuration.

**Exit:** the staged and installed products resolve the intended shim under the
documented conditions and preserve a real tmux installation otherwise.

### Phase 5 — Real agent-team product qualification

Run the same representative workflow under controlled X11 and input-capable
Wayland:

1. launch a real leader Ghostty terminal/PTY;
2. invoke the real shim and CLI as separate processes;
3. first split, second split, list/display, send named and literal keys;
4. capture real terminal output and buffer it;
5. select panes without leaking focus or state across worklanes/windows;
6. kill the subagents and restore the exact leader width; and
7. restart, corrupt, or substitute the endpoint/store and prove safe failure.

Only the external Claude/model response may be controlled. Receipts must prove
the shim, CLI, socket, server, product action, Ghostty surface, PTY, and child
process boundaries. Missing dependencies are not passes.

## 4. Store and wait-for corrections requiring explicit decisions

The source uses an in-process `NSLock` around load/modify/save, which does not
serialize separate CLI/app processes. It also places `wait-for` signal files
directly in `/tmp`. Linux must instead use a private runtime directory and a
cross-process synchronization mechanism. The design must cover permissions,
symlinks, stale state, atomic replacement, concurrent writers, schema/version
rejection, corruption quarantine or recovery, and cleanup.

These are Linux security necessities, not invented user features. Their tests
must be written before the store implementation.

## 5. Drift controls

- No source command may be implemented until it exists in the frozen contract.
- No command may return silent success unless the contract identifies it as an
  intentional source no-op and the Linux decision explicitly preserves it.
- No second socket, workspace model, terminal emulator, PTY owner, or session
  store may be introduced.
- No test-only behavior may be compiled into the product.
- No generalized GUI automation layer or hidden retry loop may be introduced.
- New Ghostty work requires a demonstrated Ghostty-owned defect and a separate,
  minimal upstream-reviewable change.
- Every discovery, failed assumption, repair, receipt, and residual limitation
  is recorded in dogfood as it occurs.

## 6. Publication gate

Each independently green phase may be committed. Before any product-completion
claim, run focused tests, complete workspace tests, strict Clippy, formatting,
ShellCheck for changed scripts, architecture/inventory/matrix validators, both
real compositor journeys, installed staging, and one final `qualify-local`.

Do not claim release or full Linux qualification while the matrix contains any
required FAIL, BLOCKED, XFAIL, or NOT_IMPLEMENTED entry.
