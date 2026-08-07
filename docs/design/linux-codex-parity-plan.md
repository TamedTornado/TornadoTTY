# Linux Codex workflow parity plan

- **Status:** implemented; explicit hook/notify parity, bounded transcript recovery,
  source terminal-title classification, task progress, canonical
  title-to-sidebar reconciliation, physical user-submit stabilization, Ctrl-C
  interrupt suppression, shell-return cleanup, and title-driven asynchronous
  transcript enrichment/retry/cache application, OSC progress beyond title
  task counts, transactional resolver-state write-back, and a controlled
  completed turn through the installed Codex notify callback implemented
- **Date:** 2026-08-06
- **Owner:** [#7 — essential Zentty workflow parity](https://github.com/TamedTornado/zentty/issues/7)
- **Inventory ID:** `agent.codex`
- **Field record:**
  [`zentty-linux-dogfood-2026-08-01.md`](zentty-linux-dogfood-2026-08-01.md)

## Objective

Codex is a primary Zentty workflow. Linux must reproduce the behavior owned by
the checked-in Swift source rather than treating successful process launch as
feature completion. The finished path must cover discovery, wrapped launch,
hooks and notify callbacks, transcript-backed questions, terminal-title
reconciliation, sidebar attention, transactional resolver write-back, clean
interruption, and exact resume behavior using the real Codex CLI, Ghostty
surface, PTY, helper, authenticated Unix socket, reducer, and visible product
state.

Only the model endpoint may be controlled. Tests may not replace Codex,
Ghostty, the PTY, the wrapper, the helper, IPC, or product reducer with mocks
when claiming product integration.

## Source-owned behavior and current Linux gaps

The authoritative implementation is principally:

- `Zentty/AppState/Agent/EventAdapters/CodexEventAdapter.swift`;
- `Zentty/AppState/Agent/EventAdapters/CodexNotifyEventAdapter.swift`;
- `Zentty/AppState/Agent/CodexTranscriptQuestionExtractor.swift`;
- `Zentty/AppState/Agent/Status/CodexToolStatusResolver*.swift`;
- `Zentty/AppState/Agent/Presentation/Reducers/CodexTitlePromotionReducer.swift`;
- `Zentty/AppState/Agent/AgentLaunchBootstrap.swift`; and
- `Zentty/Restore/SessionRestoreStore.swift`.

The Linux implementation now proves real wrapped launch, all configured hooks,
notify adaptation, bounded transcript discovery, exact session persistence,
real `codex resume` relaunch, terminal-title status/task reconciliation,
physical Return promotion, physical Ctrl-C suppression, shell-return cleanup,
and delayed title-driven transcript question enrichment through the canonical
reducer. The real Ghostty OSC 9;4 action now also reaches the canonical reducer,
and skipped title transitions are proven not to write back partial state. It
The installed Codex 0.147.0 journey now completes a real Responses turn against
a deterministic loopback endpoint and observes Codex itself invoke the notify
callback before persisting and resuming the exact thread.

## Ordered delivery slices

### 1. Explicit hook and notify parity

Write failing source-pinned tests first, then:

- map all eight configured hook events without unsupported-event failures;
- parse question tools and option labels from `tool_input`, `toolInput`, and
  JSON string forms in `tool_args`, `toolArgs`, and `arguments`;
- add the source `codex-notify` command and adapter, including positional or
  stdin payloads, no-routing best effort, and debug-only transport errors;
- classify turn completion, approvals, decisions, generic input, and auth;
- ignore auto-approval lifecycle chatter exactly as the source does;
- inject `notify=[<staged helper>,"codex-notify"]` unless explicitly disabled
  or already overridden, preserving Codex subcommand config scoping; and
- unset only inherited Zentty-owned `CODEX_HOME` overlays, never a user's real
  Codex home.

Integration evidence must extend the installed-Codex journey so the real CLI
consumes the generated notify configuration and the real staged helper drives
both attention and turn-complete state through the real socket and sidebar.

### 2. Transcript-backed questions

Port the bounded 256-KiB JSONL tail reader, exact response/function-call
shapes, question/option formatting, session/CWD association, bounded recent
candidate search, cache identity, and malformed/symlink/non-regular-file
handling. Tests must use real files and preserve source size/count bounds.
Wire the result into Codex attention recovery rather than creating a parallel
status store.

### 3. Terminal-title and lifecycle reconciliation

Port the source classifier and resolver as a focused core state machine:

- status/spinner/project/task-progress parsing;
- action-required and ready-title promotion;
- stale notify/title ordering protection;
- user-input promotion and interrupt suppression;
- shell-return cleanup;
- OSC progress interaction; and
- transactional resolver-state write-back rules. This is **not** automatic
  pane renaming: the source discards local reducer mutations on skipped title
  transitions and commits them only at real transition points.

Feed it real Ghostty title/progress/input callbacks. Test event-order
permutations and then one consolidated real-product journey; do not add a
second Codex state path beside the canonical reducer.

### 4. Qualification and promotion

Run focused mutation testing with `gitignore=true` and `copy_target=false`,
then formatting, strict Clippy, workspace tests, ShellCheck, controlled X11
and Wayland product cells, the installed-Codex journey, session restore, and
every presently executable qualification cell. Record exact receipts and
remaining limitations. `agent.codex` may become implemented only when no
source behavior named above remains prose-only or untested.

## Stop conditions

- Do not modify Ghostty unless the missing behavior is proven to be owned by
  Ghostty's public embedding boundary.
- Do not persistently rewrite the user's Codex configuration or trust state.
- Do not infer notify compatibility after an installed Codex version change;
  review and repin it.
- Do not call Codex complete while transcript, title, interrupt, or write-back
  behavior remains absent.
