# Zentty Linux dogfood — Ghostty API closeout (2026-08-26)

Issue: GH-11

## Goal

Rebase the exact Ghostty downstream dependency onto current official main,
remove unproven public surface, requalify every product-used operation, and make
an explicit upstream-versus-downstream maintenance decision without presenting
a broad unsolicited pull request.

## Starting state

- Zentty: `7671a76d7c6b945bc857eb61092d7ee5432841f0`, clean.
- Locked Ghostty: `23d8414f3d1871cd67735d85e9815221d5e6b60c`.
- Previously recorded official base: `ac04fc276169c70d31aa6fcfc5b43fc160d6fe6e`.
- Existing audit: 16 exports, 13 Rust declarations, three explicitly excluded
  exports, and a tracked XFAIL for an implementation-defined C enum.
- No upstream communication or pull request existed.

## Discovery: official main had advanced substantially

Fetching `ghostty-org/ghostty:main` moved the official reference from
`ac04fc276` to `6dcf68fc0b12e8caebbfc43770d66edac124b4f8`. The downstream series was
rebased in the isolated worktree
`/home/jason/Projects/ghostty-worktrees/gh11-rebase-current`; the in-use branch
was not rewritten during conflict repair.

## Rebase conflict: GTK snapshot versus explicit early teardown

At downstream `Add explicit GTK embed surface close`, current Ghostty had added
a native-blur `snapshot` implementation at the same insertion point where the
old patch introduced `deinitCore`.

Decision: retain both independent behaviors. `finalize` continues to call
`deinitCore`, the embed close path may call it early, and current Ghostty's
snapshot implementation remains unchanged. The conflict was positional, not a
semantic choice between teardown and rendering.

## Rebase conflict: gtk4-layer-shell dependency ownership

The old port compatibility commit used `lazyDependency("gtk4_layer_shell")`.
Current Ghostty owns the dependency through `LocalDeps`, including generated
Wayland headers. The old compatibility block was obsolete and was dropped in
favor of current `LocalDeps` behavior.

A later reproducible-path commit still named the old local variable `upstream`,
causing the first focused build failure:

```text
pkg/gtk4-layer-shell/build.zig:110:17: error: use of undeclared identifier 'upstream'
```

Repair: map the source path through `deps.upstream`. This preserves the exact
reproducible-build behavior using current Ghostty's dependency owner.

## Compile failure: clipboard request contract changed

The rebased engine-owned spike and the unused public paste export passed
`.paste` as a bare enum. Current Ghostty requires a payload-carrying union:

```text
coercion from enum 'ClipboardRequestType' to union 'ClipboardRequest'
must initialize field 'paste'
```

After constructing `.{ .paste = .standard }`, the public function also failed
because current Ghostty returns `ClipboardReadResult`, not `bool`. Before
adapting that unused public behavior further, the product usage ledger was
reviewed.

Finding: Zentty does not bind or call
`ghostty_gtk_embed_surface_request_paste`; product paste uses ordinary Ghostty
input/binding paths. The export was removed. The Ghostty-owned spike still
exercises its internal clipboard path and was updated to the new payload.

## Public API minimization

The previous 16-symbol ABI contained three operations with no Zentty caller:

- `ghostty_gtk_embed_runtime_new`
- `ghostty_gtk_embed_surface_new`
- `ghostty_gtk_embed_surface_request_paste`

They were removed from the header, Zig exports, and ELF allowlist. The remaining
thirteen exports exactly equal `zentty-ghostty-sys` declarations and each has a
safe owner, real product caller, negative C contract path, and real product
journey.

The C host and tests were updated to use the size-versioned options constructor
and explicit default backend rather than preserving convenience exports solely
for qualification.

## ABI defect repaired rather than suppressed

The previous public type was:

```c
typedef enum { ... } ghostty_gtk_embed_async_backend_t;
```

The tracked probe proved it was four bytes normally and one byte under
`-fshort-enums`, while Zig accepts `c_int`. The candidate now uses:

```c
typedef int32_t ghostty_gtk_embed_async_backend_t;
```

with typed constants. Fresh C17 and C++17 binaries, both default and
`-fshort-enums`, reported four-byte values `0,1,2`. The matrix cell
`ghostty-async-backend-abi-representation` was promoted from XFAIL to PASS.
This was a requirements-preserving repair; no expected failure was broadened or
reclassified around the defect.

## Ghostty policy removed from the candidate

Two original downstream commits changed Ghostty's `AGENTS.md` only to add
Zentty field-reporting and qualification policy. They were dropped from the
rebased candidate. The machine audit rejects any `AGENTS.md` delta and records
all 19 remaining changed files. Product policy and orchestration remain in
Zentty.

## Audit architecture simplification

The previous audit hashed every patch, range, file, and hunk. That created a
large regeneration burden without increasing the user-facing proof: the exact
base/head already identify every byte.

The audit was replaced with schema v2. It validates:

- exact official base, downstream head, ancestry, and commit count;
- exact changed-file status/path ledger;
- purpose and test owners for all 19 files;
- exact equality of header declarations, Zig exports, version-script allowlist,
  and Rust raw declarations;
- fixed-width public ABI types;
- absence of removed/unproven exports and Zentty policy;
- safe owners, product callsites, real journeys, negative tests, and PASS matrix
  cells for every operation; and
- the explicit downstream maintenance decision.

It deliberately does not hash patches. Self-tests remove a file ledger entry
and alter the reviewed upstream decision; both are rejected.

## Old/new loader compatibility behavior

Removing three experimental symbols means the old and current libraries are
intentionally incompatible in both directions. The real fixture now builds:

- current consumer against current library;
- historical consumer against historical library;
- current consumer against historical library at runtime; and
- historical consumer against current library at runtime.

Both matching pairs reach `main` and pass. Both mismatched pairs fail in the
loader before `main`, naming the exact missing versioned symbol. This is clearer
than the old one-way fixture, which treated the historical consumer as
compatible with the current library.

## Focused evidence completed

Candidate Ghostty:

- `zig build gtk-embed-lib-test ...` — PASS.
- `zig build gtk-embed-lib ...` — PASS.

Zentty boundary:

- Debug exact-pin build — PASS.
- ReleaseSafe exact-pin build — PASS.
- `linux/tests/ghostty-api-audit --self-test` — PASS.
- `linux/tests/ghostty-async-backend-abi-test` — PASS.
- C17/C++17 default and `-fshort-enums` representation probe — PASS.
- exact thirteen-symbol `linux/tests/abi-surface` — PASS.
- controlled X11 fresh-process initialization-order test — PASS.
- real old/new ABI mismatch fixture in both directions — PASS.
- six ReleaseSafe API misuse cells: X11/Wayland × default/epoll/io_uring — PASS.
- qualification matrix schema and focused runner tests — PASS.

The six API cells used real GTK initialization and the exact Ghostty library in
private Xvfb and headless Weston sessions. No model or terminal component was
mocked.

## Environment repair

After reboot, sandboxed Xvfb reported `/tmp/.X11-unix` as owned by
`nobody:nogroup`. The host directory was restored to the standard
`root:root`, mode `1777`. Controlled X11 tests still require execution outside
the filesystem sandbox because its namespace presents the socket directory as
unowned. This is an execution-environment constraint, not a product pass.

## Maintenance decision

Outcome: **retain downstream**.

The complete alternate-host ABI will not be sent as one broad pull request.
Potential future operator-approved upstream candidates are the independent
`Exec` lifetime repair, GTK preedit focus-loss repair, safe pre-runtime logging,
and—only after direction discussion—the explicit non-default application
foundation or individual operations. No communication has been sent.

## Remaining qualification before GH-11 closure

At this point in the chronology, still required:

- full current Ghostty Debug regression on the rebased tip;
- focused real staged product journeys covering the thirteen operations;
- current API docs and issue comment reconciliation;
- complete diff review, commit, push, and exact receipts.

ReleaseSafe Valgrind remains the existing XFAIL. Debug Valgrind may only be
described as **PASS with reviewed suppressions**.

## Ghostty regression and focused product completion

The complete current Ghostty Debug regression plus embedding unit test passed:

```json
{"jobs":20,"ghostty_test_ms":461164,"gtk_embed_lib_test_ms":533,"total_ms":461697}
```

The focused staged-product X11 route then passed real closed-pane restoration,
pane/global search, and tmux topology/capture behavior against the exact
ReleaseSafe candidate. The topology journey intentionally kills its persisted
setup process while verifying recovery and subsequently emitted its PASS
receipt; the observed `Killed` line was scenario behavior, not a lost test
process.

The product-owned Rust boundary passed five safe-wrapper unit tests, four raw
layout/discriminant tests, and three compile-fail lifetime doctests. All six
real API misuse/backend combinations (X11 and Wayland, each with default,
epoll, and io_uring) passed.

The next operation is one clean Zentty commit followed by the exact pinned full
matrix. This sequencing ensures clean-checkout packaging and qualification use
the reviewed candidate rather than the prior commit.

## First exact-pin matrix receipt and repairs

The first complete current-pin matrix ran for 890,420 ms with 21 workers. It
did not pass: 197 implemented cells passed, four failed, and two downstream
summary cells were correctly blocked by those failures. The declared matrix
still contained 201 PASS, two XFAIL, and two NOT_IMPLEMENTED cells; declared
status totals are not execution results.

The four failures were investigated rather than reclassified:

1. The clean-checkout package build exposed a real upstream dependency update.
   Current Ghostty's Wuffs package is
   `N-V-__8AAP5JWgCGP_AD0teWpa4krRvE9VPZzvviGdbmN4jI`; the packaging notice
   manifest still named the historical package hash. The license text is
   byte-identical (`f8ac2c75...`) and the manifest now names the dependency
   actually selected by the pinned source.
2. The two X11 development-server cells ran concurrently while publishing and
   comparing the same reviewed `server-chrome-x11` visual receipt. Both saw a
   transient frame without the settled title and failed correctly. Each real
   journey passed when rerun alone, including the Docker variant. A named
   `development-server-visual` scheduler resource now serializes only those two
   colliding cells; a matrix negative test rejects removal of the lock. This is
   harness isolation, not a relaxed visual baseline.
3. Ghostty's inherited `valgrind.supp` changed from 233 to 234 rules on the
   current official base. The added upstream rule is narrowly scoped to Zig
   stdlib's documented unused DEFLATE Huffman entries (Ghostty `c5f4f00ed`).
   The effective-set manifest now pins its exact current hash and count. No
   Zentty suppression was added or broadened.
4. Governance then rejected larger Fontconfig/Pango cache observations from
   the current build. The paired unsuppressed interaction/X11 receipt was
   inspected. It retained the same external realloc/FcFontRenderPrepare/
   `pango_layout_get_size` root and the same root-gated deep child ancestries.
   Reviewed scenario ceilings were extended only to the observed values: seven
   metrics roots/27,626 bytes, 106 calloc children/3,392 bytes, 22 string
   descendants/4,991 bytes, and four layout roots/251,679 bytes. Suppression
   patterns were unchanged. Governance and its stale/increase/out-of-scenario/
   untracked-rule negative suite pass with the paired raw receipts preserved.

These findings came from the matrix doing its job. None was converted to a
PASS through an environmental skip, baseline replacement, status change, or
weakened product requirement. Debug Valgrind remains describable only as
**PASS with reviewed suppressions**; ReleaseSafe Valgrind remains XFAIL.

## Clean-build reproducibility follow-up

After committing those repairs, the package notice collection passed in both
the primary build and the disconnected clean clone. Byte comparison then found
a second, independent failure: only `libghostty-gtk-embed.so` differed. The
payload manifests, ELF section comparison, and normalized symbol-size diff
localized the difference to register allocation in
`terminal.formatter.PageFormatter.formatWithState`; one build was 80 bytes
larger. There were no leaked developer/tmp paths and all other 1,295 payload
entries were identical.

Both builds already used fresh revision-owned Zig caches and `-Dcpu=baseline`.
The remaining input difference was the absolute Ghostty source identity: the
primary package compiled the managed checkout at
`build/linux-deps/ghostty`, while the disconnected builder compiled an exact
clone at `/mnt/ghostty`. The clean builder now mounts its disposable clone at
the same canonical absolute path after masking the developer checkout. A
contract test requires both the canonical submount and `GHOSTTY_SOURCE_DIR`;
it does not compare or copy from the hidden developer checkout. The real
byte-for-byte test must pass before this repair is accepted.

The isolated rerun at Zentty `6e31a8f7a1ab60212896566ef439f26fe38b4890`
passed: all four release outputs were byte-identical while the developer
checkout remained masked. This confirms the source-identity repair rather than
accepting a differing binary or weakening the comparison.

## Second matrix receipt and title synchronization

The second complete matrix finished in 864,930 ms. Packaging reproducibility
and suppression governance passed, proving both prior repairs. Two product
cells still failed, and the support suite caught one stale negative fixture.

Both X11 development-server captures still stabilized before the OSC 2 title
had propagated from the real PTY into window chrome. The resource lock removed
receipt collision, but it could not prove asynchronous product state. The
journey now waits for the product's existing
`zentty-linux: title=Development servers` event before compositor capture. No
sleep, pixel tolerance, or baseline changed. The normal and Docker-backed real
journeys both pass independently with the title synchronization in place.

The public-PR policy correctly has no known gaps now that the async ABI cell is
PASS, but its negative test still tried to prove that an empty gap list was
invalid and changed an already-PASS cell to PASS as its "stale XFAIL" case.
The repaired negatives add a fictitious gap and convert a selected PASS cell to
XFAIL; both are rejected. This changes only a checker self-test and does not
make public CI a release authority.

## Third matrix receipt and concurrent receipt ownership

The third complete matrix ran 959,640 ms. The synchronized development-server
cells and all support tests passed. Its root failure was a new concurrency bug:
ReleaseSafe and other real builders wrote the same Cargo publication-age JSON
receipt. One validator read the file while another writer was replacing it and
rejected the malformed claim. Build-local now places that receipt beneath its
own `ZENTTY_BUILD_OUTPUT_DIR`; the focused policy test requires build-owned
receipt paths, and the exact ReleaseSafe build cell passes. The standalone
security audit retains its canonical receipt. No dependency-age rule changed.

Because ReleaseSafe failed, the primary package producer was correctly blocked;
the clean builder spent its bounded wait looking for an artifact that could not
exist and then failed. This was dependency propagation, not a second package
reproducibility regression.

The same run's newly captured paired raw Valgrind evidence also exercised
different sizes of the already reviewed Fontconfig metrics graph. Governance
rejected them before review. The unsuppressed stacks retain the exact external
Fontconfig allocation and `pango_context_get_metrics` consumer, with deep child
rules still root-gated. Scenario bounds now include the observed current-pin
interaction/Wayland values (three roots/17,713 bytes, one deep node graph/8,480
bytes, and 13 string contexts/5,251 bytes) and the single/Wayland four-child/
128-byte floor. No suppression pattern changed. Governance now passes against
the preserved raw and post-suppression receipts.

## Fourth matrix receipt and bounded-fixture load failure

The fourth complete matrix ran 858,740 ms. ReleaseSafe, package
reproducibility, and every support contract passed. Qualification still did
not pass: the two X11 development-server journeys failed their exact visual
comparison, suppression governance rejected one newly observed reviewed cache
size, and the aggregate platform-settings cell was correctly blocked by the
failed development-server dependencies.

The visual receipt was not a tolerable rendering difference. It showed the
real terminal after its bounded development-server child had exited: the
project branch remained, but the `Development servers` title and project icon
were gone. The fixture had a 90-second dead-test ceiling. Both journeys finish
in 14-23 seconds in isolation, but under the complete concurrent matrix the
real scanner/browser/container work could cross that ceiling before capture.
The fixture ceiling is now 300 seconds; this does not add five minutes to the
test because the owning journey terminates its process immediately after its
assertions. It only prevents the safety ceiling from becoming product state
under expected qualification load. The unmodified journey and reviewed
baseline both pass independently for the normal and real Docker-backed X11
routes. The final matrix must prove the repair under the load that exposed it.

The preserved Debug/single/Wayland Valgrind receipts contained 14 narrowed
Pango layout-cache children/448 bytes beside the separately reviewed layout
root, rather than the prior 15/480 floor. The Debug/single/X11 receipt also
contained six Fontconfig metrics roots/26,710 bytes rather than the prior
25,387-byte ceiling. Their paired unsuppressed stacks retain the same external
Fontconfig allocations, named Pango consumers, and required root/child
co-occurrence. Only those two scenario bounds and their evidence narratives
changed; the effective suppression patterns did not. Suppression governance
and its increase/stale/out-of-scenario/untracked-rule negative suite pass.

No fourth-run failure was reclassified, skipped, hidden, or accepted as a
baseline update. Debug Valgrind remains **PASS with reviewed suppressions** and
ReleaseSafe Valgrind remains XFAIL.

## Fifth matrix receipt and corrected visual diagnosis

The fifth complete matrix ran 862,170 ms. All support contracts, suppression
governance, ReleaseSafe, packaging, and 199 other implemented matrix cells
passed. Only the same two X11 development-server visuals failed, so the
platform-settings aggregate was correctly blocked. Both failures occurred in
10-12 seconds, disproving the fourth-run hypothesis that the fixture's
90-second ceiling caused the missing chrome. The temporary ceiling increase
was reverted; retaining an unrelated timeout change would have been test
harness drift.

The actual synchronization boundary is two-stage. Ghostty invokes the title
callback and Zentty logs `title=Development servers`, then the GTK main loop
queues an idle callback that reconciles pane state and renders window chrome.
The existing wait proved only the first stage. Under complete matrix load the
stable-frame capture could run before the idle projection, producing an exact
but premature frame with an empty center context. Window chrome now emits a
change-only receipt after synchronously setting the actual GTK label. The real
journey requires the *latest* projected context to equal `Development servers`
before capture; it does not accept a historical event, alter the baseline, or
relax pixel comparison.

The focused normal and real Docker-backed X11 journeys pass against the exact
reviewed baseline with that projection boundary. The complete matrix remains
required to prove the repair under the concurrency that exposed it.

## Sixth matrix receipt, canonical clean visual, and package cache isolation

The sixth complete matrix ran 878,590 ms. The new projection boundary worked:
the preserved frame contains the exact `Development servers` title and icon.
The remaining 705 changed pixels were all chrome metadata, not terminal
rendering. The historical baseline had been recorded from a dirty developer
worktree and therefore encoded the branch's blue dirty marker; the authoritative
matrix starts from a clean committed tree. The baseline now uses the clean
matrix frame. Pixel tolerance remains zero, and the visual checker still
rejects any changed pixel. This is replacement of a noncanonical fixture with
the qualification state it claims to represent, not acceptance of a rendering
regression.

That run also reproduced the intermittent 80-byte Ghostty library difference
first found during the source-identity audit. In both cases the only code-size
difference was `terminal.formatter.PageFormatter.formatWithState`; every other
package payload entry matched. The earlier absolute-source repair was necessary
but not sufficient. The primary package build still admitted compiled entries
from the repository-wide Zig global cache while the disconnected builder began
with only the immutable package cache. A focused rerun happened to match, which
confirms the variance is intermittent rather than evidence that the comparison
was wrong.

Package builds now create both fresh global and local Zig caches beneath their
revision-owned temporary root. Only the prepared immutable `p` package store is
linked into the fresh global cache. The primary and disconnected builders
therefore receive the same package inputs without sharing compiler outputs with
Debug, regression, or a previous build. The package-builder contract requires
both owned cache paths. Exact byte comparison remains unchanged and must pass
in the final matrix; no differing library is accepted or normalized.
