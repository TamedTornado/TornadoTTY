# Codex goal continuation and false completion attention — 2026-09-07

Issue: [#143](https://github.com/TamedTornado/TornadoTTY/issues/143).
Previous field report: [bounded ingress](tornadotty-linux-dogfood-2026-09-05-ingress-backpressure.md).

## Observed and confirmed

Jason received a Bro notification at 08:06:28 CEST, clicked it at 08:06:29,
and found Codex still working. The running GUI was `870d0cea89da`, not an
unpatched pre-approval-fix binary. The journal showed `agent.idle` and desktop
delivery at 08:06:28, followed by `agent.running` at 08:06:38. No permission
request or terminal-notification callback explained this particular alert.

Jason identified the active goal. A bounded, read-only transcript inspection
confirmed `task_complete` at 06:06:28.408Z and `task_started` at .414Z, followed
by goal-continuation steering at .473Z. The agent was continuing before the
notification was clicked; the next tool hook merely reached us ten seconds
later. This was not a stale notification that happened to precede new work.

Local Codex source at `95c7265e84`,
`codex-rs/tui/src/chatwidget/turn_runtime.rs`, already suppresses its completion
notification when a goal is active or queued input starts a follow-up. Its
low-level Stop and legacy notify hooks do not express that TUI decision.
TornadoTTY incorrectly turned those hooks into completion attention.

## Repair and compatibility

- Codex Stop and legacy `agent-turn-complete` are no longer authoritative
  waiting-for-human events. Neither creates idle/ready inbox attention.
- Managed launches enable `approval-requested` and `agent-turn-complete` on
  the existing focus-independent Codex TUI OSC 9 channel.
- The parsed Ghostty callback retains the producer's text and uses existing
  `NeedsInput / GenericInput` attention. OSC 9 carries no subtype: classifying
  every callback as Approval would now be false. Explicit question-tool events
  retain their question/decision types. No notification wording is parsed.
- No goal database polling, terminal-title classifier, delay, retry, new
  harness, dependency, or Codex modification was introduced.
- Existing tests that expected the invalid raw-idle transition were changed
  to assert the terminal notification contract. Tests still require genuine
  human attention, delivery, duplicate suppression, and subsequent resolution.

## Focused evidence

- New goal-boundary adapter → reducer → inbox regression failed before repair
  with an unexpected attention item; passes after repair. It checks both raw
  completion hooks, genuine notification, duplicate delivery, and new activity.
- Core adapter, launch, and status integration files: **99 PASS**.
- Real helper-process / Unix-socket Codex tests: **7 PASS**. Initial sandboxed
  invocation could not bind sockets; rerun with required permissions passed.
- Product-only ReleaseSafe build: **PASS**, 42.25 seconds; dependency-age audit
  91 packages / zero exceptions and package notice collection passed.
- Existing private-X11 `rust-agent-ipc` review-routing journey: **PASS**;
  fixture now includes both raw completion hooks before genuine OSC attention.
- Existing `installed-codex-integration`, real Codex 0.147.0 with a controlled
  loopback model endpoint: **PASS**, including initial and resumed completion,
  actual GTK state, durable session identity, cwd, and return to shell.
  First attempt reached genuine attention but the resume assertion still
  expected raw `agent.idle`; correcting that obsolete assertion passed.
- ShellCheck and formatting of changed Rust files: **PASS**. Workspace-wide
  formatting check also reports pre-existing differences in untouched files;
  those were not swept into this repair.
- Cross-window `rust-attention-inbox`: **FAIL**, before the changed completion
  path, waiting for two requests in the shared inbox. The previous `build/gh163`
  product fails at the same point with the same journey. This is a retained
  baseline failure, not a passing result. The first invocation also lacked the
  required private D-Bus session; subsequent runs used `dbus-run-session`.
- Review caught shared Small Harness handling in the Codex-family adapter;
  its Stop behavior is explicitly preserved and separately regression-tested.

## Deployment and remaining checks

The isolated product is in `build/gh143/bin`. The running client and installed
files were not replaced or restarted. Managed Codex notification configuration
changes take effect on a new Codex launch, not in an already-running session.
Installed goal-continuation dogfood remains pending; no full qualification or
fresh matrix totals are claimed.
