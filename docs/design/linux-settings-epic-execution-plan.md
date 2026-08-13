# Linux settings epic execution plan

Status: active plan for GitHub epic #20 and child issues #36–#41  
Ratified: 2026-08-13

## Why issue #20 was decomposed

Issue #20 had grown into an epic while still being operated as a single feature
issue. It combined the settings shell, nine source pages, shortcut capture,
Ghostty appearance/resources, notification audio, safe XDG persistence, live
reload, last-known-good recovery, and real-system qualification. Those are not
one reviewable implementation unit.

The implementation boundaries used so far were generally cohesive, but the
delivery cadence was too fine: individual ordinary pages were followed by
repeated aggregate qualification and operator-facing status checkpoints. That
made the epic appear to advance one control at a time and spent minutes rerunning
unrelated product cells.

GitHub #20 is now the umbrella acceptance contract. Its remaining owned work is
split into:

- #36: authoritative config reload, last-good recovery, and safe persistence;
- #37: Worklanes & Panes settings;
- #38: Open With settings over #18's existing authority;
- #39: Dev Servers settings over #19's existing authority;
- #40: Agents settings over #14/#21's existing authorities; and
- #41: custom notification sound import and lifecycle.

## Source and ownership rules

The Swift/AppKit source remains behavioral authority. Linux ports the feature
and terminology, replacing only platform mechanics. A settings page is a view
and intent surface over an existing product authority:

- pane/worklane settings use the existing layout, focus, and projection owners;
- Open With settings use the one launcher/catalog delivered by #18;
- Dev Servers settings use the scanner/browser/ignored-port owners from #19;
- Agents settings use the IPC, bootstrap, team, and sleep owners from #14/#21;
- all pages use the one `zentty_core::AppConfig` and Linux `ConfigStore`;
- settings callbacks do not directly spawn tools, scan processes, edit external
  agent configuration, or own Ghostty terminal lifecycle.

No child may create a second store, watcher, launcher, scanner, agent registry,
IPC server, focus authority, or pane-layout engine. Existing owners are extended
only where a source control exposes a genuinely missing product operation.

## Test construction order

For each child page:

1. Audit its source controller, config keys, tests, copy, and runtime owner.
2. Write focused model/config tests for the missing contract first.
3. Write or extend the real staged-product journey so it initially fails for the
   missing control/effect, not for a fabricated test seam.
4. Implement the smallest page and authority wiring that makes those tests pass.
5. Run formatting, Clippy, focused Rust tests, the page's controlled X11 and
   Wayland journeys, architecture ownership validation, and affected matrix
   runner tests.
6. Review the diff for duplicated authority, source drift, test-only product
   paths, accidental operator-home access, and misleading platform claims.

Unit tests cover deterministic parsing, validation, and decisions. Integration
tests use the staged executable, real GTK controls/input, real Ghostty surfaces
and PTYs, isolated real files, and the real owning subsystem. External programs
may be controlled fixtures when exact side effects must be asserted, but Zentty
itself and the boundary under test are not mocked.

## Batch and qualification policy

### Batch 1 — remaining core settings UX

Implement #37, #38, #39, and #40 as one operator-review batch. They remain
separate child issues and focused commits because their acceptance criteria and
runtime authorities differ, but they share one planned implementation window
and one final aggregate qualification run. Focused tests run continuously.

### Batch 2 — configuration transaction

**Completed by #36.** The single XDG config authority now owns secure creation,
bounded validation, independently valid live sections, last-good retention,
atomic/durable product writes, product/external last-writer outcomes, logical
and target symlink watching, interrupted-temporary refusal, open-settings
refresh, no-loop self-writes, and multi-window projection without PTY changes.
Page-specific unfinished controls remain in their own issues and must consume
this authority rather than reopening or duplicating it.

### Batch 3 — custom audio

Complete #41 against a controlled real Linux audio session. Audio absence is
BLOCKED, not a pass, and no broad file or process simulation may substitute for
the actual playback boundary.

### Aggregate cadence

Run `linux/tests/qualify-local` once after all focused gates for a coherent batch
pass and before committing/pushing that completed batch. Do not rerun the full
matrix after each settings page unless a change touches a cross-cutting harness,
Ghostty ABI/lifecycle, or qualification runner itself. A targeted failure is
repaired and rerun directly before the final aggregate.

## Completion and reporting

A child closes only when its source behavior, Linux alternative decisions,
focused tests, real controlled-X11/Wayland journey, documentation, dogfood
record, and applicable matrix cells agree. Epic #20 closes only when all children
are closed and `platform-settings-contract` is no longer `NOT_IMPLEMENTED`.

No report may call QA exhaustive while any required cell is FAIL, BLOCKED,
XFAIL, or NOT_IMPLEMENTED. Valgrind wording remains **PASS with reviewed
suppressions**, with raw and post-suppression evidence governed separately.
