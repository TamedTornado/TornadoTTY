# Zentty Linux dogfood: full public qualification

This record begins GH-58 after the bounded public PR gate passed and GH-57
closed. It records discoveries, failures, repairs, receipts, and remaining
uncertainty for public execution of `linux/tests/qualify-local`.

## Ratified boundary

The workflow invokes the existing authoritative runner. It must not grow a
second scheduler, CI-only product suite, or alternate package and Valgrind
receipt formats. The complete reviewed Ubuntu profile is distinct from the
reduced PR profile and contains the tools required by every executable cell.

A passing run may establish that the presently executable suite passed. It may
not claim release or full-Linux qualification while the matrix contains
BLOCKED, XFAIL, or NOT_IMPLEMENTED cells. Debug Valgrind is described only as
PASS with reviewed suppressions; ReleaseSafe remains XFAIL.

## Initial orchestration decisions

- Weekly schedule plus manual dispatch, bound to the exact event SHA.
- Ubuntu 24.04, read-only contents permission, no persisted checkout token,
  and commit-pinned actions.
- Six-hour job ceiling and independent 30-minute apt ceilings. The apt ceiling
  accommodates the previously observed full package graph while retaining
  per-request timeouts and retries.
- Two build jobs and two support-test jobs, matching the stabilized local
  resource policy rather than consuming arbitrary concurrency.
- Explicit stale-output deletion before `qualify-local`; always-run preflight
  revalidation, bounded human claim, and evidence upload.
- Commit/attempt-specific artifacts with 14-day retention. Raw and reviewed
  Valgrind logs, suppression sources, matrix receipts, package evidence, Debian
  artifacts, checksums, manifests, and provenance are listed explicitly.

The workflow is not qualified merely because it exists. Its mutation-tested
contract and a real public run must expose incorrect paths, missing packages,
runner-only races, unsafe receipts, or duration problems.

## Discovery: manual dispatch registration follows the default branch

The first `workflow_dispatch` request for revision `2b0d51c3` returned GitHub
HTTP 404. GitHub does not register a newly introduced dispatchable workflow
until that workflow exists on the repository's default branch; this repository
still uses `main`, while the port is developed on `linux/port`. Changing the
default branch or copying mutable port orchestration into the old product branch
would be a larger repository-policy decision, not a test repair.

The same workflow now has a path-bounded `linux/port` push trigger for its own
workflow, full-CI contract/runner files, and authoritative matrix. This gives
the integration branch a real first run without causing every product commit to
launch the multi-hour suite. Schedule and manual dispatch remain declared and
will become registered when this workflow reaches the default branch. The
contract rejects loss of any of the three trigger modes.
