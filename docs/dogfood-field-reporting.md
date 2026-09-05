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

The canonical report lives in `TamedTornado/TornadoTTY`; engine fixes may live in
`TamedTornado/ghostty`. A repair is complete only when:

- the report describes the observation, evidence, repair, proof, and outcome;
- the relevant code and regression tests are committed in the owning repo;
- the Zentty and Ghostty commit hashes cross-reference each other where both
  repositories changed;
- both commits are pushed as one documented repair operation; and
- the real Linux host demonstrates the intended behavior.

If a follow-up is deliberately deferred, track it publicly and link it from the
report.

## Integration qualification standard

This section describes broader milestone/release qualification, not a gate
to rerun for every bug fix or commit. Daily repairs use focused regressions and
the affected existing integration journeys. GitHub CI checks our work; its
public receipt is not an additional approval step. Keep unperformed live QA
explicitly pending without blocking unrelated development.

The Linux port must be externally auditable. A passing unit test is necessary
but is not evidence that an embedded terminal works. Each milestone must retain
the commands and concise receipts for all applicable layers:

1. **Upstream regression.** Run the focused Ghostty tests for changed modules,
   the full Zig test gate before milestone integration, formatting checks, and
   an unchanged standalone Ghostty build.
2. **Embedding contract.** Build a host that is not `GhosttyApplication` and
   verify runtime initialization, one and multiple surfaces, PTY input/output,
   resize, focus transfer, configuration, host callbacks, child exit, window
   close, and deterministic teardown.
3. **Display backends.** Run the same host through native Wayland and X11. Use
   automated headless backends in CI and retain live compositor checks for
   behavior that a headless server cannot establish.
4. **Desktop matrix.** Qualify GNOME/Mutter and KDE/KWin, mixed and fractional
   scaling, standard and primary clipboards, dead keys, Compose, ibus, fcitx,
   and representative Intel/AMD/NVIDIA paths as infrastructure permits.
5. **Stress and resources.** Repeatedly create and destroy surfaces, run a
   many-pane workload, close terminals with active and exited children, and use
   Valgrind or equivalent tooling to detect GTK/GObject, PTY, and renderer
   lifecycle leaks.
6. **Zentty end to end.** Exercise launch, worklane and pane operations, session
   persistence and restoration, agent status, command routing, and clean quit
   through the packaged Linux application rather than isolated models.

Record skipped environments as explicit qualification gaps with a reason and a
tracked follow-up. Never collapse “not run” into “passed.” Prefer semantic
assertions and process-visible evidence over screenshots alone.
