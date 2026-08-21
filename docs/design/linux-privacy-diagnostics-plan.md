# GH76 Linux privacy and diagnostics plan

## Product outcome

Zentty Linux is network-silent by default. Crash/support information is created
locally only when the user has enabled local crash capture or explicitly asks
for a support report. A report is redacted, bounded, stored privately, and
shown before any explicit submission. Production builds have no transport
unless a reviewed HTTPS endpoint is compiled/configured; absence of a transport
is visible and never reported as success.

## Ownership and non-goals

- `zentty-core` owns typed diagnostic state, strict payload construction, and
  deterministic redaction. It has no network or GTK dependency.
- `zentty-linux` owns the XDG-private atomic store, retention, optional fixed
  transport, panic-hook capture, and GTK review surface.
- The existing Updates & Privacy settings page and
  `linux/tests/rust-updates-privacy-settings` journey are extended. No parallel
  settings page, report store, event bus, or product harness is permitted.
- Terminal output, commands, agent prompts/results, complete environment,
  workspace recipes, IPC credentials, and network breadcrumbs are never report
  fields. Redaction is defense in depth, not permission to collect them.
- Automatic Sentry-compatible sessions, tracing, performance, network
  breadcrumbs, PII, and watchdog reports remain absent.

## Construction order

1. Add focused red tests for disabled defaults, typed local/pending/sent/failed/
   cleared states, allowlisted fields, every secret class, bounds, and consent.
2. Implement the pure model and Linux store/retention authority.
3. Wire explicit settings actions: enable local crash capture, create/review,
   clear, and submit only after a second confirmation.
4. Extend the real GTK settings journey with a controlled capture endpoint and
   filesystem inspection. Prove default silence before exercising explicit
   submission.
5. Run focused mutation, reducer, staged X11/Wayland, clippy, shellcheck, and
   inventory validation. Do not use full qualification as the development
   loop.

## Acceptance boundary

The feature is not complete if an ordinary launch produces a network request,
if raw sensitive material reaches a stored/captured payload, if a malformed or
oversized store prevents launch, if retention is unbounded, if a report can be
sent without review plus confirmation, or if unavailable transport is called a
successful submission.

