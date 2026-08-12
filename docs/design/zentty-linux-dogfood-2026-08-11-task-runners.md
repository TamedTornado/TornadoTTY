# Zentty Linux dogfood — project task runners

Date: 2026-08-11
Issues: GH-19, GH-22

## Slice contract

The authoritative acceptance and construction order is
[`linux-task-runner-feature-plan.md`](linux-task-runner-feature-plan.md).
Every discovery, red test, failure, repair, real-system receipt, and remaining
limitation belongs here while this feature is built.

## Initial source audit

- Source Zentty discovers tasks synchronously from the focused pane's directory
  ancestry when the command palette opens. It does not maintain a task watcher
  or daemon. Linux will preserve that bounded snapshot architecture.
- The advertised feature is broader than package scripts: package.json,
  Taskfile, VS Code JSONC tasks, just, make, and mise are all source-defined
  requirements.
- Source disabled tasks remain visible and searchable. Selecting one opens its
  source rather than attempting to invent values for required parameters. The
  Linux feature must likewise make the limitation explicit.
- Source prefers the focused pane only when it can prove an idle prompt and no
  task environment override. Linux does not yet own an equivalent trustworthy
  prompt-idle signal, so the safe initial parity route is a real new Ghostty
  pane with the task source CWD rather than injecting into a possibly active
  program.
- The command palette action cannot safely carry executable shell text. Linux
  will retain a bounded discovered snapshot and accept only its stable opaque
  identity, followed by source revalidation at activation.

## Construction and repair record

- Eight source-derived core cases were written before the implementation. The
  first offline build failed on the intentionally absent task-runner exports,
  proving the repository had no hidden Linux implementation. An attempted
  `tempfile` dev dependency also tried to refresh crates.io in the restricted
  environment; it was removed rather than expanding the dependency graph, and
  the tests use a small std-only owned temporary directory.
- The focused core now covers every source format, nearest-to-parent ordering,
  duplicate identity, package-manager precedence, shell quoting, JSONC plus
  Linux overrides/environment, Taskfile includes and required variables,
  private recipes, malformed-source isolation, source-size bounds, symlink and
  include escape rejection, and changed/deleted/forged activation snapshots.
- The first JSONC implementation removed comments but only recognized a comma
  immediately adjacent to `]` or `}`. The real source-style fixture retained
  whitespace after its trailing commas and correctly failed discovery. The
  repair scans outside quoted strings and ignores a comma when the next
  non-whitespace character closes an array or object. The focused suite then
  passed 8/8.
- Linux stores one task snapshot on the existing `ApplicationShell`, adds one
  typed parameterized action to the existing action router, and queues one-shot
  launch configuration on the existing `PaneRuntimeCoordinator`. There is no
  task daemon, watcher, host-side process runner, alternate command registry,
  second workspace, or second pane/surface registry.
- The action-registry unit test failed at its fixed 74-action closed-world
  count after `run-task` was registered. Updating the expected count to 75 and
  the exact string-parameter inventory made the addition explicit; it was not
  bypassed with a looser assertion.
- The staged X11 journey's first run exposed a harness error: a caller-supplied
  relative product path became invalid after the journey changed into its real
  fixture project. The script now canonicalizes that path before `cd`; the
  corrected X11 run passed.
- The first controlled Wayland run sent Ctrl+Shift+P after terminal readiness
  but before Cage had delivered focus, so no palette opened. Environmental
  absence was not converted into a pass. Waiting for the product's real
  `focus-pane pane=pane-1` receipt removed the race, and the corrected Wayland
  run passed.
- The first focused mutation run found 40 surviving parser mutations out of
  155. That was useful evidence of assertion gaps rather than a reason to waive
  the gate. Boundary, ordering, Taskfile indentation/include, JSONC quoting,
  source-type, and revalidation assertions reduced the survivors to 15 and
  then 5. The last JSONC operator mutations exposed semantically awkward
  comment-state conditions; replacing them with explicit character-pattern
  state transitions and adding leading/post-string comment fixtures left no
  surviving viable mutant. The final repository-safe run (`gitignore=true`,
  `copy_target=false`) tested 153 mutants: 148 caught, 5 compiler-rejected
  unviable, 0 missed.

## Real product evidence

- Controlled X11: `rust-task-runners-x11: PASS real-palette
  real-ghostty-pty stale-revalidation cwd environment quoted-argv`.
- Controlled Wayland: `rust-task-runners-wayland: PASS real-palette
  real-ghostty-pty stale-revalidation cwd environment quoted-argv`.
- Both journeys drive Ctrl+Shift+P and text/Enter through the compositor,
  discover a real JSONC `.vscode/tasks.json`, mutate it after palette creation
  to prove stale activation cannot execute, then take a fresh snapshot and run
  the task in a real Ghostty PTY. The only receipt is written by the actual
  project task and proves its source CWD, Linux task environment, and quoted
  space-containing argument.
- A final code review found two edge cases before promotion: JSONC block
  comments were not yet accepted, and a Taskfile include could traverse an
  out-of-project symlinked directory even though direct file symlinks and `..`
  escapes were rejected. The parser now treats block comments as whitespace,
  and every resolved include must canonicalize beneath the source root while
  its final source still passes the non-symlink regular-file check. Both cases
  have focused regression fixtures.
- Focused core: 13/13 task-runner cases pass. Final mutation receipt: 153
  tested, 148 caught, 5 unviable, 0 missed.

## Full-matrix discoveries during promotion

- The first complete rerun passed both new task-runner cells but correctly
  failed support qualification because the feature-inventory runner still
  pinned the pre-promotion PARTIAL/NOT_IMPLEMENTED totals. Its closed-world
  expectations were updated from 24/30 to the authoritative 26/28; no schema
  assertion was relaxed.
- That run also exposed the development-server PID-attribution synchronization
  race recorded in its feature dogfood report. A bounded exact-confidence wait
  repaired it, and the controlled Wayland cell passed on rerun.
- The second full run again passed both task-runner cells, then exposed a
  pre-existing geometry assumption in the real X11 sidebar drag journey. GTK's
  selection reveal can legitimately place the inactive source card at several
  scroll offsets, while the harness searched only a fixed 60-pixel band. The
  journey now waits for the controlled 1000x700 GTK allocation, physically
  scrolls the real sidebar when needed, resolves the source header through its
  product receipt, and drives the same drag path relative to that rendered
  header. Two consecutive controlled reruns passed; no direct action shortcut
  or synthetic reorder replaced the physical drag.
- Repetition then exposed the deeper GTK DnD defect: when live reflow moved the
  fully rendered preview underneath the pointer, that preview had deliberately
  been non-targetable, so release could intermittently end with GTK
  `NoTarget`. The preview is now a real MOVE drop target that derives the exact
  before/after placement from its adjacent stable worklane cards and routes
  through the existing typed reorder action. Card drop targets remain the
  primary route. Four consecutive controlled physical-drag journeys passed
  after the repair, including varying selection-driven scroll offsets.
- The corrected complete local qualification finished in 392.780 seconds with
  all presently executable support and matrix cells passing. Declared totals
  are PASS=99, FAIL=0, BLOCKED=7, XFAIL=1, NOT_IMPLEMENTED=21. This establishes
  the implemented-local-suite claim only; release and full-Linux qualification
  remain not passed because the authoritative matrix still contains explicit
  non-PASS cells. The Debug Valgrind cell is **PASS with reviewed
  suppressions**, not an unsuppressed-clean claim; suppression governance
  passed, and ReleaseSafe Valgrind remains XFAIL as designed.
- After the final JSONC/include security repairs, strict workspace Clippy and
  the focused mutation gate passed again, the ReleaseSafe product was rebuilt,
  and both affected real-product cells were rerun: X11 and controlled Wayland
  each passed the real palette, Ghostty PTY, stale-revalidation, CWD,
  environment, and quoted-argv journey.

## Remaining limitation

- Disabled tasks are visible and searchable but intentionally insensitive on
  Linux. Source Zentty opens their source file; that requires the separately
  tracked Linux Open With implementation under GH-18 rather than an ad-hoc
  launcher in this slice.
- Source Zentty reuses the focused pane only when shell integration proves an
  idle prompt with no active progress. Linux does not yet own a trustworthy
  equivalent signal, so it conservatively opens a real new pane. It never
  injects a task into an unknown foreground program.

## Parity closeout resumed — 2026-08-12

- GH-18 is complete, so the earlier disabled-task source limitation no longer
  has an external dependency. Source audit confirms that selecting a disabled
  palette entry closes the palette and opens `action.sourcePath` using the
  primary Open With target; it does not leave the row insensitive.
- Linux currently marks the row insensitive and its runtime merely logs a
  rejected disabled action. That is a real source-parity gap. The closeout plan
  now requires the existing opaque snapshot to be revalidated and its own
  discovered source path sent through the one Open With authority. No new file
  launcher, command registry, watcher, or task execution path is permitted.
- The existing controlled journey proves only a VS Code task despite core
  parser coverage for every advertised source format. The closeout expands
  that same journey so product evidence cannot silently rely on parser-only
  tests for Taskfile, just, make, mise, or package scripts.

## Closeout discoveries and focused evidence

- A regular task source could not use the existing Open With plan because that
  policy intentionally accepted directories only. The shared core now exposes
  a local-path plan for regular files and directories. Desktop applications
  receive a canonical file URI, executable editors receive one canonical path,
  and directory-option terminal launchers still reject files. Missing and
  non-regular nodes remain rejected.
- The first governed mutation run caught 11 of 15 mutants and rejected three as
  unviable, but missed deletion of the non-file/non-directory check. A real
  Unix-socket fixture closed that gap. The final repository-safe campaign
  (`gitignore=true`, `copy_target=false`) tested all 15: 12 caught, 3 unviable,
  0 missed.
- Disabled palette items are now activatable exactly as in source Zentty. The
  existing opaque task snapshot is revalidated first; only then does the one
  Open With runtime launch the action's discovered source path. No GTK path,
  disabled command text, guessed parameter, or feature-local launcher crosses
  the boundary.
- The expanded journey initially failed its disabled-source assertion because
  it activated before the new query receipt arrived. Count-based palette
  synchronization repaired the harness rather than adding a product delay.
  The next run reached the later make task but reused an older matching query
  receipt, exposing the same harness flaw there; the second launch now also
  waits for a strictly newer receipt.
- Controlled X11 and Cage/Wayland now pass one consolidated journey that
  discovers package.json, Taskfile plus include, VS Code JSONC, just, make, and
  mise; opens the disabled VS Code task's exact canonical source in the
  configured controlled editor without executing it; rejects a changed source;
  and launches both VS Code and make tasks in real Ghostty panes with exact CWD,
  environment, quoted argument, and command receipts.
- The first complete promotion run passed the expanded Wayland cell but failed
  X11 at the second, make-backed launch. The product had correctly opened the
  palette on the short-lived VS Code task pane just as that task exited; its
  physical query input then had no stable focused target. The harness now waits
  for the real `child-exited-pane=pane-2` lifecycle receipt and a strictly newer
  focus return to pane 1 before opening the next palette. Focused X11 and
  Wayland reruns pass. This is lifecycle synchronization in the existing real
  journey, not a delay, alternate action route, or retained task pane.
- The corrected full local qualification completed in 451.540 seconds. Every
  presently executable cell passed: PASS=113, FAIL=0, BLOCKED=7, XFAIL=1,
  NOT_IMPLEMENTED=21. The machine-readable receipt has SHA-256
  `835026ed51feb8fd9c7559b04bca35fab3c5754b40bec9983c24a345f8dd86a7`.
  It establishes the implemented-local-suite and product-boundary claims only;
  release and full-Linux qualification remain false because the matrix still
  contains explicit non-PASS cells.
- Debug Valgrind is **PASS with reviewed suppressions**, not an unsuppressed
  clean result. Its preserved raw receipt reports 427 errors in 427 contexts,
  6,080 definite bytes, and 41,395 indirect bytes. The reviewed suppression
  run reports zero post-suppression errors, contexts, and leak bytes, with all
  427 errors/contexts explicitly suppressed. Suppression governance passed;
  ReleaseSafe Valgrind remains the tracked XFAIL.
