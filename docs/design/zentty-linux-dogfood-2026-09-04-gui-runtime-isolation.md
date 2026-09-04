# GUI runtime isolation dogfood record

Date: 2026-09-04

Issues: GH-160, GH-161, GH-162, GH-163, GH-164

## Objective

Prevent terminal workloads and high-rate agent integration signals from making
the GTK client unresponsive or consuming unbounded GUI memory. Preserve real
PTYs and real agent integrations; do not hide environmental failures or replace
the system with mocks.

The work is ordered deliberately:

1. GH-161: remove duplicate canonical mutations and unbounded GUI work.
2. GH-162: place pane process trees in explicit per-pane process groups and
   systemd user cgroups outside the GUI scope.
3. GH-163: bound transport queues and main-loop work.
4. GH-164: consider an out-of-process broker only if reproducible native
   crashes remain after the preceding controls.

## Field evidence

### Long-lived process inspection

The installed TornadoTTY process inspected before the second failure had:

- PID `3677559`;
- approximately 18 hours 46 minutes of uptime;
- `VmRSS` 8,452,860 KiB;
- anonymous PSS 8,327,316 KiB;
- approximately 7,890,348 KiB resident in the main heap mapping;
- 50 threads;
- approximately 37 GiB current and 41.9 GiB peak memory in the application
  cgroup, which also contained pane descendants;
- no cgroup OOM or kernel OOM record.

The application log repeatedly showed an `agent.running` event causing all of
the following work even when its projected status had not changed:

- pane context ownership logging;
- project-context refresh requests;
- whole-sidebar reconciliation;
- fleet snapshot refresh and popover rendering;
- active-worklane reveal scheduling.

`AgentStatusStore::apply_for_target_with_signal` also refreshed `updated_at` for
identical running events, so downstream equality checks saw a new fleet
snapshot every time.

### 08:35 GUI wait/force-quit incident

At approximately 08:35 CEST the user received GTK/GNOME's wait-or-force-quit
dialog again and the terminal disappeared. No source edits, build, or test run
had occurred before this incident; only bounded source inspection commands were
running in a pane.

The affected process was PID `1441610`. Initial inspection incorrectly treated
the last record in the queried journal window as process termination. Later
records from the same PID prove that it survived or recovered from this first
dialog and continued running. There were 4,193 application records in the
roughly 87 seconds ending at `2026-09-04T08:35:56.765+02:00`.

The final records show two overlapping sources of high-rate work:

1. authenticated `agent.running` events repeatedly triggered project context,
   sidebar, and fleet refreshes;
2. both running Codex panes emitted changing Braille spinner frames through the
   terminal-title callback while TornadoTTY's own frame-clock animation also
   rendered those titles.

The second observation is **not** evidence of multiple installed GTK tick
callbacks. `ensure_codex_title_animation_tick` stores one `TickCallbackId` and
rejects a second registration. The defect is that
`CodexTitleAnimation::reconcile` compares the full incoming title, including
the spinner glyph. Each source-generated frame is therefore treated as a new
semantic title, resets `last_frame`, and causes redundant derived presentation
work. This distinction must remain explicit in the repair and tests.

The force-quit action itself prevents post-mortem heap attribution. These
records strongly identify unbounded application-owned main-loop work and heap
retention, but the exact retained allocation path still requires controlled
measurement.

### 09:17 GUI force-quit incident

The same installed PID `1441610` produced a second wait/force-quit dialog and
the TornadoTTY window died at `2026-09-04T09:17:41+02:00`. The PID no longer
existed immediately afterward. Again, there was no Rust panic, clean shutdown,
kernel OOM, or cgroup OOM record.

The two minutes before termination contained:

- 9,840 terminal-title records;
- 4,902 Codex title-animation records;
- 190 authenticated agent-event records;
- 904 sidebar-agent-status renders;
- 204 fleet refreshes;
- 305 project-context records.

That is approximately 82 title records per second before counting the UI and
project work caused by duplicate `agent.running` events. This incident occurred
against the installed pre-repair build while the focused repaired build was
being tested separately. It is direct field confirmation of the reproduced
event-amplification path, not a failure of the repaired test binary.

## Retained allocation path

Source review and a red-before/green-after GTK ownership test identified one
specific application-owned cycle:

1. a pane context popover owns its descendant rename and move controls;
2. the rename button's signal closure strongly owned that same parent popover;
3. the move control and manually parented submenu also used strong parent and
   submenu references in descendant signal closures;
4. sidebar reconciliation replaced the context popover, but the detached
   widget tree retained itself through those closures;
5. repeated full sidebar reconciliation therefore accumulated detached GTK
   trees in the main heap.

Before repair, a weak reference to a detached context popover still upgraded
after the owner replaced it. After repair, all descendant-to-ancestor
references are `glib::WeakRef`, and the move control is a supported
`GtkMenuButton` using `set_popover` rather than a `GtkButton` with a manually
parented popover. The controlled test performs 128 real replacements, detaches
the final popover, drains the GTK main context, and proves that all 128 weak
references are dead. Converting the move control also removed GTK's
`Finalizing GtkButton but still has GtkPopover child` warning.

## Repair

### Canonical event reduction

`AgentStatusStore` and `WorkspaceState` now return whether an accepted event
made a semantic state or lifecycle change. Identical idempotent events preserve
the original status timestamp and lifecycle deadlines. Duplicate
`session.end`, task snapshots, task identities, phase events, and metadata no
longer manufacture changes. The explicitly cumulative `task.delta` event
remains non-idempotent by design.

The GTK event coordinator now performs pane-context projection, project
refresh, transcript enrichment scheduling, persistence, fleet refresh, and
sidebar metadata refresh only for changed events. Multiple changed events in
one runtime drain produce one metadata refresh, and project pane IDs are
deduplicated within the drain. Unchanged-event diagnostics are aggregated and
rate-limited to one record per five-second reporting window.

### Terminal-title frames

Each real Ghostty surface now owns a `TerminalTitleEventGate` before the title
log and GTK idle boundary. The gate canonicalizes Codex Braille spinner frames
to the stable semantic title. Frame-only changes therefore produce no log, no
idle callback, and no second animation reconciliation. Meaningful task counts,
titles, and ordinary shell-title changes still cross the boundary exactly
once. `CodexTitleAnimation` also retains a canonical frame-zero template so a
source-generated spinner frame cannot restart TornadoTTY's own animation.

### GTK widget ownership

Pane context menus use weak child-to-popover references, and nested move menus
use GTK's managed `MenuButton` ownership. Full sidebar replacement is no
longer required for agent status changes; the coordinator uses the existing
metadata-only refresh path.

## Repair constraints

- Canonical event reduction must report whether observable state changed.
- Identical idempotent events must not update timestamps, request project
  probes, rebuild the sidebar/fleet, persist state, or append history.
- Non-idempotent protocol events such as `task.delta` must retain their defined
  semantics; they cannot be globally deduplicated by payload.
- Spinner-frame-only title changes must not be treated as semantic title
  changes, and TornadoTTY must have only one animation owner for projected
  sidebar/window titles.
- Legitimate phase, interaction, task, working-directory, and title changes
  must propagate exactly once.
- No sleeps or timing-only assertions may substitute for state/effect
  assertions.
- Focused unit and integration tests run before any broader qualification.
- Do not restart or replace the user's installed client without explicit
  coordination.

## Test-first sequence

1. Core reducer test: a second identical `agent.running` event returns an
   unchanged outcome and preserves the first event's timestamp.
2. Core reducer test: meaningful running metadata changes return changed and
   update the projection once.
3. Core reducer tests for idempotent snapshots/task identities, without
   changing `task.delta` semantics.
4. Linux title-animation test: different Braille frames with otherwise
   identical text reconcile as one semantic title.
5. Coordinator effect test: unchanged events request no project/sidebar/fleet
   work; a real transition requests only the affected work.
6. Controlled real-IPC burst test and bounded memory/queue soak receipt.

## Current status

### Focused tests

- `zentty-core` agent-status, workspace-state, and app-config integration
  binaries: 143 passed, 0 failed.
- Linux title-gate and event-coalescing unit tests: 9 passed, 0 failed.
- Real Unix transport tests: 6 passed, 0 failed. These cover fresh client
  reconnection, canonical routing, token revocation, wrong tokens, malformed
  and oversized frames, socket ownership, and a stalled client.
- Controlled GTK ownership test: 1 passed, 0 failed under private Xvfb and
  llvmpipe. All 128 detached context menus were released.
- Bash syntax and ShellCheck: passed for both changed journey scripts.
- Focused Clippy: changed core and GTK code passed after allowing only five
  pre-existing lints in unrelated files. The changed event logger initially
  exposed one `uninlined_format_args` warning and was repaired before rerun.

An intentionally broader `zentty-agent-ipc` package run found three unrelated
launch tests that inherit the active dogfood pane's `ZENTTY_AGENT_TOOL=codex`
environment while claiming to model execution outside a managed pane. The
directly affected `unix_transport` binary is green. The ambient-environment
test defect is recorded rather than being relabeled as a product result or
silently ignored; no pane token value was printed. GH-165 tracks making that
launch harness hermetic inside managed panes.

### Mutation strength

The first reducer mutation run established a real green baseline only after it
found a stale pre-rebrand configuration-error expectation. That test now uses
the canonical product name and still proves that invalid input cannot expose a
secret. The first substantive reducer run caught 27 of 35 mutations and left
eight redundant-condition survivors. The implementation and behavioral
assertions were simplified rather than waiving those survivors. The rerun
killed 23 of 23 reducer mutants. A separate title-animation/gate run killed 6
of 6 mutants. The changed autonomy-critical logic therefore killed 29 of 29
final mutants. Mutation runs used the repository's governed wrapper with
`gitignore = true`, `copy_target = false`, and its existing memory/process
limits.

### Real-system receipts

The existing `linux/tests/rust-agent-ipc` journey gained an
`event-coalescing` scenario; no parallel harness was introduced. It launches
the real ReleaseSafe application, GTK, Ghostty surface, PTY child, CLI helper
processes, and authenticated Unix socket. It uses explicit producer and GTK
presentation receipts rather than sleep-as-success.

The normal 500-event/500-title-frame run passed with 304 KiB anonymous RSS
growth, 320 KiB PSS growth, zero main-heap growth, zero descriptor growth, a
four-millisecond GTK drain, zero duplicate state changes, and exactly one
meaningful task/UI/persistence transition.

The bounded soak sent 5,000 duplicate authenticated events and 5,000 changing
Braille terminal-title frames over 26.903 seconds. It passed with:

- 724 KiB anonymous RSS growth;
- 693 KiB total PSS growth;
- 1,040 KiB main-heap PSS growth;
- zero descriptor growth;
- minus one thread and zero child-process growth;
- zero duplicate state changes, project refreshes, full sidebar rebuilds,
  attention changes, desktop notifications, semantic title events, or
  animation restarts;
- exactly one meaningful fleet/status/persistence transition;
- a four-millisecond GTK response after the producer's completion;
- six aggregate unchanged-event log records against a duration-derived maximum
  of seven.

The first soak invocation failed because the journey assumed the short run
could emit at most two aggregate diagnostics. A 26-second producer correctly
crosses multiple five-second reporting windows and emitted six. The assertion
was repaired to enforce the documented reporting cadence plus one boundary
allowance; no product behavior or resource threshold was weakened. The rerun
passed. The private display became unreachable, its run root was removed, and
no controlled process remained.

The memory ceiling is 16 MiB for RSS, PSS, anonymous PSS, and main-heap PSS.
That is deliberately conservative relative to the repeated observed baseline
of approximately 0.3--1.0 MiB growth while still failing growth remotely close
to the installed process's multi-gigabyte defect.

### Remaining uncertainty and deployment

No repaired product has been installed or launched during this work. The
09:17 incident occurred in the older installed build. The focused evidence is
green, but GH-161's field acceptance remains open until the repaired installed
application survives real dogfooding without renewed monotonic heap growth or
event storms. Raw transport channel bounding and pane process/cgroup isolation
remain explicitly owned by GH-163 and GH-162 respectively; they were not
smuggled into this repair.

## Desktop-notification routing bound

### Discovery

The desktop-notification adapter still retained two unbounded parallel maps:
one from freedesktop notification IDs to pane targets and another from those
IDs to activation tokens. Entries disappeared only if the desktop notification
service eventually emitted `ActionInvoked` or `NotificationClosed`. A server
that omitted those callbacks, or a long-lived event storm, could therefore
grow application-owned state without limit. Reuse of a service ID also risked
combining a replacement target with an earlier activation token. The D-Bus
signal callback additionally fed an unbounded Rust channel on GTK's process
boundary.

The installed package was inspected before this repair and is still
`tornadotty 0.1.1+gite064dd1151e8`. That source predates the original
event-coalescing repair at `c9e5c9d63980e2085a1882cb47f2401fd163585a` as
well as this notification repair. It was not replaced or restarted while the
user was working.

### Repair

The parallel maps are replaced by one owning notification record and an
insertion-ordered registry. Reusing an ID replaces the complete old record,
including its token. The registry retains at most 128 notification records and
evicts the oldest complete record on overflow. This is greater than the UI's
50-item attention-history bound while still providing a hard process-memory
limit when a desktop service never closes notifications.

The D-Bus callback now uses nonblocking `try_send` into a 256-entry bounded
channel. A full queue drops the new signal rather than blocking the D-Bus/GTK
callback or allocating indefinitely. Cumulative eviction and drop diagnostics
are emitted at powers of two, preserving evidence without recreating a log
storm. Disconnection remains a normal shutdown condition rather than a drop.

Tests were written against the missing registry API before implementation; the
expected compile failure established the red state. Seven focused unit tests
now prove exact-token single use, stale/invalid signal rejection, oldest-record
eviction, complete ID replacement, and bounded nonblocking ingress. The
reused-ID test specifically proves that a token from the previous record cannot
authorize the replacement target.

### Focused evidence

- `cargo test -p zentty-linux notification_service::tests --no-fail-fast`:
  **7 passed, 0 failed**.
- A direct strict Clippy run exposed five already-present lint classes in
  unrelated GTK files (`assigning_clones`, `too_many_lines`, `map_unwrap_or`,
  `cast_precision_loss`, and `unused_self`). Repeating strict Clippy while
  allowing exactly those five existing classes passed the changed binary. No
  new lint was waived in the changed module.
- The isolated ReleaseSafe product build completed successfully in 22.60
  seconds with dependency-age and notice checks enabled. It was written under
  `build/linux-profiles/release-safe`; the installed product was untouched.
- The first real notification/settings journey attempt failed before product
  startup because sandbox isolation prevented Xvfb from acquiring its display
  socket. Environmental absence was not converted into a pass. The corrected
  run used the existing `nested-x11` harness outside that sandbox, a private
  D-Bus session, Xvfb, llvmpipe, the real ReleaseSafe TornadoTTY binary,
  Ghostty, GTK, and the real desktop-notification/settings path. It exited 0
  after eight seconds. Session
  `fc7e808daf5d0d6d944636d8fdf6843da984e6c6168b4fee824d20fdd09ecbb0`
  reports ambient display/D-Bus sanitization, removed run root, unreachable
  display after exit, and no remaining controlled processes. Environment
  receipt SHA-256:
  `3f013615e9ce0c2d5de3595ddf6e8370fe65815514f797fa2896979f13be1cad`.
- The governed mutation wrapper selected only
  `DesktopNotificationRegistry`, `enqueue_desktop_signal`, and
  `apply_desktop_signal`. Four workers ran in a dedicated systemd scope with a
  12 GiB aggregate memory cap, 6 GiB per-process virtual-memory cap,
  `gitignore = true`, and `copy_target = false`. Of 28 generated mutants, **26
  were caught, 2 were compiler-unviable, 0 survived, and 0 timed out** in about
  two minutes. Machine outcome SHA-256:
  `7ec42ea2e1af3253b610a1b00e077fdf4f1013f111994cc1a39efcb3de29f636`.

This closes an additional application-owned unbounded collection and queue;
it does not claim the installed client is repaired. Installed field validation
still waits for the user's coordinated restart. Broader PTY/IPC backpressure
and process-group isolation remain GH-163 and GH-162 work respectively.

## Codex pre-policy permission notification routing

### Field failure and corrected source contract

At approximately 17:20 CEST the installed client notified that Bro needed
input even though Codex continued working. The authenticated event sequence
showed a Codex `PermissionRequest`, one desktop delivery, correct click routing,
and a later `agent.running` transition. Jason identified the missing semantic
distinction: **Approve for me** consumes the permission request, whether its
reviewer approves or denies it, so that request never becomes a human prompt.
When automatic review is off and the request actually reaches the approval UI,
notification is appropriate.

Two early diagnoses were incorrect and were discarded. The hook payload does
contain `permission_mode`; the missing fact is the later
`approvals_reviewer`/routing outcome. A proposed terminal-title inference was
also rejected as an unmaintainable text-parsing dependency. Neither mistake is
part of the repair.

Local Codex source establishes a cleaner boundary. Codex's TUI has a typed
`approval-requested` notification kind and emits it from its approval handlers
only after routing reaches a human. Managed TornadoTTY launches now configure:

- `tui.notification_method=osc9`;
- `tui.notifications=["approval-requested"]`;
- `tui.notification_condition="always"`.

Ghostty already owns OSC parsing and delivers a desktop-notification callback
to TornadoTTY. TornadoTTY does not parse escape sequences, match notification
text, inspect terminal titles, scrape transcripts, read Codex configuration,
or use a timer. Because the managed producer channel is restricted at launch
to one typed Codex notification kind, any nonempty parsed callback for an
existing Codex session is projected as `NeedsInput`/`Approval`. TornadoTTY's
existing pane/window visibility policy remains responsible for deciding
whether that actionable state warrants a desktop notification.

The original ZenTTY adapter has the same underlying defect: its Swift Codex
adapter maps every `PermissionRequest` directly to `needsInput`. This repair is
therefore a Linux-side correction to shared source behavior, not evidence that
the upstream implementation had already solved automatic-review routing.

### Tests-first repair and focused evidence

Tests were changed before the product implementation. The red receipts proved
three independently missing contracts: the Codex launcher lacked the typed
notification filter, the status reducer ignored parsed Codex notifications,
and the adapter/inbox route could not distinguish the pre-policy hook from the
post-policy notification. Those failures are retained at:

- `/tmp/gh172-launch-red.log`;
- `/tmp/gh172-status-red.log`;
- `/tmp/gh172-adapter-red.log`.

The final focused evidence is green:

- three Codex launcher tests passed;
- seven Codex adapter tests passed;
- the exact Codex reducer test and both Gemini notification regression tests
  passed;
- six Codex real-helper/Unix-socket tests passed outside the sandbox;
- Rust formatting and ShellCheck passed for every changed Rust and journey
  file;
- a product-only ReleaseSafe build passed dependency-age and notice checks and
  rebuilt `build/linux/bin/zentty-linux` from the final source;
- the existing `rust-agent-ipc` harness, under private nested X11 session
  `d252c30bd0089a4db4323e299e8cacf6d47373afa84cd3304e6e906f35ba7541`,
  exercised the real wrapper, helper process, authenticated Unix socket, PTY,
  Ghostty OSC parser, GTK shell, status store, and attention projection. It
  proved `pre-policy-hook=non-attention` followed by
  `parsed-osc9-approval=attention`.

The controlled OSC payload says only `Run cargo test?`; it deliberately
contains no `approval` or `action required` phrase. The passing journey
therefore guards against reintroducing a regex or other wording classifier.
The existing orchestration layer was extended with one selectable scenario;
no second agent journey or sleep-as-success path was added.

The first combined mutation run evaluated 29 mutants: 20 were caught, one was
compiler-unviable, and eight survived. Six survivors exposed real weakness in
the notification reducer assertions. Independent idempotence, single-field
interaction, and Gemini idle-retention contracts were added rather than
waiving them. The focused rerun caught all 19 notification-reducer mutants.
After Clippy-driven function extraction, a final audit of the exact committed
Codex hook and notification function boundaries evaluated 40 mutants: **37
were caught, one was compiler-unviable, and no relevant mutant survived**.
Cargo-mutants continued to include two unrelated `seed_restored_starting`
mutants despite both its include filter and explicit exclude; those pre-existing
survivors are recorded as filter leakage, not claimed as part of this repair.
Every run used `linux/tests/mutate-rust`, four workers, the isolated 12 GiB
cgroup, a 6 GiB per-process address-space limit, `gitignore = true`, and
`copy_target = false`.

### Remaining limits and deployment

The controlled actor does not invoke Codex's LLM reviewer; it deterministically
proves that the real pre-policy hook is neutral and that only the separately
parsed semantic notification becomes attention. The OSC channel is not a
security boundary: another child in the same managed Codex PTY could emit OSC
9. It is appropriate for UI attention projection, never for granting or
denying permission. A future Codex change to its documented notification kind
or configuration keys would require the launcher contract to be updated.

The first installed field check was invalid because the client had restarted
before, not after, the 19:42 deployment. After the current GUI and public CLI
were installed, the restarted GUI (PID 53226) still received a false
`agent.needs-input` event at 19:54. This was initially treated as a failed
semantic-routing repair, but the absence of a corresponding
`terminal-notification` record disproved the OSC hypothesis.

The installed agent wrapper resolved a third executable copy at
`/usr/lib/tornadotty/libexec/zentty/agent-wrappers/shared/zentty`. That copy
still had the old `c703e5cd...` ELF build ID and therefore continued mapping
raw pre-policy `PermissionRequest` hooks to attention. The public CLI already
had corrected build ID `267186d1...`. At 19:59 the wrapper-owned copy was
replaced from the same corrected build; both installed CLI paths then had
SHA-256
`a4fbfc3cac880de0c1d756336ce127bc3aa21110e6e5cead43c0d2eb93fcf0ce`.
Because each hook starts this helper by pathname, no GUI or Codex restart was
required for that correction.

A controlled `PermissionRequest` was then sent through the exact
wrapper-owned CLI, live pane credential, and authenticated product socket.
The helper succeeded and the product journal contained no needs-input event,
attention item, or desktop delivery after the probe. This establishes the
correct installed negative route. GH-172 remains open until a naturally
occurring Approve-for-me request is silent and one request genuinely routed to
the user still notifies during dogfooding. No full Linux qualification run was
performed for this issue-sized fix.

### Single Linux CLI authority repair

The deployment failure exposed a packaging design defect rather than merely a
missed copy step. `build-local` had installed the same Rust CLI at both
`bin/zentty` and `libexec/zentty/agent-wrappers/shared/zentty`, and the shared
agent wrapper deliberately preferred the private copy over the
`ZENTTY_CLI_BIN` path exported by the application. That made the private copy
an independent, silently stale implementation authority.

Linux now stages and packages one CLI executable. Installed packages expose it
as `/usr/lib/tornadotty/bin/tornadotty-cli` (with the public `/usr/bin` entry
point remaining a symlink). The application exports that canonical path to
each pane. Agent wrappers first use the explicit export and otherwise resolve
`tornadotty-cli` from `PATH`; they neither stage nor search for a wrapper-owned
CLI. The source macOS app-bundle placement remains separate and unchanged.

Tests were red before the repair: a wrapper fixture selected a deliberately
stale private CLI despite a valid explicit canonical path, and the rebuilt
stage contained the private executable. Focused green evidence now proves:

- `agent-wrapper-cli-authority-test` selects the explicit canonical CLI even
  when a stale private candidate is present, preserves arguments exactly, and
  falls back to the public `tornadotty-cli` command when no export is present;
- `staged-bundle` rejects any wrapper-owned CLI and invokes the authority test;
- `debian-package-audit` rejects any package that reintroduces the private
  executable;
- a product-only ReleaseSafe rebuild contains no
  `libexec/zentty/agent-wrappers/shared/zentty` path.

The already-running Codex processes inherited the obsolete absolute pathname
before this repair and cannot have their environment rewritten. The live
installation therefore uses a temporary compatibility symlink at that old
pathname, pointing to the canonical CLI, until those sessions are restarted.
This is one executable rather than two independently deployable copies; new
builds and packages do not contain the compatibility path. No broad
qualification run was performed for this packaging-focused correction.

## Attention arriving in an already-focused pane

### Field discovery

During live dogfooding, a normal Codex turn completed while its pane was
already being viewed. Codex was waiting for the next ordinary user prompt,
not presenting a permission request, but the attention marker did not clear
after the pane remained focused. The authenticated event history contained an
`agent.idle` transition, and later sidebar projections for the affected live
panes reported `phase=Running`, `interaction=None`, and `attention=false`.
This ruled out a current actionable approval and exposed a separate inbox
acknowledgement defect.

`AttentionInbox::observe_with_context` remembered that the pane was already
active and correctly prohibited a desktop notification. It nevertheless
committed the new item with `resolved_at_ms=None`. Its only automatic
resolution path required a later inactive-to-active transition. A user who
continued looking at the pane could therefore retain an unresolved badge
indefinitely. An existing test explicitly expected that incorrect result.

### Repair and focused evidence

Pending agent attention now carries an explicit acknowledgement timestamp when
it arrives in an actively viewed pane. Commit preserves the item as resolved
history while leaving desktop-delivery policy independent. This applies to an
immediate Ready/Idle item and to a debounced NeedsInput item that was already
visible when first observed. Attention first observed outside the active pane
remains unresolved until the user focuses it.

Two new assertions failed before the implementation change: both the immediate
completion and deterministic debounced-attention fixtures were committed as
unresolved. After the repair:

- all 15 focused attention-inbox tests pass;
- the complete `zentty-core` suite passes;
- both generated mutants for `AttentionInbox::commit` are caught by the
  resource-isolated mutation runner in 21 seconds;
- Rustfmt for the two changed files and `git diff --check` pass.

No sleep, retry, text inference, terminal-title inference, or new test harness
was added. The currently running installed client does not contain this repair;
activation requires a later coordinated client restart. Switching away from
and back to the affected pane remains the workaround for that running build.
