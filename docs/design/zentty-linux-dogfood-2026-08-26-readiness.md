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
