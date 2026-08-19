# Zentty Linux dogfood: staged and installed CLI artifacts

Date: 2026-08-19

Issue: GH-49

## Scope correction

GH-49 resumes after the operator retired hosted CI as qualification authority.
The issue requires real staged and installed artifacts, but it does not require
a GitHub-hosted receipt, artifact approval, or another aggregate test system.
GH-22, GH-32, and GH-49 were corrected accordingly.

## Coverage audit

The source CLI contract contains 40 commands in 12 families. Existing real
product journeys already covered topology, pane/worklane control, themes,
servers, notifications, tmux compatibility, agent IPC, and agent hooks. The
audit found two real discrepancies:

1. the `agent-launch` inventory still called Copilot, OpenCode, Pi, OMP, and
   Small Harness incomplete after GH-47 had delivered them; and
2. `install`/`uninstall` had unit-level filesystem coverage but no invocation
   through the actual staged or installed CLI artifact.

The existing `cli-source-contract` validator now requires every command family
to name at least one `linux/tests/` journey that is executed by a PASS matrix
cell. Its negative suite removes the agent-launch journey and proves that the
contract fails rather than silently losing a family. This extends the existing
inventory validator; it does not add another runner.

## Real artifact repair

The existing `rust-agent-ipc` journey now invokes the staged `zentty install`
and `zentty uninstall` commands against an isolated Cursor hook file. It proves
the staged absolute CLI path is installed, user content survives, and owned
hooks are removed. The existing installed-package journey performs the same
operation through `/usr/lib/zentty/bin/zentty` and records it in the installed
product receipt. No source-tree CLI path is accepted by the installed journey.

## Failures and repairs

- The new family validator first failed on the preexisting contract, proving
  that agent-hook, agent-integration, and agent-launch lacked matrix-linked
  real-product journey declarations.
- The first X11 product run found that hook commands safely quote the absolute
  CLI pathname. The assertion expected an unquoted pathname; it was narrowed
  to require both the exact staged path and the exact adapter command without
  weakening either identity.
- A full matrix-form X11 agent command then reached an unrelated environment
  drift: the installed Gemini CLI reports `0.55.1` while the reviewed matrix
  prerequisite remains `0.53.0`. This is not converted into a pass and the
  pinned expectation was not silently changed. Focused X11 and Wayland runs
  with the external Gemini prerequisite explicitly disabled passed every
  staged wrapper, real PTY, private socket, hook, lifecycle, and new integration
  CLI path. The external model-boundary journey remains governed separately by
  its reviewed-version prerequisite.

## Focused results before installed-package rebuild

- `linux/tests/cli-source-contract`: PASS, 40 commands and 12 families.
- `linux/tests/cli-source-contract-test`: PASS, including missing-family
  rejection.
- Controlled X11 `linux/tests/rust-agent-ipc`: PASS with real staged product,
  PTYs, wrappers, socket, adapters, and staged install/uninstall.
- Controlled Wayland `linux/tests/rust-agent-ipc`: PASS with the same product
  boundaries.
- ShellCheck and `git diff --check`: PASS.

The installed package embeds the source commit identity, so its modified
journey cannot honestly run against a dirty checkout or a package from the
previous commit. The change is committed first, then the exact-commit package
is rebuilt and the installed X11/Wayland journeys are run before GH-49 closes.
