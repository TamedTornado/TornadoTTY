# Zentty Linux port dogfood field report

Date: 2026-08-01

Status: **in progress**. This is a contemporaneous field report. Entries record
what was observed while extracting Ghostty's GTK surface, the consequence for
the Linux port, the repair, and the evidence available at the time. A patch is
not called successful until focused tests and the live Linux host demonstrate
the intended behavior.

## Scope and topology

The project is building a Linux Zentty application around Ghostty's existing
GTK/OpenGL terminal surface rather than reimplementing terminal emulation or
rendering.

- Product fork: `TamedTornado/zentty`, branch `linux/port`
- Engine fork: `TamedTornado/ghostty`, branch `zentty/gtk-embed`
- Zentty baseline: `dedene/zentty` at `6e021b0`
- Ghostty baseline: `dedene/ghostty` at `4e9fe4bb5`
- Official Ghostty base recorded by Zentty: `19e20f766`
- Required Zig version: `0.15.2`
- Development host: Ubuntu 24.04, native Wayland session
- Alternate display backend target: X11 through GDK/Xwayland

The upstream Ghostty application and the downstream embedding host are separate
claims. An unchanged Ghostty build establishes the baseline. The alternate host
must independently prove surface creation, PTY operation, input, resizing,
multiple surfaces, focus, and teardown.

## Acceptance and evidence standard

This port uses integration qualification as its primary credibility gate. Each
milestone must publish reproducible commands and concise result receipts for
the upstream Ghostty regression suite, a real alternate GTK host, Wayland and
X11, multi-surface lifecycle and stress behavior, and applicable live desktop,
IME, scaling, clipboard, and GPU checks. Later Zentty milestones must exercise
the packaged application end to end.

An environment that was not run is a documented gap, not a pass. Screenshots
may supplement evidence but cannot replace semantic assertions, process state,
test receipts, leak checks, or observed live behavior.

## Qualification baseline

Baseline qualification has started but is not complete.

- The public Zentty and Ghostty forks exist and are locally configured with
  their source remotes.
- Zentty pins Ghostty revision `4e9fe4bb5` on `zentty/smooth-scroll`.
- Static inspection found `GhosttySurface` implemented as an `Adw.Bin` with a
  `GtkGLArea` and a full GTK terminal interaction stack.
- Static inspection found 18 direct `Application.default()` dependencies in
  `src/apprt/gtk/class/surface.zig` plus a direct `GhosttyWindow` ancestor
  dependency.
- The development host did not initially provide Zig, GTK4 development
  metadata, or libadwaita development metadata. Toolchain installation and the
  unchanged build remain the first executable gate.

## Incidents and repairs

No implementation incident has yet reached diagnosis or repair. New entries
must be added here as soon as a baseline, embedding, integration, or live Linux
failure is observed.

## Current next gate

Install the pinned Zig and Linux GTK development toolchain, build unchanged
Ghostty at `4e9fe4bb5`, and validate it under both Wayland and X11 before making
embedding changes.

The first architectural experiment will use a minimal Zig/GTK host. Its purpose
is to determine the smallest explicit runtime context that can replace the
surface's process-global `GhosttyApplication` assumptions. It is not initially
a public ABI and is not initially intended for upstream submission.

## AI disclosure

Initial repository analysis, implementation assistance, and this report were
prepared with OpenAI Codex under Jason Maskell's direction. Any proposed
upstream Ghostty communication or contribution must be reviewed, understood,
edited, and submitted by the human contributor in accordance with Ghostty's
contribution policy.
