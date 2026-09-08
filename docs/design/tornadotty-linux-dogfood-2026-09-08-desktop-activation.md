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
