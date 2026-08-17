# Zentty Linux public CI gate dogfood — 2026-08-18

This append-only report covers GH-57. GH-56 established the pinned public
environment; this issue turns that foundation into a pull-request gate without
creating a CI-only product or a second qualification matrix.

## Ratified execution design

The complete 181-cell aggregate is intentionally not the pull-request gate.
Its final critical path is 1,047.930 seconds, dominated by the pinned Ghostty
regression and two independent clean package reconstructions. Running that
complete release evidence on every edit would reproduce the test-architecture
problem already rejected during local qualification work.

GH-57 therefore adds one versioned subset policy whose cell IDs must resolve
to executable PASS cells in `linux/qualification-matrix.json`. The policy must
prove both Debug and ReleaseSafe builds, X11 and Wayland, single and multiple
terminals, lifecycle, native input, recovery, staged packaging, product
boundaries, and the Ghostty API audit. Commands and runtime-environment profiles
come from the authoritative matrix; the gate may not restate or weaken them.
The known blocked Wayland physical-key cell remains explicit in the policy and
cannot be converted into coverage or silently omitted.

The subset runner owns orchestration only. It invokes the existing isolated,
nested-X11, nested-Wayland, and input-capable nested-Wayland wrappers and
validates their receipts through the existing controlled-environment library.
Ghostty, GTK, compositors, virtual input, PTYs, Rust product, persistence, and
staged bundles remain real. There is no test-only terminal implementation,
product fake, alternate socket, or CI-specific feature path.

The machine summary has one new claim only:
`public_pr_subset_passed`. Implemented-local, release, and full-Linux claims
must remain false. A nonzero command, exit 77, absent or invalid compositor
receipt, stale source identity, missing selected cell, changed declared status,
or missing coverage fails the gate. Failure artifacts are retained, but cannot
become a PASS receipt.

For untrusted pull requests, the workflow uses `pull_request`, never
`pull_request_target`; grants read-only contents; persists no checkout token;
receives no secrets; and pins every action. Required apt installation and the
Ubuntu user-namespace policy change are literal reviewed workflow operations,
not arguments or commands derived from pull-request code. Repository scripts
never execute through `sudo`.

## Discovery: policy validation needed semantic and graph checks

The first policy validator draft proved only that named IDs existed. That was
not sufficient: a syntactically valid edit could remove Debug coverage, move a
cell ahead of its build prerequisite, silently drop the known Wayland physical
key gap, or select a cell whose authoritative status had changed.

The validator now checks the closed policy schema, exact capability coverage,
both displays and optimizations, single/multi terminal behavior, PASS plus
command/environment ownership for every selected cell, the explicit BLOCKED
gap, and topological order against the matrix root, profile, and per-cell
dependencies. While implementing those checks, three jq mistakes were caught
by the negative suite: generator context was lost inside `all`, the selected
cell helper captured a nested string instead of the matrix, and the duplicate
fixture mutated the wrong array. Each was repaired without weakening the
policy. The negative suite now proves missing and unknown cells, duplicates,
false qualification claims, hidden and stale gaps, dependency reordering,
missing Debug coverage, and selected-status drift are rejected.

## Discovery: runner outcomes and claims require a closed contract

The first runner draft classified command exit codes inline and wrote a
summary without independently validating that summary. That made the final
claim too dependent on the orchestration code which constructed it. Outcome
classification and the closed summary schema now live in
`linux/ci/pr-subset-result`, which the runner itself invokes. Exit 77 is
`UNEXPECTED_SKIP`, nonzero is `FAIL`, and an absent, stale, mismatched, or
malformed controlled-environment receipt is
`INVALID_ENVIRONMENT_RECEIPT`. Receipt failure takes precedence because an
unproven environment cannot substantiate even the reason for a skip.

The summary validator derives the only permitted positive claim from complete,
unique PASS results. It rejects incomplete execution, duplicate results,
outcome/exit mismatches, malformed evidence identities, a false subset PASS,
and any implemented-local, release, or full-Linux claim. The runner deletes
stale summaries and logs before execution, stops on the first failed cell, and
still emits a failed machine receipt. Its focused test covers stale evidence,
failed commands, unexpected skips, invalid receipts, incorrect claims,
incomplete results, and duplicates. A dirty checkout is rejected for an actual
run so a commit hash cannot misrepresent uncommitted policy or code; validation
and workspace-preparation modes remain available during development.

## Discovery: pull-request privilege boundary differed from the foundation

The GH-56 foundation workflow obtained its apt list by executing a repository
script immediately before `sudo apt-get`. That is acceptable only for a trusted
branch run and is not an acceptable untrusted-pull-request boundary. The gate
therefore carries a literal reviewed package array. Its contract compares that
array token-for-token against `environment-v1.json`, permits only the reviewed
apt update, apt install, and AppArmor sysctl sudo operations, and rejects a
repository command or shell-generated command crossing sudo.

The gate uses `pull_request`, push to `linux/port`, and manual dispatch. It
rejects `pull_request_target`, secrets, write permissions, mutable runners,
mutable actions, persisted checkout credentials, a non-event source revision,
missing retained evidence, and removal of the authoritative subset runner.
The older foundation remains manually dispatchable for focused environment
diagnosis, but no longer duplicates every branch push.

## Repair: public-CI contracts are part of local support qualification

The GH-56 CI contract tests existed but were not in `qualify-local`'s support
array. That was an orchestration omission: a full local run could have ignored
drift in the environment manifest, bootstrap preflight, receipt validation,
workflow, or product-output reset. The eight foundation and gate contract
suites are now named support tests. They share the existing bounded support
runner; no second aggregate or CI-only test layer was added.

All eight focused CI support tests passed locally. Shellcheck passed for the
new gate scripts. The subset policy resolved to 18 authoritative cells.

## Preliminary real-system run and environmental failure

The first real subset attempt failed its first cell with exit 128 because the
sandbox denied DNS while `prepare-ghostty-source` verified the GitHub remote.
The runner correctly emitted a failed summary with 0/18 PASS, retained the
valid isolated-session receipt, and made no broader qualification claim. This
was environmental absence, not a pass and not a product defect.

The same runner was then executed with the required network permission. All 18
selected commands ran through the real existing isolated, nested-Wayland,
nested-X11, and input-capable nested-Wayland harnesses and passed in 368
seconds. It produced 18 command logs and 18 validated environment receipts.
The machine result correctly stated only `public_pr_subset_passed=true`; all
implemented-local, release, and full-Linux claims remained false.

This was an orchestration proving run against a dirty development checkout at
HEAD `53e8a4b41e337c989d139c9fb4be6927a637645d`, not final release evidence.
That discovery caused the runner's explicit clean-tree guard. Final local and
public receipts must be rerun from the committed implementation before either
controlled-public-CI matrix cell can move from BLOCKED to PASS.

## Repair: support tests and stale XFAIL are inside the gate contract

Review against GH-57's written acceptance criteria found two omissions before
commit. The preliminary runner selected real matrix cells but did not itself
run the public-CI support contracts, and the policy watched the BLOCKED Wayland
physical-key gap but not the matrix's existing Ghostty async-enum XFAIL.

The versioned policy now owns the exact eight CI support-test paths. Both the
public subset runner and `qualify-local` consume that one list, avoiding two
independent allowlists. Support outcomes and hashes are part of the same
machine summary, and subset PASS requires every support test plus every real
matrix cell to pass. A support skip is an unexpected skip, a support failure
stops product execution, and either still yields a failed summary. The policy
also pins `ghostty-async-backend-abi-representation` to XFAIL and rejects it if
the outcome becomes stale rather than silently treating the old expectation as
coverage.

The runner now compares its clean HEAD with `ZENTTY_EXPECTED_COMMIT` when the
workflow supplies it. The workflow contract separately pins the event SHA and
requires commit- plus attempt-specific artifact names, while ref-scoped
concurrency cancels older branch executions. This closes the wrong-event and
older-publication cases without putting the commit in the concurrency group,
which would incorrectly prevent cancellation across successive branch commits.

## Committed exact-source local receipt

After review and commit, the complete gate was rerun from clean revision
`8832ea614da38af60c1869e297c1bf42ee0cb624` with
`ZENTTY_EXPECTED_COMMIT` set to that same full identity. All 8 versioned support
tests and all 18 real matrix cells passed in 366 seconds. Both X11 and Wayland,
Debug and ReleaseSafe, single and multi-terminal behavior, staged product
boundaries, native key input, lifecycle, shortcuts, API audit, and recovery ran
through their authoritative commands and environment profiles.

The machine summary SHA-256 is
`cfb7c0dc3370fb968e662c0fd9b7c1b49240b219a18aab769983d8ca3728fd99`.
The policy SHA-256 is
`795d87778403875bb686b1dc8bb87924f3632cef8ba1d101aa4005d8f8df8c14`;
the referenced qualification matrix SHA-256 is
`cfb00b392def7069ca9b9baa0a2ff3587f9e65955fc54232064808d7f7387851`.
The receipt claims only the public PR subset. Implemented-local, release, and
full-Linux qualification remain explicitly unclaimed.

## Public run 32080633377: clean-run-only test dependency found

The first gate push ran publicly at
<https://github.com/TamedTornado/zentty/actions/runs/32080633377>. The workflow
reconstructed the pinned environment, exact Ghostty source, ReleaseSafe product,
both release display journeys, both staged journeys, API audit, Debug product,
X11 physical keys, and both product boundaries. It then failed
`shortcut-binding-runtime-wayland` after 18 minutes. The failure remained a
failure, the preflight receipt revalidated, and the artifact upload retained
the machine summary, all earlier logs, and every controlled-environment receipt.

The retained command log showed `rust-shortcuts-settings` trying to inspect
`build/linux-deps/ghostty/zig-out/lib/libghostty-gtk-embed.so`. The authoritative
ReleaseSafe build deliberately installs Ghostty into the selected product
profile and does not create that source-tree output. The local gate had passed
only because an unrelated old `zig-out` survived from development. This was a
real clean-environment test bug: the shortcut journey could load a dependency
different from the binary under test.

The journey now derives `bundle_root/lib` from `ZENTTY_LINUX_BINARY`, requires
both delivered Ghostty and layer-shell libraries there, reads the runpath from
that delivered Ghostty library, and runs the product with only the selected
bundle library path. `test-orchestration-contract` rejects reintroduction of
the mutable source-tree dependency and is now the ninth support test in the
single versioned gate policy.

The first focused repair run could not start Xvfb because the restricted
namespace presents `/tmp/.X11-unix` with the wrong ownership. That environmental
absence failed rather than passing. Repeating through the approved host
namespace produced controlled X11 session
`559534892023366b5f4658346e77ecf23dcd6547ab37e3168d9a5bfdbdfcaf93` and
input-capable Wayland session
`e25a4b82fd30a207925c70bb45f21dc4410130df9ed467c8cbcbdb29d71cecc3`;
the complete physical shortcut/settings/reload/persistence journey passed.
