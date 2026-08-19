# ADR 0005: CI is an advisory check, not qualification authority

## Status

Accepted by the operator on 2026-08-19.

## Decision

GitHub Actions checks repository changes. A CI pass or failure is useful
feedback about the commit executed by that job, but it is not a product
capability, a release artifact, a release approval, or authoritative
qualification evidence.

- The product qualification matrix contains product, dependency, platform,
  packaging, and testable environment behavior. It does not contain CI-host
  cells.
- A development slice does not wait for a hosted run after its required local
  tests pass. CI failures are fixed when observed, like any other regression.
- CI logs and artifacts are debugging conveniences. They require no download,
  independent hash approval, current-evidence election, or dogfood ceremony.
- CI must not publish or imply release/full-Linux claims. A future release
process, if needed, will be designed separately and explicitly.
- The bounded pull-request/integration check may remain because it catches
  regressions on a clean Ubuntu runner. It must reuse repository tests and must
  not create a second product harness.

## Removed policy

The scheduled/manual six-hour “full public qualification” workflow, its
workflow-governance test layer, the `controlled_public_ci` capability, and its
two BLOCKED matrix cells are retired. Historical dogfood records remain as a
record of what happened, but this decision supersedes their claims that a
hosted receipt is required for completion or release.
