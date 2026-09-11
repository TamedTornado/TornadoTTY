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
