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

## Batch 1 implementation discoveries — #37–#40

### Source contracts recovered before wiring

- The source Worklanes & Panes page is not a generic layout-preferences page.
  It owns the exact keys for worklane insertion, adaptive right-pane behavior,
  the visible-split threshold, compact pane path labels, pane borders, project
  icons, smooth terminal scrolling, focus-follows-mouse/delay, and inactive
  pane opacity. Linux now parses and safely persists those exact sections rather
  than inventing a second preference model.
- Open With and Dev Servers already had product authorities. Their settings
  pages now edit the existing normalized catalogs; neither page gained a second
  launcher or scanner. Custom applications are selected through the real GTK
  file chooser and receive stable IDs derived from canonical executable paths.
- The source Agents inventory contains persistent and ephemeral integrations.
  The staged Linux product currently installs authenticated wrappers only for
  Claude Code, Codex, and Gemini CLI. Those three settings now gate the actual
  wrapper directories used by newly created panes. Every other source entry is
  still shown and explicitly unavailable rather than silently disappearing.

### Failures and repairs

- The browser catalog deliberately treats an empty enabled-ID list as its
  first-run “enable all discovered browsers” default. The initial settings
  implementation could therefore turn the final optional browser off, persist
  an empty list, and accidentally re-enable every browser. The repair persists
  the always-present `system-default` ID as an explicit sentinel for “no
  optional browser”; a focused catalog test proves it cannot expand to all.
- A first Worklanes & Panes draft exposed switches whose values persisted but
  had no Linux runtime effect. That would have been dishonest UI. Compact pane
  labels and border suppression were subsequently connected to the existing
  pane frame, and focus-follows-mouse gained a cancellable pointer-enter timer
  guarded against inactive windows, settings, search, the command palette, and
  Worklane Peek. Smooth terminal scrolling and inactive opacity remain visible
  but insensitive with specific reasons until their Ghostty/compositor paths
  are qualified. Project icons, worklane placement, and right-pane behavior use
  their existing live owners.
- GTK opacity applies to the fully composited Ghostty surface. Earlier dogfood
  showed that reducing it washed out terminal content instead of reproducing
  the source backdrop-aware inactive-pane treatment. No suppression or cosmetic
  workaround was added; the limitation remains explicit.
- Agent menu/status presentation and sleep inhibition have no reviewed Linux
  backend yet. Their source controls remain visible with the stored state and a
  concrete unavailable reason. The settings page does not pretend persistence
  alone implements those lifecycle features.

### Test and ownership evidence so far

- Focused config tests cover source defaults, exact TOML keys, invalid values,
  unknown-key/comment preservation, and symlink-safe atomic updates.
- Workspace-state tests cover top, after-current, and end insertion without a
  parallel worklane store. Agent-runtime tests prove only explicit `off`
  removes an installed wrapper; default, `ask`, and `on` preserve the existing
  ephemeral integration behavior.
- The workspace passed formatting and strict workspace Clippy after the new
  pages were wired. The real staged X11/Wayland settings journeys subsequently
  ran as part of the aggregate receipt recorded below; this earlier checkpoint
  is retained as sequence evidence, not as the current qualification state.

### Real compositor failure: pointer-leave reentrancy

- The first controlled Wayland Worklanes & Panes journey exposed a real abort,
  not a harness artifact. Creating a worklane unmaps the old pane frame. GTK
  synchronously emitted the frame's pointer-leave callback while
  `ApplicationShell::render` still held its mutable `RefCell` borrow. The new
  focus-follow callback attempted a second borrow and panicked across GTK's C
  callback boundary, which correctly aborted the process.
- The repair makes pane pointer presence delivery asynchronous on the GLib main
  context before it enters the shell authority. Generation cancellation still
  discards stale enter/leave timers. No `try_borrow` suppression or compositor
  special case hides the lifecycle error.
- After repair, the staged X11 journey passes real worklane insertion, the
  source policy-dispatched Add Pane Right command, two live Ghostty columns,
  physical pointer focus, settings deep linking, and exact config loading. Its
  controlled Wayland companion passes the same real product boundaries; the
  physical pointer-focus assertion remains explicitly owned by the X11 cell
  because the current nested Wayland input harness exposes a virtual keyboard
  but no scoped pointer injector.

### Aggregate qualification regressions and repairs

- The first post-batch aggregate receipt correctly failed eight previously
  passing cells. Five failures converged on the same product regression: the
  new Agents projection replaced the established
  `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1` process opt-in with the absent
  configuration default before the first pane was created. Real tmux product,
  staged-bundle, agent-integration, Ghostty API usage, and multi-window journeys
  consequently never received their private compatibility endpoint. Startup
  now preserves an explicit process opt-in while allowing an enabled persisted
  setting to opt in too; a live settings change remains authoritative in both
  directions. The isolated real tmux and multi-window journeys pass after the
  repair.
- The source-UX journey had silently depended on the old invariant that the
  pane-local right control was always Split Right. The new source-compatible
  adaptive policy correctly turned that control into Add Pane Right in its
  919px terminal viewport. The journey now writes the exact
  `pane_layout.right_split_behavior = "alwaysSplit"` setting because its later
  geometry assertions intentionally qualify Split Right. Adaptive dispatch is
  independently exercised by the new workspace/pane settings journey. The
  corrected real X11 source-UX journey passes.
- Running controlled X11 journeys inside the tool filesystem sandbox presents
  host-root-owned `/tmp/.X11-unix` as `nobody`, which Xorg properly rejects.
  This is not converted into a pass: compositor journeys are executed outside
  that user-namespace view, where the system directory is root-owned as
  required. The failure and rerun are retained here because confusing the
  sandbox projection with a product or Xvfb defect would corrupt qualification.

### Final presently executable receipt

- After both repairs, `linux/tests/qualify-local` passed every presently
  executable support test and matrix cell in 469,370 ms. Declared totals are
  **PASS=123, FAIL=0, BLOCKED=7, XFAIL=1, NOT_IMPLEMENTED=23**.
- The implemented local suite is passed. Release qualification and full Linux
  qualification are **not** passed because the authoritative matrix still
  contains explicit BLOCKED, XFAIL, and NOT_IMPLEMENTED cells. No exhaustive-QA
  claim is made.
- Debug Valgrind is **PASS with reviewed suppressions** and the suppression
  governance review is accepted. ReleaseSafe Valgrind remains XFAIL; no
  suppression was broadened to change that result.
- The machine-readable receipt is `build/linux/qualification-summary.json` and
  its per-cell evidence is under `build/linux/matrix-logs/`. Both are generated
  evidence rather than source artifacts.

## Open With settings control qualification

- Extending the real Open With journey beyond page presentation immediately
  found that Linux retained an unavailable configured target (`xcode`) when the
  page opened. The macOS source sanitizes preferences against its detected
  catalog during presentation. Linux now performs the same operation through
  `OpenWithConfig::reconciled_available`: stale custom rows are removed,
  unavailable IDs cannot remain enabled, primary selection falls back through
  surviving enabled order, and the existing config/open-with runtime authority
  publishes the result. Controlled X11 and Wayland both prove the normalized
  catalog survives a real process restart.
- The first physical X11 checkbox click found a real non-unwinding
  `RefCell already borrowed` abort. `apply_and_rebuild` invoked the shell apply
  callback through a temporary immutable state borrow and then attempted to
  store the accepted value. Cloning the callback before invocation fixes that
  ownership error. The identical latent pattern in Dev Servers was repaired at
  the same authority boundary rather than waiting for a second crash.
- The second click found another synchronous GTK reentrancy bug: rebuilding a
  dropdown model emits `selected` notification while the settings state is
  mutably borrowed. Rebuild guards now live in independent `Rc<Cell<bool>>`
  markers for Open With and Dev Servers, so expected programmatic notifications
  can return without borrowing the model they interrupt. The repaired X11
  journey physically disables and re-enables the controlled custom application,
  inspects exact persisted IDs, restarts, launches the real custom executable,
  and retains the prior real local/SSH launch assertions.
- A controlled screenshot exposed hard-coded dark card colors under GTK's light
  theme, producing nearly unreadable dark-on-dark settings rows. Shared settings
  cards and appearance-list selection now derive background, border, and
  selected foreground colors from GTK theme colors. This was a product defect,
  not test-harness polish.
- The settings window previously always focused the hidden Shortcuts search
  entry even when command-palette deep linking opened another section. The
  settings shell now exports its visible section search as the initial focus
  target; the shortcut search remains initial only for the Shortcuts section.
  This removes a hidden-focus trap without adding another navigation system.
- Nested Wayland still has no scoped pointer injector. Its Open With cell proves
  real settings presentation, catalog reconciliation, persistence/restart, and
  launcher behavior; the physical checkbox mutation is explicitly owned by the
  controlled X11 companion rather than being silently skipped or called a pass.
- The first aggregate rerun exposed an unrelated but real load-sensitive input
  race in the controlled Wayland shortcut/settings journey. Its fourth command
  palette invocation typed immediately after the chord; under concurrent load,
  the overlay had logged only its initial empty query and had not yet accepted
  text, so `Appearance Settings` never arrived. The prior waits checked query
  results but did not establish that each newly requested overlay was ready.
  The journey now counts `command-palette=shown` receipts and waits for the next
  distinct presentation before every physical query. An isolated controlled
  Wayland rerun passed before this repair; the aggregate failure remains useful
  evidence that isolation alone could not reveal the readiness defect.
- A second aggregate rerun passed the repaired shortcut journey but exposed two
  different load-sensitive assumptions. Open With allowed only five seconds
  for the compositor to acknowledge its physical palette chord, while the
  shared product-input contract allows ten; it now uses the same bounded
  acknowledgment window for presentation and query delivery. The staged X11
  tmux journey also asserted pane order immediately after an asynchronous real
  respawn. It now polls the real IPC `list-panes` result for up to two seconds
  and still fails with the last observed topology. Both cells passed alone;
  neither environmental absence nor an isolated rerun is treated as an
  aggregate pass.
- The corrected aggregate run passed all presently executable support tests and
  matrix cells in 479,620 ms. Declared totals remain **PASS=123, FAIL=0,
  BLOCKED=7, XFAIL=1, NOT_IMPLEMENTED=23**. The implemented local suite is
  passed; release and full Linux qualification remain not passed. Debug
  Valgrind is **PASS with reviewed suppressions**, suppression governance is
  accepted, and ReleaseSafe Valgrind remains XFAIL.
