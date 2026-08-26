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
