# Zentty Linux dogfood: CI policy correction — 2026-08-19

## Operator decision

GitHub CI checks the work. It is not authoritative qualification evidence, a
release process, or an acceptance ceremony. A locally completed issue must not
remain open while an agent waits for a hosted job, nor does anyone need to
download, approve, hash-elect, or separately document CI output before the
work is complete.

This corrects an agent-created policy error. Requirements for real integration
tests, explicit matrix gaps, controlled environments, public trust, and useful
CI were incorrectly escalated into a hosted-evidence publication regime. That
regime was not requested by the operator.

## Cleanup

- Hosted full run `32176872517` was cancelled.
- The scheduled/manual six-hour full workflow was deleted. The bounded normal
  CI gate remains and continues to report regressions on clean Ubuntu.
- The full-workflow contract and its mutation tests were deleted; they tested
  an invented evidence ceremony rather than Zentty.
- The full workflow's now-unused logind-inhibit policy was deleted rather than
  retained as unexplained CI privilege configuration.
- `controlled_public_ci` and its Wayland/X11 BLOCKED cells were removed from
  the product qualification matrix. A CI provider is not product behavior.
- ADR 0005 now records the boundary so this policy cannot silently accrete
  again.
- The retained workflow and job summaries are labeled advisory regression
  checks; uploaded logs are described only as debugging aids.
- Historical CI dogfood remains for failure/root-cause value, but prominent
  corrections supersede its claims of hosted authority.
- GH-10, GH-58, GH-59, and GH-9 are reconciled separately: normal workflow
  security remains useful, while public-receipt approval and release-current
  evidence are removed.

## Test expectation

Matrix validation must report the two removed CI cells as absent rather than
PASS, BLOCKED, or NOT_IMPLEMENTED. Existing real product/environment gaps
remain explicit. The ordinary CI gate must still validate its pinned actions,
least privilege, clean checkout behavior, and selected real-system tests.

## Verification

The correction was checked with the existing focused validators rather than a
new harness:

- `linux/tests/qualification-matrix --validate-only`
- `linux/tests/qualification-matrix-test`
- `linux/ci/workflow-contract-test`
- `linux/ci/gate-workflow-contract-test`
- `linux/ci/validate-environment-test`
- `linux/ci/reset-product-outputs-test`
- `linux/ci/validate-preflight-receipt-test`
- `linux/ci/validate-pr-subset-test`
- `linux/ci/run-pr-subset-test`
- `linux/tests/test-orchestration-contract`
- ShellCheck over every changed shell program and `git diff --check`

All passed. The authoritative product matrix now reports `161 PASS`,
`3 BLOCKED`, `1 XFAIL`, and `14 NOT_IMPLEMENTED`. The two removed BLOCKED
entries were CI-provider policy, not product behavior; they were removed rather
than falsely converted to PASS.

The parent epic and GH-9/GH-10/GH-56 through GH-59 were reconciled. GH-58 and
GH-59 are closed as not planned; the closed GH-56/GH-57 histories carry an
operator-correction notice while retaining the useful secure-CI work.

The clean-checkout `linux/ci/preflight-test` initially failed inside Codex's
filesystem sandbox because the user namespace maps the host's root-owned
`/tmp/.X11-unix` directory to `nobody:nogroup`. The host directory itself was
the required `root:root` mode `1777`; rerunning that host-environment preflight
outside the sandbox passed. This was an execution-environment mismatch, not a
reason to weaken the ownership check.
