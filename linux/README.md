# Zentty Linux product and qualification

This directory contains the real Rust/GTK Linux product's build, packaging and
external-boundary integration tests. The former C qualification application is
retired; retained C files are narrow dependency probes and are never packaged
as Zentty.

## Architecture

- The production shell is Rust + `gtk4-rs` as specified in
  [`docs/architecture/0001-rust-gtk4-linux-product.md`](../docs/architecture/0001-rust-gtk4-linux-product.md)
  and implemented in the root Cargo workspace.
- `ghostty.lock` pins the public Ghostty fork and exact embedding revision.
- `scripts/build-local` fetches that revision unless `GHOSTTY_SOURCE_DIR`
  selects an already-verified checkout, builds the shared embedding library,
  builds the Cargo product, and stages a relative `bin`/`lib` bundle under
  ignored `build/linux`.
- `tests/rust-product-smoke` and `tests/rust-product-lifecycle` launch that
  exact staged executable and assert behavior across Rust, the C ABI, GTK,
  Ghostty, PTY, renderer, and compositor boundaries.

The platform-neutral core mirrors ZenTTY's `WorkspaceRecipe` version 3,
`SessionRestoreEnvelope`, atomic snapshot/lifecycle files, migration,
meaningfulness filtering, restore-draft merging, and stale-generation rule.
The GTK product now restores and persists one source window with ordered
columns, vertical pane stacks, stable IDs, focus, and stored numeric geometry,
then proves a real relaunch from the result. Multiple windows, live divider
resize/persistence, CWD launch, corrupt-state recovery UI, and crash relaunch
remain, so the broad Wayland/X11 product-restore cells are still
`NOT_IMPLEMENTED`.

The delivered GTK binary currently has an interaction scaffold for the first
source-backed worklane shell slice: compound worklane cards with nested pane
rows and contextual rename, plus named GTK actions for worklane
creation/selection/rename/reorder/color and horizontal/vertical pane
split/close/four-direction movement, with one real Ghostty surface/PTY per
pane. Real action coverage is divided by behavior rather than repeated in an
application-embedded scenario: `rust-source-ux-x11` and
`rust-sidebar-management-x11` drive physical sidebar, palette, pane, history,
and reorder interactions, while `rust-session-restore` proves controlled X11
and input-capable Cage/Wayland persistence, background agents, physical visits,
clean relaunch, and cancellation. These scenarios reject an ambient desktop.
Exact divider
sizing, the source contextual cross-worklane move affordance, and the remaining
rich sidebar states are not implemented, so the broad product-worklane
qualification cells remain `NOT_IMPLEMENTED`. The underlying same-window move
model and parameterized GTK action are implemented and covered with a clean
snapshot; no temporary text toolbar button was added for that action.

The compound cards are not yet full ZenTTY sidebar parity. Agent status and
attention, progress, server/remote details, bookmarks, drag gestures, complete
context menus, and accessibility qualification remain active work.

## Qualification

`linux/qualification-matrix.json` is the single authoritative qualification
inventory. Every row declares one of `PASS`, `FAIL`, `BLOCKED`, `XFAIL`, or
`NOT_IMPLEMENTED`, and the runner rejects missing terminal combinations,
unknown states, unexplained blockers, untracked expected failures, unexpected
skips, and stale XFAILs. Prose documentation is explanatory only and may not
override that file.

Run every presently executable cell with:

```sh
linux/tests/qualify-local
```

The command writes `build/linux/qualification-summary.json` and a log per cell
under `build/linux/matrix-logs/`. Its report deliberately distinguishes an
implemented local-suite pass from release qualification and full Linux
qualification. Non-pass required cells prevent the latter claims even when
every executable local command succeeds.

Each authoritative matrix Valgrind cell producer writes an unsuppressed
`*.raw.log`, a suppression-enabled candidate `*.suppressed.log`, and a JSON
report containing raw and post-suppression error/leak totals. Here, “raw” means
that no Ghostty or Zentty suppression file is loaded; Valgrind's version-pinned
built-in suppressions remain active. Producer success means only that the
semantic and post-suppression checks passed while suppression governance
remains pending. The qualification summary embeds those reports and describes
a successful Debug result as **PASS with reviewed suppressions** only after
governance accepts the complete unchanged report set. It never describes the
result as an unsuppressed-clean run.

For every ordinary X11 Valgrind report, the raw and suppression-enabled phases
run sequentially in two fresh `tests/nested-x11` sessions rather than sharing a
cell-level display. The report binds the current nested harness hash plus each
phase's distinct session ID and `*.environment` receipt identity. Matrix and
suppression governance reject missing, changed, stale, reordered, or reused
phase identities; exit 77 remains an unexpected skip rather than a pass.

`tests/valgrind-suppressions.json` is the suppression manifest and is validated
against the adjacent closed-world Draft 2020-12 schema. It identifies every
Zentty rule, pins/audits the inherited Ghostty and Valgrind sets, and records
structured identities for reviewed local characterization evidence. The
standalone `Debug/ibus-focus/x11` contract pins GTK4 synchronous IBus mode,
requires four exact baseline-to-active-to-baseline focus acknowledgements, and
therefore bounds eight source-derived external clear-preedit calls. Only that
scenario may use its protocol ceiling: the object rule is limited to
`4/1160/16` matches/bytes/blocks and the child string rule to `2/8/8`, with
string bytes equal to blocks and the child forbidden without its root. Missing
or reordered acknowledgements, identity drift, and any over-ceiling use fail.
These gitignored identities are not public proof; public access remains
`NOT_IMPLEMENTED` under GH-10 until retained public artifacts exist.

Three earlier exploratory archives—
`2026-08-03-ibus-one-cycle-characterization`,
`2026-08-03-ibus-two-cycle-characterization`, and
`2026-08-03-ibus-focus-ack-characterization`—each contain eight
suppression-enabled receipts but no raw companions. Their original `/tmp`
executables are unavailable, and raw-before-suppressed chronology cannot be
reconstructed. Their checksums preserve what was observed, but they are not
paired, reviewed, public, or qualification evidence and do not authorize a
suppression.
`tests/suppression-governance` rejects untracked, stale, out-of-scenario, or
expanded or over-ceiling suppression usage; its negative self-test is
`tests/suppression-governance-test`. The historical C-host ReleaseSafe
Valgrind XFAIL receipts were not transferred to the Rust product. The two
Rust-product ReleaseSafe cells are therefore explicit `NOT_IMPLEMENTED` gaps
and are not made green by expanding suppressions.

Validate matrix structure without executing product tests:

```sh
linux/tests/qualification-matrix --validate-only
linux/tests/qualification-matrix-test
```

The focused support policy lives in `test-policy/`. Framework self-tests run
once before the matrix through `tests/qualify-local`; they are not product
qualification cells. The former recursive runner, Bash-governance mutation,
attestation, and archive layers were intentionally retired by the recovery
plan because they tested the evidence machinery rather than Zentty.

`ghostty-async-backend-abi-representation` is a command-backed tracked XFAIL:
the C17/C++17 probe rejects generated-header drift, prints default and
`-fshort-enums` sizes, and exits exactly 99 for the current enum-versus-`c_int`
mismatch. Its focused self-test runs before the matrix. Missing checkout,
compiler, or header prerequisites exit 77; every other nonzero exit is an
unexpected XFAIL failure rather than the tracked defect.

## Controlled local environments

Executable matrix cells do not inherit the developer's desktop session. Each
cell declares exactly one closed environment profile:

- `isolated-none-v1` uses `tests/isolated-session` with private HOME, XDG and
  temporary roots and no ambient display, D-Bus, AT-SPI or IBus endpoints.
- `nested-x11-v1` uses a fresh private Xvfb server and software renderer through
  `tests/nested-x11`.
- `nested-wayland-v1` uses a fresh Weston headless compositor and Pixman
  renderer through `tests/nested-wayland`.
- `nested-wayland-input-v1` uses Cage/wlroots with Pixman nested on the private
  Xvfb transport through `tests/nested-wayland-input`. It proves a real
  keyboard-capable Wayland seat and `zwp_virtual_keyboard_manager_v1`; it is
  reserved for physical Wayland-input cells and is not described as native
  Wayland or as the Weston Valgrind environment.
- `phase-managed-x11-v1` and `phase-managed-wayland-v1` are reserved for tests
  such as Valgrind whose raw and suppression-enabled phases must own distinct
  controlled sessions. Controlled input/resize tests that already own Xvfb use
  the phase-managed profile to prevent accidental double nesting.

The runner removes inherited report destinations and session markers before
dispatch. Every wrapper emits a machine-readable receipt that identifies its
current wrapper hash, session, command, timestamps, backend, renderer and
cleanup result. The runner validates those receipts before and after the cell
set and retains them as qualification evidence. Missing prerequisites exit 77
and remain unexpected skips; environmental absence never becomes a pass.

Focused environment tests are:

```sh
linux/tests/isolated-session-test
linux/tests/nested-x11-test
linux/tests/nested-wayland-test
linux/tests/lib/controlled-environment-test
```

Useful real-product diagnostics remain available:

```sh
GDK_BACKEND=wayland linux/tests/nested-wayland linux/tests/rust-product-smoke
GDK_BACKEND=x11 linux/tests/nested-x11 linux/tests/rust-product-smoke
GDK_BACKEND=wayland linux/tests/nested-wayland linux/tests/rust-product-lifecycle
GDK_BACKEND=x11 linux/tests/nested-x11 linux/tests/rust-product-lifecycle
```

The nested environments prove real GTK/GDK, display-protocol and software-
renderer integration. Weston headless intentionally has no input seat. These
receipts therefore do not qualify a representative GNOME/KDE session, physical
GPU, native input, IME, clipboard or fractional scaling; those remain separate
matrix cells with their actual statuses and prerequisites.

See `docs/design/zentty-linux-dogfood-2026-08-01.md` for the chronological
field and qualification record, including superseded snapshots, current gaps,
commands, failures, repairs, receipts, and the latest completed run.
