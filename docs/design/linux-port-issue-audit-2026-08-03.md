# Linux port issue and parity audit

- **Date:** 2026-08-03
- **Status:** Corrective audit; implementation expansion is paused
- **Repositories:** `TamedTornado/zentty` and `TamedTornado/ghostty`
- **Reason:** The Linux plan began implementing a newly designed persistence
  model before completing the source-backed ZenTTY feature inventory required
  by issue #7.

> **2026-08-04 supersession note:** This corrective audit identified the right
> rule but did not finish the source-wide inventory. The exhaustive evidence
> audit in `zentty-exhaustive-feature-audit-2026-08-04.md`, its machine-readable
> evidence ledger, and public issues #15–#23 supersede any implication here that
> the earlier five-category inventory was complete.

## Audit rule

ZenTTY's Swift implementation and tests are the authority for product parity.
Every Linux requirement must be classified as one of:

- **PORT_PARITY:** behavior already present in ZenTTY;
- **LINUX_NECESSITY:** new work required only to deliver that behavior on Linux;
- **OPTIONAL_HARDENING:** a separately approved improvement that cannot block
  parity unless the operator explicitly promotes it; or
- **REMOVE_INVENTED:** an unsupported requirement or design introduced by the
  Linux planning process.

Test requirements prove one of those behaviors. They do not create product
features by themselves.

## Source-backed persistence truth

The implemented ZenTTY behavior is defined principally by:

- `Zentty/Restore/WorkspaceRecipe.swift`;
- `Zentty/Restore/SessionRestoreStore.swift`;
- `Zentty/AppDelegate.swift`;
- `Zentty/Restore/PaneRestorationBuilder.swift`;
- `ZenttyLogicTests/WorkspaceRecipeTests.swift`; and
- `ZenttyLogicTests/SessionRestoreStoreTests.swift`.

It consists of:

1. `WorkspaceRecipe` schema version 3;
2. windows, optional frames, ordered worklanes, column layout, panes, focus,
   titles, working directories, last activity, and last-run command metadata;
3. a `SessionRestoreEnvelope` containing the recipe and separate agent restore
   drafts;
4. debounced live snapshots and a synchronous clean-exit snapshot;
5. generation ordering that prevents an older queued save from replacing a
   newer accepted save;
6. `restore-snapshot.json` written using Foundation's atomic write option;
7. `restore-lifecycle.json`, whose `cleanExit` flag distinguishes ordinary
   restoration from crash restoration;
8. optional restoration controlled by `restore_workspace_on_launch`;
9. consuming a successfully launched snapshot and deleting an unusable one;
10. an unversioned-title migration, with version 3 adding `customTitle`; and
11. forward-compatible handling of newer recipe version numbers under the
    current Swift decoder behavior.

ZenTTY does **not** implement a `.bak` generation, backup browser, journal,
checksummed history, or explicit backup-recovery command.

## Divergence in the current Rust implementation

Commits `8f90c6e` through `acdde4c` implemented a different product contract:

| ZenTTY source behavior | Current Rust behavior | Disposition |
| --- | --- | --- |
| `WorkspaceRecipe` schema version 3 | Independent schema version 1 | REMOVE_INVENTED |
| No workspace identity or durable revision | Required workspace UUID and revision | REMOVE_INVENTED |
| Newer versions currently migrate forward | Newer versions are rejected | REMOVE_INVENTED |
| Swift decoding tolerates unknown keys | Unknown keys are rejected | REMOVE_INVENTED |
| Window frame is persisted | Window frame is absent | Replace with PORT_PARITY |
| Worklane color, next-pane number, bookmark origin, focused column | Reduced worklane record | Replace with PORT_PARITY |
| Column ID, width, focused/last-focused pane, pane heights | Flattened panes plus invented weights/rows | Replace with PORT_PARITY |
| Pane custom/title seed, CWD, last activity, last command | Title, CWD, launch-profile ID, inline agent resume | Replace with PORT_PARITY |
| Agent restore drafts are envelope data | Agent resume is pane data | REMOVE_INVENTED |
| Atomic snapshot plus lifecycle marker | Primary, `.bak`, lock, and explicit store model | Replace with PORT_PARITY |
| Default window/worklanes come from `WorklaneStore` | Explicit state-file first-run API | Replace with PORT_PARITY |

The pure-Rust/Ghostty foundation remains useful, but the persisted model and
the product projection built on this schema cannot be treated as port parity.

## Public issue audit

### #1 — Epic: production-quality Zentty Linux port

**Disposition: rewrite.**

Keep Rust, gtk4-rs, the generic Ghostty ABI boundary, product-owned worklanes,
real integration tests, packaging, and public CI. Remove invented persistence
language, stale matrix totals, cryptographic receipt attestation as a product
gate, and the requirement that every theoretical QA cell block basic parity.
Make the feature inventory the source for product scope.

### #2 — Rust product architecture and workspace contract

**Disposition: split the valid architecture from the invalid product model.**

Rust/gtk4-rs, crate boundaries, GLib main-context ownership, and FFI isolation
are ratified. The workspace contract must be replaced by a Rust representation
of `WorkspaceRecipe` and `SessionRestoreEnvelope`, not an independently
designed schema. The required macOS/Linux behavior map was not completed before
dependent implementation began.

### #3 — Persistent workspace and worklane model

**Disposition: rewrite substantially.**

Port the actual recipe, migration, meaningfulness, snapshot, generation, and
lifecycle behavior. Remove `.bak`, backup recovery, invented workspace
identity/revision, launch-profile persistence, strict-newer-version rejection,
and generalized filesystem-fault campaigns from parity acceptance. Focused
atomic-write and corruption tests remain useful Linux hardening, but do not
define the product model.

### #4 — GTK worklane and pane application shell

**Disposition: retain and prioritize.**

This is core product parity: sidebar/worklanes, pane layout, actions, focus,
empty/failure states, and multiple real Ghostty terminals. Test the actual
supported product paths on Wayland and X11, without requiring a combinatorial
matrix for each small UI action.

### #5 — Pane lifecycle, focus, and restoration handoff

**Disposition: retain after source correction.**

Base command/CWD behavior on `PaneRestorationBuilder`, `WorklaneStore`, and
`TerminalPaneHostView`. Do not substitute an invented launch-profile model.
Keep real PTY, focus, child-exit, callback, and teardown integration tests.

### #6 — Restart and crash recovery

**Disposition: rewrite substantially.**

Port the existing snapshot plus lifecycle-marker behavior. ZenTTY reconstructs
the UI from recipe data; it does not adopt PTYs or provide backup/journal
selection. SIGTERM/SIGKILL, orphan audits, concurrent-instance coordination,
and kill-at-every-filesystem-phase campaigns are optional Linux hardening until
separately promoted, not parity prerequisites.

### #7 — Essential workflow parity inventory

**Disposition: retain and move to the front of the dependency order.**

This should own the source-backed inventory for worklanes, panes, restoration,
agent integrations, commands, servers, bookmarks, search, settings, themes,
clipboard, and platform alternatives. No new product slice begins without an
inventory entry linking its Swift source and Linux owner.

### #8 — Native input, IME, resize, and scaling

**Disposition: retain as LINUX_NECESSITY.**

These are legitimate Linux delivery risks rather than macOS features to copy.
Qualify supported configurations against the real product. Do not expand every
case across every build/backend/pane dimension unless code or a discovered
defect makes that dimension relevant.

### #9 — Packaging

**Disposition: retain as LINUX_NECESSITY.**

An installable artifact, XDG integration, resources, upgrade, and uninstall
ownership are required for Linux delivery. Packaging must follow the product,
not block construction of the worklane shell.

### #10 — Controlled public CI

**Disposition: retain as LINUX_NECESSITY, decouple from feature sequencing.**

Public Wayland/X11 execution is valuable. It must not prevent focused local
product implementation while the public environment is being constructed.

### #11 — Ghostty upstream decision

**Disposition: retain.**

The minimal language-neutral API and downstream-fork fallback remain correct.
Final API pruning waits for real product callers.

### #12 — Test and qualification architecture

**Disposition: rewrite aggressively.**

Retain real executable/system integration, focused unit tests, explicit matrix
gaps, controlled Wayland/X11, no silent skips, Valgrind suppression governance,
and focused mutation testing for critical deterministic logic. Remove recursive
test-system qualification, mandatory external signed attestation, mandatory
full-matrix execution for every small change, and up-front mutation campaigns
that test governance rather than product behavior.

### #13 — Rust/Ghostty foundation

**Disposition: retain the delivered foundation and prune speculative gates.**

The Cargo workspace, safe adapter, staged Rust executable, and real Ghostty/GTK
integration are useful. Completion should be based on operations required by
the real product, not every conceivable negative/mutation/axis combination.

## Corrected dependency order

1. Source-backed parity inventory (#7).
2. Retained Rust/Ghostty boundary and corrected architecture (#2/#13).
3. Actual `WorkspaceRecipe` and session snapshot semantics (#3).
4. Worklane/sidebar and pane shell (#4).
5. Product-owned terminal behavior and restoration handoff (#5).
6. Actual lifecycle-marker crash restoration (#6).
7. Agent hooks/status and remaining initial-release parity items (#7 children).
8. Linux input/IME/scaling, packaging, public CI, and upstream decision
   (#8–#11) as their product prerequisites become real.

## Current commit disposition

| Commit | Disposition |
| --- | --- |
| `620b296` and earlier Rust/Ghostty/C-host retirement work | KEEP, subject to ordinary adapter audit |
| `8f90c6e` topology model | REMOVE/REPLACE; built before parity inventory |
| `cbad7d4` strict schema codec | REMOVE/REPLACE; conflicts with WorkspaceRecipe v3 |
| `87a5545` primary/backup store | REMOVE/REPLACE; `.bak` is not a ZenTTY feature |
| `56c196b` persisted-pane product projection | REMOVE/REPLACE; consumes the incompatible schema |
| `434a889` mutation sequences | REMOVE; tests the incompatible model |
| `acdde4c` first-run/atomic-fault contracts | REMOVE/REPLACE; implements invented state-file workflow |

The uncommitted explicit backup-recovery work was saved to a private temporary
patch for audit and removed from the worktree before this document was written.
It was never committed or pushed.

## Test policy after correction

For each product slice:

1. identify the existing Swift behavior and tests;
2. write focused Rust model tests where deterministic logic exists;
3. exercise the staged Linux executable against real Ghostty, GTK, PTYs, and a
   controlled display when the behavior crosses those boundaries;
4. add only the negative/fault cases relevant to the implementation or a real
   discovered defect;
5. use focused mutation testing where it proves assertions depend on critical
   deterministic behavior; and
6. update only the matrix cells whose exact claim was exercised.

No recursive harness certification, exhaustive cross-product, or evidence
artifact is allowed to delay a product feature without a named risk and
operator-approved reason.
