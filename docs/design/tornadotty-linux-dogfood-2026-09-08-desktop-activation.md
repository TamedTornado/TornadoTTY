# GH-184: dock activation instead of duplicate workspace restoration

Issue [#184](https://github.com/TamedTornado/TornadoTTY/issues/184), scoped
under the broader GApplication ownership issue #171.

## Report and reproduced failure

Jason's dock icon on another monitor opened another instance instead of
presenting the existing application. GNOME dash-to-dock monitor and workspace
isolation were both false; changing those global preferences was not justified.
Installed and source desktop IDs matched. Live X11 window properties were not
available during the initial read-only inspection.

The existing private-session product-smoke journey now has an `activation`
scenario. With the previous build, a second ordinary launch remained alive and
created another workspace instead of forwarding activation and exiting. The
test failed at that assertion (bounded eight-second secondary process timeout).
Neither launch used Jason's real session bus, state, display, or configuration.

## Ownership repair

- Ghostty still initializes its native runtime before GTK, as required by the
  embedding contract. Its private runtime application is not repurposed into
  TornadoTTY's desktop controller, avoiding Ghostty's own activation handlers.
- A focused `DesktopActivation` module owns a plain GTK application with the
  public TornadoTTY ID. It becomes the default application and registers before
  persistence/restore or agent IPC starts. Every host window is associated with
  it, including windows added through the existing coordinator.
- Normal launches use GApplication uniqueness and forward standard activation
  to the owner. The secondary exits without constructing panes or saving a
  workspace. The owner presents its currently active host window.
- Activations arriving during initial construction coalesce into one pending
  request consumed when the coordinator is ready; no retry timer or restore
  replay is added. GTK owns activation platform data handling.
- Explicit `--command`, `--state-directory`, or `--no-session-restore` launches
  retain their independent semantics. Normal launch without a usable session
  bus fails before workspace construction instead of silently restoring a
  second uncoordinated workspace.

This fixes desktop ownership and activation, not all terminal-notification
delivery requirements in #171. That parent remains open. No .desktop identity
change, custom IPC forwarding protocol, additional instance registry, or
Ghostty engine change is introduced.

## Focused checks and discoveries

Integrated ReleaseSafe build passed. The final activation checks passed on
private X11 and private Wayland: ordinary second launch forwards/exits; the
primary still has exactly one window and pane; an explicit isolated command
executes and exits independently; an unavailable bus fails promptly without
workspace creation; physical New Window creates the second window in the same
primary; both windows close normally and the primary exits successfully.

The first post-fix actor reached all activation/New Window assertions but failed
in teardown because it referenced a nonexistent wait-helper name and attempted
an unbound Ctrl+Shift+Q. Corrected the actor to use the existing owned-process
wait and the product's actual Close Window shortcut, twice. Assertions for the
reported defect were not weakened. X11 `_GTK_APPLICATION_ID` readback passed
with the public TornadoTTY ID. The previous opener regression also passed on
both X11 and Wayland against this build. The activation scenarios are scheduled
in the existing release matrix; no aggregate qualification was run.

Shell syntax, the sourced journey's ShellCheck, matrix schema/coverage
validation, and `git diff --check` passed. Workspace-wide `cargo fmt --check`
still reports pre-existing formatting differences in unrelated code; the new
module passed formatting review and its declaration was placed in sorted order.
No repository-wide formatting rewrite was performed.

```sh
ZENTTY_BUILD_OUTPUT_DIR=build/gh184 linux/scripts/build-local
ZENTTY_LINUX_BINARY="$PWD/build/gh184/bin/zentty-linux" \
  ZENTTY_RUST_SCENARIO=activation \
  linux/tests/nested-x11 linux/tests/rust-product-smoke
ZENTTY_LINUX_BINARY="$PWD/build/gh184/bin/zentty-linux" \
  ZENTTY_RUST_SCENARIO=activation ZENTTY_RUST_PHYSICAL_INPUT=true \
  linux/tests/nested-wayland-input linux/tests/rust-product-smoke
```

Jason reported sluggishness during the work. A bounded read-only check found
the live process still mapped to the old, replaced executable, about 842 MiB
RSS, no empty opener warnings in its last 120 records, and about 53 GiB system
memory available. The concurrent compiler used about three cores and 1.7 GiB;
this does not establish the live slowdown's cause. Jason cannot restart soon
and explicitly asked development to continue. No installation, live restart,
or full qualification was performed. Real second-monitor dock QA remains for
his next convenient restart; these results are isolated automated tests.

## GH-185: slow shell startup investigation

Jason subsequently reported roughly ten seconds to start a pane in Regulate
and laggy input. Live logs show the split at 12:09:07, surface initialized at
12:09:08, and the first prompt signal only at 12:09:15. The initialized receipt
does not establish a usable prompt. No live restart or deployment was performed.

Focused `ZENTTY_RUST_SCENARIO=shell-startup` uses the existing private product
journey, real interactive Bash, staged hooks/CLI and GUI IPC. It measures the
first prompt-hook completion and physical input execution, and verifies the
GUI actually received PID/prompt signals. Initial clean-instance checks passed:
GH-184 X11 1.074s startup / 0.073s input, GH-183 X11 1.607s / 0.133s,
GH-184 Wayland 2.271s / 0.125s. **These did not reproduce the full live delay.**
Three read-only live CLI queries returned in 10–14ms; a signaling-disabled
installed hook scan took 30–39ms. Neither supports a constant one-second IPC
delay. The live process had no memory pressure or swapping during inspection;
high I/O pressure with little NVMe activity remains unexplained. A brief CPU
sample was inconclusive, not evidence that rendering is correct.

A process-only trace of the isolated journey reproduced the same erroneous
startup sequence as the live log: Bash DEBUG treated PROMPT_COMMAND assignment,
wrapper setup, key binding setup, and the prompt hook itself as user commands.
Each triggered another wrapper scan and synchronous GUI notification. The trace
completed product assertions but its outer wrapper timed out; this diagnostic
run is not counted as a passing journey.

Added an assertion rejecting running-command events before the first prompt:
it failed against the old staged hook at 1.621s. The repair arms DEBUG after
initialization and skips the prompt-hook function entry before its guard is set.
It does not disable genuine preexec reporting or abandon replies. The X11
journey then passed at 1.324s / 0.131s. Staged Bash compatibility checks passed.
Only the shell resource in the private GH-184 stage was refreshed, not installed
resources, and no Rust rebuild or full qualification was run.

The subsequent Wayland startup check passed its initialization semantics at
1.794s but **failed physical input**: the logged command path contained an extra
`3;5u` sequence. This is an input-corruption failure, not proof of a three-second
IPC delay. Preserve it as outstanding under #185; do not retry it green or
claim the full sluggishness issue is fixed. The exact live seven-second delay
and laggy input remain open even though the redundant startup events are fixed.

## GH-185: worklane switching and server-menu ownership repair

Jason corrected the investigation: seven server entries matter because of
switch-time GUI setup/teardown, not simply their subprocess CPU usage.
Two defects were reproduced:

- Every ordinary worklane render detached terminal column widgets. GTK
  unrealize/realize consequently destroyed/recreated Ghostty renderer resources.
  The new real two-worklane/PTY journey failed on the old build when the first
  switch initialized OpenGL again.
- Server-menu buttons strongly captured their owning popover. Replacing the
  menu leaked that GTK object graph; unchanged server scans also rebuilt it.
  A real GTK seven-entry ownership test failed because the replaced menu stayed
  alive. Open With and Arrange had the same callback ownership cycle.

Ordinary navigation now retains materialized column widgets, hiding inactive
layouts without unparenting terminals. Topology/model-geometry changes invalidate
only affected layouts; viewport changes resize existing widgets and dividers.
Peek's deliberate reparenting explicitly invalidates the inactive layouts.
Server menus reuse unchanged visible entries (including return from a lane with
no servers), and the three menu callback families use weak parent references.
No server process is restarted or stopped by these changes.

Focused results against `build/gh185`:

- PASS: real GTK menu replacement/finalization, unchanged seven-entry reuse,
  switch-away/return reuse, and Open With/Arrange finalization.
- PASS: X11 and native Wayland four alternating lane switches, physical input
  delivered to the original PTYs, no additional renderer initialization or
  terminal creation, followed by hold-to-Peek/release and usable terminal input.
- PASS: existing X11 development-server journey, including actual listeners,
  authenticated discovery, browser argv, ignore, safe stop and restart.
- PASS: existing X11 pane-drag-only journey with real GTK DnD, exact topology,
  and preserved live PTY.
- PASS: Wayland real Bash startup, 1.263s hook-to-prompt / 0.064s Enter-to-command.
  This does not erase the earlier intermittent `3;5u` input failure.
- PASS: matrix schema/coverage, shell syntax and diff whitespace checks.
  ShellCheck through the parent script reports only SC2317 on its EXIT-trap
  cleanup; standalone checking of the sourced helper lacks parent variables.

Failed intermediate checks were not waived: initially keying layouts on viewport
size recreated them after allocation, so viewport resizing was moved in-place.
The Wayland Peek actor initially destroyed its virtual keyboard before holding
and then omitted a physical Control release; it now performs the entire chord
in one keyboard lifetime with physical press/release events. The server journey
sent XSendEvent Escape to the parent, bypassing GTK's popover grab; it now uses
its existing physical-key helper. The drag journey expected an obsolete
`terminal-strip` source receipt (also failed against GH-184); its expectation is
now the actual `pane-controls` source, with topology/PTY assertions unchanged.

The full source-UX journey still fails its old `empty-dark-x11` visual baseline
before interactions: that baseline has the previous title strip/sidebar labels.
It was not rebased or counted as passing. No full qualification was run. The
new regressions are wired into the existing matrix, not a new runner.

The integrated product is built in `build/gh185`, not installed. Jason's live
client was left alone. GH-185 remains open for the previously recorded input
corruption and confirmation of long-running responsiveness after a coordinated
restart; the isolated fixes do not establish that all live lag is eliminated.

### Deployment regression: default application and lost recovery snapshot

After the requested atomic installation, Jason force-quit the unresponsive old
client. New launches at about 13:49–13:50 aborted immediately. A separate-state
GDB launch reproduced SIGABRT in Ghostty's IO thread:
`postFork -> Application.default -> unwrapNull`. GH-184's host registration
called `set_default`, replacing GhosttyApplication with plain GtkApplication;
the cgroup post-fork path requires the former. This was our integration bug.

Removed that global replacement and explicitly passed the desktop application
through the coordinator to associate every host window. Merely removing the
replacement first failed the existing window-application identity assertion;
explicit ownership fixes both activation and cgroup startup. The existing
activation journey now enables linux-cgroup=always (non-hard-fail on its private
bus): the deployed build aborted, repaired X11 and Wayland journeys pass,
including forwarding and explicit new windows. A separate-state launch on the
actual GNOME desktop with its real configuration also exited normally after
running `sleep 1`. No saved user state was used for those launches.

The first failed launch projected three agent lanes, then complete_launch queued
deletion of the recovery snapshot before PTY startup. The second launch therefore
had no snapshot. Removed that deletion: retain the durable snapshot until an
atomic newer save replaces it. Updated storage assertions to require survival
and crash recovery of the identical envelope; all 23 coordinator tests pass.
This deliberately corrects the previous destructive requirement, rather than
waiving a failure. No full qualification was run.

The repaired GUI was atomically installed and byte-verified. The previously
installed CLI and Bash hook remain current. Original user snapshot recovery is
NOT established: its file is absent. The journal still identifies Bro/pane-6,
ZenTTY/pane-14, and Regulate/pane-22 and their working directories, but that is
not a replacement for the lost complete restore envelope. No guessed workspace
was written. Original long-running hang remains unproven separately from this
confirmed startup regression.

### Sep 9: sidebar resize clips terminal edges; narrow divider target

Jason observed right-edge clipping after widening the sidebar and broken left
alignment at minimum width, plus a hard-to-grab divider. Repair single-column
allocation updates (previously conditional on multi-column model scaling),
including the first allocation. Single-column resize need not await the
multi-column sidebar-settling guard. Use GTK External horizontal policy rather
than Never: Never propagates retained child minimum width into its viewport.
Reset the single-column horizontal offset on resize. Native GtkPaned now uses
an eight-pixel separator hit area with a thin painted line.

Extended the existing X11 worklane-switch journey with physical divider drags
and real `stty size` replies from the PTY. The finalized test waits for initial
sidebar allocation before its baseline and locates the return drag using actual
sidebar width, rather than an assumed coordinate. Against the installed build
it fails: columns 71 -> 71 -> 81 (widening does not shrink). Repaired build
passes: 70 -> 66 -> 80. It then passes original worklane switching and Peek;
native Wayland switching/Peek also pass (sidebar pointer drag tested on X11).
Intermediate single-column-only fixes failed until the GTK viewport sizing
feedback was removed. No assertion was waived. ShellCheck and diff checks pass.
No full qualification or screenshot-baseline rewrite. Actual Codex visual QA
remains for Jason's restart; PTY reflow is tested, not a claim about every TUI.

Built in build/gh185 and atomically installed without restarting the active app.

### Follow-up: real left-edge overlap after resize

Jason's screenshot after restarting into 51bf7e73 still showed clipped left-edge
text. Confirmed running executable matched installed; this was not stale-client
QA. Column-count testing missed a separate geometry defect: sidebar is an
overlay above a reserved column, and GTK minimum sizing can make it exceed that
reservation. A previous Working spinner populates a non-ellipsizing prefix in
a homogeneous GtkStack; the hidden activity child still affects Ready sizing.

New real-GTK composed sidebar/terminal-viewport test reproduces after Working ->
Ready and narrowing to 180px: sidebar width 206px, terminal starts at x=188px,
so 18px of the terminal are covered. An initially stable title did not reproduce;
the test now exercises both active and previously active title states. The fix
ellipsizes the activity prefix and uses External horizontal policy for sidebar
scrolling so child minimum sizes cannot enlarge the overlay reservation.
No protocol/session data or terminal text interpretation changed.

PASS: allocated widget bounds never overlap through 420/180/280/180 widths,
including Working and Ready; same focused X11 physical-drag/real-PTY journey
(70 -> 66 -> 80 columns), lane switching and Peek. Added the geometry check to
the existing matrix. No full qualification. Rebuilt and atomically installed;
Jason's running client is not restarted. User visual confirmation remains open.

Jason confirmed wrapping/clipping improved after 412f7852, but the divider looked
like a thick white strip. Keep the native 8px hit area; paint it in the sidebar
background with a centered 1px muted line, explicitly clearing theme borders,
padding and shadows. This is a styling correction, not a geometry change.
Focused physical divider/PTY resize, lane switching and Peek pass after the
styling change. An isolated Yaru launch had no CSS parser errors, but attempted
Xvfb image captures were blank, so those are not visual confirmation. Installed
atomically; final appearance remains user QA, no full qualification run.

## GH-186 / GH-187: restored-agent exit and notification ownership

At 11:49:28, Bro's restored child exited, a new surface reused pane-1's ID,
then no replacement terminal-ready arrived. At 11:49:32 a pane-control
split-right action created pane-4; its trigger remains unconfirmed with Jason.
Do not describe the split as spontaneous model mutation. The blank pane is
reproduced by the existing completed-restore journey: shell PID never attaches.

Mounted layout keys now include actual frame identity as well as durable pane
IDs/geometry. Replacing a runtime under the same ID invalidates its old layout;
ordinary navigation keeps the same widgets. Existing completed/retried restore
checks now also type a command into the real replacement shell and assert its
filesystem result. Both X11 cases pass. Successful replacement exposed the typed
receipt reader's false duplicate-ready rule: it now permits Ready -> Exit ->
Ready for the same pane while still rejecting consecutive duplicate Ready or
Exit. Nine receipt contract tests pass, including repeated replacement cycles.
This is lifecycle correction, not removing event validation.

GH-187 is independent: Ghostty emits its desktop-notification signal to the
host then calls GIO with an intentionally unregistered engine application.
Engine commit 34c80b9d58ad4906311af4c9722801afb9a23e10 keeps the signal delivery,
then performs native delivery only for registered applications. No second
application registration/global-default replacement, no change to core OSC
policy/rate limiting. Standalone registered Ghostty retains its native branch.
Existing Gemini OSC journey, exposed as a focused scenario, reproduces the
critical on the predecessor and passes on X11 and Wayland after repair, with
host approval/ready state assertions retained. Test setup now clears inherited
ZENTTY_AGENT_TOOL (we run inside Codex), captures actor errors, and uses the
existing start-gate mechanism plus a completion gate so the second notification's
rate-limit interval begins after GTK consumes the first. Initially both queued
notifications arrived together and rate-limited; no product limiter was changed.
A discarded canonical-executable-alias hypothesis did not change shipped code.

Remaining failures are not concealed: native Wayland completed-restore now
mounts its shell but physical input acquired a stray `4;5u` sequence, like the
prior #185 input failure. A labwc comparison failed test input-window identity.
X11 switch regression failed to grab the minimum sidebar target; Wayland
switching/Peek passed with no repeated renderer initialization. These are NOT
full green qualification claims and remain under #185/#182; no broad retries
or full qualification were run. Matrix validates the focused cases using the
existing drivers. Separate unregistered cgroup-connection assertions seen in
live logs are not fixed by the notification-delivery guard.

Build staged in build/gh186; GUI, CLI and matching engine library installed
atomically (engine RUNPATH normalized as packaging does). Live client and Codex
accounts untouched. User logout QA pending next restart.

### GH-185: deterministic printable-key corruption repair

Extended the existing keyboard-compose journey with exact mixed-case alphabet,
digits and punctuation PTY text. Predecessor emits ESC[67;5u instead of C and
function-key sequences instead of punctuation. This reproduces the same class
as stray `4;5u` in the restored-shell command, without random filenames or
repeated retries. GTK remapKey did not recognize uppercase/shifted symbols and
fell back to physical Ctrl/function-key positions assigned by virtual keymaps;
eventMods then synthesized Ctrl. Engine 7e695b7a1e841086ed8a9ba14d73397fb37cf8ca
normalizes uppercase lookup and rejects non-writing-system physical fallback
for printable logical symbols. Real modifier mapping and writing-system physical
identity are preserved; focused Zig assertions added (not independently run).

PASS: extended real PTY compose/dead-key/mixed-text checks on X11 and Wayland;
X11 runtime US/German keymap switching/remapping/multi-pane focus; native Wayland
completed-restore now executes the physical replacement-shell command and exits
cleanly. Initial X11 extension switched keymaps before GTK consumed prior
compose events; actor now publishes their actual PTY receipt before the switch.
No assertion relaxed. The existing restored-exit Wayland path joins the matrix.

Intermittent live switch latency is NOT claimed fixed. Added slow-only (>=50ms)
per-stage render/chrome timings with lane ID and durations only: no terminal
text, timer, polling service or new framework. Future slow switches can identify
chrome/server/context/attention/layout costs without a short live trace window.
Build staged in build/gh185-input; installed atomically with matching engine,
GUI and CLI without restarting Jason's active client. No full qualification.
Wayland runtime keymap/remap/multi-pane focus check also PASS after building its
missing existing wayland-keycode-driver prerequisite; the first invocation was
blocked by that absent executable, not a passing run.

### GH-171: real terminal desktop delivery and activation

The GH-187 native-send guard removed an invalid API call but did not deliver
ordinary terminal notifications. The extended `rust-notifications-settings`
journey is red on dc3e2687: a real OSC777 crosses the embedded surface callback,
but its title/body never reaches the real notification daemon. The strengthened
`product-desktop-activation` journey is independently red on the remaining
unregistered-application cgroup lookup.

Ownership is now explicit: Ghostty retains its private process-default
GhosttyApplication; the host DesktopActivation owns desktop identity,
registration, activation and window association. The private engine application
is deliberately not registered as a competing desktop application. Ordinary
OSC delivery uses the host's existing notification proxy and bounded pane-action
registry, asynchronously with a two-second call deadline and bounded in-flight
requests. Managed agents still use canonical attention policy, not duplicate raw
OSC delivery. Stale surface callbacks are rejected and terminal title/body are
no longer copied into production logs. Engine commit
0f3d611e9d0402738dfd93dbe163c9c99d8ea1f2 acquires the shared GIO session bus for
cgroup scopes independently of application registration; hard/soft failure
policy and connection-reference ownership are preserved.

Initial focused X11 and native-Wayland delivery/click checks PASS: actual
notification-daemon receipt has the intended title/body once, physical click
on its real X11 popup selects the original of two product panes, and the CLI
confirms canonical focus. Wayland here means the real product uses Wayland;
Ubuntu's notification-daemon itself uses the wrapper-owned X11 display.
The missing-daemon extension PASS on Wayland without attempting another Notify.
The first click extension exposed a fixture bug: split children inherited the
same command and competed to emit the notification. A first-child ownership
claim now makes the originating PTY deterministic. A wrong expected credential
label was also corrected from `absent` to the existing `none` representation.

The first engine integration attempt was BLOCKED by the bundle validator:
using the sibling source with an absolute cache path embedded an absolute
layer-shell RUNPATH. No validator was relaxed. Rebuilding the committed engine
through the normal pinned managed checkout restores the supported bundle path.
Focused notification payload/visibility-policy mutation run: 7 generated,
5 caught, 2 unviable, no survivors. This is not blanket mutation coverage of
GTK registration or all asynchronous service paths. Full qualification not run.
Final pinned integration and installed/live status follow below.

Final pinned ReleaseSafe integration PASS on X11 and Wayland: terminal OSC
receipt once, real popup click, canonical origin-pane focus with two surfaces,
missing-daemon containment, primary/secondary desktop activation, two windows,
missing-bus startup rejection, clean normal activation shutdown, no application
assertion, and both Gemini OSC state transitions without generic duplication.
Existing X11 notification/settings journey PASS, including CLI delivery,
sound hints, import, persistence and explicit unavailable paths. Eight focused
notification unit tests PASS (mutation baseline). Matrix schema/coverage PASS.
Shell syntax PASS; standalone shellcheck reports pre-existing sourced-global
and source-following diagnostics, not claimed clean.

Broader `rust-attention-inbox` is FAIL on both this change and predecessor
build/gh185-input: it expects two unresolved approvals but gets two items with
only one unresolved after a viewed approval. Removing inherited agent-tool
environment did not change that result. This is not certified as a passing
agent activation integration run, nor repaired by changing its assertion;
tracked with existing lifecycle-versus-unread issue #175. The new terminal path
and unit token/action registry checks pass independently. GH-171 remains open
for outstanding acceptance audit and live desktop QA; no claim of blanket
registration/async mutation coverage or full release qualification.

Deployment receipt: TornadoTTY ed4c6e8cd807f34255d1ddd02d09596538d32a9e and
Ghostty 0f3d611e9d0402738dfd93dbe163c9c99d8ea1f2 are pushed. The clean committed
ReleaseSafe build repeats the X11 real terminal delivery/click/unavailable check
successfully. GUI, CLI and both matching shared libraries were installed by
atomic rename, with the engine RUNPATH normalized to `$ORIGIN` as packaging
requires; all four installed files byte-verified against the prepared artifacts.
`/usr/bin/tornadotty --version` reports ed4c6e8cd807. Existing GUI/agent processes
were not restarted or changed; live validation awaits Jason's next restart.

### September 10: Bro notification bursts and asymmetric wheel report

Installed ed4c6e8c / engine 0f3d611e9, native Wayland. GH-143 recurrence:
Bro pane-1 delivered Ready notifications at 10:11:18, 10:11:47 and 10:12:06 CEST
(service IDs 9–11); its actual Codex terminal notification at 10:12:06 then
produced ID 12 at 10:12:09. Confirmed remaining owner mismatch: the agent store
keeps Codex title/Idle presentation separate from goal-aware semantic OSC, but
the shared AttentionInbox still promoted every Codex Idle transition to Ready
attention. It bypassed the existing semantic-only hook policy.

Repair: Codex Idle remains presentation and is not an attention candidate;
Codex's semantic OSC still supplies attention. No debounce, timer or global
notification suppression added. Other agents' explicit Idle completion behavior
is unchanged. New store-to-inbox regression is RED for repeated Working/Ready
cycles before the repair, then GREEN with exactly one semantic notification and
no duplicate. Existing real codex-review-routing journey is independently RED
when the controlled PTY emits Ready without semantic OSC, then PASS on X11 and
Wayland after the repair. Core checks: 40 agent-status + 15 attention-inbox tests
PASS. Focused attention_candidate mutation: 6 caught, 4 unviable, no survivors.
Matrix schema and shell syntax PASS; full qualification not run.

GH-188: Jason reports ordinary new pane -> start Codex immediately gives wheel
cycling input history in the right Bro pane while left scrolls output. Both
observed processes used codex-cli 0.154.0, the same executable inode and terminal
identity. The right pane disappeared before live terminal mode inspection; no
live pane was closed/restarted by the investigation. Alternate-scroll cursor-key
translation matches the symptom, but its cause is UNCONFIRMED, not attributed
to a picker/transcript action Jason did not report.

Added an opt-in diagnostic to the existing rust-agent-ipc journey:
ZENTTY_AGENT_IPC_SCENARIO=codex-startup-scroll plus ZENTTY_REAL_CODEX_BINARY.
It launches the installed real CLI through managed wrappers in two private
panes, captures real PTY input/output via script, uses a credential-free local
provider without submitting a model prompt, and sends physical wheel input.
X11 and labwc-backed native Wayland controls PASS with no cursor-key injection;
the reported failure was NOT REPRODUCED. This does not qualify real-account
history scrolling or claim a repair. Early diagnostic attempts had an incorrect
missing-file poll condition and lacked isolated close-confirmation settings;
those fixture failures were corrected, not reported as product failures.
Private evidence directories are printed by the diagnostic; no user auth or
terminal contents were copied into it. GH-188 stays open.

Sidebar screenshot: the small `~` under the named Bro worklane is the focused
pane's home directory. Source and live CLI confirm focused-pane cwd drives that
subtitle; it is not a missing label. Mixed-directory worklanes make the meaning
misleading. No speculative sidebar rendering change was made.

### September 11 / GH-189: remove the system-tray feature

Jason explicitly requested removal, not another tray implementation. GNOME's
watcher listed one live process twice, through the dedicated SNI name and the
public desktop application name, with inert red warning icons. The old client's
supported config reload (`menu_bar.show_status_item=false`) withdrew the owned
SNI name and unexported /StatusNotifierItem without restarting any pane. GNOME
retained the public-name alias despite that object returning UnknownMethod.
The desktop application identity was not released and GNOME Shell/extensions
were not restarted to force cache cleanup; the cached alias may remain until
Jason's next coordinated client restart.

Deleted the complete tray module, publication/refresh/activation wiring, settings
switch and active config model/writer. Legacy menu_bar input is ignored by new
builds, including old true values; it cannot restore the feature. Retained the
in-window Agent Status route, Settings/Quit, sleep inhibition and regular desktop
notifications. Removed positive tray scenarios and current inventory claims;
kept the controlled watcher solely to prove absence of publication.

Focused regression RED on the previous build: its tray name still exists.
GREEN X11 and native Wayland: no SNI name, watcher registration or SNI object on
any product-owned D-Bus connection, while the real GTK in-window panel and Quit
work with legacy setting=true. Also PASS: 27 core config tests; config-store
transaction test; full X11 two-window fleet lifecycle/exact-PTY routing; X11
Agents settings; native Wayland primary/secondary activation, multiple windows
and missing-bus handling; real terminal notification receipt/click/unavailable
service journey on X11. Matrix schema and shell syntax PASS. No full qualification.

Initial fixture failures were obsolete Zentty-branded palette queries (updated
to the current action description), and inherited installed wrapper PATH in the
Agents settings test (rerun with an explicit real-Codex/system PATH). An early
compile caught over-removal of the shared in-window show_agent_fleet method;
restored it while removing only its tray caller. No failed assertion was relaxed.
Feature-inventory JSON parses; its broader audit is BLOCKED by the checkout's
missing audited public tag v0.1.7, not certified as a passing release audit.

### September 11 / GH-190: unexpected Wayland disconnect and GUI exit

At 09:56:59 CEST the old daily-dogfood GUI (PID 2796474, started September 10)
exited after GNOME logged `WL: error in client communication` and GDK logged
`Error flushing display: Broken pipe`. The user relaunched at 09:57:01; the new
GUI is the installed clean 9899e7d951ad build. The dying process predates the
tray-removal install. GNOME Shell stayed alive; its stack-position assertion
occurred after relaunch and is not established as causal. No contemporaneous
panic, segfault, OOM or kernel GPU-reset evidence was found. Existing apport
reports predate this incident and must not be treated as its crash trace.

Bounded private journal extracts and recovery snapshot copies were preserved
under /tmp/tornadotty-crash-20260911.LRBmlt; originals were not modified. Relaunch
accepted three agent restore drafts and retained the saved worklane topology;
this is not certification of every resumed agent's health. No live client was
launched, killed or restarted by the investigation.

The display disconnect is a confirmed exit mechanism, not a root cause. Neither
client nor compositor journal gives a protocol opcode or underlying reason.
GH-190 tracks reproduction, bounded diagnostics and a focused repair; no
speculative fix or passing regression is claimed. Separately, pre-exit timing
records repeatedly locate 1.38–1.47 seconds in ranked_servers/configure_servers,
with other chrome phases near zero (GH-185). Repeated engine page-capacity logs
occur both before and after relaunch. Neither observation is proven causal.

GH-190 logging follow-up (local, publication/install pending): added a hook to
Ghostty's existing GLib writer for GDK fatal display-I/O messages. Before GDK
exits it queries the default native Wayland display without dispatch/roundtrip
and records observed errno, wl_display_get_error, protocol code, object ID and
at most 96 bytes of static protocol-interface name. An atomic guard permits
one record per process. No terminal contents, protocol payloads, full
WAYLAND_DEBUG, network submission or new log files are introduced. The host
also logs PID and compiled build identity once on actual GUI startup, after
help/version early exits. Records use existing stderr/journal logging; a lost
transport without available protocol state still requires compositor evidence.

Extended the existing updates/privacy journey with display-disconnect. The
private nested compositor PID must actually own the selected private socket
before the fixture terminates it; the user's desktop is never selected by name.
RED on gh189-no-tray: no native failure record. GREEN on the local integrated
engine/host build: actual GTK/Ghostty terminal ready, compositor termination,
nonzero client exit and exactly one bounded record (errno=32, display-error=0,
protocol-code=0, object-id=0, interface=none), plus running build identity.
This reproduces controlled transport loss, NOT the original incident or an
EPROTO violation. GDK can fail flushing before libwayland stores a display
error; the fixture therefore checks the observed EPIPE/ECONNRESET errno as well
as display/protocol state rather than incorrectly requiring get_error != 0.

Also PASS: X11 real in-window fleet/tray-removal smoke, matrix schema/coverage,
shell syntax and Zig formatting. The CLI journey's help/version checks pass,
but its unknown-option assertion FAILS unchanged on both old and new products:
it expects TornadoTTY while the existing error prefix is zentty-linux. No
unrelated prefix change or relaxed assertion was made. Initial fixture mistakes
(relative socket name and nonexistent surface-ready marker) were corrected;
a Zig binding namespace compile error was corrected to wl.Interface. No full
qualification, protocol-violation test or original-crash repair is claimed.

Local evidence: /tmp/gh190-red.log, /tmp/gh190-green-abi.log,
/tmp/gh190-x11.log, /tmp/gh190-cli-baseline.log. Auto-review blocked committing
and pushing the native hook to the separate TamedTornado/ghostty fork even after
remote ownership verification. Engine edits and host tests remain local pending
explicit repository publication approval; linux/ghostty.lock is unchanged and
no installation or live-client restart has occurred.

Jason subsequently explicitly approved publication, pinning and installation.
Engine b62d1c57fe7d265ab030fd25c96742e1b11442b8 is pushed to his
TamedTornado/ghostty fork and pinned by this host change. Final clean pinned
build verification and atomic installation follow; the running client remains
untouched and will acquire these diagnostics only on its next launch.

Final receipt: clean host d194792702f6f5287ed5ced77fa5fcc47916ff97 and engine
b62d1c57fe7d265ab030fd25c96742e1b11442b8 are pushed. Integrated stage
build/gh190-diagnostics passes the native Wayland disconnect journey again
(/tmp/gh190-green-final.log) and X11 in-window fleet/tray-removal smoke
(/tmp/gh190-x11-final.log). GUI, CLI and matching libraries were atomically
installed; executable/library comparisons pass, with the engine compared after
normal packaging RUNPATH normalization to $ORIGIN. Installed --version reports
clean d194792702f6 and ldd resolves both bundled libraries. Live GUI PID 2570984
retained its executable inode and was not restarted. Logging activates on the
next user launch, not retroactively in that process. GH-190 remains open for
original-cause investigation; the diagnostic transport-loss test is not a fix.

### September 18 / GH-191: new pane bottom clipped behind the dock

Jason's screenshot reports bottom-of-terminal clipping, NOT a theme/contrast
defect (the initial investigation misread it). Switching worklanes away and
back clears it. Running host is clean d194792702f6, Ghostty b62d1c57fe7d.
Bounded sizing-only journal inspection shows a vertical split at 10:36:51
increasing the pane viewport from 1298 to 1312 pixels without a window resize;
the new right pane at 10:57:07 inherited that height. Switching away/back at
11:18 restored 1298. No live input, settings, terminals or processes changed.

Confirmed related GTK defect: the pane scroller used vertical PolicyType::Never,
which lets content minimum requests enlarge its allocation. The focused
pane_geometry_tests regression stacks framed panes, shrinks the available
window, then adds a full-height neighbor: RED, viewport/pane bottom 606 in a
600-pixel window. External keeps the vertical scrollbar hidden without letting
content dictate viewport size; the same test is GREEN. This corrects actual
allocation, not merely paint clipping or a dock-specific height subtraction.

The existing divider-layout Rust journey now drives below/right splits and
worklane return, obtains real PTY size reports and checks a colored last-row
band in compositor screenshots. Initial real-product controls pass on BOTH
the old and repaired builds; those controls alone do not reproduce or certify
the original GNOME incident. Final coverage includes zero terminal padding,
space-reserving horizontal scrollbars, full-width overflowing columns, and all
21 one-pixel height alignments within a terminal row. Those controls also pass
on the old build; only the focused GTK allocation test is a RED/GREEN repair
receipt. The exact original GNOME/dock incident remains NOT REPRODUCED.
First fixture run selected the wrong pane on return (Ctrl+Tab selects the
first pane); corrected to reverse navigation, not a relaxed geometry assertion.
The initial sandboxed Xvfb startup and Ghostty-source fetch were blocked;
private-display execution and the already verified clean pinned engine checkout
were used. No dependency policy was bypassed.

Final local stage: build/gh191-clipping (ReleaseSafe, unchanged pinned engine).
PASS: focused GTK regression; 15 allocation tests; 9 receipt-contract and 3
driver tests; driver Clippy; shell syntax; matrix schema/coverage. Real-product
X11 and native Wayland journeys both PASS last-row visibility, split/switch
geometry and all row-boundary resize checks (/tmp/gh191-final-x11.log and
/tmp/gh191-final-wayland.log). Strict host Clippy FAILS on unchanged workload
documentation/style errors; no unrelated repair or lint suppression added.
An operator invocation omitted matrix --validate-only, started two prerequisite
cells, and failed (network/suppression evidence); it is not a full qualification
receipt. The corrected validation-only command passes. No full qualification
completed, installation performed, or live client restarted. GH-191 stays open
for original-incident desktop verification; the confirmed allocation defect
is repaired locally.

Clean-commit rerun caught a fixture isolation error: X11's initial/new-neighbor
PTY height was 651, then 630 after async Git metadata expanded the inherited
checkout's header (pane viewport 656 -> 641). Native Wayland passed that run.
This was a legitimate available-height change, not evidence of clipped rows.
The layout supervisor now starts in its private non-project directory rather
than the caller's checkout; geometry equality and full-row assertions remain
unchanged. Corrected-fixture revalidation PASS on X11 (651 pixels before/new
neighbor/return) and native Wayland (672 throughout), including all last-row
checks; /tmp/gh191-isolated-{x11,wayland}.log. Driver Clippy, ShellCheck and
edited-test formatting PASS. Repair commit d51877e5ed6bd7c2e65a4587681b24eca1159faf
is local, unpushed and uninstalled; the source checkout is not a desktop test
fixture anymore.

Jason then authorized pushing and local installation for the next restart.
Repair d51877e5ed6bd7c2e65a4587681b24eca1159faf and fixture correction
248373e4f35dd5a5257b613bed5c847aa796f406 are pushed to the fork's linux/port.
Clean ReleaseSafe build/gh191-clipping is installed: GUI and CLI replaced by
same-filesystem atomic renames and byte-verified. Existing libraries match the
stage exactly after the normal engine `$ORIGIN` RUNPATH normalization, so they
were retained; themes, shell integrations and engine resources also match.
Installed GUI --version and CLI version both identify 248373e4f35d; ldd resolves
both bundled libraries. CLI --help is unsupported on both old and new builds;
the documented version command is the successful CLI verification.

Live clients 5627 and 2292325 retain their original executable inode 66307:75555043
and start times; neither was restarted or signaled. Installed executable inode
is now 66307:75497503. Root-private rollback executables are retained at
/tmp/tornadotty-gh191-rollback.CcxikO. The first push invocation misparsed its
refspec as a repository; explicit repository/refspec push succeeded and its
remote head was verified. The repair becomes active only on Jason's next
restart; GH-191 stays open for confirmation of the original desktop incident.

Subsequent user report, still on the OLD running build: Codex was producing a
large amount of text over a prolonged period; Jason scrolled upward into
scrollback while output continued, then observed the terminal bottom below
the desktop dock again. Jason explicitly confirmed he had NOT restarted into
the newly installed build. This is additional reproduction evidence, not a
failed verification of 248373e4f35d. Recovery behavior for this recurrence was
not reported; switching worklanes cleared the earlier incident.

GH-191 follow-up validation must exercise sustained real PTY output plus
physical upward scrolling while output continues, checking terminal geometry
and last-row visibility before/after returning to the live bottom. Existing
split/resize checks do not cover this sequence. Automated reproduction and
verification of this sequence on the repaired build are NOT RUN; post-restart
desktop confirmation remains pending. These steps are recorded in GH-191 as
well as this report, rather than relying on conversation context.

### September 25: live lag inspection (no repair or restart)

Jason reported current lag. Read-only host sampling at approximately 10:35–10:37
CEST identified GUI PID 269662, running for three days. Its executable inode
matches the installed clean ReleaseSafe build 248373e4f35d. Recent user-journal
slow-worklane diagnostics confirm stalls: server-chrome processing took 826,
3702, 971 and 642 ms; the corresponding worst full worklane render was 3729 ms
(sidebar 3726 ms, layout 2 ms). This localizes measured wall-clock delay to the
server-control ranking/configuration portion of render_chrome, not terminal
layout; it does not establish the underlying blocking operation.

Five-second pidstat sample: GUI CPU 12.6% of one core, RSS about 928 MiB,
no major faults or measured disk traffic. Main-thread CPU in a subsequent
three-second sample was 6%. Host had about 75 GiB available and no swap use,
but substantial I/O pressure (some avg10 about 51%, full about 32%) and bursts
of Java and other background CPU work. Contention may contribute; it is not
proven to explain the measured UI stalls. Last five minutes had 2137 application
journal lines, 68 fleet-popover renders and 65 changed agent events. No recent
kernel warning entries were returned. Checks: pidstat, vmstat, /proc/pressure,
journalctl filtered to the GUI PID, executable inode/version comparison, and
source inspection. No live client settings, processes or workspace were changed;
no repair, regression test, install, commit or push performed. Root cause and
post-repair responsiveness remain unverified.

Follow-up: Jason localized the symptom to intermittent worklane switches, then
reported input lag too. Bounded live traces and low-rate perf sampling did not
capture another logged server-control stall. The ioctl trace with stack unwinding
coincided with the reported heavier lag and may have amplified it; detached it
promptly, and do not use that interval as clean reproduction evidence. No ioctl
over 30 ms appeared in that capture. Subsequent sampling without strace found
the UI thread mostly in event polling (78/100 wait-state observations), with
6 futex waits and 16 running/other observations. Jason then reported no issues.
All captures completed; TracerPid is zero. No repair applied. Server-menu
replacement on a changed visible server list remains a candidate for the earlier
logged stalls, not a confirmed root cause. The existing unchanged-menu fast
path explains why that work is conditional, but does not prove why it stalled.

### September 25: Roblox Working state without spinner (GH-192)

Jason reports Codex working in Roblox with Working labels but no spinner.
Read-only discovery identifies worklane-5/pane-1; status logs confirm Running.
Same installed/running build 248373e4f35d. Journal sequence: animation active at
09:23:05; Ready/Idle at 09:23:37.410 stops it; Working + Braille at 09:23:37.441
arrives while lifecycle remains Idle and does not restart it; agent.running at
09:23:43.712 restores Running, but no later animation-active transition appears.
Pane subsequently moved to Roblox; no move was performed by this investigation.

Code inspection explains the lost rearm: TerminalTitleEventGate deduplicates
subsequent spinner frames; only accepted title callbacks reconcile/start the
animation, whereas lifecycle events update status without rearming it. A title
accepted while ineligible is therefore not retried when Running returns.
Tracked with regression and existing real-product journey acceptance criteria:
https://github.com/TamedTornado/TornadoTTY/issues/192 (related #123/#132).
No automated reproduction or repair yet; no tracing, restart, settings changes,
installation, commit or push. Preserve canonical lifecycle precedence and title
frame deduplication when repairing presentation reconciliation.

GH-192 repair: retain one canonical recognized source-title template per live
pane separately from currently active animation. Reconcile eligibility at the
existing bounded agent-input completion boundary, including when no semantic
title changes; canonical lifecycle/ownership remains authoritative. Ready and
other non-spinner source titles clear intent; pane removal forgets it. Stable
identity, frame deduplication, reduced motion and GTK frame-clock rendering are
unchanged. No engine change.

Focused regression RED before the repair (no frame after Working-while-Idle ->
Running); all 8 animation tests GREEN after it, including ownership exclusions
and removal. Existing fleet journey extended with PTY titles and authenticated
IPC separated by its existing FIFO. First fixture incorrectly used physical
Enter between titles, which itself marks Running: rejected by the fixture's
Idle guard, not counted as product reproduction. Corrected fixture RED against
248373e4f35d specifically at missing rearm, GREEN against build/gh192-spinner on
X11, including animation/stable chrome/idle teardown and no hierarchy rebuild.
Native Wayland passes those spinner checks but the later cross-window activation
credential assertion FAILS; separately tracked as GH-193, not waived. Existing
X11 event-coalescing journey PASS (real socket/PTY/GTK, sibling input during
traffic, bounded growth). Receipts: /tmp/gh192-{tests,build}.log,
/tmp/gh192-before-x11-gated.log, /tmp/gh192-after-{x11,wayland}.log and
/tmp/gh192-coalescing-x11.log.

Integrated linux/scripts/build-local PASS, ReleaseSafe dirty 59dccb56e860,
unchanged pinned Ghostty b62d1c57fe7d265ab030fd25c96742e1b11442b8; dependency-age
audit PASS with zero exceptions. Bash syntax, edited animation-file formatting,
and git diff --check PASS. Strict Clippy FAILS on pre-existing core lint errors;
--no-deps attempt also FAILS on pre-existing Linux library workload documentation
errors before checking the binary. ShellCheck FAILS on existing source-following
and intentional single-quoted child-command diagnostics; no suppressions added.
No full qualification, install, live restart, commit or push. Live confirmation
pending; repair is local for Jason's requested bug-fix batch before a new release.

### September 25: GH-185 follow-up investigation, no latency repair

Reviewed server ranking/control code and prior issue evidence. A temporary
measurement in the existing window_chrome lifecycle-test module alternated two
seven-entry server lists in a real GTK window, 40 times per run, on private
software-rendered X11 and native Wayland. Also opened each menu before its next
replacement. Updates remained millisecond/sub-millisecond scale; the live
multi-second delay was NOT REPRODUCED. This small GTK control does not include
the full live workspace, hardware driver or three-day process history. GTK
4.14.5 source confirms replacement unparents the old popover and realized
popovers own native renderers, but this alone does not establish our stall cause.

A bounded 60-second eBPF duration-only sample measured three live GTK calls:
completed tooltip and accessibility updates peaked at 14/41 microseconds;
there were zero set_popover completions. No stacks or terminal contents were
captured, and no ptrace stops used. Jason confirmed no lag at this time. Probe
process completed and detached. This non-recurrence cannot exonerate the menu
replacement path or attribute earlier lag to host I/O pressure.

No speculative behavior fix retained. Temporary diagnostic test removed after
saving /tmp/gh185-menu-diagnostic.patch; receipts are
/tmp/gh185-menu-{baseline,open-baseline,wayland-baseline}.log and
/tmp/gh185-widget-timing.{bt,log}. No integrated rebuild, full qualification,
installation or live restart. Existing slow-worklane stage logs remain in place;
next useful evidence is a recurrence separating ranking, primary-control
updates and menu replacement. GH-185 remains unresolved.

### September 25: GH-174 notification launch capability repair

Initial issue/recommendation described the older approval-only receiver. Current
355f17d7 behavior intentionally accepts goal-aware Codex TUI completion AND human
approval as GenericInput, not Approval. Preserve that later contract; the remaining
defect was trusting every existing Codex session without launch evidence.
Unknown-session regression RED: arbitrary Work finished OSC changed Running into
NeedsInput. Added an additive optional notificationCapability on authenticated
agent events, canonically owned by per-session bookkeeping (not persisted).

Managed launcher clears inherited declarations and emits an argv digest only
for explicit, unambiguous OSC9/always/approval-requested+agent-turn-complete CLI
settings. Conflicting notification overrides or broad tui overrides fail closed.
Values use the existing TOML parser, with Codex-compatible bare string handling.
The hook helper checks a bounded native Codex ancestor's actual /proc argv against
that declaration; mismatched, unreadable, deleted, nested-unverified, legacy and
external launch evidence is Unknown. It never reads this on GTK, logs argv, or
changes approval policy. Fresh hooks establish resume/restored session capability;
PID replacement and session restart revoke old capability. Multiple candidate
Codex sessions remain ambiguous because OSC has no session ID. Unknown channels
use ordinary terminal delivery under existing visibility settings instead of
inventing NeedsInput. Other agent notification policy is retained.

This is supported-producer launch-contract verification, not cryptographic OSC
sender attribution or proof of arbitrary custom executable behavior. Descendants
share a PTY, and authenticated local producers already hold authority to publish
agent state. Non-native wrappers that cannot supply matching native argv remain
ordinary notifications. Limitations are documented in docs/cli.md. Source checks:
official https://developers.openai.com/codex/config-reference and local Codex
95c7265e849e config override/TUI configuration paths; no Codex repo edits.

Evidence: 42 agent-status, 34 adapter, 2 policy tests PASS; IPC ancestry-proof,
restored-capability and Codex launch tests each PASS. Existing review-routing
journey extended using its controlled actor inside the existing native platform
helper, real launcher/helper/socket/PTY/Ghostty/GTK and notification daemon. This
is a SYNTHETIC producer, not actual model approval QA. Old stage gh192-spinner
FAILS legacy ordinary-delivery control (instead projects NeedsInput); repaired
X11 and native Wayland PASS verified approval/completion attention, pre-policy
non-attention, legacy and override ordinary delivery exactly once, and teardown.
Both first final runs passed behavior but FAILED daemon Quit with D-Bus NoReply;
the fixture exited before its queued reply flushed. Flush before test-daemon exit
repairs that race; final runs PASS without retry logic or weakened assertions.

Targeted mutations through linux/tests/mutate-rust: final policy 20/20 caught
(after three initial survivors exposed missing negative controls for packed flags
and two-item lists with unsupported kinds). Final receiver run catches all seven
relevant mutants; two unrelated seed_restored_starting field mutants leaked the
function filter and survive, so the raw run exits 2, not an all-green mutation
receipt. Initial broader receiver run also leaked an unrelated task-parser mutant.
No suppression or unsafe worker copying. Receipts: /tmp/gh174-{core-final,
proof-tests-final,restore-test,launch-tests,policy-mutation-final,
receiver-mutation-final,x11-verified,wayland-verified}.log; failed runs retained.

Integrated build-local ReleaseSafe stage build/gh174-capability PASS (dirty
baseline d5ec0c51, unchanged pinned engine); dependency-age audit PASS, exceptions=0.
Diff whitespace, Bash syntax and new-module formatting PASS. IPC library strict
Clippy --no-deps PASS; core strict Clippy still FAILS on four pre-existing errors
in attention_inbox/workload_policy/workspace_recipe. One new semicolon lint was
fixed before the final lint run. No full qualification, installation, live client
restart, commit or push. Actual installed Codex human/automatic-review QA remains
pending the coordinated bug-batch release. GH-174 remains open for that adoption.

### 2026-09-25: Regulate selection and blank worklane reports (#194, #195)

- Jason reported that the third of three full-width panes could not be selected,
  then that returning to the same lane after arranging three stacked panes showed
  white until selecting a specific pane. Running installed GUI remains
  `248373e4f35d`; newer source fixes are not installed.
- Read-only journal evidence at 14:10:51 CEST shows duplicate column identities:
  `column-pane-2,column-pane-18,column-pane-18`. Repeated pane-18 selections at
  14:10:57–59 were followed by focus returning to pane-17. After the user's
  arrange-height-three action at 14:11:11, all three selections succeeded.
  Duplicate identities are confirmed; their creation path is not yet reproduced.
  Tracked with regression and safe persistence acceptance criteria in
  [#194](https://github.com/TamedTornado/TornadoTTY/issues/194).
- At 14:12:13, lane selection logged one-column layout and pane-13 focus; the
  next explicit pane-17 selection at 14:12:14 matches the reported recovery.
  Logs establish navigation, not rendered pixels. All three panes remain in the
  model. Cached layout visibility/allocation, scrolling, and redraw are hypotheses,
  not a confirmed common cause. Separate visual regression criteria are in
  [#195](https://github.com/TamedTornado/TornadoTTY/issues/195).
- Evidence obtained with bounded `journalctl --user _PID=269662` time filters
  and read-only workspace listing. No live client restart or workspace changes,
  no behavior patch, and no new automated regression run yet.

### 2026-09-25: reflow repair and divider oscillation investigation

- #194 reproduced red in `reflow_expansion_keeps_columns_unique_and_every_pane_selectable`:
  expanding `[pane1,pane2],[pane3]` reused the second column's ID and generated
  that same ID for column three. Reflow now reserves existing window-wide IDs
  before generating new ones. Focus, order and recipe round-trip are covered.
- #195 also reproduced visually in the existing isolated divider-layout journey:
  old `248373e4f35d` rendered an entirely white content area after reflow and a
  worklane-header round trip (`/tmp/gh195-return-baseline.log`, screenshot under
  `/tmp/gh195-baseline-evidence/`). Duplicate IDs overwrote the mounted-column
  map entry, leaving an untracked empty overlay. Preventing the identity
  collision restores visible terminals without unmounting inactive worklanes.
- The extended X11 journey passes header return pixel checks and physical
  sidebar selection/input to all three full-width PTYs with the reflow repair.
  A separate setup race exposed a queued 50 ms scroll for the old selection
  overriding a newer sidebar selection; the callback now rejects stale lane/
  pane targets. This was real viewport misselection, not missing PTYs.
- Saved live snapshots can retain duplicate IDs. Startup previously discarded
  such a workspace (new persistence regression red). Restore projection now
  renames only colliding column IDs, reserves existing suffixes, preserves all
  panes/geometry/drafts, and retains the original snapshot. Ambiguous same-lane
  focus keeps the prior first-match interpretation. Focused checks: 82 workspace
  tests, 17 persistence tests, and 6 divider semantic tests PASS.
- Jason then reported flickering/extreme resize after moving pane three to a
  new window. [#196](https://github.com/TamedTornado/TornadoTTY/issues/196):
  14:26:46–47 CEST journal alternates 120/1177 and 1177/120 pixel pane heights
  approximately every 16 ms. Candidate repair uses surface-relative event
  coordinates rather than the moving handle's local drag offsets, and leaves
  handle position projection solely to the model callback (previously applied
  twice). A single-step synthetic drag did not reproduce oscillation; the
  regression is being checked with incremental physical motion over allocations.
- Validation failures retained: the multi-window restore-layout journey stopped
  before divider testing on cross-window input routing; closing pane three in
  the extended journey aborted both pre-divider-fix and candidate builds with
  GTK invalid weak refs and heap corruption. The latter is separately tracked
  in [#197](https://github.com/TamedTornado/TornadoTTY/issues/197), with raw
  `/tmp/gh196-close-crash.log`. No workaround is presented as a lifecycle fix.
- Native Wayland pointer coverage is BLOCKED: Cage driver transport is
  keyboard-only; labwc's outer output lacks `_NET_WM_PID` and fails the existing
  strict owner check. Independently reproduced without launching the product;
  [#198](https://github.com/TamedTornado/TornadoTTY/issues/198). No ownership
  check was relaxed. Exact post-move-to-new-window drag and live desktop QA
  remain pending. No installed binaries or live workspace changed.
- Final #196 focused result: ten physical 2-pixel movements separated by 30 ms
  reproduce the old defect: the top PTY shrinks from 273 to 105 pixels despite
  20 pixels of downward pointer travel (`/tmp/gh196-slow-drag-red.log`). With
  the repair, the same journey PASSes small positive resize, stationary-pointer
  stability, all three full-width sidebar selections, stacked header-return
  pixels, last-row clipping checks and normal exit (`/tmp/gh196-slow-drag-green.log`).
  This establishes the moving-divider defect independently of window transfer;
  the exact post-transfer path remains a stated acceptance gap, not passed QA.
- Integrated `build-local` PASS, stage `build/gh196-divider`, source based on
  `9af9ec52` plus uncommitted fixes and pre-existing dirty work, unchanged pinned
  Ghostty `b62d1c57fe7d265ab030fd25c96742e1b11442b8`; publish-age audit PASS
  (91 packages, zero exceptions). Linux Clippy remains FAIL on six pre-existing
  workload-library errors, before the binary checks. No full qualification run.
- The screenshot capture subprocess now has a five-second timeout: ImageMagick
  otherwise entered interactive capture after the test window disappeared in
  #197. Only identified isolated-test capture processes were stopped; live
  TornadoTTY was untouched. Source fixes and regression coverage are committed
  with this entry; the running client remains unchanged and unpatched.

### 2026-09-25 14:45:56: live GUI exit after Wayland disconnect (#190)

- Jason reported another terminal crash. Original installed GUI PID 269662
  logged `slow-worklane-chrome` with servers-ms=15372, then Wayland display
  failure errno=32, display-error=0, protocol-code=0, object-id=0, interface=none,
  followed by GDK `Error flushing display: Broken pipe` at 14:45:56 CEST.
- GNOME Shell PID 4525 logged `WL: error in client communication (pid 269662)`
  at 14:45:56.760. A new GUI PID 1409371 was launched by GNOME at 14:45:59;
  investigation did not launch it or change recovery state. Subsequent GNOME
  stack-position assertions occurred after relaunch. This matches the observed
  exit mechanism in #190, not the isolated heap-corruption failure in #197.
- The 15.4-second stall belongs to the measured server-chrome path in #185;
  causal connection to the disconnect remains unproven. No protocol object or
  code identifies a root cause. `coredumpctl` is unavailable; no crash backtrace
  was obtained. Bounded client journal receipt: `/tmp/tornadotty-latest-crash.log`.
- Pane fixes were committed as `f82602fbe8d49270c44623e355235e34afd913c6`
  after review and fresh PASS of 82 workspace, 17 persistence, and 6 divider
  semantic tests. They were not installed and cannot explain this old GUI exit.
  Live recovery and agent health are not certified by process existence alone.

### 2026-09-25: capture live stall context before display failure (#199)

- Jason requested actionable logs after the #190 recurrence. Added a fixed-state,
  off-GUI progress observer with a 100 ms heartbeat; reports start at 500 ms,
  then 2 s and 10 s. At most three samples per stall and twelve records per
  minute including recovery. No GTK calls from the observer, no event queue,
  and no log I/O under the shared state lock. Owned shutdown removes the timer,
  signals the worker and joins it.
- Symbolic scopes distinguish application ticks/layout, chrome, server ranking,
  projection, tooltip, accessibility, menu construction and popover replacement.
  Reports include correlation sequence and bounded kernel wait/syscall-number
  metadata, never syscall arguments, URLs, paths or terminal/agent payloads.
  Delivery uses nonblocking local journal datagrams so stderr backpressure
  cannot stop the observer. Missing/full journal delivery can drop records;
  initialization failure is explicit. This is diagnostics, not a crash repair.
- Existing Ghostty/GDK fatal display logging is unchanged. Added GDK display-close
  status, with the documented caveat that fatal GDK exit may bypass that signal.
  No protocol payload tracing, ptrace, stack unwinding or network submission.
- Focused observer tests PASS for reporting before blocked work returns, nested
  attribution, scope identity, recovery, caps, healthy silence, shutdown, absent
  kernel metadata and nonblocking delivery under journal backpressure.
- Real GTK blocked-popover regression PASS on isolated X11 and native Wayland:
  `ServerPopoverReplace` was reported at 500 ms while GTK's popover property
  callback was still blocked (`wait=futex_do_wait syscall=202`). A private marker
  used as a server label/URL was absent. Existing menu reuse/lifetime test PASS
  with the observer installed. Receipts: `/tmp/gh199-tests-final.log`,
  `/tmp/gh199-gtk-blocked-{x11,wayland}.log`, `/tmp/gh199-gtk-direct.log`.
- Integrated build PASS. Broader divider product journey FAILed the same
  full-width-pane-1 missing-marker assertion on both the logging candidate and
  pre-logging build; retained as an unresolved #194/#195 validation gap, not
  called a diagnostics PASS or retried until green. Receipts:
  `/tmp/gh199-product-x11.log`, `/tmp/gh199-baseline-product.log`.
- Jason authorized installation without restart. Installation will use matching
  GUI/engine artifacts with atomic file replacement; the active GUI and its
  saved state will not be touched. New diagnostics start only on the next
  launch. Retrieval/limits are documented in `docs/configuration.md`.
- Deployment completed with Jason's explicit no-restart approval. Diagnostics
  commit `f61bf209604cba645d484d39af71a168c1894d2e` was rebuilt using build-local
  (ReleaseSafe, dirty due to preserved pre-existing work, Ghostty clean pinned
  `b62d1c57fe7d265ab030fd25c96742e1b11442b8`). Publish-age audit PASS, 91 packages,
  zero exceptions. Five observer tests PASS; the blocked GTK evidence tests
  PASS on X11 and native Wayland, plus the existing menu lifetime test on X11.
- Staged and byte-verified the GUI and matching embedding library, then renamed
  each over its installed destination; no executing file was overwritten in
  place. Installed GUI sha256
  `173795f0ad00f944d96d19c9ed15b2d0d391245acb128f70ea3e831a0902bf24`;
  engine sha256 `d8ea90942a00958ca121f14e12b903027a05159de571a504dd9c71277b751481`.
  `tornadotty --version` reports f61bf209604c, release-safe (dirty). Existing CLI
  checksum was verified unchanged; unrelated CLI/worktree edits were not deployed.
- Live GUI PID 1409371 retained inode 75497503 and size 224766416 before/after;
  no restart, launch, agent interruption or saved-state change was performed.
  Thus the currently running GUI still has old code, and the new observer will
  activate only on the next launch. Rollback GUI/library and build metadata are
  retained at `/tmp/tornadotty-gh199-rollback-f61bf209`. No release or push performed.

### 2026-09-25: secondary shell exit quits all windows (#200)

- Jason reported exiting restored Codex and then its fallback shell in a
  secondary one-worklane/one-pane window terminated all terminals. Read-only
  journal review identifies the 15:11:42 CEST shutdown in old GUI PID 1409371
  (248373e4f35d): child-exited-pane=pane-18, application-shutdown, sibling PTY
  teardown, lifecycle-windows=2 and cleanExit persistence containing both windows.
  This is an unintended clean application shutdown, not the #190 Wayland failure
  or #197 native heap-corruption signature. Raw receipt:
  `/tmp/gh200-secondary-exit.log`.
- New process PID 1540262 starts at 15:11:46 with f61bf209 and confirms the new
  progress observer is enabled. Absence of stall records is consistent with this
  being a quit path rather than a frozen GUI; it is not evidence of a crash fix.
- Confirmed current-source cause: natural child exit maps ClosePaneOutcome::CloseWindow
  directly to the process-wide main_loop.quit(). Manual pane closure already
  delegates to its owning-window coordinator. Natural exit now uses that same
  callback and logs its window/pane close disposition before delegation. Only
  the coordinator decides whether closing this window should end the process.
- Added a focused mode to the existing multi-window journey: secondary real PTY
  exits naturally, its window closes, the same GUI PID and sibling PTY accept
  further input, then the final PTY exits and a clean snapshot retains the
  surviving window's restore intent. Baseline f61bf209 FAILed (sibling lost);
  repaired product PASSes on isolated X11 and native Wayland. Receipts:
  `/tmp/gh200-red.log`, `/tmp/gh200-green-{x11,wayland}.log`.
- Pane-runtime tests: 13 PASS, 1 existing GTK accessibility test ignored in this
  non-display run; includes confirmed restored-agent-to-shell policy. The GUI
  regression uses controlled PTY children, not an actual Codex login/session.
  `bash -n`, diff checks and integrated build PASS (91 age-audited packages,
  zero exceptions). No full qualification run and no live workspace manipulation.
- Fix committed as `7532ff2febf329356b8024d42ba641761a688859`, rebuilt and
  installed by same-filesystem rename without restarting the current session.
  Installed GUI sha256
  `33f5de4dcfb3d63f34a0aa48d22ca72970a3c436c92253aff680e8e2f6368356`;
  version reports 7532ff2febf3, release-safe (dirty from preserved pre-existing
  work). The matching Ghostty library was verified byte-identical to installed
  and not replaced; CLI and saved state were untouched. Rollback executable and
  metadata: `/tmp/tornadotty-gh200-rollback-7532ff2f`.
- Running GUI PID 1540262 retained inode 75497515, size 225202760 before/after.
  It still runs f61bf209, so the #200 repair becomes active only on next launch.
  No push or release performed; live user confirmation remains pending.

### September 25: selection marker advanced without revealing the pane (#201)

- Jason reports desktop notification activation opens Regulate but leaves the
  previously selected terminal visible; direct sidebar clicks likewise move the
  green marker without changing the displayed pane until another interaction.
  Live f61bf209 PID 1540262 at 15:50:18 CEST logged the correct notification
  target and focus callback (pane-17), in a three-full-width-column lane.
  Selection/focus receipts alone did not prove viewport visibility.
- The common explicit-focus path relied on GTK to scroll its focused widget.
  It now schedules an explicit post-layout column reveal for both sidebar and
  notification selection, coalescing to one cancellable request per window and
  retaining lane/pane stale-request checks. Inactive windows receive logical
  widget focus without presenting/activating the native window. Shutdown cancels
  the outstanding source. No engine change.
- Existing private-notification journey now uses distinct terminal backgrounds,
  activates pane two from another worklane, checks pixels before further input,
  and verifies physical input reaches pane two. Old gh200 product FAILed the
  pixel assertion despite logging the correct selected pane; repaired product
  PASSes on isolated X11 and native Wayland. Receipts:
  `/tmp/gh201-notification-{old,fixed,wayland}.log`.
- Existing sidebar/reflow/divider journey now distinguishes each pane's last-row
  pixels and checks its PTY input. Repaired X11 product PASSes
  (`/tmp/gh201-sidebar-green.log`). Old product also PASSes this stronger journey
  (`/tmp/gh201-sidebar-distinct-baseline.log`): Jason's intermittent direct-click
  case was not reproduced in isolation; common-path repair is not live QA proof.
  Physical Wayland sidebar coverage remains blocked by #198; notification and
  keyboard Wayland coverage above does not bypass pointer ownership checks.
- Fixture setup initially lacked the explicit previous-pane binding, used a
  nonexistent daemon method, and painted only initially allocated terminal cells.
  Corrected setup uses the existing ActivateLatest method, OSC 11 background and
  semantic ready titles. These were fixture failures, not product regressions.
- Focus Rust tests: 8 PASS. Integrated product build, shell syntax and diff checks
  PASS; dependency age audit 91 packages, zero exceptions. No full qualification,
  mutation claim, live restart or saved-workspace manipulation. Live confirmation
  remains pending; issue #201 stays open.
- Repair commit `c1a9c98f4971c4da05c46131398040945a1b4059` rebuilt successfully
  and installed by atomic sibling rename without restart. Installed GUI sha256
  `30b7a05491789ed9ff604f15917e28b240d13a04a50a6cfb013d904314dd12f1`;
  version c1a9c98f4971, release-safe, dirty from preserved pre-existing work.
  Matching engine verified byte-identical; engine, CLI and saved state untouched.
  Rollback: `/tmp/tornadotty-gh201-rollback-c1a9c98f`. Live PID 1540262 retained
  executable inode 75497515 / size 225202760 before and after; it still runs
  f61bf209. Repair takes effect next launch. No push or release performed.

### 2026-09-27: backlog and accumulated-log review

- Reviewed open fork issues/comments and retained journals through approximately
  08:00 CEST. September 25 15:11:46 startup identifies PID 1540262 as f61bf209
  and enables the 500 ms observer. Later journal entries retain that PID;
  newer on-disk installs do not establish that their repairs ran in this session.
- #185 has fresh but not root-causal evidence: 216 slow server-chrome records
  after startup, all on September 26, measure 50–259 ms in the combined server
  stage. Maximum at 18:30:56; other chrome/context/attention timings there are
  zero. Changing scanner endpoints accompany the inspected 16:38–16:39 interval.
  This supports investigating repeated server presentation work, not assigning
  cost specifically to ranking or popover replacement. Filtered local receipt:
  `/tmp/tornadotty-backlog-events.log`.
- No `tornadotty-ui-diagnostics` records returned from either user or accessible
  combined journal since that startup. Measured calls are below its threshold;
  absence is not proof of no stalls or reliable delivery. No new Broken-pipe
  signature found in this interval. #190 still lacks a disconnect cause; the
  earlier 15.4-second stall remains temporal association only.
- #197's retained `/tmp/gh196-close-crash.log` confirms invalid refcount/weak-ref
  diagnostics followed by heap corruption. Its existing isolated close/reflow
  sequence is ready for lifetime investigation, but the offending unref is not
  established. #188 still lacks the vanished pane's terminal-mode evidence;
  #191's sustained-output/scrollback recurrence lacks original-incident proof.
- #192, #194/#195, #196, #200, and #201's notification path already have causal
  regressions/repairs; #186 and specific #143 triggers have prior repairs too.
  Their documented validation gaps remain, rather than all being blocked on
  more logs. No tests, code repair, install, restart, live input, issue mutation,
  or closure performed in this review.

### 2026-09-27: restart shrinks inactive full-width columns (#202)

- Jason restarted and reported Regulate's full-width panes exposing neighboring
  columns. New GUI PID 1262083 logs c1a9c98f at 08:03:43 CEST: the #201 fix is
  now running, unlike the earlier session. Old columns were 810 pixels; startup
  records previous=892/current=810/factor=0.908072, then three 736-pixel columns
  in an 810-pixel viewport. Tracked in
  [#202](https://github.com/TamedTornado/TornadoTTY/issues/202).
- Confirmed cause: the sidebar-settling guard applied only when the active lane
  had multiple columns, although proportional scaling mutates all lanes. A
  single-column startup lane admitted the provisional baseline and shrank the
  inactive restored columns. The shared reconciliation now waits for settled
  sidebar allocation regardless of the active lane's column count.
- Focused regression RED with the exact 892-to-810 transition; allocation tests
  PASS (16), including preservation of inactive widths and later intentional
  resizing. The existing layout driver gained a restore-only mode with synthetic
  private state, three inactive full-width columns, and real PTY pixel queries.
  Old c1a9c98f FAILs: 670 text-area pixels instead of 750 in a 754-pixel viewport.
  Repaired product PASS on X11 and native Wayland for all three panes, physical
  input and normal exit. Existing full X11 layout/reflow/divider journey PASS.
  Receipts: `/tmp/gh202-red.log`, `/tmp/gh202-final-red.log`,
  `/tmp/gh202-x11-final.log`, `/tmp/gh202-wayland-synchronized.log`,
  `/tmp/gh202-divider-x11.log`, `/tmp/gh202-tests-final.log`.
- Fixture failures preserved: missing focus/allocation synchronization, sampling
  the initial single pane as an unreliable width reference, and assuming an
  unbound navigation shortcut. Final fixture waits for native Wayland initial
  allocation/focus and explicitly binds navigation. Initial single-pane PTY
  width was 830 on both baseline and candidate despite the settled 754 viewport;
  this separate sizing observation needs follow-up, not a #202 success claim.
  Final restored-column checks use the known fixture viewport and cell rounding.
- Integrated `build-local` PASS at `build/gh202-restore` (dirty bea8003e base,
  unchanged Ghostty b62d1c57f); age audit 91 packages/zero exceptions. Formatting
  and diff checks PASS. Driver Clippy FAIL remains on the unchanged 103-line
  `observe_divider_layout`; new lint findings were corrected without suppressions.
  No full qualification, install, restart, or live input. Already
  shrunken saved widths are not guessed back to full width; user arrangement
  intent must be reapplied once. Issue remains open for deployment/live QA.
- At Jason's request, the repair, focused regression coverage and these review
  notes are committed together after diff review; unrelated pre-existing changes
  are excluded. No additional test run is implied by committing. Not pushed or
  installed; the built artifact above still identifies its pre-commit base.
