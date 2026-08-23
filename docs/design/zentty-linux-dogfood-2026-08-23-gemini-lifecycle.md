# Zentty Linux installed-Gemini lifecycle dogfood

Date: 2026-08-23
Tracking: GH-83

This append-only record begins before implementation. The ratified plan is
`docs/design/linux-gemini-lifecycle-qualification-plan.md`.

## Preserved starting failure

- The post-GH-80 Wayland qualification run spent roughly 27 minutes in
  `rust-agent-ipc` before it was interrupted. The product, real Gemini CLI, and
  controlled endpoint were still alive.
- The endpoint had received the expected probe and Zentty had received both
  running and idle events. Physical close opened **Quit Zentty?** because the
  earlier OpenCode theme fixture had replaced the whole config with an
  appearance-only document, deleting the test's confirmation policy.
- Polling loops were bounded, but many were followed by unconditional `wait`
  calls. A missed deadline therefore became an unbounded hang rather than a
  failed test.
- Ambient Gemini is newer than the reviewed `0.53.0` prerequisite. The
  repository-owned pnpm installation still provides exact `0.53.0`; changing
  the expected version to match ambient state is not an acceptable repair.

## Decision before changes

Repair the existing actor and shared process/input support. Add an isolated
scenario mode, not another Gemini harness. Make process ownership, deadlines,
CLI provenance, and config preservation executable contracts before rerunning
the real journey.

## Repair and focused evidence

- Added one shared owned-child wait contract. A real sleeping child now returns
  status `124` at its declared deadline and names its PID/owner; exited and
  SIGKILLed children are reaped with their actual status. No raw `wait` remains
  in `rust-agent-ipc`.
- The OpenCode live-theme rewrite now retains all three confirmation disables
  and asserts the quitting policy before continuing. This repairs the physical
  close path rather than teaching the actor to dismiss an accidental dialog.
- Required Gemini qualification rejects ambient PATH discovery. Both matrix
  cells name `linux/ci/node-tools/node_modules/.bin/gemini`, the actor verifies
  that canonical path, and the observed version was exactly `0.53.0`.
- Matrix validation now rejects a missing repository Gemini path or a changed
  version. Its negative runner also proves that interrupted, scheduler-owned
  `.cell-results.*` scratch is removed under the exclusive evidence lock while
  unrelated directories are still rejected.
- Focused shellcheck, `product-input-test`, matrix validation, matrix runner
  tests, orchestration contracts, JSON parsing, and the repository Gemini
  version check passed.
- The complete non-isolated X11 agent actor also passed in about 45 seconds,
  proving the config repair and bounded wait conversion across the existing
  adapter journeys.

## Real installed-Gemini journeys

The installed-only scenario passed independently on controlled X11 and on the
input-capable nested Wayland compositor. Each run used the real ReleaseSafe
Zentty binary, Ghostty surface and PTY, repository Gemini CLI, hooks, Unix
socket, controlled loopback model endpoint, physical close input, persisted
workspace, and resumed Gemini TUI. Each completed in roughly ten seconds.

Final post-review reruns passed on both backends with the receipt fields
`scenario=installed-gemini`, `adapters=not-run-in-installed-gemini-scenario`,
and `real-gemini=true`; the explicit adapter field prevents the focused mode
from claiming the unrelated compatibility-adapter sequence ran.

## Full local qualification receipt

Only after both isolated journeys passed, `linux/tests/qualify-local` was run.
The first attempt correctly failed two support contracts that still encoded the
old matrix command and found scheduler scratch retained by an interrupted prior
run. Both were repaired and covered by focused negative tests before rerunning.

The rerun completed in `835970ms`; it did **not** pass overall. Declared matrix
totals were `199 PASS`, `0 FAIL`, `0 BLOCKED`, `3 XFAIL`, and
`2 NOT_IMPLEMENTED`. Observed outcomes were `157 PASS`, `34 FAIL`,
`2 MISSING_OR_INVALID_VALGRIND_REPORT`, `6 BLOCKED_BY_FAILED_DEPENDENCY`,
`3 XFAIL`, and `2 DECLARED_NOT_IMPLEMENTED`. Accordingly, implemented-local,
release, full-Linux, and suppression-review claims all remain false.

The installed Gemini portion itself completed before later commands in each
consolidated cell failed: X11 failed in the chained tmux product journey, and
Wayland failed in the chained session-restore output-size assertion. The broad
run also exposed numerous pre-existing concurrency/environment-sensitive cells.
Those failures are preserved in `build/linux/qualification-summary.json` and
its matrix logs; they are not relabeled as Gemini failures and no exhaustive QA
claim is made here.
