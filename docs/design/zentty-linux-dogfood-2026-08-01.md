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

Baseline qualification is complete for the unchanged Ghostty application.

- The public Zentty and Ghostty forks exist and are locally configured with
  their source remotes.
- Zentty pins Ghostty revision `4e9fe4bb5` on `zentty/smooth-scroll`.
- Static inspection found `GhosttySurface` implemented as an `Adw.Bin` with a
  `GtkGLArea` and a full GTK terminal interaction stack.
- Static inspection found 18 direct `Application.default()` dependencies in
  `src/apprt/gtk/class/surface.zig` plus a direct `GhosttyWindow` ancestor
  dependency.
- The development host did not initially provide Zig, GTK4 development
  metadata, or libadwaita development metadata. The baseline was built with
  checksum-verified Zig 0.15.2, Blueprint Compiler 0.16.0 at tag commit
  `04ef0944`, GTK 4.14.5, and libadwaita 1.5.0.
- Unchanged debug build passed:
  `zig build -Doptimize=Debug -Dcpu=baseline -fno-sys=gtk4-layer-shell`.
  `ghostty +version` reported GTK runtime 4.14.5, libadwaita runtime 1.5.0,
  OpenGL, X11, and Wayland support.
- The full unchanged regression command passed:
  `zig build test -Doptimize=Debug -Dcpu=baseline -fno-sys=gtk4-layer-shell`.
  The warning output was produced by intentional negative-path tests; the
  command exited zero.
- A native Wayland smoke test created and realized the GTK surface, created an
  OpenGL 4.6 EGL context, started a real PTY child, observed its zero exit after
  2002 ms, unrealized the surface, and exited zero.
- An X11/Xwayland smoke test exercised the same path with
  `GDK_BACKEND=x11`; it selected the X11 protocol, realized OpenGL 4.6,
  observed the PTY child's zero exit after 1999 ms, unrealized, and exited
  zero.

## Incidents and repairs

### Spike root was initially below the Zig module boundary

- **Observation:** The first spike compile rejected
  `@import("../class/surface.zig")` as an import outside the module path.
- **Consequence:** The alternate host did not compile, so it had not yet
  exercised Ghostty code.
- **Diagnosis:** Zig scopes relative imports to the root module's directory;
  placing the executable root below `src/apprt/gtk` prevented it from
  importing the rest of Ghostty's source tree.
- **Repair:** Moved only the executable root to `src/gtk_embed_spike.zig` and
  retained the GTK-specific implementation and build step isolation.
- **Evidence:** The next build compiled 91 of 93 steps and launched the host.
- **Outcome:** Harness construction issue resolved; no production API change.

### Plain GtkApplication cannot construct GhosttySurface

- **Observation:** The minimal host activated a plain `GtkApplication`, then
  panicked while executing `Surface.new(.none)`.
- **Consequence:** Ghostty's existing GTK widget cannot currently be embedded
  by another GTK application, despite GTK, GObject, Blueprint, and OpenGL
  already being portable.
- **Diagnosis:** Template initialization queried the surface's `key-sequence`
  property. `getKeySequence` called `Application.default()`, which runtime-
  checked the process default application as `GhosttyApplication`; the actual
  default was the host's `GtkApplication`, so the optional cast was null and
  the forced unwrap panicked at `class/application.zig:232`. This is the first
  dynamically proven instance of the 18 static process-global dependencies.
- **Repair:** Not yet applied. The extraction will introduce explicit runtime
  ownership for a surface instead of weakening or bypassing the GObject type
  check.
- **Evidence:** The debug stack passed through `getKeySequence`, the GObject
  property getter, `gtk.Widget.initTemplate`, `Surface.new`, and the spike's
  activation callback. The process exited 1, as expected for this gate.
- **Outcome:** Reproduced and diagnosed; this is the first source-change gate.

### Baseline display access was unavailable inside the filesystem sandbox

- **Observation:** The first Wayland smoke command exited 1 with
  `Gtk: Failed to open display`.
- **Consequence:** No compositor claim could be made from the sandboxed run.
- **Diagnosis:** The managed command sandbox could not access the user's
  compositor socket. The Ghostty binary had already built successfully.
- **Repair:** Re-ran the identical executable and backend selection with
  explicit permission to access the live graphical session.
- **Evidence:** Both the Wayland and X11/Xwayland runs then selected their
  requested protocol, realized the OpenGL surface, ran and reaped a real PTY
  child, unrealized the surface, and exited zero.
- **Outcome:** Harness/environment issue resolved; no Ghostty source change.

## Current next gate

Replace the surface's first process-default lookup with explicit runtime
ownership, then repeatedly extend the same host until a real PTY initializes,
renders, exits, and tears down. Zentty application code remains out of scope at
this gate.

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
