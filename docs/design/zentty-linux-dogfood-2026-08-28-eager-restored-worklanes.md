# Eager restored-worklane initialization

## Request

Jason reported a real two-relaunch failure mode: an unvisited restored worklane
did not start its saved Codex resume command, and the next clean snapshot could
therefore lose the still-pending resume intent. The durable-intent repair remains
required regardless of launch policy. Jason additionally requested a Settings
switch that initializes every restored worklane even when it is never visited.

GH-136 owns the setting. GH-137 owns the broader register of Linux-originated
enhancements that may later be surfaced to the macOS maintainer.

## Design

- `start_restored_sessions_in_background` is persisted under `[restore]` and
  defaults to `false`.
- With the default lazy policy, inactive worklanes retain their accepted restore
  intent but do not start PTYs until they are visited.
- With the eager policy, the existing GTK background host mounts every live pane
  frame outside the active worklane during initial setup. Realizing those frames
  starts ordinary shells and supported agent resume commands through the same
  Ghostty lifecycle used for a visible visit.
- The active worklane remains mounted by the ordinary layout and retains keyboard
  focus. There is no timer, duplicate surface model, alternate restore store, or
  synthetic agent launch path.

## Discovery and correction

The first draft iterated only agent restore launches. That was narrower than the
requested behavior: “initialize every worklane” includes ordinary shells as well
as Codex panes. It also repeated an older ineffective assumption that the
agent-command staging map remained populated after surface construction; that
map is consumed while building each surface.

The corrected boundary is the pane runtime's live surface inventory after
initial surface construction. The active worklane's pane IDs are excluded because
its real layout already mounts them. All remaining startup panes belong to
inactive restored worklanes and are mounted in the existing hidden host.

## Evidence plan

Focused configuration tests cover the default, TOML parsing, type rejection, and
comment-preserving persistence. The existing real Settings actor physically
reaches the switch, toggles it in both directions, and checks the written config.

The existing staged ReleaseSafe restore actor runs under controlled X11 with real
GTK, Ghostty surfaces, PTYs, filesystem state, and controlled Codex processes. Its
eager scenario requires:

- two hidden Codex resumes authenticate without any worklane visit;
- two ordinary shell panes in inactive worklanes become terminal-ready;
- the active worklane and keyboard focus remain unchanged;
- an immediate hidden resume failure preserves topology and becomes visible as
  the existing recovery UI when the user later visits that pane.

The lazy two-relaunch actor remains mandatory in the focused rerun so adding the
preference cannot silently change the default policy.

## Upstream classification

This preference is Linux-originated but platform-neutral in concept. It belongs
in [`docs/linux-originated-enhancements.md`](../linux-originated-enhancements.md).
No communication with `dedene/zentty` is authorized by this record.
