# Linux settings children completion plan

Date: 2026-08-13
Scope: GH-37, GH-38, GH-40 after GH-41 implementation

## Governing outcome

Finish the three remaining implementation-sized children of settings epic
GH-20 without inventing page-local runtime authorities. Each child is complete
only when its issue-level acceptance criteria, real-system journey, focused
policy tests, mutation evidence, architecture inventory, matrix declaration,
dogfood record, and public issue state agree.

## Construction order

1. Audit the source page, current Linux runtime owner, persisted schema, open
   issue body, matrix cell, and existing journey before editing product code.
2. Extend the existing journey first so missing acceptance boundaries fail
   against the staged product. Do not create a second actor or settings harness.
3. Repair the smallest owning product boundary. Settings pages remain projections
   over `ApplicationShell`, `ConfigStore`, Open With, pane/runtime, and agent IPC
   authorities.
4. Add focused decision tests and governed cargo-mutants shards for newly
   introduced policy. All runs retain `.cargo/mutants.toml`'s
   `gitignore=true`/`copy_target=false` policy.
5. Run focused X11 and Wayland cells while developing. Run the complete local
   qualification matrix once after the coherent three-child batch, then review,
   commit, push, and reconcile GH-20 and its children.

## GH-38 — Open With

- Prove native add/remove, enable/disable, deterministic primary selection,
  explicit no-target presentation, exact launcher argv, SSH rejection, restart,
  and an application disappearing while settings is open.
- Preserve the source reconciliation policy while visibly reporting removed
  unavailable entries; never silently select or execute a stale path.
- Refresh through the existing discovery/config authority. The settings page
  may request refresh but may not discover applications itself.
- A failed write must leave the accepted page projection and runtime unchanged.

## GH-37 — Worklanes & Panes

- Drive every available control against real worklanes/panes and verify
  persistence/restart plus immediate multi-window projection without shell PID,
  pane identity, topology, worklane selection, or surface recreation.
- Exercise real widths immediately below/at each adaptive threshold, physical
  pointer focus and delay/cancellation, and settings/dialog/window focus guards.
- Keep smooth scrolling and inactive opacity visible, disabled, and value-
  preserving until their named Ghostty/compositor prerequisites exist; do not
  revive the rejected whole-surface washout workaround.
- Pin placement, threshold, availability, and opacity-normalization decisions
  with focused tests and governed mutants.

## GH-40 — Agents

- Reconcile the source inventory with the existing authenticated wrapper,
  consent, IPC, team, and agent-status authorities. Distinguish requested,
  available, active, unavailable, stale, and failed states without trusting
  config alone.
- Drive supported integrations, consent, idempotent enable/disable, restart,
  external changes, malformed/permission/interruption failures, real agents,
  and team projection under isolated HOME/XDG roots.
- Keep status-item and sleep-inhibition controls visible with their stored
  values and explicit prerequisites. The existing BLOCKED sleep-inhibitor cell
  remains honest until a controlled desktop service exists.
- No persistent edits to the operator's agent configuration and no second IPC,
  registry, timer, or inhibitor authority.

## Qualification language

Passing implemented cells is not release or full Linux qualification while any
required matrix cell remains BLOCKED, XFAIL, or NOT_IMPLEMENTED. Valgrind is
reported only as **PASS with reviewed suppressions**. ReleaseSafe Valgrind stays
XFAIL.
