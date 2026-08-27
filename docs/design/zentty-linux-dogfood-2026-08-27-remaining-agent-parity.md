# Dogfood record: remaining coding-agent parity

Date: 2026-08-27

## Scope discovery

The authoritative feature inventory still classified five coding-agent cells
as `NOT_IMPLEMENTED`: Grok, Antigravity, Hermes Agent, Mistral Vibe, and the
shared explicit custom-agent protocol. The hook installers and event adapters
already existed in the Linux product, but that was not sufficient to claim
parity. Source review found these missing or unqualified contracts:

- the source-compatible restore command for each resumable tool;
- authenticated Hermes launch arguments and `HERMES_HOME` preservation;
- Hermes' source-owned PTY title glyph presentation;
- proof that the installed hook files and generated commands actually execute;
  and
- proof that an unknown explicit agent name survives the canonical store and
  visible sidebar without being guessed as a known tool.

The repair uses the existing adapter, helper, socket, restore, and product-test
systems. It does not add a second agent protocol or another integration-test
layer.

## Source-compatible repair

- Grok accepts a source-compatible safe session identifier and builds
  `grok --resume ID`. Its source fallback to `grok --resume` is retained only
  when the recorded working directory still exists.
- Antigravity builds `agy --conversation ID` for a safe conversation identity
  and retains the source `agy --continue` fallback for absent or placeholder
  identities.
- Hermes removes `chat`, prior resume arguments, and one-shot query flags from
  its authenticated launch snapshot before appending `--resume ID`. A nonblank
  authenticated `HERMES_HOME` is preserved without persisting arbitrary
  environment state.
- Mistral Vibe builds `vibe --resume ID` only for a validated UUID or safe
  opaque identifier.
- Restore construction rejects unsafe values rather than interpolating shell
  input.
- Hermes' canonical display identity is `Hermes Agent`, and its source title
  glyphs update only an already-authenticated Hermes session. Bare glyphs,
  unrelated tools, and panes without agent state are ignored. The status path
  covers Running, Needs Input, and Idle without allowing a title to invent
  durable agent identity.
- The sidebar's deterministic diagnostic receipt now includes the canonical
  agent name. This makes the real custom-agent journey observable without
  exposing user prompt text.

## Real product qualification

The existing `linux/tests/rust-agent-ipc` harness gained one focused
`remaining-hooks` scenario. For each supported tool it launches the staged
ReleaseSafe application, a real Ghostty surface and PTY child, the staged
wrapper and helper, and the authenticated mode-0600 Unix socket. The child
audits and executes the hook material actually installed into a private home:

- Grok: nine hook groups and the generated executable hook script;
- Antigravity: eight hook groups and the installed notification command;
- Hermes: ten generated hook scripts plus real hook and PTY-title transitions;
- Vibe: three installed hook commands; and
- custom agent: an explicit `Review Bot` lifecycle through the same helper,
  socket, canonical fleet, and sidebar used by known agents.

The managed-agent runs remain alive while the real application window is
closed, then verify the exact tool name, session identity, working directory,
and generated restore draft in the durable snapshot. Focused model tests prove
the exact resume invocation and rejection boundaries. External model/API calls
remain the only controlled boundary.

Controlled X11 receipt:

```text
Rust agent launch passed: x11, scenario=remaining-hooks, wrapper=real-process, child=real-pty, helper=real-process, transport=real-unix-socket, compatibility-agent-status=running+needs-input+failed+task-idempotency+reordered-completion+completed+clear, adapters=custom-explicit+grok-real-hooks+agy-real-hooks+hermes-real-hooks-and-title+vibe-real-hooks, real-gemini=prerequisite-not-requested
```

Controlled input-capable Wayland receipt:

```text
Rust agent launch passed: wayland, scenario=remaining-hooks, wrapper=real-process, child=real-pty, helper=real-process, transport=real-unix-socket, compatibility-agent-status=running+needs-input+failed+task-idempotency+reordered-completion+completed+clear, adapters=custom-explicit+grok-real-hooks+agy-real-hooks+hermes-real-hooks-and-title+vibe-real-hooks, real-gemini=prerequisite-not-requested
```

The Wayland receipt is also preserved locally at
`/tmp/zentty-remaining-hooks-wayland.log` for this development run.

## Failures and corrections

1. The first Hermes journey emitted its Running title before a session-start
   hook had authenticated Hermes identity. The reducer correctly ignored it.
   The fixture was corrected to execute the installed session-start hook before
   emitting the title; product code was not weakened to trust an unauthenticated
   title.
2. An ordinary nested-Wayland attempt exercised product behavior but failed
   teardown because it was not the private input-capable session. That
   environmental absence was not converted into a pass. The scenario was
   rerun under `nested-wayland-input` and passed with real virtual input.
3. A sandboxed helper CLI run produced Unix-socket `EPERM` failures while all
   socket-free cases passed. The same helper and integration tests were rerun
   with their required local socket permission: 15 helper tests and 8
   integration tests passed. The sandbox failures were not relabeled as product
   passes.
4. Initially preserving authenticated launch snapshots for every agent changed
   established Codex restoration behavior. Preservation was narrowed to Hermes,
   the only source contract in this slice that requires it.
5. A final source comparison found that the Hermes title reducer must require
   content after the glyph. Bare glyphs and a variation-selector-only remainder
   are now rejected and regression tested.
6. Strict crate-wide Clippy initially found that the added restore branches had
   pushed two existing functions beyond the project's 100-line limit. The new
   policy was extracted into focused restore-command and launch-snapshot
   helpers; no lint suppression was added. Core and the changed Linux library
   target then passed with warnings denied. The complete Linux binary target
   still reports an unrelated 116-line `connect_surface_callbacks` function in
   the previously committed context-menu slice; that pre-existing finding was
   not concealed or bundled into this feature.

## Focused receipts and inventory result

- core adapter/status/restore/state tests: 149 passed;
- Linux library tests: 9 passed;
- staged helper and integration CLI tests: 23 passed;
- ReleaseSafe local build and seven-day Cargo publish-age audit: passed;
- controlled X11 and input-capable Wayland product journeys: passed.

The authoritative inventory moves from 56 `IMPLEMENTED` / 7
`NOT_IMPLEMENTED` to 61 `IMPLEMENTED` / 2 `NOT_IMPLEMENTED`. This record does
not claim full Linux qualification or live external-model qualification.
