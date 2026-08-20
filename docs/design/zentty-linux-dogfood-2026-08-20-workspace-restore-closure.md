# Zentty Linux dogfood — GH3 workspace restore closure

- **Date:** 2026-08-20
- **Starting branch:** `linux/port`
- **Starting commit:** `402015c0c85ae081a0a7db53fdb2b0dcd2006ef9`
- **Scope:** Finish and close GH3 as one source-parity feature, without adding
  another persistence implementation or integration-test layer.

## Pre-implementation acceptance audit

The public issue predates several focused deliveries. Reading the current
Swift sources, Rust model/store, focused tests, and real product actors shows
that most of GH3 is already implemented:

- the Rust recipe is the source schema-v3 shape: ordered windows, frames,
  worklanes, columns, pane geometry, selections, colors, bookmark origins,
  titles, CWDs, commands, and separate restore drafts;
- unversioned migration, v3 decoding, future-compatible decoding,
  meaningful-default filtering, stale-generation rejection, atomic replacement,
  restore preference, snapshot consumption, live debounce, clean exit, corrupt
  fallback, and crash recovery have focused coverage;
- the canonical `rust-session-restore` journey launches a source-compatible
  recipe through real Ghostty PTYs on X11 and Wayland, proves stored CWDs and
  metadata, saves cleanly, relaunches, exercises disabled normal restore,
  consumes recovered snapshots, and inspects persisted JSON;
- the canonical `rust-multi-window` journey proves ordered multi-window live,
  clean, and SIGKILL restoration with real GTK windows and PTYs.

The audit also found three claims that are not yet complete enough to close:

1. Swift has an explicit schema-v2 migration case; Rust relies on the generic
   versioned path but lacks the named v2 fixture/test required by GH3.
2. Divider geometry round-trips at the model level and X11 pointer dragging is
   qualified before the actor restores the original geometry. No real journey
   starts with a physical drag and proves that exact non-default geometry after
   relaunch.
3. Physical double-click equalization is implemented by GTK and unit-tested at
   the model boundary, but no deterministic X11 or controlled-Wayland product
   journey proves the rendered gesture reaches it.

GH3 owns `product-workspace-restore-{x11,wayland}`. The two
`product-worklanes-*` cells are GH4/GH16 scope and must not be promoted merely
because the persistence contract closes.

## Test-first construction order

1. Add the missing source-derived v2 focused test before changing model code.
2. Extend the existing multi-window actor with a bounded restore-layout mode;
   do not create a new actor, fixture protocol, persistence path, or product API.
3. Through real pointer input on controlled X11 and Wayland, create horizontal
   and vertical dividers, drag both away from defaults, physically double-click
   both back to equality, drag both to final non-default values, and record the
   resulting recipe geometry.
4. Cleanly quit and relaunch the staged product, then prove the exact stored
   widths/heights, pane identities, focus, and real PTY responsiveness are
   restored. Inspect JSON for approved fields and absence of runtime handles or
   test secrets.
5. If the failing actor exposes a product defect, repair the smallest owning
   Rust/GTK component and add focused coverage there. Do not widen timeouts to
   manufacture a pass.
6. Promote only the two GH3 matrix cells when their complete commands pass;
   update the three GH3 inventory entries from observed evidence, run mutation
   testing for changed pure policy, then run every presently executable cell.

## Claim limits

- Exact Wayland toplevel coordinates remain compositor-owned and are not a GH3
  failure; persisted client size and internal layout are the portable contract.
- No release or full-Linux claim is permitted while other matrix gaps remain.
- Debug Valgrind may only be described as **PASS with reviewed suppressions**;
  ReleaseSafe Valgrind remains XFAIL.

## Dogfood record

### Source-v2 migration fixture

- Added an explicit schema-v2 fixture derived from the Swift migration case.
- The existing generic Rust migration path passed the new focused test without
  production changes, including verbatim preservation of non-ASCII titles.

### Physical divider journey, first failures and repairs

- The first test-first X11 attempt could not observe a physical pointer entering
  a divider. The product had no divider-enter receipt, so the actor could not
  distinguish a real GTK hit from a guessed coordinate. Added one diagnostic
  `EventControllerMotion` receipt to the owning divider widget; no test-only
  action or model API was introduced.
- The first rebuild used `linux/scripts/build-local`'s default
  `build/linux/bin` output while the actor deliberately ran the qualified
  `build/linux-profiles/release-safe/bin` bundle. Rebuilt the exact staged
  profile with `ZENTTY_BUILD_OUTPUT_DIR=build/linux-profiles/release-safe`.
  This was an evidence-selection error, not a product defect.
- The canonical pointer helper only scanned the sidebar half of an X11 window,
  so it could never reach pane dividers. A full two-dimensional raster scan
  would multiply journey time. Added a bounded divider-specific helper to the
  same actor: because each handle spans one viewport axis, it scans only the
  narrow axis and still accepts only the exact GTK enter receipt.
- The corrected real X11 journey proved both horizontal and vertical pointer
  drags and both physical double-click equalizations. It then failed the clean
  snapshot comparison because the actor accepted the still-valid equalized
  live snapshot before the product's documented 350 ms quiet-time debounce had
  emitted the final dragged geometry. The actor now waits through that owned
  debounce before reading the final live snapshot; the persistence system was
  not duplicated or changed.
- Relaunch then exposed a test-precision error: the JSON/model round trip
  changed only the last binary floating-point digit (for example
  `429.00000751018524` to `429.0000075101853`) while preserving the same pixel
  geometry. Requiring textual/bitwise equality would reject a correct render.
  The relaunch assertion now permits less than `1e-9` drift for persisted
  weights while still requiring exact pane/column identity and topology; the
  observed drift was approximately `6e-14`.

### Focused qualification result

- The completed X11 journey passed in the private `nested-x11-v1` environment:
  real palette actions created the two-axis layout; XTest dragged and
  double-clicked both GTK dividers; the actor inspected live and clean
  envelopes; relaunch restored the same pixel geometry, identities, topology,
  selected pane, and responsive real Ghostty PTY.
- The identical product journey passed under private Weston/Wayland using the
  controlled outer-X11 seat. No developer-desktop coordinate or compositor
  availability was treated as a pass.
- Promoted only `product-workspace-restore-{x11,wayland}`. The worklane parity
  cells remain NOT_IMPLEMENTED under GH4/GH16. The authoritative totals are now
  164 PASS, 3 BLOCKED, 1 XFAIL, and 12 NOT_IMPLEMENTED before the final complete
  qualification rerun.
- Updated the three GH3 inventory entries to IMPLEMENTED from observed product
  evidence. This change adds diagnostic GTK enter observability but no new pure
  production policy, so there is no meaningful mutation target in this slice;
  existing workspace migration, state, and persistence policy remains covered
  by its focused tests and prior mutation receipts.

### First complete-matrix attempt

- Ran `linux/tests/qualify-local` before committing, as required. Both newly
  promoted restore cells passed inside the authoritative runner. The complete
  attempt correctly did **not** qualify: clean-checkout preflight and Debian
  packaging rejected the intentional uncommitted feature diff, and the existing
  full X11 multi-window menu journey once failed to render its foreign-window
  destination while concurrent development-server discovery was active.
- Reran that exact existing X11 multi-window command alone against the same
  staged bundle; it passed in 66 seconds. No product or timeout change was made
  for the isolated pass. A clean-checkout complete rerun is still required
  after the reviewed feature commit; the failed receipt is not being relabeled
  as a pass.

### Final clean-checkout qualification

- Committed the reviewed feature as `63ca9fc1`, confirmed a clean checkout, and
  reran `linux/tests/qualify-local` without excluding any executable cell.
- Result: implemented local suite PASSED; product boundary qualification
  PASSED; release qualification and full Linux qualification remain NOT_PASSED
  because the authoritative matrix still declares 3 BLOCKED, 1 XFAIL, and 12
  NOT_IMPLEMENTED cells. Declared totals are 164 PASS and 0 FAIL.
- Both GH3 cells passed in the complete runner: controlled Wayland in 40.39 s
  and controlled X11 in 42.09 s. The complete qualification took
  1,533,490 ms.
- Debug Valgrind is **PASS with reviewed suppressions**, not unsuppressed clean.
  Its preserved raw receipt contains 427 errors/contexts, 6,160 definite bytes,
  and 41,428 indirect bytes; post-suppression totals are zero. Governance was
  ACCEPTED and the raw and suppressed receipt hashes are present in
  `build/linux/qualification-summary.json`. ReleaseSafe Valgrind remains XFAIL.
