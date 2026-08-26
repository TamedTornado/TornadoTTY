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
