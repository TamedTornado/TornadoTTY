# Linux installed-Gemini qualification repair plan

Date: 2026-08-23
Tracking: GH-83

## Outcome

Make the real installed-Gemini product journey deterministic, explicitly
version-pinned, independently runnable, and incapable of hanging qualification.
This is a harness repair; it does not change Gemini or Zentty product behavior.

## Test-first order

1. Add a shared owned-process exit helper to the existing product-input support
   module. Its focused runner must prove that a live child reaches an explicit
   timeout instead of blocking and that an exited child is reaped with its real
   status.
2. Replace every raw `wait` in `rust-agent-ipc`, including EXIT cleanup, with a
   bounded owned-process wait. Preserve the relevant product, server, or receipt
   output when a deadline fails.
3. Preserve the existing lifecycle-confirmation policy when the OpenCode theme
   fixture rewrites appearance configuration, and assert the resulting file.
4. Add one scenario selector to the existing actor so the installed-Gemini
   journey can run alone without creating a second product harness.
5. Make the authoritative matrix name the repository-owned pnpm-installed
   Gemini executable and reviewed version explicitly. Ambient `gemini` lookup
   must not satisfy required qualification.
6. Run focused runner/contract checks, then the installed-Gemini scenario alone
   on controlled X11 and input-capable Wayland. Run `qualify-local` only after
   both isolated journeys pass.

## Boundaries

- The Gemini CLI, wrapper, GTK product, Ghostty surface, PTY, hooks, Unix socket,
  controlled loopback model endpoint, close input, persistence, and resume TUI
  remain real.
- Only the external model response and private desktop services are controlled.
- No detached watchdog, ambient CLI fallback, fake process receipt, duplicate
  Gemini actor, or relaxed version pin is permitted.
- A timeout is a failure with evidence, never a skip or successful cleanup.
