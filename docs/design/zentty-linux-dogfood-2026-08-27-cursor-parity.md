# Zentty Linux dogfood: Cursor parity (GH-128)

Date: 2026-08-27

## Scope

Complete the source-defined Cursor agent integration through the existing Linux
launcher, adapter, status store, restore path, and `rust-agent-ipc` product
journey. The operator's installed and running Zentty instance must remain
untouched throughout development.

## Source audit

- The macOS source installs ten Cursor hook events. `preToolUse` and
  `postToolUse` are restricted to `TodoWrite`; the other eight events are
  unfiltered.
- Eligible launches use an isolated `.cursor/hooks.json` overlay and set
  `CURSOR_CONFIG_DIR`, while the explicit installer remains available for
  persistent user configuration management.
- Session identity accepts snake-case and camel-case conversation/session keys.
- Cursor UUID restore is `cursor-agent --resume=<lowercase-uuid>`.
- The source adapter contains a five-attempt, 50 ms transcript polling loop.
  Linux will not port this timing-dependent behavior. Hook payloads remain the
  authoritative event boundary; any later transcript fallback must be one
  bounded read with an explicit miss.
- The shipping macOS launch path still auto-installs persistent Cursor hooks;
  its real agent-bench path instead builds an isolated per-launch overlay.
  GH-128 deliberately adopts the safer overlay outcome on Linux while retaining
  explicit persistent install/uninstall commands.

## Existing Linux state

- Cursor recognition, wrapper discovery, marked persistent hook installation,
  basic canonical lifecycle adaptation, TodoWrite snapshot counts, and generic
  task identity events already existed.
- Cursor still used the generic persistent launch plan, did not create a
  per-launch overlay, and had no restore command.
- Existing product coverage exercised only a synthetic adapter error, not the
  generated hook file through the real wrapper and UI.

## Red evidence

Command:

```text
cargo test -p zentty-core --test agent_launch --test workspace_recipe
```

Result: expected compile failure. The new contract imports
`zentty_core::build_cursor_hooks`, which did not exist. The tests also require
private-overlay projection and strict UUID restore before implementation.

The first wrapper integration run then failed because the existing generic
persistent launch path rewrote the test user's `~/.cursor/hooks.json`. This
proved the launch mutation directly before the overlay repair.

## Discoveries, failures, and repairs

This section is updated while the issue is implemented.

- Added a focused Cursor launch module and strict UUID restore through the
  existing abstractions.
- Eligible Cursor launches now build a private mode-0700 `.cursor` overlay,
  project non-hook user state without copying it, write mode-0600 managed hooks,
  and avoid automatic mutation of the user's persistent hook file.
- Cursor TodoWrite partial updates could not be represented truthfully by the
  aggregate-only protocol without the macOS file-backed side store. Added a
  generic `task.snapshot` event whose identities and merge policy converge in
  the existing `AgentStatusStore`; no Cursor-only state subsystem was added.
- TodoWrite hook events now publish Running before task state, accept array and
  checklist forms, preserve workspace-root/transcript identity, and retain the
  final task projection when an authoritative Cursor stop becomes Idle.
- The old wrapper test expected malformed persistent Cursor hooks to disable
  integration. That expectation became stale: isolated launch hooks no longer
  parse or rewrite the user's hook file. The repaired test requires an
  integrated private overlay and byte-identical malformed source preservation.
- The first nested-X11 journey reached the correct final `Idle 2/4` sidebar
  state and logged every canonical event, but the runner also required an
  intermediate `Running 2/4` GTK frame. The real event burst was correctly
  coalesced into its final render. Requiring that transient frame would demand
  artificial sleeps, so the repaired deterministic assertion requires the
  authenticated Running and task-snapshot receipts plus the final Idle 2/4 UI.
- The next journey proved those events and final UI, but its controlled Cursor
  process exited before clean-exit persistence. The real lifecycle sweep then
  correctly removed the dead session, leaving no restore draft. The actor now
  blocks on an owned FIFO after its completion receipt while the outer harness
  closes the real window; this makes persistence occur while the resumable
  session is genuinely live without arbitrary sleep-based choreography.
- The FIFO rerun still published `drafts=` while Cursor was alive. Source trace
  showed that `PaneRestoreDraft::resume_command` supported Cursor but
  `WorkspaceState::agent_restore_draft_for_pane` omitted Cursor from its
  resumable-tool projection. This is a real close/restart data-loss defect, not
  an environmental absence or harness failure.
- Added Cursor to the existing restore-draft projection. The draft is still
  accepted only when the shared strict UUID validator can build the safe resume
  command; no second restore path or permissive fallback was introduced.

## Evidence

- Core agent launch suite: 26 PASS.
- Core agent adapter suite: 27 PASS.
- Canonical agent status suite: 35 PASS.
- Workspace recipe suite: 6 PASS.
- Focused workspace-state Cursor restore projection: PASS.
- Cursor wrapper private-overlay tests: 2 PASS; the unrelated authenticated
  OpenCode Unix-socket test passed when rerun outside the restricted sandbox
  after its sandboxed run returned `EPERM`.
- ReleaseSafe staged build: PASS. Cargo publication-age audit covered 91
  packages with 0 exceptions; package notice catalog covered 104 entries.
- Controlled nested-X11 Cursor product journey: PASS, session receipt
  `a8e5e88a05dcfa5206133942e355ebd3e78f51b2af67de459e4e772dd82f5ca3`.
  The journey used the staged wrapper, real PTY process, generated hook file,
  helper process, authenticated Unix socket, Linux product, Ghostty surface,
  and sidebar. It rejected the bad-token control and published durable UUID/CWD
  restore state on real window close.
- Authoritative inventory after reconciliation: 54 IMPLEMENTED, 0 PARTIAL,
  9 NOT_IMPLEMENTED.
- Complete affected crate suites: PASS. `zentty-core` and
  `zentty-agent-ipc` completed with no failures; the Linux application target
  completed 343 PASS, 0 FAIL, and 2 pre-existing intentionally ignored tests.
- Strict Clippy across all targets in `zentty-core`, `zentty-agent-ipc`, and
  `zentty-linux`: PASS with `-D warnings`. The first run rejected a 119-line
  Cursor adapter function; event-specific stop and TodoWrite projection were
  extracted into focused helpers rather than suppressing the lint.

## Remaining limitations

- A live Cursor account/model request is outside GH-128. Cursor is the
  controlled external-agent boundary; the staged wrapper, generated hooks,
  process, PTY, authenticated socket, product, Ghostty, and sidebar remain real.
