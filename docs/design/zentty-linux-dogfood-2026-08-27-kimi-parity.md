# Kimi parity dogfood record — 2026-08-27

## Scope and operating constraint

- Issue: GH-130, child of GH-7.
- Complete modern and legacy Kimi probing, hook/config ownership,
  interactions, status, and safe restore parity.
- Jason is actively using installed Zentty. All work remains staged or nested;
  do not deploy, install, launch, stop, overwrite, or otherwise disturb the
  active application.
- Do not run full Linux qualification for this issue-sized feature.

## Source authority

- `Zentty/AppState/Agent/KimiHooksInstaller.swift`
- `Zentty/AppState/Agent/EventAdapters/KimiEventAdapter.swift`
- `Zentty/AppState/Agent/AgentLaunchBootstrap.swift`
- `Zentty/Restore/SessionRestoreStore.swift`
- corresponding macOS logic and wrapper tests

## Existing Linux state

- The shared installer can place seven managed hook entries in legacy and
  modern config roots and preserve basic inline-array or array-table style.
- The wrapper recognizes Kimi, supplies PID/tool identity, excludes known
  passthrough commands, and invokes the existing adapter.
- The adapter covers basic lifecycle, permission notifications, question and
  approval hooks, and resolution.
- The generic product harness currently injects one synthetic WriteFile event.
- The inventory correctly remains NOT_IMPLEMENTED: Linux does not yet probe
  modern versus legacy Kimi, construct the legacy private config overlay,
  protect modern lexical session lookup from stale runtime homes, repair stale
  session-index paths, emit exact source approval text/CWD aliases, support the
  two validated restore forms, capture Kimi restore drafts, or execute its real
  installed hook command in a focused product journey.

## Design constraints

- Modern Kimi must use its real home because its session lookup compares path
  prefixes lexically; even a symlinked overlay breaks session resume.
- Legacy Kimi may use a private per-launch `--config-file` overlay so user
  configuration is preserved without persistent launch-time mutation.
- Persistent hook writes remain in the existing atomic integration layer. Do
  not add a Kimi status store, timer, poller, or second lifecycle reducer.
- Tests own private HOME/config roots. They must never mutate Jason's Kimi
  configuration or treat an unavailable external account/model as a pass.

## Discoveries, failures, and repairs

This section is updated as red tests expose behavior and implementation
repairs.

- Initial source comparison found that Linux's one persistent install path
  conflates modern and legacy launch behavior. The source probes bounded
  `--help` output: presence of `--config-file` selects legacy, while modern
  Kimi runs from its real home. Existing Linux tests do not exercise either
  branch through a real selected executable.
- Linux restore has no Kimi case. The source accepts only a bare UUID for
  legacy `kimi -r` and `session_<UUID>` for modern `kimi -S`, canonicalizing
  UUID case and rejecting every other session shape.
- The first exact adapter test failed because Linux produced the generic
  `Kimi needs your input for Shell` instead of the source command-specific
  approval text; it also did not project Kimi's CWD aliases. The first restore
  test failed because Linux returned no command for a valid legacy UUID. These
  red failures are retained as the reason for the focused adapter and restore
  changes rather than weakening the assertions.
- The first legacy launch test reached a private generated overlay but its test
  asserted a literal directory component named `kimi-` rather than the actual
  `kimi-<random>` basename. The assertion was repaired to check both the
  `agent-overlays` parent boundary and generated basename; no production path
  behavior was changed for that fixture error.
- The first complete launch test file had 23 passes and one sandbox denial in
  an existing OpenCode Unix-socket test. The same full file passed with the
  required socket permissions; the denial was not converted into a pass.
- The complete adapter file found an older Linux-only expectation for generic
  WriteFile wording. It was updated to the exact source string already enforced
  by the new red test; the source requirement was not weakened.
- The first staged product run delivered every Kimi event and final Idle state,
  but the controlled actor emitted Question, resolution, Approval, and Idle
  before GTK rendered an intermediate sidebar frame. The harness now uses two
  deterministic FIFOs: it advances only after the real sidebar logs Question
  and then Approval. No arbitrary delay is treated as correctness.
- Final installer review found that Kimi merged from one read transaction and
  published through a second atomic replace, leaving a lost-update window
  between concurrent processes. The merge now occurs inside one
  `AtomicFileStore` transaction; atomic publication alone was not accepted as
  equivalent to an atomic read-merge-write cycle.
- An initial eight-process installer stress test hit the shared atomic store's
  intentional bounded lock timeout in three processes; it failed explicitly
  without corrupting either config. The regression now models the actual
  two-launch race and still requires both installers to succeed plus exact
  user-content preservation and one managed block. Lock timeout was not
  broadened merely to make an unrealistic burst green.

## Evidence

- Complete affected test files: PASS (24 launch CLI, 8 integration CLI, 30
  adapters, 27 launch-policy, 8 restore-recipe, and 71 workspace-state tests).
- Strict Clippy across all targets in `zentty-core`, `zentty-agent-ipc`, and
  `zentty-linux`: PASS with `-D warnings` after final installer repair.
- ReleaseSafe staged build: PASS. Cargo publication-age audit covered 91
  packages with 0 exceptions; package notice catalog covered 104 entries.
- Controlled nested-X11 Kimi product journey: PASS, receipt
  `0d05903d108d8561f428059b2315d5dd269aec37bd81acf8a0f3517787782179`
  against the final rebuilt staged binary.
  It ran both modern persistent-home and legacy private-overlay variants through
  the staged wrapper, real PTY children, actual installed hook commands, helper,
  authenticated Unix socket, Linux product, Ghostty surface, deterministic
  visible Question/Approval states, bad-token controls, real window close, and
  distinct durable modern/legacy resume drafts.
- Authoritative inventory after reconciliation: 56 IMPLEMENTED, 0 PARTIAL,
  7 NOT_IMPLEMENTED.

## Remaining limitations

- A live Kimi account/model call is expected to remain the controlled external
  boundary; executable probing, config/hook handling, wrapper, PTY, IPC,
  product presentation, and durable restore must be real.
