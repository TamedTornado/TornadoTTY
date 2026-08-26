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
low-contrast pointer remains a known GNOME cursor-theme interaction rather than
shipping a product workaround that harms selection. A future repair must keep
text-position precision while improving contrast; no full qualification was
run or claimed for the rejected experiment.
