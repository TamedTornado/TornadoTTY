# Linux quality and architecture drift audit — 2026-08-08

## Purpose

Pause feature delivery and verify that Zentty still has one product model, one
persistence path, one test authority, reviewable dependency direction, and
real product-boundary coverage. This audit is a gate on the next feature slice;
it is not a claim of full Linux qualification.

## Audit method

1. Compare the Cargo workspace graph with the normative architecture contract.
2. Inventory production owners for workspace state, persistence, session
   restore, agent IPC, tmux compatibility, and terminal lifecycle.
3. Enumerate every executable test and prove that maintained product journeys
   are reachable from the authoritative qualification matrix, while runner
   self-tests are reachable from `linux/tests/qualify-local`.
4. Run formatting, Clippy, workspace tests, architecture/orchestration negative
   tests, the feature-inventory negative suite, installed-shell integration,
   and every newly discovered executable real-product journey.
5. Record failures before repair, make only bounded governance/test repairs,
   rerun all affected gates, and leave structural production refactoring for a
separately accepted slice.

The bounded follow-up is tracked by GH-25. It explicitly forbids parallel
models, stores, terminal registries, runtimes, and test orchestrators.

## Findings before repair

### No parallel product systems found

- `WorkspaceState` is the sole live workspace aggregate.
- `SessionRestoreStore` plus `SnapshotPersistence` is the sole persisted
  restart store. Closed-pane restoration is a bounded transient undo stack,
  not a second persisted session system.
- `zentty-agent-ipc` is the sole product IPC transport, and
  `zentty-tmux-compat` is the sole pure tmux parser/state library.
- The product has one GTK/Ghostty composition root and no retained C host,
  Electron renderer, alternate test product, or application-embedded scenario
  mode.

### Architecture enforcement drifted

The machine contract listed all seven current crates but still declared the
older dependency graph. The real graph also contains
`zentty-agent-ipc -> zentty-tmux-compat` and
`zentty-linux -> zentty-tmux-compat`. The validator checked the contract
against itself, not against Cargo metadata, so the stale contract passed.

Repair acceptance:

- update the ADR and machine contract to the reviewed real edges;
- make the validator compare the contract with `cargo metadata --no-deps`;
- add a negative test where an internally consistent contract omits a real
  edge, proving self-consistency alone cannot pass.

### Maintained tests could rot outside the gate

`feature-inventory-test`, `rust-source-ux-x11`,
`rust-sidebar-management-x11`, and `staged-shell-integration` were maintained
but not reachable from `qualify-local` or an authoritative matrix cell. The
first three were also named as evidence in public documentation.

The audit proved this was consequential: `rust-source-ux-x11`, Bash, Zsh, and
the feature-inventory negative suite passed, while
`rust-sidebar-management-x11` failed before any repair because its controlled
drag pickup coordinate had drifted after the sidebar geometry changed. A
disposable run at the corrected coordinate passed, identifying harness drift
rather than a product reorder regression.

Repair acceptance:

- runner self-tests belong in `qualify-local` support tests;
- real GTK/Ghostty journeys and staged shell processes belong in explicit
  matrix cells, not in a second aggregate runner;
- missing Fish 4+ and Nushell prerequisites remain `BLOCKED`, never PASS;
- the X11 drag journey must pass in a private Xvfb session after the narrow
  coordinate repair.

### Production concentration needs a planned boundary, not an emergency rewrite

`application_shell.rs` is 3,589 lines and currently coordinates GTK actions,
workspace projection, pane/surface lifecycle, search, peek, sidebar, agents,
and rendering. This is one system rather than duplicated systems, but it is a
god-object risk. Live persistence scheduling or further platform services must
not be added directly to it. A follow-up slice should extract focused
coordinators while retaining exactly one `WorkspaceState`, one terminal owner
map, and one persistence store. This audit does not mix that structural change
with test-governance repair.

## Initial evidence

- `cargo fmt --all -- --check`: PASS.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: PASS.
- `cargo test --workspace --locked`: the restricted tool sandbox denied real
  Unix socket binds; the same unchanged suite passed outside that sandbox.
- `linux/tests/architecture-contract`: PASS before repair, demonstrating the
  validator hole described above.
- `linux/tests/test-orchestration-contract`: PASS.
- `linux/tests/feature-inventory-test`: PASS when invoked directly.
- `linux/tests/staged-shell-integration bash`: PASS.
- `linux/tests/staged-shell-integration zsh`: PASS.
- private-Xvfb `rust-source-ux-x11`: PASS.
- private-Xvfb `rust-sidebar-management-x11`: FAIL at drag pickup with no drag
  begin receipt; a disposable corrected-coordinate run: PASS.

## Exit criteria

- The dependency graph and every maintained test are mechanically attached to
  an authority.
- All affected validator negative tests and presently executable new cells
  pass.
- The complete authoritative matrix is rerun before commit or push.
- Matrix totals and remaining non-PASS cells are reported exactly; no
  exhaustive or full-qualification claim is made.

## Final result

All affected checks and every presently executable authoritative cell passed.
The final matrix declares `PASS=87`, `FAIL=0`, `BLOCKED=7`, `XFAIL=1`, and
`NOT_IMPLEMENTED=21`. Implemented-local and product-boundary qualification
pass; release and full Linux qualification remain false. The machine summary
SHA-256 is
`f30dffea266dd9f9aac7e04d96a9b884b0c18464280d4994bee1b905122d5bfa`.

The Debug IBus focus run is **PASS with reviewed suppressions**, not an
unsuppressed-clean result: raw totals are 427 errors/contexts, 6,240 definite
bytes, and 41,461 indirect bytes; post-suppression totals are zero with all 427
contexts accounted for. Its governed report SHA-256 is
`5929447a90d6f6be903dee61121e001ea61a9e56dcfef9457fc209a440459333`.
