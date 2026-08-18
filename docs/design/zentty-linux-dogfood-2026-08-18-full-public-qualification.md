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

## First public run: cold-checkout dependency failures

Run <https://github.com/TamedTornado/zentty/actions/runs/32107609194> installed
the complete package graph and reached the real authoritative suite, then
failed after 25 minutes with retained receipts. It exposed four locally cached
assumptions rather than product failures:

1. `libxml2-utils` was absent, so Ghostty resource generation could not find
   `xmllint`.
2. Release, Debug, and regression Ghostty builds concurrently mutated the same
   source tree. They now share the existing `ghostty-source` scheduler resource.
3. several Rust/platform contract cells omitted their actual build dependency
   and, for direct Cargo invocations, the profile-specific `GHOSTTY_LIB_DIR`.
4. support contracts ran concurrently with matrix producers whose generated
   dependencies and artifacts they inspect. Support tests remain mutually
   parallel, but now start only after the matrix phase.

The cold builds also proved that the environment receipt's 900-second chronology
ceiling was not a test timeout: successful build and regression commands took
1,044 and 1,441 seconds. The receipt span is raised to 1,800 seconds while the
workflow and commands retain their independent hard deadlines. The original run
remains a public failure; no missing artifact or environment was called PASS.

## Second public run: warm-checkout assumptions and mutable build paths

Run <https://github.com/TamedTornado/zentty/actions/runs/32109787409> at exact
commit `519db59676bc545b688e2b9619f7915841c31f81` reached the complete matrix and
retained 340 evidence files. It failed after 43 minutes 54 seconds. Declared
totals remained 161 PASS, 0 FAIL, 5 BLOCKED, 1 XFAIL, and 14 NOT_IMPLEMENTED;
the executed suite did **not** pass. The run established several independent
cold-run defects:

- Seventeen real-product journeys loaded Ghostty and GTK layer-shell from the
  mutable managed source checkout's `zig-out`. Concurrent ReleaseSafe, Debug,
  regression, and historical-ABI builds legitimately replace that directory.
  Most affected cells therefore failed immediately after `readelf` could no
  longer find the source-tree library. This was test architecture drift: the
  delivered product already owns an immutable adjacent `lib` bundle.
- Direct Debug Cargo cells similarly linked against the mutable
  `ghostty-install/debug` staging prefix instead of the immutable Debug product
  bundle.
- The historical ABI builder read current headers from mutable `zig-out` while
  the Debug build owned that tree. Its current library already came from the
  immutable ReleaseSafe bundle, so the mixed ownership was internally
  inconsistent.
- The qualification host had neither a prepared host Cargo registry nor
  content-pinned Fish 4.8.1/Nushell 0.114.1. Consequently the offline clean
  package rebuild failed, package notices could not resolve `serde`, Fish 3.7
  was correctly rejected, and Nushell was correctly reported absent. No skip
  was converted to PASS.
- The real nested-Wayland wrapper uses `socat`; the full apt profile omitted
  it. Its absence failed the harness rather than being treated as an
  environmental pass.
- The external-resize journey used a fixed 250 ms sleep after asking the real
  X server to resize a four-pane window. The resize arrived after that sleep on
  the public runner, and the child correctly failed its viewport assertion.
- The architecture mirror still held the pre-repair clipboard command, so its
  exact reconciliation check correctly rejected the drift.
- Debug Valgrind itself passed with reviewed suppressions and retained both raw
  and post-suppression receipts. Governance then rejected the live Debug
  reproducer because it required the public runner's debug binary hash to equal
  the hash of a separately retained local eight-run archive. Debug symbols
  encode build-root/toolchain metadata, so that equality is not portable. The
  live report already binds its raw and suppressed receipts to the actual
  candidate executable hash and exact source/protocol/runtime identities.

The repair consolidates product library selection in one focused
`product-bundle` helper and migrates every affected journey to the libraries
adjacent to its selected executable. A mutation-style helper test rejects a
missing layer-shell library; repository search rejects remaining product-test
loads from source `zig-out`. Debug Cargo cells now link from the immutable Debug
bundle, and the ABI fixture consumes the staged ReleaseSafe header. This is a
test-boundary repair, not a scheduler resource added to serialize otherwise
independent product journeys.

The reviewed environment manifest now content-pins the official Fish 4.8.1
x86_64 archive (`39cab352...`) and Nushell 0.114.1 x86_64 GNU archive
(`8802b26e...`). Bootstrap verifies each archive before extraction, exports the
exact qualification-only paths, and preserves explicit host `CARGO_HOME` and
`RUSTUP_HOME` across private application homes. The public workflow prepares
the locked Cargo registry before network-isolated package reproduction. `socat`
is now an explicit reviewed package in both complete and PR profiles rather
than an undeclared desktop assumption.

The external-resize assertion now polls the real product topology for at most
five seconds instead of assuming a 250 ms compositor/GTK deadline. Suppression
governance retains the archive executable hash as archive identity, but binds
each live candidate to the executable hash in that run's report and paired raw
receipts. Its negative suite now corrupts the live report hash and proves the
candidate binding fails closed. The archive remains non-public and explicitly
NOT_IMPLEMENTED under GH-10; this repair does not pretend otherwise.

Focused receipts after repair: environment, full/gate workflow contracts,
matrix schema/runner, architecture reconciliation, suppression governance,
ShellCheck, and the product-bundle helper all pass. The real old/new Ghostty
ABI fixture also passes using the immutable current header/library. A further
public run is still required. The second run also contained later real-journey
failures (including attention routing, shortcut readiness, multi-window frame
restore, installed desktop launch, and two Xvfb readiness failures) that may be
load-sensitive or independent defects; they remain failures until reproduced
or repaired, not collateral passes from the build-path corrections.

Two focused follow-ups came from exercising the repaired environment locally.
The shell-prerequisite negative test inherited the newly explicit Fish/Nushell
paths and therefore no longer represented an absent-tool case; each relevant
case now clears only the variable it is testing. The full run's two Xvfb
environment-report failures also showed that `xdpyinfo` readiness does not
guarantee the first immediate `glxinfo` probe succeeds under load. The nested
X11 owner now polls the real GLX renderer for a bounded five seconds, retaining
the same software-renderer proof and failing if readiness never arrives.

The first complete local rerun on `f7b93e02` executed every presently feasible
cell in 1,414 seconds. It reached packaging and all real product journeys, but
correctly refused qualification because three direct Debug Cargo cells failed.
The immutable Debug libraries existed; the commands passed
`GHOSTTY_LIB_DIR=build/...`, and Cargo runs a package build script with the
package directory as its working directory. The relative path therefore named
a nonexistent directory below `crates/zentty-ghostty-sys`. A focused real
reproduction failed with the relative path and passed with the exact absolute
path. The matrix now anchors these inputs at `$PWD`, and both the matrix runner
test and orchestration contract reject a return to relative `GHOSTTY_LIB_DIR`
values. The failed receipt remains evidence; it is not reported as a pass.

Public PR gate `32118652113` then passed both builds, staged X11/Wayland,
physical X11 input, and the product-boundary journeys before failing the real
Wayland shortcut/settings journey. Its retained log exposed a harness defect:
the first command-palette probe had produced `zz`, while a later probe produced
only `z`; the later attempt searched the entire log and incorrectly accepted
the stale earlier `zz` receipt. It consequently raced Ctrl+A and Backspace
against the wrong attempt. Readiness is now count-scoped to the current
physical query. Once proven, the harness closes and reopens the real palette to
obtain an empty input rather than injecting an immediate two-chord reset. The
contract rejects removal of both boundaries. Three consecutive journeys pass
through the real nested Cage, virtual-keyboard, GTK, and Ghostty stack after
the repair. The failed public gate remains a failure until its replacement run
passes.

Pushing that journey repair exposed a public-control defect before it could be
silently missed: the PR gate started, but the full workflow did not. Its push
trigger used a narrow path allowlist containing the matrix and runner while
excluding individual journey scripts, Rust product code, and other qualified
inputs. A full run for the preceding commit therefore continued instead of
being superseded by the repaired candidate. The integration branch now runs
full qualification for every push; the workflow contract has a negative that
rejects any reintroduced `paths` filter. This deliberately favors trustworthy
candidate identity over saving a run for documentation-only commits.
