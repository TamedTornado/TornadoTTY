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

### Ignored-port controls and restart

- The settings control previously normalized overlapping port rules but offered
  no operator feedback: entering a port already covered by a range performed a
  redundant config write and appeared successful. The page now distinguishes
  invalid input from an already-covered port, leaves the exact config bytes
  unchanged in both cases, and reports each outcome explicitly.
- The real X11 journey physically enters invalid port `0`, privileged port
  `80`, a valid inclusive range, and an overlapping port. It proves
  rejection/no mutation, canonical persistence, duplicate rejection, and
  physical Stop Ignoring removal of both rows through the same GTK page and
  authoritative config store. The controlled listener remains live throughout;
  these controls configure relevance and never manage server processes. A
  separate real kernel-assigned listener covers an ephemeral port.
- Both X11 and Wayland journeys now terminate the original product and launch a
  fresh delivered binary against the same XDG configuration. The new process
  must resolve exactly the surviving three-target browser catalog, the selected
  custom browser, enabled passive detection, absence of the stale browser, and
  absence of the removed ignored-port range.
- GH-39's final native custom-browser chooser cell is now physically qualified
  in a controlled X11 environment with a real window manager.
- The first full parallel matrix run exposed two unrelated timing defects in
  existing physical journeys rather than product regressions: the X11 source
  UX cell sampled sidebar allocation after a fixed 200 ms sleep, and the
  Wayland shortcut/settings cell could type faster than the loaded nested
  compositor consumed command-palette input. Both failed cells passed alone.
  The X11 assertion now waits for the exact allocation receipt, while Wayland
  text entry uses a deliberately paced physical key stream. Neither repair
  bypasses GTK, Ghostty, or the controlled compositors.
- The next aggregate run exposed a pre-existing X11 clipboard ownership race in
  the agent/remote-transfer journey: `xclip` returned successfully before the
  compositor advertised `text/uri-list`, so the immediate physical paste had
  nothing to consume. The shared real-input authority now reads the X11
  `TARGETS` selection and proceeds only after the actual MIME target is owned;
  environmental absence remains a failure rather than a pass.
- That second aggregate completed in 549,700 ms with 122 expected PASS cells
  passing and only the clipboard race failing; declared matrix totals remained
  PASS=123, FAIL=0, BLOCKED=7, XFAIL=1, NOT_IMPLEMENTED=23. Debug Valgrind was
  **PASS with reviewed suppressions**. The repaired session-restore journey then
  passed against the real X11 selection, real loopback SSH/SCP, GTK, and Ghostty
  path before the full matrix was rerun.
- Final post-repair `linux/tests/qualify-local` passed every presently
  executable support and matrix cell in 512,050 ms. Declared totals were
  PASS=123, FAIL=0, BLOCKED=7, XFAIL=1, NOT_IMPLEMENTED=23. Implemented-local,
  product-boundary, and qualification-host-retired claims passed; release and
  full Linux qualification correctly remained NOT_PASSED. Debug Valgrind was
  **PASS with reviewed suppressions**: raw 427 errors/contexts and 6,080 direct
  plus 41,395 indirect bytes; post-suppression zero errors/contexts/bytes with
  all 427 reviewed contexts accounted for. ReleaseSafe Valgrind remains XFAIL.

### Custom-browser lifecycle

- The GTK page already exposed a modern native `FileDialog`, but custom-browser
  removal and live executable disappearance were prose-only gaps. The X11
  journey now removes a configured custom browser through its real row control,
  proves exact config/catalog removal, reopens the settings page, deletes the
  actual preferred executable, and observes automatic invalidation plus an
  immediate safe System Default fallback. A fresh process then proves the stale
  executable cannot return or execute.
- Reopening settings exposed a real stale-projection defect: the hidden window
  retained page-local config and could overwrite a browser choice made through
  the server action authority. Settings presentations now release the hidden
  widget tree, reload the authoritative persisted Dev Servers config, rediscover
  browser targets, and construct a fresh page. The live invalidation timer is
  tied weakly to that page and stops when its widget tree is released.
- Physical automation of GTK 4's asynchronous native `FileDialog` proved
  nondeterministic in the no-window-manager Xvfb harness: transient child XIDs
  do not identify the focused chooser reliably. This was not converted into a
  pass and no deprecated `FileChooserDialog` was retained merely for test
  convenience. At that point GH-39 remained open for a deterministic
  real-system chooser driver, while physical custom-browser removal and live
  invalidation were covered.
- The missing ingredient was not a product seam: it was a managed X11
  transient. The final journey starts a private Openbox instance inside the
  already private Xvfb session, proves `_NET_SUPPORTING_WM_CHECK`, clicks the
  real Add App control, identifies the chooser as a distinct visible transient,
  enters the exact executable path through the native location field using the
  real X11 clipboard, confirms it physically, and proves canonical persistence
  through the existing config and runtime catalog. It then removes that same
  browser physically and continues through every existing Dev Servers control.
- This also exposed and repaired a lifecycle defect in the invalidation timer:
  an unmapped old settings page could remain alive through GTK signal ownership
  and perform a second stale reconciliation. The timer now exits as soon as its
  page is no longer mapped. The complete focused X11 journey passes with one
  invalidation receipt and controlled window-manager cleanup. GH-39 is now
  complete; no chooser acceptance cell remains unclaimed.
- The first aggregate with the managed chooser correctly refused qualification:
  both X11 Dev Servers cells invalidated a non-preferred executable while the
  assertion expected System Default fallback, and the unrelated multi-window
  X11 cell missed its focus-settling deadline under load. The Dev Servers logs
  proved the authoritative preferred browser was `custom:alternate`; the test
  now removes that exact executable and requires System Default. The corrected
  focused chooser/invalidation journey passes end to end. No failed aggregate
  was described as successful.
- The next aggregate passed both Dev Servers cells but caught an unrelated X11
  bookmark dialog focus deadline under load; its immediate focused rerun passed
  the complete real management journey. The final aggregate then passed every
  presently executable support and matrix cell in 521,200 ms. Declared totals
  remained PASS=123, FAIL=0, BLOCKED=7, XFAIL=1, NOT_IMPLEMENTED=23;
  implemented-local, product-boundary, and qualification-host-retired claims
  passed while release and full Linux qualification remained NOT_PASSED. Debug
  Valgrind was **PASS with reviewed suppressions**: raw 427 errors/contexts and
  6,240 direct plus 41,460 indirect bytes; post-suppression zero errors,
  contexts, or bytes with all 427 reviewed contexts accounted for. ReleaseSafe
  remains XFAIL.
- Final post-change `linux/tests/qualify-local` passed every presently
  executable support and matrix cell in 502,440 ms. Declared totals remained
  PASS=123, FAIL=0, BLOCKED=7, XFAIL=1, NOT_IMPLEMENTED=23; implemented-local,
  product-boundary, and qualification-host-retired claims passed while release
  and full Linux qualification correctly remained NOT_PASSED. Debug Valgrind
  was **PASS with reviewed suppressions**: raw 427 errors/contexts and 6,000
  direct plus 41,362 indirect bytes; post-suppression zero errors, contexts, or
  bytes with all 427 reviewed contexts accounted for. ReleaseSafe remains
  XFAIL.

## Dev Servers settings control qualification

- The settings page existed, but its browser catalog retained an unavailable
  configured custom application until some later configuration mutation. Dev
  Servers presentation now reconciles the persisted model against the single
  existing Linux browser discovery authority, removes stale custom/enabled IDs,
  and falls an unavailable preferred browser back to System Default. Controlled
  X11 and Wayland journeys prove the stale target is removed from the real XDG
  config through the shared settings shell.
- Disabling passive detection previously removed only the GLib timer. Scanner
  and Docker records already held by the runtime remained visible, and an
  in-flight probe could repopulate them after disable. The runtime registry now
  removes only Scanner and Docker sources, preserves authenticated Watch and
  Manual sources, refuses new probes while disabled, and discards a probe that
  completes after the setting changes. Sidebar projection is refreshed without
  signaling any server process.
- The real X11 journey physically switches passive detection off and on. It
  proves exact persistence, immediate passive-source removal, survival of the
  real listener and authenticated Watch record, and rediscovery of that same
  kernel listener after re-enable. Wayland owns real page presentation,
  reconciliation, and the pre-existing real scanner/browser actions; its nested
  harness still has no scoped pointer injector, so switch mutation is explicitly
  owned by X11 rather than silently skipped.
- A controlled screenshot exposed repeated Firefox and Google Chrome rows from
  overlapping native, Snap/Flatpak desktop registrations, and executable
  discovery. The Linux browser catalog now sorts desktop IDs deterministically
  and deduplicates case/spacing-equivalent display names, preferring an already
  resolved built-in executable while retaining genuinely distinct applications.
- Remaining GH-39 scope is explicit: physical ignored-port add/remove and error
  controls, the native custom-browser chooser/removal journey, invalidation when
  a browser disappears while settings remain open, and a full process restart
  receipt. The platform settings contract therefore remains NOT_IMPLEMENTED.
- The first aggregate rerun failed the unrelated controlled Claude X11 cell
  because its model receipt missed the teammate request under load. The focused
  rerun completed all model roles, then exposed a separate harness defect: its
  relocated-wrapper assertion compared the child's canonical absolute path to
  the caller's relative `ZENTTY_LINUX_BINARY`. The journey now canonicalizes the
  staged product once before deriving its installation root. This does not
  weaken any model, wrapper, endpoint, or teardown assertion.
- The corrected aggregate run passed all presently executable support tests and
  matrix cells in 500,680 ms. Declared totals remain **PASS=123, FAIL=0,
  BLOCKED=7, XFAIL=1, NOT_IMPLEMENTED=23**. The implemented local suite is
  passed; release and full Linux qualification remain not passed. Debug
  Valgrind is **PASS with reviewed suppressions**, suppression governance is
  accepted, and ReleaseSafe Valgrind remains XFAIL.

## GH-36 authoritative configuration reload plan

- **Discovery:** Linux currently has one safe writer (`ConfigStore`) but not one
  configuration authority. `main` reads one startup snapshot, each
  `ApplicationShell` clones it, and a settings presentation performs an ad-hoc
  Dev Servers refresh. There is no parent-directory watcher and therefore no
  way for a valid external edit to reach an already open window. Invalid input
  is represented as defaults plus a warning, which is safe at startup but must
  never be mistaken for a valid live request to reset a running application.
- **No-accretion rule:** extend `ConfigStore` as the sole bounded reader/writer
  and add one application-owned reload authority. Do not add page-local file
  monitors, caches, or a second persistence format. The application authority
  owns debounce, last-good selection, diagnostics, and publication to every
  shell; shells only project an accepted snapshot into existing runtime/UI
  owners.
- **Test-first order:** (1) pure/real-filesystem authority tests for unchanged,
  valid, missing, invalid, read-error, and coalesced observations; (2) focused
  shell projection tests; (3) one staged real-GTK multi-window journey that
  replaces the config by rename while live Ghostty PTYs retain their PIDs and
  sentinels; (4) adversarial symlink, chmod, concurrent-write, and interrupted
  write journeys; (5) governed mutation, matrix registration, focused suites,
  then the complete presently executable qualification run.
- **Initial feature slice:** parent-directory observation with a 150 ms quiet
  period, content-safe last-good retention, no publication for self-write or
  semantically unchanged snapshots, and representative live projection to all
  windows without reconstructing a shell or PTY. A missing, unreadable, or
  invalid file is a retained-last-good diagnostic, never a pass or a reset to
  defaults.
- **Later slices already bounded by GH-36:** independent-section partial reload
  (which requires a parser result richer than the current all-or-nothing
  `AppConfig::parse_toml`), include dependency watching if includes become part
  of the authoritative schema, complete runtime projection for every config
  section, durability/permission adversarial cases, user documentation, matrix
  cells, and mutation closure. The issue stays open until those contracts and
  the real-system journeys pass.
- **First real-journey correction:** the controlled journey initially expected
  Ctrl+Q to exercise the per-window confirmation title. Ctrl+Q is the existing
  application-quit command, so the product correctly presented `Quit Zentty?`;
  the test expectation, not the live projection, was wrong. The journey now
  asserts the source command/title it physically invokes.
- The corrected title exposed a second test-fixture error: it enabled
  `confirm_before_closing_window` while physically invoking application quit,
  whose independent source setting remained false. The application therefore
  correctly exited without a dialog. The fixture now changes and proves both
  confirmation fields; no product behavior was weakened to satisfy it.
- The next X11 run proved live publication and dialog presentation, then the
  harness tried to send Escape to the remembered terminal XID rather than the
  active GTK confirmation transient. Existing confirmation journeys correctly
  use compositor-global input for this modal. This journey now does the same;
  the failure was retained as evidence rather than misclassified as a product
  lifecycle defect.
- After modal cancellation, the shared physical-input helper deterministically
  refocused the first product window, but the new journey expected its sentinel
  from the second pane. Logs proved the bytes reached the original real PTY and
  its title callback. The assertion now names that actual deterministic target;
  both PTY PIDs remain independently checked across publication.
- Wayland correctly preserves compositor activation on the second window after
  its modal closes, unlike the window-manager-free X11 harness which selects
  its first discovered toplevel. The cross-compositor journey now states these
  two deterministic physical targets explicitly instead of pretending focus
  policy is identical.
- A subsequent Wayland run exposed a genuine asynchronous boundary in the
  harness: Ghostty's terminal-ready callback can precede observation of the PTY
  child in `/proc`. The exact two-child assertion now uses the same bounded
  convergence already established by the multi-window journey. Environmental
  absence is still a failure, not a pass.
- Focused controlled X11 and Wayland journeys then passed the complete initial
  slice with two real windows and PTYs, replace-by-rename coalescing, live
  confirmation behavior, post-reload terminal input, invalid/missing last-good
  retention, and content-safe diagnostics. The first workspace test invocation
  ran under a restricted syscall sandbox and eight unrelated Agent IPC tests
  failed to create Unix sockets with `EPERM`; this is environmental absence,
  not a pass. The suite was rerun with its normal real-socket permissions.
- The first focused mutation run correctly found four surviving watcher
  routing mutants because only the authority's load decisions had unit tests;
  real compositor journeys killed them behaviorally but were not part of that
  fast mutation command. A real GLib directory-monitor test now distinguishes
  unrelated file renames from an atomic replacement whose destination alone is
  `config.toml`. The governed rerun tested 11 mutants: **7 caught, 4 unviable,
  0 missed**. Cargo-mutants used the repository's mandatory ignored-tree and
  no-target-copy policy, so no build tree was replicated.
- Final post-slice `linux/tests/qualify-local` passed every presently executable
  support and matrix cell in **501,870 ms**, including both new controlled
  config-live-reload cells. Declared totals are **PASS=125, FAIL=0, BLOCKED=7,
  XFAIL=1, NOT_IMPLEMENTED=23**. Implemented-local, product-boundary, and
  qualification-host-retired claims pass; release and full Linux qualification
  correctly remain NOT_PASSED. Debug Valgrind remains **PASS with reviewed
  suppressions** and ReleaseSafe remains XFAIL. GH-36 is not complete: partial
  independent-section reload, complete projection, and the remaining
  permission/symlink/concurrency/crash durability cases stay explicit.
- Final diff review caught a transaction risk not exercised by the representative
  clipboard/confirmation journey: projecting appearance or shortcuts one window
  at a time could leave earlier windows updated if preparation failed for a
  later window. Reload now validates every window's shortcut projection first,
  performs process-wide appearance preparation once, and only then executes the
  infallible per-window commit. Qualification evidence from before this repair
  was invalidated and rerun.
- That aggregate correctly failed eleven existing settings cells. Treating
  every watched change as external had withdrawn the open settings window after
  its own `ConfigStore` write, interrupting the physical journey. This exposed
  the still-missing self-write provenance contract. Closing the page is not an
  acceptable substitute: it breaks ordinary controls, and leaving a page open
  is safe for its own callback because that widget already reflects its change.
  Reload therefore no longer withdraws the page. Refreshing an open page after
  a genuinely external edit remains explicit GH-36 scope alongside a durable
  self-write generation token; it is not silently claimed by this slice.
- The focused real X11 Open With journey passed after that repair, including
  its real custom executable, canonical target, missing-target diagnostic,
  physical input, and persistence controls. A direct Notifications invocation
  outside its matrix wrapper correctly refused to run without the required
  private D-Bus session; that environmental refusal was not called a product
  pass. The complete matrix owns its proper controlled notification bus.
- The corrected final aggregate passed every presently executable support and
  matrix cell in **507,680 ms**. Declared totals are **PASS=125, FAIL=0,
  BLOCKED=7, XFAIL=1, NOT_IMPLEMENTED=23**; implemented-local,
  product-boundary, and qualification-host-retired claims pass, while release
  and full qualification correctly remain NOT_PASSED. Debug Valgrind is **PASS
  with reviewed suppressions**: raw 427 errors/contexts, 6,240 definite bytes,
  and 41,397 indirect bytes; post-suppression errors, contexts, and leaked bytes
  are zero, with all 427 reviewed contexts accounted for. ReleaseSafe remains
  XFAIL. This receipt supersedes the failed intermediate aggregate, not its
  recorded discovery.

## GH-36 symlink-managed live configuration slice

- **Pre-implementation boundary:** this slice makes the already supported
  symlink-preserving writer observable while the product is running. One
  watcher set must observe both the operator-facing config entry and the
  resolved target; replacing the symlink must rebuild that set so later edits
  to the new target are not lost. It must not add a second parser, store, or
  page-local monitor.
- **Test-first contract:** extend the controlled real X11/Wayland journey so
  the application starts from a config symlink whose target lives in a
  different directory, receives atomic target replacements, receives a
  symlink retarget, then receives another replacement of the new target. The
  symlink must remain a symlink, both windows must receive the final state, and
  the exact real PTY child set must survive. Add focused monitor coverage for
  target changes and retargeting before changing watcher implementation.
- **Failure policy:** a broken or unreadable target retains last-good state and
  remains a diagnostic. Rebuilding the watch set is required after every
  relevant logical-path event; environmental inability to watch either parent
  is a failure, not a pass. Existing regular-file replace-by-rename coverage
  remains authoritative and must not be weakened.
- **First real-system failure:** the initial X11 extension saw publication of
  the symlink itself but missed the first target replacement. The application
  had installed its new target watcher, but the harness raced that installation
  because it used the earlier reload receipt as the only barrier. The product
  now emits the existing watch-install receipt after every rebuilt watch set,
  and the journey waits for that receipt before mutating a newly watched
  target. This was a test synchronization defect, not converted into a pass.
- **Staging discovery:** the first synchronization rerun accidentally exercised
  the prior staged binary: the release-safe Cargo output had been rebuilt, but
  a preceding compound command had not copied it after a failed relative
  `GHOSTTY_LIB_DIR` build. File timestamps and the missing new diagnostic string
  proved the mismatch. The binary was explicitly restaged before any result was
  accepted; both controlled compositor journeys subsequently passed.
- **Unit-harness failure:** running all focused monitor tests concurrently
  allowed two GLib local-source callbacks to share the default main context
  across Rust test worker threads, triggering GLib's thread guard. The monitor
  tests now serialize only their real main-context ownership with a test mutex;
  production behavior and other tests remain parallel. The complete focused
  module then passed five tests, including proof that watching a broken symlink
  does not create its operator-managed target directory.
- Controlled X11 and Wayland now both pass the expanded real-product journey:
  logical symlink publication, first target atomic replacement, live retarget,
  replacement of the new target, identical final projection to two windows,
  preservation of the symlink, and preservation of both exact PTY child PIDs.
- The first focused mutation invocation was correctly refused at its unmutated
  baseline because the restricted syscall sandbox denied the existing real
  kernel-listener test with `EPERM`. The governed runner was rerun with normal
  real-socket permissions. A non-persistent command wrapper then cut that run
  off after nine of twelve mutants, so its incomplete receipt was rejected and
  the suite was rerun in a persistent session. Final result: **12 tested, 8
  caught, 4 unviable, 0 missed**; the safe ignored-tree/no-target-copy policy
  remained active.
- Final `linux/tests/qualify-local` passed every presently executable support
  and matrix cell in **535,820 ms**, including the expanded controlled X11 and
  Wayland symlink journeys. Declared totals remain **PASS=125, FAIL=0,
  BLOCKED=7, XFAIL=1, NOT_IMPLEMENTED=23**. Implemented-local,
  product-boundary, and qualification-host-retired claims pass; release and
  full Linux qualification correctly remain NOT_PASSED. Debug Valgrind remains
  **PASS with reviewed suppressions** and ReleaseSafe remains XFAIL. GH-36 is
  still open for the explicitly listed parser, provenance, permission,
  concurrency, crash durability, and complete-projection work.

## GH-36 live permission-transition slice

- **Pre-implementation boundary:** extend the same reload authority and the
  same controlled config journey; do not add polling, a second watcher, or a
  special test-only load path. A readable-to-unreadable transition must retain
  the accepted runtime state with a content-safe diagnostic. Restoring access
  and then publishing a valid edit must resume ordinary live projection without
  restarting windows or PTYs.
- **Test-first contract:** prove that the real directory monitor reports an
  attribute transition for the watched config target, then extend both staged
  compositor journeys with mode `000` refusal, mode restoration, a subsequent
  atomic valid target replacement, identical two-window projection, and exact
  PTY PID preservation. If the effective test identity can bypass mode bits,
  the journey must refuse rather than call that a pass.
- The focused real-monitor test passed: changing a symlink target from mode
  `0600` to `0000` and back produced independently debounced observations. The
  test is serialized with the other GLib default-context monitor tests, not
  with unrelated unit work.
- Both controlled compositor journeys passed as a non-root product identity.
  Mode `0000` produced a content-safe retained-last-good receipt; mode
  restoration followed by an atomic valid replacement resumed identical
  projection to both windows, and the exact two PTY child PIDs remained
  unchanged. This qualifies file read-permission loss and recovery, not every
  possible directory traversal/ownership/ACL transition; broader permission
  cases remain explicit GH-36 scope.
- The governed focused mutation rerun remained **12 tested, 8 caught, 4
  unviable, 0 missed**. Final `linux/tests/qualify-local` then passed every
  presently executable support and matrix cell in **521,210 ms**. Totals remain
  **PASS=125, FAIL=0, BLOCKED=7, XFAIL=1, NOT_IMPLEMENTED=23**;
  implemented-local passes, release/full qualification remain NOT_PASSED,
  Debug Valgrind remains **PASS with reviewed suppressions**, and ReleaseSafe
  remains XFAIL.

## GH-36 independently valid section reload slice

- **Pre-implementation boundary:** partial reload is a core schema decision,
  not a Linux watcher heuristic. Add one typed partial parser beside the
  existing strict startup parser. TOML syntax failure remains all-or-nothing;
  after syntax succeeds, each known top-level section is independently decoded
  and normalized. An invalid known section retains that section from the
  accepted last-good snapshot while absent valid sections deliberately return
  to source defaults.
- **No-accretion rule:** `ConfigStore` remains the only bounded file reader and
  `ConfigReloadAuthority` remains the only last-good publisher. Do not split
  the file, introduce per-page stores, parse diagnostic strings, or maintain a
  second schema. Partial diagnostics may name known sections but must not echo
  values or file contents.
- **Test-first contract:** core tests must prove independent valid/invalid and
  absent/default behavior across representative fallible and infallible
  sections, strict parser compatibility, parse-failure refusal, deterministic
  section ordering, and content-safe diagnostics. The real X11/Wayland journey
  must publish a valid section from a partially invalid edit to both windows,
  retain the invalid section's live behavior, preserve exact PTY PIDs, and then
  converge normally on a fully valid replacement. Mutation closure and full
  qualification are required before commit.
- **Broken-target discovery:** refactoring the bounded loader exposed that a
  broken logical symlink passed `symlink_metadata` and then the startup loader
  treated its missing target as an absent config, which would have published
  defaults during live reload. The new reload-specific bounded read classifies
  that case as unavailable and retains last-good; startup's intentional
  missing-file defaults remain unchanged. A focused authority regression test
  now pins the distinction.
- The first controlled X11 attempt executed the prior staged binary even though
  the combined build/copy command had begun a new Cargo build: its timestamp
  and absence of the new `retained-sections` receipt proved the copy had not
  occurred. No result was accepted. The completed Cargo artifact was copied in
  a separate explicit command, its embedded receipt verified, and the corrected
  X11 journey passed.
- The final explicitly staged binary passed both controlled compositor
  journeys. A syntax-valid edit with invalid `confirmations` and valid
  `clipboard` published the clipboard change to both windows, retained the
  previous confirmation behavior, named only `confirmations` in its diagnostic
  receipt, exposed no rejected value, and preserved both PTY PIDs. A subsequent
  fully valid replacement converged normally. The strict startup parser remains
  unchanged; core compatibility tests prove every fully valid partial result is
  identical to strict parsing and that absent sections use source defaults.
- Full workspace Clippy with all targets and `-D warnings` initially rejected
  the long parser method and a large reload-decision enum. The parser macro now
  derives field names and last-good ownership without repetitive branches, and
  the config payload is boxed at the decision boundary. A second ownership lint
  removed needless `ConfigSnapshot` moves through application startup. The
  complete Clippy rerun passed without adding lint exemptions.
- Governed mutation testing passed both decision layers: the core partial parser
  tested **2 mutants, both caught**, and the reload authority tested **12
  mutants, 8 caught and 4 unviable, 0 missed**. Both invocations used the
  mandatory ignored-tree/no-target-copy policy.
- Final `linux/tests/qualify-local` passed every presently executable support
  and matrix cell in **577,190 ms**, including the expanded partial-reload X11
  and Wayland scenarios. Totals remain **PASS=125, FAIL=0, BLOCKED=7,
  XFAIL=1, NOT_IMPLEMENTED=23**. Implemented-local passes; release and full
  qualification remain NOT_PASSED. Debug Valgrind remains **PASS with reviewed
  suppressions**, and ReleaseSafe remains XFAIL. GH-36 remains open for
  self-write provenance/open-page refresh, broader permission and concurrency
  cases, interrupted/crash durability, and complete projection edges.

## GH-36 product-write serialization and durability slice

- **Pre-implementation audit:** most product config writers take the shared
  advisory lock, but shortcut, ignored-port, and preferred-browser writers do
  not. Their read/modify/replace cycles can therefore lose a distinct
  concurrent product update. The lock is also misleadingly named
  `.zentty-appearance.lock`, and atomic replacement syncs the temporary file but
  not the parent directory after rename.
- **Contract:** every Zentty-owned config read/modify/replace uses the same
  target-directory lock, preserving distinct product-owned changes. Every
  successful replacement syncs file data before rename and the containing
  directory after rename. Failure to sync the directory is a failed write, not
  a durable success claim. The symlink entry remains untouched because locking
  and replacement continue to operate on the resolved target.
- **External-writer boundary:** an uncooperative external editor does not honor
  Zentty's advisory lock. Its overlapping atomic rename and a Zentty rename are
  atomic last-writer-wins operations; neither can tear the file. The live
  watcher must converge on whichever complete document is finally present.
  This slice does not falsely promise compare-and-swap semantics that POSIX file
  replacement does not provide.
- **Test-first contract:** concurrent real threads update independently owned
  Zentty sections and must preserve both; the three formerly unlocked paths
  must contend on the same lock; parent-directory sync failure boundaries must
  be explicit; existing comment, unknown-key, symlink, size, and invalid-source
  tests must remain green. Mutation and full qualification are required.
- The first governed writer mutation run tested 15 mutations and correctly
  reported five survivors. All five were missing direct boundary observations
  in the newly shared editable-source helper: missing versus non-missing read
  errors and exact versus over-limit size. A focused real-filesystem test now
  proves missing is empty, mode `0000` is an error, exactly 1 MiB is accepted,
  and one byte over is rejected. The rerun tested all 15: **13 caught, 2
  unviable, 0 missed**.
- Deterministic contention tests hold the actual advisory file lock while each
  formerly unlocked writer runs on a real thread; each blocks until release.
  A separate two-thread journey concurrently updates General and Updates and
  proves both independently owned sections survive. These tests would fail if
  lock calls were removed rather than merely hoping a scheduler exposes a race.
- A direct ad-hoc mutation command copied the source without the ignored
  `build/` dependency tree and its unmutated baseline correctly failed because
  the pinned Ghostty library was absent there; it tested no mutants and no
  result was accepted. Rerunning through `linux/tests/mutate-rust` supplied the
  governed ignored-tree/no-target-copy policy and dependency environment.
  Focused mutation of `atomic_replace` then tested four mutants: **3 caught, 1
  unviable, 0 missed**, including the durable replacement path.
- The required full qualification rerun exposed a pre-existing ordering bug in
  the development-server X11 journey: the product deliberately logs browser
  invalidation before its persistence callback completes, while the journey
  treated that trigger log as proof that the config file had already converged.
  The product subsequently writes the correct fallback, but the journey raced
  that write and failed `development-servers-x11` after 45.53 seconds. The
  repair retains the real UI and filesystem boundary and waits (bounded to five
  seconds) for the authoritative file state after observing the trigger; it
  does not weaken or skip the final exact assertions. This is harness ordering,
  not a product exemption, and the failed qualification receipt is retained as
  evidence until the clean rerun replaces the current summary.
- The repaired focused X11 journey passed through the real nested X server,
  real UI controls, real browser executable disappearance, and real config
  filesystem convergence. The subsequent complete qualification rerun passed
  every presently executable support and matrix cell in **504,870 ms**. Totals
  remain **PASS=125, FAIL=0, BLOCKED=7, XFAIL=1, NOT_IMPLEMENTED=23**;
  implemented-local passes, while release and full Linux qualification remain
  NOT_PASSED. Debug Valgrind remains **PASS with reviewed suppressions** and
  ReleaseSafe Valgrind remains XFAIL.

## GH-36 live settings-editor convergence slice

- **Pre-implementation audit:** live reload updates every terminal window's
  runtime model, but an already visible settings window retains the editable
  widgets constructed from its older snapshot. Re-presenting settings rebuilds
  it, which prevents a hidden stale page from writing later, but the visible
  page can still show and submit stale state after an external edit.
- **Contract:** when an accepted external publication changes a shell's config,
  any visible settings editor owned by that shell is rebuilt from the accepted
  authority while preserving its currently selected section. A hidden editor
  stays hidden. A product self-write whose initiating shell already equals the
  accepted document must not rebuild its editor or cause a reload loop. Other
  windows still receive and visibly converge on that product write.
- **Test-first boundary:** keep one settings implementation and one config
  authority. Expose the settings shell's selected section rather than adding a
  second navigation model. Unit tests pin the refresh decision, and the real
  X11/Wayland reload journey must keep a settings window open across an external
  atomic replacement, observe reconstructed controls from the new state, retain
  the selected section, preserve both PTYs, and prove a later product write
  produces no repeated publication.
- The first expanded X11 run reached and passed the new external-refresh and
  self-write checks, then exposed an older journey assumption: the later
  coalescing assertion waited for an absolute total of two projection receipts.
  Once the earlier scenario legitimately added projections, that wait returned
  before the writes under test had published. The journey now snapshots the
  prior receipt count and waits for exactly two additional window projections;
  no product outcome was converted into a pass.
- The next X11 run progressed past coalescing and exposed a second historic
  assumption: after asking the shared input helper to focus the first mapped
  product toplevel, the journey hard-coded that X11 would select `pane-1` while
  Wayland would select `pane-window-2`. X11 window enumeration changed after
  rebuilding the transient settings window, so the real key event reached the
  other still-live PTY. The assertion now accepts the sentinel from either of
  the two exact retained pane IDs; child PID equality is still asserted before
  and after publication, so this removes a window-order guess rather than
  weakening terminal-liveness coverage.
- The first Wayland run showed a genuine compositor-ordering gap in the new
  scenario: terminal readiness preceded activation of the newly created second
  toplevel, so the palette opened in one window and compositor activation moved
  keyboard focus to the other midway through physical text entry. The journey
  now treats the existing `active-window=window-2` receipt as the Wayland
  activation barrier before opening settings. Environmental focus absence is
  still a failure, not a pass or desktop assumption.
- The activation barrier fixed initial entry. The next Wayland run proved the
  rebuilt settings page had the accepted Beta model, but its immediate Alt+S
  arrived before the compositor activated the replacement toplevel and was
  correctly reported by the main window as an unbound shortcut. The journey now
  also waits for one additional settings-activation receipt after reconstruction
  before exercising the product self-write. The X11 path continues to focus the
  exact newly mapped settings window by ID.
- The first implementation of that second barrier sampled the prior activation
  count before the initial settings window had itself emitted an activation
  receipt. It could therefore mistake the delayed initial receipt for the
  rebuilt window's receipt. Two repeated controlled runs reproduced the exact
  `option+s` delivery to the main window. The journey now requires initial
  settings activation before it snapshots the count, making the later
  increment unambiguous.
- The complete qualification rerun then failed both expanded cells and was not
  accepted. X11 exposed an earlier race: opening settings performs a legitimate
  Open With availability reconciliation and config self-write; the external
  Beta replacement could overlap that write before authority had published it.
  The journey now requires that settings-presentation write to complete one
  authoritative publication before beginning its external-write transaction.
  Wayland also showed that an exact `pgrep -P` child-list comparison included a
  short-lived non-PTY helper started by background project discovery. The test
  now records the original two real PTY child PIDs and proves each remains alive
  with the product as its parent; it no longer treats an unrelated additional
  helper as PTY replacement. Later exact PTY checks remain in the same journey.
- After both repaired config cells passed against the exact qualified binary,
  the next complete qualification receipt passed them but failed the unrelated
  multi-window X11 cell while waiting for restored active-window focus. No code
  or exemption was applied: the same exact qualified cell was rerun alone in a
  new controlled X11 session and passed its real two-window transfer, clean and
  SIGKILL restore, size restore, and non-final-close scenarios. The failed full
  receipt remains rejected; a fresh complete qualification rerun is required.
- The fresh complete rerun passed every presently executable support and matrix
  cell in **520,560 ms**, including both expanded config-live-reload journeys
  and the unchanged multi-window X11 journey. Totals remain **PASS=125,
  FAIL=0, BLOCKED=7, XFAIL=1, NOT_IMPLEMENTED=23**. Implemented-local passes;
  release and full Linux qualification remain NOT_PASSED. Debug Valgrind is
  **PASS with reviewed suppressions**; ReleaseSafe Valgrind remains XFAIL.
  Focused refresh mutation tested three mutants: **2 caught, 1 unviable, 0
  missed**.

## GH-36 closure sweep

- **One-batch rule:** finish the issue rather than landing more partial slices.
  The remaining acceptance is grouped into one closure change: secure XDG
  directory/file ownership; deterministic external/product last-writer outcomes;
  ignored interrupted temporary writes; complete schema projection through the
  existing settings/runtime authorities; explicit include boundary; reconciled
  user/architecture/inventory/matrix documentation; governed mutation and one
  final complete qualification receipt. The issue closes only if every item is
  evidenced and no GH-36 limitation remains in authoritative prose.
- **Security contract:** Zentty owns `$XDG_CONFIG_HOME/zentty` and creates or
  tightens that directory to mode `0700`. Product config temporary/final files
  and the advisory lock are mode `0600`. A symlink-managed config remains valid:
  Zentty secures its own logical directory but does not chmod an external target
  directory selected by the operator. Test overrides resolve wholly inside the
  controlled temporary XDG tree.
- **Concurrency/crash contract:** an external editor does not participate in
  Zentty's advisory lock. Complete atomic replacements are last-writer-wins;
  Zentty never promises cross-process merge/CAS. When the product writes after
  an external replacement it rereads that complete document and preserves its
  unknown keys/comments. When an already-started external transaction renames
  after a product write, that complete external document wins. A process that
  dies after writing its sibling temporary file but before rename leaves the
  accepted config untouched; the directory watcher ignores that unrelated
  file.
- **Include boundary:** source `AppConfig` is one TOML document and has no
  include directive. Ghostty `config-file` includes belong to Ghostty's separate
  configuration/reload authority and are already qualified by the appearance
  journey. Zentty preserves unknown keys but does not invent recursive includes
  or a second watcher for them. This resolves the issue wording explicitly
  instead of silently claiming an unsupported schema feature.
- **Real product evidence:** the existing `rust-config-reload` journey, rather
  than a new harness, now begins with a deliberately permissive logical XDG
  directory/file and proves the running product tightens them to `0700`/`0600`
  on both controlled X11 and nested Wayland. It then exercises both orderings
  of an overlapping product/external atomic transaction, visible Settings
  reconstruction, preservation of an external unknown table, and a real shell
  process killed after leaving an invalid sibling temporary. Both focused
  journeys passed against the freshly staged product: X11 session
  `a4a3ba7d78dbfc8e331287cc1f3bd1c9` and Wayland session
  `2cce468a1462846bb691610f49a4b6a0`. The intentional SIGKILL produces a shell
  `Killed` diagnostic; status 137 and the retained complete config are asserted.
- **Mutation discovery and repair:** the first governed security-helper run
  exposed one missed top-level default-path mutant. An isolated child-process
  test was added so process environment is real without racing the parallel
  test binary. The next run caught that path but exposed two missed branch
  guards: non-directory parent and non-NotFound inspection failure. Focused
  fixtures now cover a regular-file parent and an inaccessible ancestor. Final
  review then identified that an existing regular logical config also needed
  tightening at startup, not only after a product write. Adding that behavior
  exposed two further branch mutants; explicit symlink-target mode preservation
  and non-NotFound path-inspection fixtures caught them. The final governed run
  tested **16 mutants: 16 caught, 0 missed, 0 timeout, 0 unviable**.
  `gitignore = true` and `copy_target = false` remained enforced by
  `linux/tests/mutate-rust`; no build tree was copied.
- **Closure qualification:** strict workspace Clippy (`-D warnings`) and the
  locked complete Rust workspace suite passed. The mandatory fresh
  `linux/tests/qualify-local` receipt then passed every presently executable
  support and matrix cell in **540,500 ms**. Declared totals are **PASS=125,
  FAIL=0, BLOCKED=7, XFAIL=1, NOT_IMPLEMENTED=23**. The implemented local suite
  passes; release and full Linux qualification remain NOT_PASSED. Debug
  Valgrind is **PASS with reviewed suppressions**: the preserved unsuppressed
  receipt reports 427 errors/contexts and 6,240 direct plus 41,428 indirect
  bytes; the governed post-suppression receipt reports zero errors/contexts and
  zero direct/indirect bytes, with all 427 contexts explicitly counted as
  suppressed. ReleaseSafe Valgrind remains XFAIL. The machine receipt is
  `build/linux/qualification-summary.json` (generated evidence, not committed).
- **Rejected final-code receipt:** after the final startup-permission repair,
  the fresh complete run passed every product/config cell but suppression
  governance correctly rejected one Debug GTK/Pango layout-cache observation:
  the same two narrowed contexts accounted for 14,731 bytes, below the reviewed
  20,865–26,208 range. No rule or range was broadened and the receipt was not
  accepted. An immediate new raw/suppressed run of the same controlled,
  non-Ghostty GTK/IBus reproducer again produced two contexts/26,208 bytes,
  zero post-suppression errors, and passed the unchanged governance manifest.
  This establishes cache-size variability rather than a Zentty product finding,
  but the one lower observation remains retained here; another complete receipt
  is required before commit.
- **Accepted final-code receipt:** the next unchanged complete run passed every
  presently executable support and matrix cell in **522,530 ms**. Declared
  totals remain **PASS=125, FAIL=0, BLOCKED=7, XFAIL=1,
  NOT_IMPLEMENTED=23**. Implemented-local passes; release and full Linux
  qualification remain NOT_PASSED. Debug Valgrind is **PASS with reviewed
  suppressions**: its preserved unsuppressed receipt reports 427 errors/contexts
  and 6,160 direct plus 41,428 indirect bytes; its post-suppression receipt
  reports zero errors/contexts and zero direct/indirect bytes, with 427
  suppressed contexts. ReleaseSafe Valgrind remains XFAIL. This is the final
  receipt used for the GH-36 closure decision.
