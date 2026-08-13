# Zentty Linux settings epic dogfood — 2026-08-13

This append-only record begins with the decomposition of GitHub issue #20. The
authoritative execution plan is
`docs/design/linux-settings-epic-execution-plan.md`.

## Discovery: issue #20 was being operated at the wrong level

- Issue #20 contained multiple independent feature systems but was still named
  and checked like one implementation issue. Its body had thirteen broad,
  unchecked acceptance/test bullets even though six coherent settings slices
  had already shipped and qualified.
- The code boundaries were not themselves evidence of duplicate settings
  systems: delivered pages use the shared settings shell and `ConfigStore`.
  The process boundary was wrong. Ordinary pages were reported and aggregate
  qualified individually, creating excessive review and qualification cadence.
- Several remaining pages configure behavior already delivered elsewhere.
  Treating them as new runtime implementations would risk duplicate authorities:
  Open With belongs to #18, Dev Servers to #19, and agent runtime behavior to
  #14/#21.

## Decision and repair

- GitHub #20 is now explicitly an epic with delivered scope, cross-page gates,
  a remaining-child checklist, and a three-batch execution policy.
- Child issues #36–#41 provide source-backed acceptance criteria, real-system
  tests, architectural boundaries, and dogfood requirements for every remaining
  item. Nothing disappeared from the qualification contract.
- The next operator-review batch is #37–#40 together. Each page retains focused
  tests and a reviewable commit, but only the completed batch receives the full
  aggregate qualification run.
- Cross-cutting configuration correctness (#36) and custom audio lifecycle (#41)
  remain distinct because they exercise filesystem/watch and audio/notification
  boundaries rather than merely projecting existing settings.

## Matrix reconciliation

- `platform-settings-contract` remains `NOT_IMPLEMENTED`; decomposition is not
  implementation.
- Its defect now names #36–#41 explicitly. The custom-sound cell tracks #41.
- No PASS total changes as a result of planning work, and no exhaustive QA claim
  is made.

## Remaining uncertainty

- Source pages may reveal platform-specific controls whose Linux runtime owner
  lacks an operation. Such discoveries must update the appropriate child issue
  before implementation; they must not be solved by a page-local service.
- Whether custom audio can be qualified in the existing controlled desktop/audio
  environment remains unknown. Absence will be represented as BLOCKED.
- Full config merge/comment preservation and watcher behavior need a fresh
  ownership audit under #36; earlier page-safe writes do not establish that
  complete contract.
