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

## Third public run: real gate pass and full-run resource/dependency failures

The replacement PR gate
<https://github.com/TamedTornado/zentty/actions/runs/32121127865> passed at exact
commit `7d7e3ca8` in 19 minutes 44 seconds. This is the first public proof that
the bounded gate, including its repaired Wayland shortcut journey, passes from
a cold hosted checkout. It does not qualify the larger release suite.

The corresponding full run
<https://github.com/TamedTornado/zentty/actions/runs/32121127845> failed after
55 minutes 57 seconds and retained its machine-readable summary and receipts.
It reported 25 failed executed cells. The failures were not collapsed into the
declared matrix totals and the run is not a qualification pass. Inspection of
the receipts separated five deterministic environment/test defects from a
larger group of concurrent real-GUI failures:

- the public archive contained `pnpm.cjs`, but bootstrap had not installed a
  command named `pnpm`; a developer-machine installation had masked this;
- `notification-daemon` was absent from the full package manifest;
- the Docker journey assumed a mutable `busybox:latest` image already existed
  while also prohibiting pulls;
- the interrupted-write recovery harness polled in a scheduler-hostile busy
  loop, allowing its 10,000 checks to finish before the writer ran; and
- the architecture mirror still contained the pre-repair relative Debug
  library path.

The run also launched four software-rendered GTK/Ghostty journeys at once on a
two-core hosted runner. The same shortcut journey that passed serially in the
public gate failed in this overloaded full run, together with many independent
X11 and Wayland journeys. This is evidence that four-way public GUI execution
is not a trustworthy qualification environment, not evidence that those cells
passed. The full workflow now limits matrix-cell concurrency to two while
retaining two build and support jobs. The workflow contract mutation-tests
that policy; this does not serialize the suite or restore the obsolete
one-worker workaround.

Bootstrap now creates `pnpm`/`pnpx` command links to the content-pinned archive,
and preflight proves the command found on `PATH` resolves to the exact reviewed
binary. The full environment explicitly installs `notification-daemon`.
Docker qualification uses and pre-pulls the immutable amd64 BusyBox fixture
`busybox@sha256:7a3ebe5bfd1a4a19797d20b0c0bb39d44393e9a03fd852c0865b0f540d868df0`;
the environment validator, matrix contract, and journey all reject a mutable
tag. The recovery harness now yields for up to ten bounded seconds rather than
consuming a core in a tight loop, and the architecture mirror again matches the
authoritative command.

Focused local proof after these repairs includes three consecutive real
SIGKILL interrupted-write journeys; real task-runner journeys on nested X11
and nested Cage Wayland input; real notification journeys on both backends with
a private D-Bus and the actual notification daemon; and a real Docker-backed
development-server journey using the exact digest. Environment, workflow,
matrix, architecture, and shell contracts pass. The local host's
`/tmp/.X11-unix` had nonstandard `nobody:nogroup` ownership and initially
prevented Xvfb from binding; correcting it to the X11-required root ownership
made the controlled X11 journeys executable. That environmental absence was
recorded as a failure and was not treated as a pass.

Valgrind governance correctly failed the public run and prompted a separate raw
receipt audit. The Debug IBus-focus candidate retained both unsuppressed and
post-suppression receipts, with 414 raw errors, 6,016 definitely lost bytes,
and 28,044 indirectly lost bytes. The project layout-cache rule still matched
exactly two contexts, but their two definite realloc roots were 6,575 and 6,710
bytes rather than the larger cache roots seen locally. Both raw records retain
the exact Fontconfig-to-Pango `pango_layout_get_size` consumer stack documented
by the non-Ghostty reproducer. The public report identifies the same reviewed
Ubuntu Fontconfig 2.15.0 and Pango 1.52.1 libraries; its raw receipt SHA-256 is
`c6bbf33fc15f2a25dac12de47689d26e4588c09df672dc449176674deb012549`
and its suppressed receipt SHA-256 is
`9e8db97d04defbe3d790b99afa0412cbcfdd711dca617ea385e827b9337bad36`.

The suppression stack and maximum allowed context count are unchanged. The
manifest's reviewed lower byte bound now includes the independently retained
13,285-byte public observation, while the existing maximum still rejects any
increase. Zero use remains stale, use outside the IBus/X11 scenario remains an
error, and the child rules still require this exact root. This is an evidence
range correction, not a broader Valgrind suppression. Debug can therefore only
be described as PASS with reviewed suppressions after governance passes again;
it is not an unsuppressed clean result. ReleaseSafe Valgrind remains XFAIL.
Until a corrected public full run passes every executable cell, the only valid
claim is that the replacement public gate passed.

A fresh local paired run subsequently exercised the unchanged non-Ghostty
GTK/IBus reproducer and governance. Its unsuppressed receipt reported 427
errors from 427 contexts, 6,160 definitely lost bytes, and 41,428 indirectly
lost bytes. The post-suppression receipt reported zero errors and zero
definite/indirect bytes; the layout root remained exactly two contexts and
26,176 bytes. Governance passed against the full inherited Ghostty plus Zentty
suppression set. The correct result wording is **PASS with reviewed
suppressions**, never an unsuppressed clean result.

After committing the repair so clean-checkout package and preflight contracts
could execute, `linux/tests/qualify-local` reran every presently executable
support and matrix cell at commit `c731bb66`. The implemented local suite
passed in 880,420 ms, including the real staged install/uninstall lifecycle,
clean package rebuild, Docker fixture, both display backends, agent journeys,
and the paired Debug Valgrind run. Declared matrix totals remain 161 PASS,
0 FAIL, 5 BLOCKED, 1 XFAIL, and 14 NOT_IMPLEMENTED. Therefore implemented
local qualification passed, but release qualification and full Linux
qualification remain correctly NOT_PASSED.

## Fourth public run: two GUI workers are still not a controlled host

The public PR gate for exact commit `e2c6ff23`
<https://github.com/TamedTornado/zentty/actions/runs/32129795002> passed in
20 minutes 36 seconds. The corresponding full run
<https://github.com/TamedTornado/zentty/actions/runs/32129794981> retained its
receipts and failed after 50 minutes 8 seconds: 141 executed cells passed, 18
failed, and two were blocked by failed dependencies. The declared matrix still
contains 161 PASS, 0 FAIL, 5 BLOCKED, 1 XFAIL, and 14 NOT_IMPLEMENTED cells;
those declarations do not conceal the execution failures. No release or full
qualification claim is valid for this run.

The attempted reduction from four to two simultaneous GUI cells was
insufficient on the two-core hosted runner. Physical-input and real process
journeys on both backends timed out in groups while the same journeys passed
in the serial public gate and in the four-worker local qualification on this
development host. Public full qualification will therefore execute one matrix
cell at a time. This is scoped to the resource-constrained public GUI runner:
public compilation and support work retain two jobs, and local qualification
retains its four-cell default. The workflow contract now rejects relaxing the
public isolation accidentally. This supersedes the earlier expectation that
two public GUI workers would be trustworthy; retained public evidence proved
otherwise.

Four independent deterministic defects were also present and are repaired
rather than attributed to load:

- installed-package journeys had a private display but inherited no D-Bus
  session, so the real status-notifier startup failed; both authoritative cells
  now run inside `dbus-run-session` as well as their nested display;
- GitHub's runner account has no active logind seat, so the real
  `systemd-inhibit --what=sleep --mode=block` call was denied. The full workflow
  installs a reviewed polkit rule granting only user `runner` and only
  `org.freedesktop.login1.inhibit-block-sleep`; its exact content is hashed and
  mutation-tested by the workflow contract. No fake inhibitor replaces the
  production integration;
- the custom-sound test relied on a source-tree Ghostty library that is absent
  in a clean public checkout. Its matrix and architecture commands now select
  the immutable ReleaseSafe bundle explicitly; and
- the real interrupted-write child could finish its 8 MiB publication between
  public polling intervals. The harness now writes 64 MiB, preserving a wide
  observable interruption window without adding another watcher or mock. Three
  consecutive local SIGKILL journeys passed.

Valgrind again retained paired raw and post-suppression evidence. The public
raw receipt SHA-256 is
`40300a5c8349bd5d5c66d4da38c02567d4ff7979f03f68019b8f747dcaf6e4b3`
and reports 414 errors/contexts, 5,936 definitely lost bytes, and 28,043
indirectly lost bytes. Its post-suppression receipt SHA-256 is
`82de13b66d999113993cbd08a95672b450f0917f2883cbd6c2728bf9d1ce7d33`;
the retained machine-readable report contains zero post-suppression errors or
leak bytes. Governance rejected the run because the hosted Fontconfig/Pango
cache graph used the same narrowed
roots but split descendants differently: metrics roots 2/9,552 bytes, node
1/2,880 bytes, strings 20/1,836 bytes, and children 30/960 bytes. The preceding
public run independently recorded the same graph. The manifest now admits
these exact reviewed observations while retaining root co-occurrence,
scenario restrictions, and maximum root counts. No suppression pattern was
broadened.

A fresh local non-Ghostty GTK/IBus run after the manifest correction retained
its unsuppressed receipt with 427 errors/contexts, 6,080 definitely lost bytes,
and 41,395 indirectly lost bytes. The paired suppressed receipt contained zero
errors and zero definite/indirect bytes; governance passed across the effective
inherited Ghostty and project suppression set. This result is **PASS with
reviewed suppressions**, not an unsuppressed clean result. ReleaseSafe Valgrind
remains XFAIL. Remaining uncertainty is whether serial public execution plus
the narrowly authorized logind operation is sufficient; only the next clean
public full receipt can answer that.

After committing the fourth-run repairs, a clean `linux/tests/qualify-local`
run at exact commit `2f2cbef8` passed every presently executable support and
matrix cell in 1,031,890 ms. This rerun includes the modified installed-product
journeys on nested X11 and nested Cage Wayland, the 64 MiB real-child SIGKILL
recovery journey, custom-sound import against the immutable ReleaseSafe bundle,
and Valgrind governance. The Debug Valgrind result is **PASS with reviewed
suppressions**. Declared totals remain 161 PASS, 0 FAIL, 5 BLOCKED, 1 XFAIL,
and 14 NOT_IMPLEMENTED, so implemented local qualification passed while release
and full Linux qualification remain NOT_PASSED.

The replacement public gate for documentation commit `76d890d3` then exposed
a distinct real Wayland focus race in `shortcut-binding-runtime-wayland`. Its
retained log proves the first two-character readiness query delivered only one
`z`. The harness closed the partial palette but immediately sent the next
opening chord before GTK had returned focus to the terminal. That chord was
lost; worse, the nominal three-attempt loop aborted on a missing `shown` receipt
instead of allowing its next attempt. This was a harness failure, not a product
pass, and public run 32136889427 remains failed.

The readiness state machine now waits for both the compositor-visible palette
close and the subsequent real `focus-pane` transition before retrying. A lost
opening chord proceeds to the next bounded attempt rather than aborting the
loop. The orchestration contract preserves both requirements. Three consecutive
real nested-Cage Wayland journeys and one nested-X11 journey passed after the
repair. This focused local evidence does not replace the next public receipt.

Public replacement gate 32138712649 showed that the focus wait was necessary
but not sufficient: its physical `zz` probe again arrived as one `z`, and the
first physical Escape event was itself lost before any `command-palette=hidden`
transition. The new wait correctly refused to accept that absence as a close,
but treated it as fatal instead of retryable. The close boundary now sends at
most three compositor-visible Escape events, stopping only after a newly
counted hidden transition, and then still requires the subsequent terminal
focus transition. This is bounded input recovery, not a sleep or a fabricated
application receipt. The orchestration contract rejects removing the bounded
Escape retry. Three further real nested-Cage journeys passed locally; run
32138712649 remains failed and the next public run remains authoritative.

Public gate 32140781822 invalidated that diagnosis. It delivered all three
Escape events, but none reached the palette: after the first `z`, a deferred
Ghostty surface-focus callback had selected the pane and refreshed its
presentation while the palette overlay remained visible. This was a product
focus-ownership bug, not unreliable physical input. The preceding harness
retries were therefore removed rather than allowed to accrete.

Deferred surface-focus callbacks now refuse to apply while either global search
or the command palette owns focus. Command-palette presentation also confirms
entry focus from the GTK idle queue, after already queued surface callbacks,
and the journey waits for that product-owned receipt before typing the actual
command. A unit contract covers shutdown, both overlay owners, absent surface
focus, and the valid terminal case. The orchestration contract rejects
reintroducing the sacrificial `zz` query. After rebuilding the real ReleaseSafe
bundle, three consecutive nested-Cage Wayland journeys and one nested-X11
journey passed. Run 32140781822 remains failed; only its replacement can prove
the hosted behavior.

## Fifth public run: focus repair passed; full-suite defects remain explicit

The replacement public gate for exact commit `03e50a2e`
<https://github.com/TamedTornado/zentty/actions/runs/32143657847> passed in
19 minutes 18 seconds. This is the first hosted receipt proving the product
focus-ownership repair; it does not establish full qualification.

The corresponding serial full run
<https://github.com/TamedTornado/zentty/actions/runs/32143658025> failed after
4,466,510 ms. It retained 161 declared PASS cells, 0 declared FAIL, 5 BLOCKED,
1 XFAIL, and 14 NOT_IMPLEMENTED, while actual execution failed 14 cells and
dependency-blocked two more. The failures were installed-product on both
backends, clean package reproducibility, agent integration on both backends,
the architecture contract, both X11 development-server cells, X11 Open With,
X11 workspace/pane settings, X11 agent settings, Zsh integration on both
backends, and suppression governance. The run therefore establishes neither
the implemented-local, release, nor full-Linux claim.

The architecture failure was deterministic: the focus repair added
`surface_focus_event_should_apply` but omitted it from the authoritative pane
runtime ownership inventory. The ownership mirror now names that function and
both the positive validator and its mutation/negative suite pass.

The retained Debug IBus report preserves an unsuppressed receipt with 414
errors/contexts, 5,936 definitely lost bytes, and 28,043 indirectly lost bytes.
Its SHA-256 is
`0e62ab9694cbb6cc30bb80919c394a2bc50b5471fff3a9bf1f0017bab40c724d`.
The paired post-suppression receipt reports zero errors and zero definite or
indirect bytes; its SHA-256 is
`e84cd565ca998b150e57eeb9809a44aeee8da619de657177aba35457c7fd2433`.
Governance rejected, rather than silently accepting, a new
13,253-byte observation for the same two narrowed Pango layout roots on the
same pinned Fontconfig 2.15.0/Pango 1.52.1 public environment. The reviewed
scenario lower bound now includes that exact observation; context count,
maximum bytes, consumer stack, child-root co-occurrence, and allowed scenarios
are unchanged. The governance mutation suite passes. Any successful Debug
result remains **PASS with reviewed suppressions**, never an unsuppressed clean
result, and ReleaseSafe Valgrind remains XFAIL.

Both installed-package journeys completed the direct launch, authenticated CLI
mutation, real clipboard/Open-With action, injected crash, and restart before
the desktop-entry phase exited. The same exact public package and desktop-entry
phase passed locally under a private Xvfb, D-Bus session, and namespace. The
public artifact did not include `build/linux/installed-package`, so the decisive
`desktop-product.log` was unavailable. Full CI now uploads that directory on
success or failure. No speculative product or package change is justified
until a replacement public receipt retains the missing evidence.

The clean-clone comparison proved that only
`libghostty-gtk-embed.so` differed between otherwise identical package payloads.
Two same-source builds with distinct install directories and a third build
from a distinct local Ghostty clone produced byte-identical normalized
libraries locally. The current public evidence therefore does not support a
source-path or install-prefix diagnosis. On the next mismatch the package
journey retains both normalized libraries plus their ELF notes, in addition to
the existing payload and provenance diffs, so the defect can be diagnosed
rather than guessed away.

The serial full run legitimately exceeded the preflight receipt's former
one-hour validity window. Full-profile receipts now remain valid for three
hours, below the workflow's six-hour hard timeout; the shorter public-PR profile
remains one hour. Tests reject a four-hour full receipt and a two-hour public-PR
receipt while preserving exact commit, Ghostty revision, environment manifest,
tool versions, package versions, prerequisites, and secret scanning.

Several hosted X11 settings failures show that fixed pointer coordinates are
not portable across runner font/layout state. An attempted replacement using
semantic GTK focus receipts was exercised repeatedly against the real nested
X11 product and failed to reach the intended dynamic controls; those
experiments were reverted rather than left as an unproven parallel harness.
The existing failures remain visible and unresolved. The real Gemini journey
also discarded its decisive product log because `set -e` exited directly from
a ten-second wait helper. Its installed-Gemini readiness boundaries now allow
60 seconds on the two-core hosted runner and route timeout through the test's
evidence-preserving failure path. No agent component is mocked: the real pinned
Gemini CLI, controlled loopback model endpoint, installed hooks, reducer, PTY,
and restore path remain in the journey.

The complete local real-agent journey passed with the installed Gemini 0.55.1
binary after the evidence-preserving wait repair. An intentional run demanding
the public pin 0.53.0 failed at the version boundary before exercising hooks,
so local evidence is not misreported as a test of the public binary. The next
hosted run remains authoritative for pinned Gemini 0.53.0.

The public Zsh logs already contained the product-owned shell-integration
`state=prompt` receipt even though util-linux `script --flush` did not publish
the literal prompt transcript within 20 seconds. Shell readiness now waits for
that semantic receipt, while the subsequent compositor-typed command and real
environment receipts still prove interactive PTY input. This removes a
transcript-flush race without converting missing shell readiness into a pass.

## Isolated repair pass before another full qualification

The first clean local rerun at `cd0fcb10406d` completed in 1,220,220 ms with
161 PASS, zero actual failures, 5 declared BLOCKED, 1 XFAIL, and 14
NOT_IMPLEMENTED cells. It established that the evidence-boundary repairs did
not leave the local suite red, but it ran before the hosted-X11 interaction
repair below and is therefore not the final qualification receipt for that
change.

The exact failed public artifact from run 32143658025 was downloaded and its
individual logs inspected before changing another test. They confirmed that
the five X11 settings failures were not a common product exception: each
fixed-coordinate interaction reached either the wrong rendered row or no
control when the hosted runner's font metrics differed. The affected cells
were Dev Servers with and without Docker, Open With, Worklanes & Panes, and
Agents. The repair removes those layout coordinates and drives the real GTK
controls through the settings window's physical keyboard focus chain,
mnemonics, stable widget identities, and resulting persisted configuration.
This is the same product and the same native controls; it does not call an
internal settings setter or add a second automation authority.

The Open With conversion initially failed because a focus controller attached
to `GtkCheckButton` did not receive focus when GTK focused its internal toggle
child. The settings shell's existing global focus receipt already reported the
stable widget identity, so the redundant experimental controller was removed
and the test now consumes that one existing receipt. A second attempt reached
the controlled target but failed after the native file chooser because the
settings toplevel had not regained X focus. Explicitly restoring focus to the
real settings window before keyboard traversal repaired that boundary. The
isolated real X11 Open With journey then passed under nested X11 session
`51183b42f1c7da5c7e53a9fcdf2408f42bd297490f10e8565cf81a412c64ea81`.

The first semantic Dev Servers attempt reached every dynamic browser control
but exposed that a GTK Entry focuses its internal `GtkText`, not the Entry's
widget name. The product page now gives the visible “Port or range” label a
real mnemonic associated with that Entry, which improves keyboard
accessibility and provides a layout-independent physical-input path. Dynamic
browser and ignored-port removal buttons also have stable widget identities.
The non-Docker X11 journey passed in session
`afe4d5856f49118a45dc1f392d94675fca3e7af3005ee3a0f75373aa72473b9c`.
One attempted Docker run was rejected before product launch because the
operator command accidentally duplicated part of the required image digest;
it was not recorded as product evidence. The corrected content-pinned Docker
journey passed in session
`f56c209317f07f2dc82ed1849763fd254baf59bcbf40924f71668343b4d69df0`.

The Worklanes & Panes journey passed after both backends were placed on the
already-present mnemonic path; the isolated X11 session was
`0acd21c91fe73943493b0c3f4ace73e92827986adfefdd33436a7d2f2789726e`.
The Agents test now reaches the rendered Codex switch through the same stable
focus chain for both the rejected malformed-config write and the subsequent
accepted write. Its isolated X11 session
`18b217846c2c44e4994bc98037f5fae88b37e24e956cf388852248039e9eb39d`
passed the transactional rollback, persistence, live refresh, wrapper, and
tmux-team boundaries.

The installed-package journeys were also rerun independently rather than via
the matrix. X11 passed with session
`d8c0a0e1dc6ba2e8e07756482f084f70694cb52111c65165fe2780b84d26bb0a`;
Wayland passed with session
`75f88f139acf220167c28d99e4a7ac4f48f49928efa8b809ac43adce441486e7`.
During the Wayland run the host's GNOME portal backend repeatedly exited with
signal 11, but xdg-desktop-portal selected the GTK backend and the complete
installed-product command returned zero. This is retained as environmental
noise, not silently promoted into evidence that the GNOME portal is healthy.

The clean-checkout reproducibility cell correctly refused to run while these
repairs were uncommitted. It must be rerun from the resulting clean commit,
followed by the pinned hosted Gemini cells and the other repaired regression
cells, before another complete matrix invocation. No release or full-Linux
qualification is claimed by these isolated passes.

After commit `5f1d6e7aeb69`, the first reproducibility invocation reported that
the exact primary artifact was missing. The preceding package builder was
still running even though its interactive command had yielded, so this was an
operator sequencing failure rather than a package comparison result. The
builder was allowed to finish and the exact-commit artifact was verified
before retrying. The clean-checkout comparison then returned zero in isolated
session
`2c1adcc34d44f3890067922e6defcad625dbab378fe64a10cedf297de3d85321`;
the package reproducibility cell is locally green for the repair commit.

The real Gemini boundary was then exercised independently with the exact
content-installed 0.53.0 CLI used by public CI, not the workstation's newer
0.55.1 executable. The X11 journey passed in session
`48582492a2a5ad3b22592b41ff226b3320e8913b3278bcac75dc8f802cc95494`;
the Wayland journey passed in session
`48d694bdd03214fc06ee5aef45b2621e3c9de070a39eab41c4eea62b23ca7b5b`.
Both retained the real CLI, controlled loopback model endpoint, installed
hooks, reducer, PTY, and Codex lifecycle rather than substituting an agent
mock.

The remaining repaired regression boundaries were also kept isolated. The
architecture contract and its negative validators passed. Valgrind
suppression governance and its mutation suite passed against the retained raw
and post-suppression receipts. Zsh physical shell integration passed on X11
session
`dfef0150823b4757883b0894dd5877fd980958522cbd25e2d5fe0a797b5dc070`
and Wayland session
`da29cad2ddd397c887bb2df67002698296daf15d2eabef4e0cbd1efce3ae2e43`.
Every known failure group is now green in its smallest locally controlled
integration journey. The next permitted step is one complete local matrix
run, followed by the authoritative hosted run; a hosted pass is still required
before declaring the public failures repaired.

## Full-local failure and focused GTK repair

The next complete local qualification run at commit `9002476c` took roughly
19 minutes. Its declared inventory was 161 PASS, 5 BLOCKED, 1 XFAIL, and 14
NOT_IMPLEMENTED; its executable result was 159 PASS, 1 FAIL, and 1
BLOCKED_BY_FAILED_DEPENDENCY. `workspace-pane-settings-x11` failed and its
`platform-settings-contract` aggregate was consequently blocked. Implemented,
release, and full-Linux qualification claims were all correctly false. This
was a real red full run, not an exhaustive-QA pass.

The isolated failure initially looked like another coordinate problem, but
several attempted repairs exposed two distinct GTK input semantics. First,
the Settings window's activation callback always re-focused the Shortcuts
search field even when the window had been deep-linked to Worklanes & Panes.
Under scheduler timing, that stole focus after a mnemonic had reached the
intended control. Settings now gives initial and activation focus to the
Shortcuts search only while the Shortcuts section is current; other sections
retain their own child focus. The generic activation receipt remains visible
and now records the current section instead of silently changing it.

Second, a `GtkDropDown` mnemonic opens its native popup and transfers focus to
an internal list item, whereas a switch mnemonic activates the switch
immediately. Addressing follow-up keys to the Settings toplevel, or re-focusing
that toplevel after the mnemonic, steals/bypasses the popup grab. The journey
now sends the dropdown selection through XTEST to the current native focus and
does not send an extra Space after a switch mnemonic has already applied the
change. The Agents journey had the same addressed-input defect after keyboard
traversal: its Space event was routed into the sidebar search rather than the
focused Codex switch. It now sends Space through XTEST to the physically
traversed GTK child.

During diagnosis, one experimental product build failed to compile because it
attempted an invalid GTK root downcast. A yielded build command then allowed a
stale staged binary to be exercised in subsequent attempts. Those attempts
are invalid evidence and are not cited as passes. Later iterations explicitly
waited for build completion and checked each nested environment receipt before
using its result. Bounds-based pointer experiments and temporary key traces
were removed rather than retained as a second automation path.

The final Worklanes & Panes journey passed three consecutive isolated X11
sessions:

- `1b916e96a5177d04cb9eecce5b96fe068af07bb19c1ea68134812a4ee6eca35c`
- `a503171369535ee3f8e45bf2626e5d50ef07b97be663163483fe77840c2d229a`
- `66f196eba92cdf2a2cdac40ea0ae274cb6f69272f22d63be31cec3dd41f9ff77`

Because the focus repair is shared by every Settings section, the other known
hosted failure groups were rerun against the rebuilt product rather than
assuming their earlier receipts still applied. Open With passed session
`f9642aad2986c9c0f558470dcec7909a5474c79a905ca19f04d2734200baa30d`;
Dev Servers passed normal and pinned-Docker sessions
`7a8d4c305ca840fe0ee01c12ca304b73d21023b5cc4116620224a0ae40749ea4`
and `8513ff5422404f3cf32265c2c274eb71c7e0274c720ef510f7618a531a144e48`;
Agents passed session
`799085cc45147097b488d862b212fbf61ea1c6904d4f72242d0524e687dbf620`.
All presently known isolated failure groups are green. Only now is another
complete local qualification run permitted.

The permitted full run at clean commit `a8e8a607` completed in 1,024,760 ms.
It again correctly refused every qualification claim: 159 executable cells
passed, `agent-settings-runtime-x11` failed, and its aggregate dependency was
blocked; the 5 declared BLOCKED, 1 XFAIL, and 14 NOT_IMPLEMENTED entries
remained explicit. The Worklanes & Panes cell that failed the preceding run
was green, as were Open With and both Dev Servers cells.

The retained Agents log proved that the malformed-config action reached the
Gemini switch although the journey had stopped traversal when it observed the
Codex focus receipt. The traversal loop could enqueue another addressed Tab
before GTK had processed the preceding Tab; under full-suite load, observing a
target receipt did not prove that no later Tab remained in flight. The earlier
isolated pass was therefore a scheduler-dependent false green.

The repair adds one semantic receipt when the Settings shell has actually
processed Ctrl+F, then requires an acknowledged GTK focus transition after
each individual Tab before the next Tab may be injected. Space remains a real
XTEST event delivered to the resulting focused switch. Three consecutive
isolated Agents journeys passed after this repair in sessions
`7e4d3dd8f03db81c98332ee9f27ab9855392b96f308818277e8f42d99620afa4`,
`2f53bb021e883b4a829c023605dde046dbb3882a93125e1099ddab5ca813c2bc`,
and `d1ae373e167b6c7911303f7a23abf2605046096e30d4b170074a3c607dd4d358`.
This is synchronization with the real GTK event stream, not a delay or an
internal control mutation.

The complete local rerun at clean code commit
`26720a5ea053fe2ae8339155296bbe4fee7395e1` returned zero in 1,004,930 ms.
All 161 presently executable PASS cells passed, the tracked ABI defect remained
the sole XFAIL, and all 5 BLOCKED plus 14 NOT_IMPLEMENTED declarations remained
explicit. The implemented local suite and product-boundary claims are true;
release and full-Linux qualification remain false because those declared gaps
have not been erased or converted into passes. Suppression governance was
accepted. The Debug IBus Valgrind receipt records 427 raw errors/contexts,
6,240 definitely lost bytes, and 41,461 indirectly lost bytes; after the
reviewed effective suppression set it records zero errors/contexts and zero
definite/indirect bytes, with exactly 427 suppressed errors/contexts. This is
**PASS with reviewed suppressions**, not an unsuppressed clean result.
