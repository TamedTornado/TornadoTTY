# Linux port dogfood field reporting

The Zentty Linux port is being developed against a real Ghostty fork and real
Linux desktop sessions. Failures found during that work are product evidence.
Preserve the evidence while it is fresh enough to be accurate.

## When to write

Update the active field report whenever the port exposes:

- a Ghostty GTK embedding, lifecycle, renderer, input, or packaging defect;
- incorrect Zentty behavior caused or admitted by the Linux port;
- a Wayland, X11, compositor, IME, scaling, clipboard, or GPU incompatibility;
- an ownership, teardown, resource, build, or upstream-rebase problem;
- a recovery action that leaks, loses, repeats, or falsely certifies work;
- behavior that contradicts a tested invariant or reveals missing coverage.

Continue the active report instead of creating disconnected incident files.
Start a new report only when a milestone is complete and a materially new
qualification or production run begins.

## Write during the incident

Add or update an incident entry as facts become known. Preserve:

1. **Observed behavior and impact.** State what happened and what work was
   blocked, degraded, lost, repeated, or put at risk.
2. **Evidence identity.** Record commands, revisions, tests, logs, issues,
   screenshots, display backends, desktop environments, and hardware details
   sufficient to reproduce the claim.
3. **Diagnosis.** Separate confirmed root cause from hypotheses. Retain failed
   theories and attempted repairs when they affected the investigation.
4. **Repair.** Describe the invariant restored and why the fix is general
   rather than specific to the observed machine.
5. **Regression proof.** Name the focused reproduction, automated tests,
   baseline suite, and Wayland/X11 or live application checks.
6. **Outcome.** State whether the real embedding host or Zentty application
   recovered, remained blocked, or exposed another defect.
7. **Durable references.** Link the relevant Zentty and Ghostty commits and any
   tracked follow-up.

## Evidence rules

- Do not turn a failure into a success by omitting the failed attempt.
- Do not claim a root cause or successful repair before evidence establishes it.
- Distinguish Ghostty defects, Zentty defects, integration defects, operator
  mistakes, unsupported environments, and external driver/toolkit failures.
- Prefer durable identifiers and reproducible commands over pasted megabytes
  of output.
- Do not commit credentials, secrets, private user data, raw crash dumps,
  disposable build trees, caches, or large generated proof bundles.
- When raw evidence is disposable, retain the exact recreation procedure and a
  concise result summary.

## Cross-repository completion rule

The canonical report lives in `TamedTornado/zentty`; engine fixes may live in
`TamedTornado/ghostty`. A repair is complete only when:

- the report describes the observation, evidence, repair, proof, and outcome;
- the relevant code and regression tests are committed in the owning repo;
- the Zentty and Ghostty commit hashes cross-reference each other where both
  repositories changed;
- both commits are pushed as one documented repair operation; and
- the real Linux host demonstrates the intended behavior.

If a follow-up is deliberately deferred, track it publicly and link it from the
report.
