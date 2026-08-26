# Zentty Linux dogfood record — readiness reconciliation

Date: 2026-08-26

## Purpose

Reconcile the stale feature inventory and parent trackers after the Ghostty API
closeout, then move the port into ordinary operator dogfooding. This is not a
new qualification campaign. The existing terminal remains the repair lifeboat
while Zentty carries the operator's new coding-agent work.

## Rejected A/B handoff exploration

An initial discussion considered separate Stable and Candidate installations
with workspace handoff. The operator rejected that work before implementation
because it would delay dogfooding. GH-99 is closed as not planned.

The discussion also corrected a product misconception. Zentty's source
`TmuxCompatIPCHandler` describes a small compatibility subset for Claude Code
agent-team commands. It translates those commands into the running app's
window/worklane operations. `TmuxCompatStore` persists only compatibility
anchors, buffers, and active pane identifiers. It is not a background tmux
server, and another Zentty process cannot attach to its PTYs. Ghostty surfaces
remain the PTY owners.

Crash and reboot restoration therefore reconstructs topology and starts new
PTYs. Supported agents receive validated resume commands; arbitrary running
commands, PIDs, SSH connections, and Ghostty scrollback do not survive. The
existing SIGKILL product journeys prove restart restoration, not live process
survival.

## Inventory discoveries and repairs

Nine entries retained stale `PARTIAL` status after their actual delivery work
had landed:

- pane drag/drop and cross-window transfer were completed by GH-81;
- pane search received the later light/dark/scaled visual closeout;
- Ghostty fidelity, resources, configuration reload, packaging, input, and ABI
  work completed independently of the deferred performance overlay;
- shared agent consent/bootstrap and IPC/status/attention were completed by the
  managed-launcher and durable-event work;
- Claude lifecycle/team and OpenCode live-theme behavior were completed by
  their later real-product journeys;
- the implemented Task Manager deliberately excludes the dead Network column;
  future network/container accounting remains separate in deferred GH-65.

The authoritative inventory now separates the optional terminal performance
overlay from the implemented Ghostty foundation rather than keeping the
foundation partial because GH-77 is deferred. Task Manager similarly claims
only CPU, memory, process-tree, and interaction behavior already present in the
product. The two qualification-matrix `NOT_IMPLEMENTED` cells owned by GH-65
remain unchanged.

`commands.palette-routing` remains honestly `PARTIAL`. The palette is usable
and routes many real workspace, pane, settings, task, server, Git/review,
Open-With, and agent-fleet actions, but `command_palette_action_items` is still
an explicitly interim hand-maintained registry rather than the intended single
source command/availability registry. That is a real closeout item, not a
reason to postpone daily use.

After reconciliation the 60-entry inventory contains:

- 47 `IMPLEMENTED`;
- 2 `PARTIAL` (`commands.palette-routing` initial-release and `agent.copilot`
  required-later);
- 11 `NOT_IMPLEMENTED`, all platform-alternative or required-later work.

## Focused verification

- `linux/tests/feature-inventory-test`: PASS.
- `linux/tests/feature-inventory`: PASS.
- Machine summary: 60 entries, 47 implemented, 2 partial, 11 not implemented.

The runner test now pins the reconciled entries individually, verifies that
performance diagnostics are not smuggled into Ghostty-foundation completion,
verifies Task Manager's network exclusion and GH-65 ownership, and requires the
palette to remain partial until its registry is genuinely consolidated.

No full Linux qualification was run or claimed for this documentation/status
repair. The exact preceding executable matrix remains 201 PASS, 0 FAIL, 0
BLOCKED, 2 XFAIL, and 2 NOT_IMPLEMENTED.

## Daily-use boundary

The supported immediate workflow is intentionally simple:

1. keep the existing terminal open as the repair path;
2. install and launch one exact Zentty dogfood build;
3. create new worklanes and coding-agent sessions in Zentty;
4. record operator-visible bugs as focused issues;
5. reproduce and repair each bug with focused tests before updating the
   installed dogfood build;
6. reserve broader qualification for meaningful integration boundaries and
   release preparation, not every fix.

This accepts that crashes may restart supported agent sessions rather than
preserve their original PTYs. It does not describe command replay or topology
restoration as live session persistence.

## GH-100: final worklane close silently did nothing

### Discovery

The first operator dogfood launch restored an old test workspace whose final
worklane contained three panes. The worklane menu offered **Close Worklane**
and the confirmation accepted the destructive request, but the worklane and
all three panes remained. A confirmation followed by no visible result is a
product bug, not an acceptable expression of an internal invariant.

### Cause and decision

`WorkspaceState::close_worklane` deliberately rejects removal of its final
worklane because the state model is non-empty. Multi-worklane integration
coverage exercised only the case where another existing worklane survived, so
`ApplicationShell::perform_close_worklane` passed the final-worklane request to
that rejecting transition and silently returned. The sidebar additionally
disabled its final-worklane close button even though other action routes could
still present the confirmation.

The state invariant remains correct. Product orchestration now creates a fresh
single-pane default-shell worklane and its real Ghostty surface *before*
removing the requested final worklane. If replacement creation fails, the
original worklane, panes, and processes remain untouched and the normal action
error path reports the failure. After replacement succeeds, teardown removes
all live surfaces belonging to the old worklane and focuses the new shell. The
sidebar close action remains enabled for a single worklane.

### Evidence

- The controlled GTK widget test roots a one-worklane context menu under its
  real `workspace` action group and proves **Close Worklane** is enabled and
  routed to `workspace.close-worklane`.
- The focused nested-X11 lifecycle journey uses the real command palette,
  confirmation dialog, three live Ghostty PTYs, and physical keyboard input.
  It proves cancellation preserves the original shell, acceptance creates the
  replacement first, removes exactly the old worklane's three surfaces, focuses
  the replacement pane, routes new terminal input to it, and subsequently
  preserves window-close lifecycle behavior.
- `cargo fmt --check`, the focused workspace-state regression, and
  `cargo check -p zentty-linux` pass.

The first integration replay exposed one stale test assumption: its physical
X11 window probe still searched for the old final pane after the replacement
had correctly removed it. The product replacement assertions had already
passed. The probe now targets the replacement pane, and the complete focused
journey passes.

The controlled Wayland lifecycle was also attempted twice with Debug and once
with ReleaseSafe. It did not reach the new final-worklane section: the Debug
runs failed while restoring focus to the still-existing temporary worklane
after the earlier cancellation step, and ReleaseSafe failed an even earlier
physical-input route to `pane-1`. Those are environmental/harness failures in
pre-existing journey steps, not passes and not evidence for this repair. The
new orchestration is backend-neutral, but this dogfood fix claims real GTK and
Ghostty coverage on controlled X11 plus the operator's GNOME retest, not a
green controlled-Wayland receipt. No full qualification was run or claimed.

## GH-101: spinner-driven UI starvation and stale Codex attention

The next live dogfood session exposed a related cluster rather than four
independent GTK failures. A running Codex pane retained **Needs input** while
its title visibly reported `Working`. Every Braille spinner frame in that title
was stored as a distinct pane title, which triggered project-context work,
sidebar reconstruction, fleet snapshot replacement, and Agent Status popover
replacement. The resulting main-loop churn closed the popover, delayed
worklane reorder feedback and attention delivery, and prevented ordinary hover
tooltips from settling reliably.

Codex spinner frames now normalize to one stable UI title (`Working · Bro`).
Only a semantic title change requests project-context/sidebar work, and Codex
transcript enrichment is scheduled only when canonical agent state changes. A
fresh explicit input request still wins over an already-in-flight stale
`Working` title, but a persistently animated `Working` title clears that state
after a one-second grace period rather than leaving a false alert forever.

Focused evidence consists of two passing core regressions for stable spinner
identity and bounded attention reconciliation, `cargo check -p zentty-linux`,
shell syntax/static analysis for the extended existing multi-window actor, and
an added real-product assertion that an open Agent Status popover survives
three complete controlled spinner cycles. No full qualification is part of
this dogfood deployment.

## GH-103: failed automatic agent restore erased its worklane

### Operator discovery and durable evidence

After quitting and relaunching Zentty from the GNOME launcher, the operator's
**Bro** worklane was missing while **Consulting** remained. This was real
workspace loss, not a stale sidebar projection. The user journal for process
796313 establishes the exact sequence on 2026-08-26:

1. at 13:27:45 the restored topology was
   `worklane-4[title=Bro]:pane-6*|worklane-3[title=Consulting]:pane-5`;
2. Zentty configured pane-6 with the stored Codex resume command and Ghostty
   initialized its terminal surface;
3. Ghostty delivered `child-exited-pane=pane-6`;
4. Zentty immediately projected only `worklane-3: pane-5`;
5. live persistence published that damaged one-worklane topology; and
6. the later clean exit therefore correctly, but destructively, retained only
   Consulting.

The callback does not expose an exit code, signal, or child identity. It proves
that the configured terminal child exited during automatic restoration; it
does **not** prove that Codex itself was the process that exited. The original
cause of that child exit remains uncertain.

The pre-loss state was reconstructed in an isolated `/tmp` state directory
using the journaled worklane, pane, directory, and restore-draft identities.
The deployed dogfood binary reproduced the failure exactly: Bro appeared,
pane-6 initialized, `child-exited-pane=pane-6` arrived, Bro disappeared, and
the isolated live snapshot contained only Consulting. The operator visually
confirmed the reproduced disappearance. No real user snapshot was modified.

### Repair and GUI behavior

An automatic restore command is now pending until a routed, authenticated
agent lifecycle event proves that the resumed agent owns the pane. Surface
construction or initialization alone does not establish success, and there is
no timing heuristic. If the terminal child exits while restoration is still
pending, Zentty preserves the pane and worklane, clears the failed agent draft,
mounts a real fallback shell, and covers it with a persistent pane-local
recovery panel. The panel explains the failure and offers **Retry**, **Open
Shell Instead**, expandable **Details**, and the existing confirmation-aware
**Remove Pane** action. Retry retains the in-memory resume command without
printing its session identifier in the new recovery receipts.

The first implementation attempted to classify a restore as stable ten seconds
after surface ownership. The focused integration test disproved that design:
inactive Ghostty surfaces can be owned before they are initialized, and elapsed
time is not evidence of successful agent restoration. The timer was removed
entirely. Authentication is now the sole success boundary.

Persistence now emits structural receipts to the user journal at live queueing
and clean publication. They include reason, generation, active window, ordered
window/worklane/pane IDs, and restore-draft pane IDs. They deliberately exclude
terminal text, commands, directories, environment, prompts, session IDs, and
tokens. This makes future topology loss diagnosable for GNOME-launched builds
without requiring an attached stderr session.

### Focused evidence

- pane-runtime disposition regression: PASS;
- core failed-draft clearing plus pane/worklane projection regression: PASS;
- 14 persistence coordinator tests after adding structural receipts: PASS;
- shellcheck with sourced helpers, formatting, and targeted binary check: PASS;
- ReleaseSafe build, including cargo publication-age audit and package-notice
  collection: PASS;
- controlled nested-X11 real-product journey:
  `PASS failed-agent-restore topology=preserved fallback=real-shell snapshot=clean`.

The controlled actor exits from a real command running behind a real Ghostty
PTY. The focused mode asserts the persistent recovery panel was mounted, closes
the real window through compositor input, and proves the clean snapshot retains
all three worklanes and all five panes while excluding the failed resume draft.
No full Linux qualification was run or claimed.

The staged ReleaseSafe build was then launched on the operator's GNOME desktop
with a second fresh reconstruction of the lost Bro state. The first overlay
implementation incorrectly exposed the fallback shell even though GTK reported
the recovery overlay mounted; this failed visual acceptance. `PaneFrame` now
uses explicit terminal and restore-failure stack pages rather than relying on
native-widget overlay ordering. The operator confirmed the corrected recovery
page and terminal-row presentation as **all good**. The build remained staged
and isolated; it was not installed over the dogfood binary during this repair.

### Adjacent visual and CLI discoveries

The screenshot accompanying this incident also showed a passive pane label
overlapping Ghostty's first terminal row. Single-pane worklanes no longer render
that redundant overlay label; multi-pane labels remain available for
disambiguation. The deployed binary also rejects `--help` as an unknown
argument. That CLI defect is tracked separately rather than being folded into
the restore repair.

## GH-106: terminal mouse pointer lost contrast over the shell

### Discovery and diagnosis

While using the installed GNOME dogfood build, the operator reported that the
mouse pointer became nearly invisible over the shell. Neither the primary nor
the compatibility Ghostty configuration enabled `mouse-hide-while-typing` or
specified another mouse policy. Local Ghostty source inspection established
that terminal surfaces intentionally initialize their mouse shape to `text`
and translate that shape to GTK's cursor-theme-provided `text` pointer. The
active GNOME cursor theme's thin I-beam did not retain sufficient contrast over
the selected terminal background.

### Rejected repair and remaining limitation

A focused host-side experiment replaced only Ghostty's `text` cursor with
GTK's ordinary `default` arrow while preserving all other semantic shapes.
Automated policy tests and lint passed, but operator dogfood immediately found
that the arrow made terminal text selection feel wrong because it removed the
text-position affordance. The experiment therefore failed interaction QA and
was reverted before commit.

Zentty again preserves Ghostty's native text cursor without modification. The
operator then localized the apparent contrast problem to the first terminal
row: the pointer became an I-beam only after crossing below it. Inspection
found a transparent, full-width 15-pixel pane-drag overlay intercepting input
above Ghostty. The drag affordance now lives inside the visible contextual
control cluster, and that cluster is non-targetable while hidden. Terminal
content—including its first row—therefore retains Ghostty pointer and selection
ownership except beneath controls that are actually visible. No full
qualification is run or claimed for this focused dogfood repair.

## GH-107: GNOME launch omitted Codex restore tracking

### Operator discovery and durable evidence

The operator started the Bro Codex session in a new pane, cleanly quit Zentty,
and relaunched it. The worklane and pane returned, but Codex did not resume and
the failed-restore recovery page did not appear. Durable structural receipts
showed the new pane in generations 3 through 7, including the clean-exit
publication, while every generation reported an empty restore-draft set. The
relaunch consequently admitted zero restore drafts and opened an ordinary
shell. No authenticated agent event for the pane appeared before shutdown.
The saved pane retained the non-sensitive fact that its last command was a
Codex resume invocation, confirming that Codex had run without establishing an
authenticated Zentty session identity.

### Root cause and repair

`AgentRuntime` selected bundled agent wrappers using the GUI process's startup
`PATH`. A GNOME launcher does not see the Codex installation added by the
operator's interactive Bash startup files, so Zentty omitted the Codex wrapper
directory before the shell existed. Bash later found the real Codex binary but
could not select a wrapper directory it had never received. This made the TUI
functional while silently disabling lifecycle capture and session restore.

The GUI now exports every executable bundled wrapper directory unless that
integration is explicitly disabled. The existing shell integration remains
responsible for adding a wrapper only after it can also find the corresponding
real binary in the shell's final `PATH`; absent tools are therefore not
shadowed. A new non-sensitive receipt lists installed and active wrapper tool
names per pane. Focused coverage models a launcher with no Codex, proves no
wrapper is initially selected, adds Codex as a shell startup file would, and
then proves the staged wrapper becomes the resolved command.

Focused validation passed eight agent-runtime tests, the staged Bash boundary
scenario, shellcheck for the changed runner, Clippy with warnings denied, and a
controlled nested-X11 real-Codex journey. The latter used Codex 0.147.0, a real
Ghostty PTY, the staged wrapper, real Codex hooks and notify callback,
authenticated IPC, a controlled loopback Responses endpoint, physical window
close, a persisted UUID resume draft, exact relaunch command, and a rendered
resumed TUI. Operator confirmation that the GNOME-launched Bro pane now either
resumes or exposes the explicit failed-restore recovery page remains pending.
No full qualification is run or claimed for this dogfood repair.

### Automatic retry follow-up

The installed wrapper-selection repair made a manually entered `codex resume`
observable, but the operator's next GNOME relaunch still reached the recovery
page and both Retry attempts failed. The journal showed that the draft was
accepted and the exact resume command was selected, but the terminal child
exited before an authenticated event. This was a second PATH boundary: an
automatic Ghostty command starts before an interactive prompt, while the real
Codex path is added by the user's shell startup files. The wrapper was present
in the pane environment but could not find the real executable in the desktop
launcher's PATH.

The first correction ran the canonical resume command through the configured
interactive login shell. A new controlled desktop-PATH journey correctly
failed it: the user's startup file put the real executable ahead of Zentty's
wrapper, and the normal prompt-time reconciliation never ran for `-c`. Restore
launches now run the validated canonical command through Bash, Zsh, or Fish
with `env PATH="$ZENTTY_ALL_WRAPPER_BIN_DIRS:$PATH"` applied *after* shell
initialization. This preserves user tool discovery while selecting the
instrumented wrapper first; it does not hardcode Bun, duplicate shell PATH
parsing, poll, or use a timer.

The first version of the new fixture also failed because repository Codex is a
Node launcher and the simulated login environment exposed its shim without
the `node` runtime. After the fixture exposed both as login-shell-only paths,
automatic resume reached the real Codex Ready TUI and retained the pane draft.
Codex does not emit a new lifecycle hook merely by displaying an already-idle
resumed fixture, so the final journey physically focuses that TUI, submits a
real prompt to the controlled Responses endpoint, and then requires the
authenticated restore confirmation. The focused result is PASS: restricted
desktop PATH, real login-shell initialization, staged wrapper, real Ghostty
PTY, exact UUID resume, rendered TUI, physical input, real hook and notify,
authenticated IPC, controlled model turn, and physical close. No full
qualification was run or claimed.

The validated ReleaseSafe build was installed over the existing dogfood
package as one matched binary/CLI/wrapper/shell-integration set. The operator
then cleanly quit the old process and relaunched Zentty from GNOME. Bro resumed
automatically into the real Codex session without the recovery page. Journal
receipts for process 1058201 show one draft requested and accepted, exact UUID
launch in pane-6, authenticated `session.start`, and
`agent-restore-launch ... result=authenticated`. Live snapshot generation 5
retains both worklanes, both panes, and the pane-6 restore draft. Operator
dogfood acceptance: **Got my Bro session back**.

## GH-109: GNOME dock used a generic gear icon

The installed application launcher displayed Zentty's packaged icon, but the
running GNOME dock item displayed a generic gear. This distinguished icon
installation from window association: GNOME could resolve
`Icon=com.zentty.zentty`, but could not associate the live toplevel with
`com.zentty.zentty.desktop`.

An isolated real-Wayland launch with client protocol tracing proved the defect:
Zentty sent `xdg_toplevel.set_app_id("zentty")`. The Rust adapter set GIO's
application ID to `com.zentty.zentty` but separately set GLib's program name to
`zentty`. Because the product owns plain `gtk::Window` toplevels rather than
`GtkApplicationWindow`s, GTK used the program name as the compositor-facing
Wayland app ID. An isolated X11 launch also showed
`WM_CLASS = "com.zentty.zentty", "com.mitchellh.ghostty-debug"`; the packaged
`StartupWMClass=Zentty` matched neither value.

The packaged reverse-DNS ID is now a single Rust constant and is supplied as
both the host application ID and program name. `StartupWMClass` now matches the
host-owned X11 instance rather than Ghostty's inherited class. The new focused
`desktop-window-identity` actor launches the actual ReleaseSafe product and
asserts the real Wayland protocol value or X11 property, while also rejecting a
desktop-entry mismatch.

Focused evidence passed: 82 core tests, ReleaseSafe build with publication-age
audit and package notices, shellcheck, formatting, controlled Wayland
`set_app_id("com.zentty.zentty")`, and controlled X11 WM_CLASS instance
`com.zentty.zentty`. No full qualification was run or claimed. Installed GNOME
visual acceptance then passed after the matched binary and desktop entry were
installed and the desktop database refreshed: the operator relaunched from the
GNOME launcher, confirmed the dock icon looked correct, and confirmed Bro still
resumed automatically. The identity repair therefore did not regress GH-107's
agent recovery path.

## GH-102: live fleet updates closed the open Agent Status popover

While the Agent Status popover was open, any semantic agent-state change
replaced the `GtkPopover` attached to its `GtkMenuButton`. GTK correctly
unmapped the old transient, so a normal Running-to-Idle or Needs-input
transition made the operator's open fleet view disappear. Spinner-only title
updates had prior coverage, but that did not exercise the semantic snapshot
path that replaced the widget.

`WindowChrome` now creates and retains one fleet popover for its lifetime.
Fleet refreshes replace only that popover's child presentation, preserving the
mapped transient and menu-button relationship. Focus-on-map handlers belong to
each replacement child rather than accumulating on the long-lived popover.

The existing `rust-multi-window` actor gained an event-driven regression rather
than another harness or a timing assumption. A PTY child arms an authenticated
Codex Idle event and blocks on a private FIFO. The actor opens Agent Status,
proves it is mapped, releases the event, and requires a newly observed shared
fleet refresh plus a newly rendered idle presentation without a new popover
close receipt. Initial review found that simple text waits could have accepted
identical idle summaries from an earlier lifecycle step; the test was repaired
to capture pre-event counts and require both counts to increase.

The first nested-X11 invocation did not start because sandboxed Xvfb could not
bind its Unix display socket. That environmental failure was not treated as a
test result. The identical focused journey then passed on an isolated Xvfb
display outside the GUI sandbox: two native Zentty windows, two real Ghostty
PTYs, physical command-palette input, authenticated agent IPC, and the live
semantic transition all completed while the popover remained open. Formatting,
shellcheck, warnings-as-errors Clippy, and diff-integrity checks also passed.
The package run passed 317 tests before its existing real-`/proc` listener test
received sandbox `EPERM`; that exact test passed outside the sandbox, while two
display-only accessibility tests remained intentionally ignored. No full
qualification was run or claimed for this focused dogfood repair.

## GH-105: public product help and version commands

The deployed product rejected `--help` as an unknown option even though its
launch parser already owned four public options. The same parser now returns
explicit Run, Help, or Version startup actions. Help and version return before
configuration, Ghostty runtime, or GTK initialization. Unknown options remain
errors and now point to `zentty-linux --help`. Version output reuses the About
view's compiled version, build profile/tree state, and validated commit rather
than introducing a second provenance source.

The first staged output inspection caught that Rust's escaped multiline string
had removed intended option indentation. The help constant was changed to an
explicit concatenation. An attempted exact byte comparison for the entire help
page was rejected during review as brittle and not useful; the artifact actor
instead asserts successful display-free execution, clean stderr, usage, and
the presence of every supported option. Version output remains exact because
its values are the artifact's security and provenance identity, not visual
presentation. Unknown-option stdout, stderr, and nonzero status are also exact.

The display-free actor runs the actual staged ReleaseSafe binary with display,
session-bus, runtime-directory, and GDK backend variables removed. It is
registered as the `product-cli-options` cell in the authoritative matrix and
required by the existing orchestration contract; `--help` and `--version` were
also added to that contract's reviewed public-option allowlist. The actor,
formatting, shellcheck, and the focused CLI unit test passed. A mistakenly broad
Rust filter ran 322 binary tests: 319 passed, two display-only tests were
ignored, and the existing real-`/proc` test received the already documented
sandbox `EPERM`; the three new CLI tests passed within that run.

Running `test-orchestration-contract` accepted the new registrations and then
reported its pre-existing `staged-shell-integration` inline-agent finding. A
similarly named command was then invoked under the mistaken assumption that it
only validated matrix schema; `linux/tests/qualification-matrix` actually
starts qualification. It stopped after 13 seconds with existing
`prepare-ghostty` FAIL and suppression-governance PASS. That run is not a GH-105
result, was not rerun, and no local, release, or full qualification is claimed.

## GH-110: notification click did not foreground the notifying pane

The operator clicked a **Codex needs input** desktop notification but did not
arrive in the notifying pane. A later **Zentty is ready** notification then
provided a second click that reached the application. The product already
stored the exact window/worklane/pane target and handled freedesktop
`ActionInvoked`, but it called `activate_attention_target` without a window
activation credential. That selected the pane internally and then issued an
uncredentialed `GtkWindow.present()`, which GNOME/Wayland is entitled to deny.

The official desktop-notification 1.3 protocol defines an `ActivationToken`
signal immediately before `ActionInvoked`; its payload is either an X11 startup
ID or Wayland XDG activation token. Zentty ignored this signal. The notification
service now correlates a bounded token and target by service notification ID,
consumes both on the first supported action, and supplies the token to the
existing `WindowActivation`/`GtkWindow.set_startup_id` path before presenting.
Stale IDs, invalid tokens, unsupported actions, and close events cannot route.
Logs now record service ID, signal/action kind, target lookup, credential
availability, and exact target result without exposing the token capability.

Review of the prior integration evidence found that X11 asserted only the
internal `result=focused` receipt and then manually raised/focused the window
with `xdotool`; it did not prove the product could foreground itself. The
controlled notification daemon now advertises protocol 1.3 and emits the real
`ActivationToken` signal before `ActionInvoked`. Focused unit coverage passed
for exact single-use correlation and stale/closed/invalid inputs. The existing
isolated X11 attention journey passed with two native windows, two real Ghostty
PTYs, authenticated agent events, a private freedesktop notification service,
token-before-action ordering, exact target selection, and subsequent physical
input in the routed PTY. That journey still cannot mint a compositor-authentic
GNOME token; operator dogfood on the real GNOME session remains required before
GH-110 can close. No full qualification was run or claimed.

The clean audited Debian package for commit `79442eb938bc` was installed over
the previous dogfood build. After a normal quit and GNOME-launcher relaunch,
the operator clicked a real agent attention notification. The first click
foregrounded Zentty and delivered the operator directly to the notifying Codex
session; no second notification was required for navigation. This supplies the
real GNOME compositor acceptance that the controlled fixture cannot mint and
closes GH-110's remaining uncertainty.

## GH-111: the palette hid the non-resizing right-pane operation

The operator selected **Add Pane Right** from the command palette and observed
a visible split rather than another full-width column in the horizontally
scrolling worklane. Live application receipts confirmed that the palette
invoked `new-pane-right`, which selected `split-pane-right` at the 2279-pixel
adaptive viewport and produced two 1139-pixel columns. Source review corrected
an initially over-broad diagnosis: macOS deliberately defines **Add Pane
Right** as the adaptive command. Its separate forced worklane operation is
**Add Pane Right Without Resizing**. Linux already owned that typed action and
shortcut registration but omitted it from the command palette. **Add Pane
Left** is not similarly adaptive; it already preserves the source column width
and inserts a full-width column before it.

The missing source command is now present in the palette and routes to the
existing `add-pane-right` operation. A focused regression pins **Split Right**,
adaptive **Add Pane Right**, **Add Pane Right Without Resizing**, and **Add Pane
Left** to four explicit action targets. The first version of that regression
incorrectly required ordinary Add Right to be non-adaptive; it failed against
`new-pane-right`, was reconciled against the checked-in Swift source, and was
replaced rather than changing product semantics to satisfy a mistaken test.

Inspection also found a stale Linux comment claiming horizontal pane gestures
and Worklane Peek were absent. Both are already installed. The comment now
describes the actual adaptive presentation and reachable offscreen-column
routes. The existing real implementation switches adjacent panes on horizontal
touchpad gestures or Shift-wheel input and scrolls the focused column into
view; its focused unit remains green. The four-way palette test, source
vocabulary test, gesture unit, and all-target `zentty-linux` clippy with
warnings denied passed. No broad qualification was run or claimed.

## GH-112: ordinary agent activity rendered as a permanent alarm banner

The Agent Status chrome control showed a persistent orange/red vertical strip
while Codex was simply running. Source inspection found two independent causes:
the aggregate indicator policy deliberately exposed every non-idle state,
including ordinary Active, and the nominal eight-pixel GTK box had minimum CSS
dimensions but no fixed alignment or expansion policy. GTK could therefore
allocate it the full button height. The result communicated an alarm even when
the fleet popover correctly said Running.

The chrome indicator is now reserved for exceptional aggregate states: Needs
Input, Stopped Early, and Compacting. Active and Idle remain available through
the unchanged tooltip, accessibility label, and fleet popover but do not light
the chrome. A single typed geometry contract drives the GTK widget's eight-pixel
size, centered alignment, and disabled horizontal/vertical expansion. Focused
coverage pins empty, Active, Idle, Compacting, Stopped, Waiting, and mixed-state
priority plus every applied geometry property. The adjacent status/progress
copy test and all-target `zentty-linux` clippy with warnings denied passed.

This is focused state and widget-property evidence, not a claim that a real GTK
allocation has been visually accepted. The installed GNOME dogfood build still
requires relaunch and operator review before GH-112 closes. No broad
qualification was run or claimed.

The audited package for commit `07acda1e5af4` passed its release-safe build,
Rust publish-age audit, notice collection, and Debian package audit. APT called
the descendant package a downgrade because the package version embeds a Git
hash and `07ac...` sorts below the previously installed `7944...`; installation
therefore required an explicit `--allow-downgrades`. This is a package-version
ordering limitation, not evidence that the source revision moved backward, and
must be reconciled before release update ordering is claimed. After a normal
GNOME relaunch, the operator confirmed that ordinary Running produced no alarm
strip and that the Agent Status popover still reported the active Codex
session. That supplies the real allocation/visual acceptance and closes
GH-112's remaining uncertainty.

## GH-113: untouched shell bootstrap looked like user session history

After creating full-width and visibly split panes, the operator closed them
without typing into them. Both showed **Close this pane?**. The live journal
showed each new pane emitting several authenticated `shell-state running`
signals with command payloads during integration bootstrap before its first
prompt. The Linux shell-signal handler stored every such payload as
`last_run_command`; existing close evidence correctly treated that field as
session history and therefore asked for confirmation. The close-decision
policy was not missing—the provenance supplied to it was false.

Pane lifecycle state now owns one ephemeral pending-submission marker. A
physical terminal Enter records the pending submission before input reaches
Ghostty. The next authenticated shell command consumes that marker and becomes
durable `last_run_command`; command-bearing bootstrap signals without physical
submission cannot create history. Explicit launch, task, restore, and agent
resume recipes continue to set their meaningful command directly. The
existing close-evidence priority for active agents and non-idle foreground
processes is unchanged.

A focused regression covers bootstrap command rejection, first physical
submission and authenticated command acceptance, later bootstrap rejection,
and explicit Codex resume history. The complete 69-test workspace-state file,
all six close-decision tests including idle-history-free immediate closure, the
existing Linux bootstrap-history test, and all-target core/Linux clippy with
warnings denied passed. The boolean return from terminal submission remains
the existing debounced agent-presentation contract; the new ephemeral marker
does not cause unrelated redraws. Installed GNOME dogfood still needs to prove
that a newly opened untouched pane closes without a dialog. No broad
qualification was run or claimed.
