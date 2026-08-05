# Linux product-test orchestration consolidation plan

- **Status:** Complete; implemented-local qualification passed, while declared
  release/full Linux gaps remain
- **Date:** 2026-08-05
- **Owner:** Zentty Linux port
- **Primary issue:** [#12 — Linux QA architecture](https://github.com/TamedTornado/zentty/issues/12)
- **Blocked feature slice:** [#24 — inactive-worklane agent restore](https://github.com/TamedTornado/zentty/issues/24)
- **Field record:**
  [`zentty-linux-dogfood-2026-08-01.md`](zentty-linux-dogfood-2026-08-01.md)
- **Supersedes:** further expansion of application-embedded `--exercise-*`
  scenarios or per-scenario fake-agent launch code

## 1. Why this corrective slice exists

The Linux product has one persisted session-restore implementation, but its
tests have accumulated overlapping orchestration:

- three end-to-end agent/session-restore journeys;
- four agent integration scripts with independently arranged actors;
- two independently implemented fake-Codex argument parsers; and
- five `schedule_*` scenario state machines compiled into `zentty-linux`.

The duplicate fake-Codex parser in `linux/tests/rust-session-restore` drifted
when current Codex moved session configuration into the agent subcommand
scope. The first issue #24 implementation then added three more driver flags
and pre-presentation GTK action activation to the shipping application. A
focused run passed, but the authoritative matrix exposed a non-unwinding GTK
callback panic in both agent cells during that cancellation path.

This is the failure mode issue #12 was intended to prevent: tests are real at
the system boundary, but their control plane is becoming a parallel product.
No issue #24 code may be committed until this corrective plan is implemented
and its consolidated journey passes.

## 2. Baseline inventory

### 2.1 One production session-restore path to preserve

The following are complementary layers of one implementation, not competing
systems:

1. `SessionRestoreEnvelope` and source-compatible workspace recipes;
2. `SessionRestoreStore` and its atomic snapshot/lifecycle files;
3. the launch decision and single-window selection in `zentty-linux`;
4. accepted pane drafts converted to resume commands;
5. `ApplicationShell` surfaces and the ordinary persistence path.

This slice must not introduce a new store, schema, restore manager, backup
file, or platform-specific restore implementation.

Closed-pane restoration is a separate transient undo feature. It may share
test-launch primitives, but it must not be conflated with application restart
restoration.

### 2.2 Overlapping journeys to consolidate

| Current journey | Boundary it legitimately owns | Duplicated material |
| --- | --- | --- |
| `rust-session-restore` | source envelope, persisted topology, deterministic agent resume, clean relaunch | actor parser, fixture mutation, staged launch, environment, receipts |
| `installed-codex-integration` | compatibility with the installed real Codex CLI and resumed TUI | staged launch and state setup only; it must remain a distinct compatibility journey |
| `rust-inactive-agent-restore` | inactive startup, multiple agents, focus stability, reveal/reuse, cancellation | actor parser, fixture mutation, staged launch, environment, relaunch |
| `rust-agent-ipc` | hook/wrapper/socket/reducer/sidebar event lifecycle | controlled actor arrangement and staged launch |

### 2.3 Application-embedded test control to remove or justify

Current scenario schedulers:

- `schedule_workspace_actions`;
- `schedule_agent_restore`;
- `schedule_pane_search_actions`;
- `schedule_pane_layout_actions`;
- `schedule_closed_pane_restore`.

Current paired control enums and flags include `Scenario`, `ExitPolicy`, all
`--exercise-*` flags, all scenario-specific `--quit-after-*` flags, and the
new issue #24 `expected`, `reveal`, and `close-before-present` modifiers.

The default decision is removal. An exception requires a named behavior that
cannot be driven through the delivered application's real UI, lifecycle, child
process, persisted fixture, or public action surface. Convenience is not a
sufficient reason.

## 3. Non-negotiable boundaries

1. **Test the delivered product.** Product journeys launch the same
   `zentty-linux` artifact staged for users. No test-only product build or
   compile-time behavior fork may qualify a product claim.
2. **Drive real boundaries.** GTK, Ghostty, PTYs, child processes, filesystem,
   Wayland/X11 services, wrapper/helper/socket paths, and user-visible state
   remain real.
3. **External control over internal choreography.** Use persisted starting
   state, deterministic children, physical input, window-manager close, and
   public GTK actions exposed by real controls. Do not add timers and scripted
   state transitions inside `ApplicationShell` merely to make assertions easy.
4. **One deterministic agent actor.** Tests that intentionally control the
   external agent response use one reviewed actor implementation and one
   argument parser. Tests may select declared profiles through environment or
   arguments, but may not generate new `codex` scripts inline.
5. **Real installed-agent compatibility remains separate.** The installed
   Codex journey is not replaced by the deterministic actor and must never be
   described as model-backed network execution.
6. **No generalized framework expansion.** Shared support is limited to
   repeated staged-product/environment/actor operations proven by the baseline
   duplication. No new evidence schema, daemon, RPC control plane, or test DSL.
7. **No hidden retries.** A race must be removed with an observable readiness
   boundary, not repeated until green.
8. **No qualification claim from fixtures alone.** Focused model tests support
   lifecycle ordering and invalid drafts; the real compositor journey owns the
   product claim.
9. **No Ghostty change for Zentty orchestration.** Issue #24 remains Zentty
   policy unless a minimal independently owned Ghostty defect is demonstrated.
10. **Dogfood continuously.** Every removed path, retained exception, failure,
    repair, receipt, and residual limitation is recorded while it is found.

## 4. Target architecture

### 4.1 Product composition root

`zentty-linux` owns product behavior only:

- parse legitimate product/developer launch options;
- load one restore envelope;
- construct one application shell;
- map restored inactive agent surfaces according to product policy;
- react to real user, child, hook, and window-lifecycle events;
- persist and shut down normally; and
- emit useful structured diagnostic lines without branching on a test
  scenario.

The composition root must not select worklanes, submit search queries, arrange
panes, close worklanes, or quit merely because an integration-test scenario
asked it to do so.

### 4.2 Deterministic model/API tests

Focused Rust tests own:

- restore-draft validation and command construction;
- duplicate-start eligibility and stable pane ownership;
- cancellation before surface realization;
- invalid or removed pane drafts;
- lifecycle ordering and idempotent teardown;
- workspace and closed-pane model transitions; and
- CLI option parsing for options that remain legitimate product behavior.

These tests may use fixtures and test clocks. They do not qualify GTK,
Ghostty, PTY, compositor, or installed-agent behavior.

### 4.3 Shared integration primitives

Create only the following reviewed support, using existing
`linux/tests/lib/` and fixture conventions:

1. a staged-product preflight that verifies the product and pinned Ghostty
   library/runpath;
2. temporary state/log/receipt creation and reliable cleanup helpers;
3. one deterministic agent executable or fixture script that validates the
   complete current argument contract, emits declared hook events through the
   real staged helper, writes an authenticated receipt, and can remain alive
   or delay startup when a scenario requires cancellation; and
4. bounded log/readiness helpers that fail on timeout or premature exit.

The controlled actor is the only fake below the coding-agent network boundary.
It does not replace the Zentty wrapper, hook helper, socket, PTY, or process.

### 4.4 Consolidated journeys

#### A. Deterministic restore product journey

Extend and rename the existing source-envelope journey as necessary so one
Wayland/X11 scenario owns:

- source-compatible topology and metadata;
- two inactive agent drafts in separate worklanes;
- an initially active ordinary-shell worklane;
- exactly-once background starts and independent authentication;
- stable active worklane and keyboard focus;
- ordinary inactive-shell exclusion;
- a real physical visit to the restored worklane;
- same surface, PTY, PID, session, and exact scrollback after the visit;
- clean return/close using real UI or window lifecycle;
- clean relaunch and exact persisted topology/drafts; and
- cancellation by a real close interaction against a deliberately delayed
  inactive actor, with no child/socket ownership remaining.

This journey replaces `rust-inactive-agent-restore`; it does not run both old
and new versions in the matrix.

#### B. Installed Codex compatibility journey

Retain one X11 journey that verifies the pinned installed Codex version,
ephemeral real hook configuration, authenticated real SessionStart, persisted
UUID, exact `codex resume`, and resumed TUI. It reuses staged-launch helpers but
does not use the deterministic actor.

#### C. Agent event lifecycle journey

Retain `rust-agent-ipc` only for event types and reducer/sidebar transitions
not already proven by the restore journey. It uses the shared actor and must
not retest persistence or relaunch.

### 4.5 Real UI control

Existing controlled X11 `xdotool` and Wayland `wtype` mechanisms are preferred
for commands reachable from the UI. Pointer coordinates may be used only after
an observable window/layout readiness receipt and must target stable public UI
geometry. Keyboard shortcuts use the source-compatible action vocabulary.

Where Wayland cannot expose a window identifier, the private Weston session
and single-client focus proof remain the authority. Environmental absence is a
failure, not a pass.

#### Controlled-Wayland input amendment (recorded before harness implementation)

The installed Weston 13 headless backend does **not** advertise
`zwp_virtual_keyboard_manager_v1`; therefore `wtype` cannot drive it. A real
attempt exits nonzero with `Compositor does not support the virtual keyboard
protocol`. The existing Weston environment remains authoritative for its
current renderer, lifecycle, Valgrind, and suppression-governance receipts and
must not be silently relabelled as input-capable.

Physical Wayland journeys instead use one narrowly scoped
`nested-wayland-input-v1` profile backed by Cage/wlroots and Pixman. A direct
wlroots headless attempt advertised the virtual-keyboard manager but exposed a
seat with no keyboard capability, so `wtype` connected successfully while GTK
received no keys. The reviewed configuration nests Cage's X11 backend on the
existing private Xvfb harness: the resulting Wayland seat advertises pointer,
keyboard, and touch, and a real `wtype` client succeeds. This qualifies a real
Wayland client/compositor/input path, while the matrix must identify the
controlled X11 transport rather than calling it native Wayland. That wrapper
must:

1. own a fresh runtime directory, Cage socket/process group, and nested-Xvfb
   cleanup receipt;
2. prove `wl_compositor`, `wl_output`, `wl_shm`, `xdg_wm_base`, and
   `zwp_virtual_keyboard_manager_v1` from the live protocol inventory;
3. expose only the wrapper-owned Xvfb transport and no ambient X11, Wayland,
   D-Bus, AT-SPI, or IBus endpoints;
4. run the product and `wtype` as real clients of that compositor;
5. exit 77 when Cage, `wayland-info`, or `wtype` is unavailable; and
6. be used only by cells that genuinely require physical Wayland input.

This is a capability boundary, not a duplicate product scenario: Weston stays
the stable non-input/Valgrind environment, Cage supplies the protocol Weston
lacks, and product journeys remain single and shared across display backends.

## 5. Ordered implementation — tests and guards first

### Phase 0 — Freeze and record the failed baseline

1. Record both authoritative agent-cell failures and the GTK callback panic in
   dogfood.
2. Preserve the failed summary and log hashes.
3. Confirm no qualification, compositor, Valgrind, or agent child remains.
4. Make no issue #24 product claim and no commit.

**Exit:** the failed baseline is reproducible or fully explained from retained
logs, and the worktree contains this plan.

### Phase 1 — Add anti-accretion contract tests

Add focused checks that initially fail because:

- more than one deterministic `codex` actor/parser exists;
- `zentty-linux` contains forbidden `schedule_*` scenario drivers;
- forbidden `--exercise-*` or scenario-specific quit/modifier flags are parsed;
- a matrix cell names both the superseded and consolidated restore journeys;
- an integration script creates an inline agent executable; or
- a test-only action sequence is reachable in the ordinary product binary.

The checks inspect explicit stable contracts, not broad source snapshots. Add
them to the existing architecture/support gate, not as product matrix cells.

**Exit:** failures identify the exact known duplicate paths and cannot pass by
renaming one forbidden symbol.

### Phase 2 — Centralize the controlled agent and launch primitives

1. Implement the single deterministic actor with focused parser/profile tests.
2. Move repeated staged Ghostty/runpath, temporary-state, launch, timeout, and
   cleanup operations into minimal shared shell support.
3. Convert `rust-agent-ipc` and the existing restore journey to those
   primitives one at a time.
4. Run each converted journey before removing its old setup.

**Exit:** one actor parser exists, no integration script writes an inline
`codex` executable, and event-lifecycle behavior remains green.

### Phase 3 — Remove application-embedded scenario orchestration

For each scheduler, first establish its replacement owner:

| Scheduler | Replacement owner |
| --- | --- |
| workspace actions | physical product journey plus existing `WorkspaceState` tests |
| pane search | existing physical search journeys and focused shortcut/parser tests |
| pane layout | physical controls/shortcuts plus layout model tests |
| closed-pane restore | physical shortcut journey plus closed-pane model tests |
| agent restore | consolidated deterministic restore journey plus lifecycle model tests |

Then remove the scheduler, its scenario/exit-policy variants, its private
assertion helpers, and its test-only flags. Any retained general option must be
listed in the architecture document with a product/developer use independent
of testing.

**Exit:** `zentty-linux` has no scenario scheduler and no test-only state
transition; existing product behavior remains externally testable.

### Phase 4 — Consolidate restore journeys and repair cancellation

1. Merge issue #24 coverage into the deterministic source-envelope restore
   journey.
2. Delete `rust-inactive-agent-restore` after the consolidated journey is
   green, not before.
3. Replace pre-presentation internal action activation with a real delayed
   actor and real close interaction after observable presentation.
4. Prove cancellation in focused lifecycle/model tests and the real product
   journey.
5. Keep the installed-Codex journey distinct and reuse only neutral launch
   support.

**Exit:** no callback panic, no duplicate child, no hidden focus change, exact
scrollback and PID continuity, clean cancellation, and clean relaunch pass on
controlled Wayland and X11.

### Phase 5 — Reconcile authority and documentation

1. Update the qualification matrix and architecture JSON to name only the
   consolidated journeys.
2. Reconcile the feature inventory and issue #12/#24 acceptance evidence.
3. Update dogfood with removed lineages and exact receipts.
4. Prove no old command, test name, flag, or inline actor remains referenced.

**Exit:** documentation, matrix, implementation, and executable test names
cannot contradict one another.

### Phase 6 — Qualification and publication

Run, in order:

1. focused actor/parser and anti-accretion tests;
2. full Rust workspace tests;
3. strict workspace Clippy;
4. shellcheck for every changed shell file;
5. architecture, inventory, matrix-runner, and suppression-governance tests;
6. focused consolidated Wayland and X11 journeys;
7. installed Codex compatibility;
8. the complete documented `linux/tests/qualify-local` gate once.

Do not start a second aggregate matrix manually. Do not alter code or tests
while evidence is being produced.

**Exit:** every declared executable cell passes; declared gaps remain explicit;
Debug Valgrind is reported only as **PASS with reviewed suppressions**;
ReleaseSafe Valgrind is not made green or assigned inherited evidence; the
final diff has been reviewed; dogfood
contains final hashes and totals; only then may the slice be committed and
pushed.

## 6. Acceptance criteria

### Architecture

- [x] Exactly one persisted session-restore implementation remains.
- [x] Exactly one deterministic agent actor and argument parser remain.
- [x] No integration script generates an inline `codex` or `claude` program.
- [x] No `schedule_*` integration scenario remains in `zentty-linux`.
- [x] No `--exercise-*`, scenario-specific quit, expected-count, synthetic
      reveal, or pre-presentation close flag remains.
- [x] No test-only behavior branch is compiled into the delivered product.
- [x] Shared support is limited to evidenced duplication and has focused
      failure/cleanup tests.

### Real-system fidelity

- [x] The consolidated restore journey runs the staged product under real
      nested Wayland and X11.
- [x] GTK, Ghostty, renderer, PTYs, processes, wrapper, hook helper, Unix
      socket, reducer, persistence, and sidebar are real.
- [x] Only the external agent/model response is deterministic.
- [x] Worklane visit and cancellation use real user/window interactions.
- [x] Absence of a compositor, actor dependency, or input driver fails.

### Restore behavior

- [x] Source-compatible envelope metadata/topology survives save and relaunch.
- [x] Two inactive agents start once and authenticate independently.
- [x] Active worklane and focus remain stable during background startup.
- [x] Ordinary inactive shells do not start solely for agent restoration.
- [x] Visiting reuses surface, PTY, PID, session, and exact scrollback.
- [x] Cancellation leaves no child, socket registration, or stale pane owner.
- [x] Clean relaunch repeats only the intended per-launch resume.
- [x] Installed Codex still produces and resumes an exact real UUID session.

### Regression governance

- [x] Anti-accretion tests fail for a second actor/parser or embedded scenario.
- [x] The stale feature-inventory summary expectation is repaired and tested.
- [x] Both prior authoritative callback-panic failures are recorded and
      replaced by passing receipts.
- [x] Matrix and documentation reference no deleted harness or flag.
- [x] No commit or push occurs with a required executable cell failing.

## 7. Explicit non-goals

- Rewriting the established Weston/X11 compositor wrappers. The separately
  documented `nested-wayland-input-v1` capability wrapper is the reviewed
  exception required for physical Wayland input.
- Introducing a GUI automation framework, test RPC server, or screenshot DSL.
- Combining installed real Codex compatibility with the deterministic actor.
- Claiming real installed Claude background-resume coverage without running it.
- Expanding Ghostty's public API for Zentty-specific lifecycle policy.
- Resolving unrelated NOT_IMPLEMENTED, BLOCKED, or ReleaseSafe Valgrind cells.
- Refactoring unrelated product features merely because their tests are large.

## 8. Drift controls

Before each phase, compare the intended files and exit criteria with this
document. If implementation requires a new daemon, schema, product flag,
embedded scheduler, generalized harness layer, Ghostty patch, or additional
journey, stop and amend this plan with the operator's approval before coding.

Every commit must correspond to one completed phase or one independently green
migration step. Do not mix feature additions into this corrective slice. The
issue #24 behavior already implemented may be simplified or temporarily
removed to achieve the target architecture; preservation of uncommitted code
is not an acceptance criterion.
