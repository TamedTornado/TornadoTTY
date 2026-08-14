# Zentty Linux dogfood: GUI readiness under matrix load

Date: 2026-08-14

## Discovery

Three complete local qualification runs after the theme-preview repair failed
in different declared-PASS GUI cells. Every failed cell passed immediately in
isolation, but a five-journey concurrent reproduction made the bookmark failure
repeat. This established a test synchronization defect rather than permission
to relabel environmental absence as a pass.

The affected journeys mixed up three independent boundaries:

- a GTK widget reporting focus;
- the compositor delivering a later physical key event; and
- the product acknowledging the state transition caused by that event.

Several journeys asserted immediately after input, reused an old receipt, or
opened a feature before the shell had published the pane working directory.
The bookmark journey additionally reactivated its X11 toplevel after reaching
a popover button; Openbox could restore the popover's initial search focus as
part of that activation.

## Repair

- Appearance opacity now paces focus traversal, snapshots the application
  receipt count, and requires a new apply receipt after physical Home. Home is
  idempotently retried only while no acknowledgement exists.
- Worklane Peek snapshots the exact pane-preview receipt before repeated Tab
  and waits for a new receipt rather than reading an older preview immediately.
- Open With paces Wayland focus traversal and requires a newly counted primary
  application receipt; the idempotent End selection is retried only before
  acknowledgement.
- Task Runners does not open its first palette until the real shell title
  boundary has published the project working directory used for discovery.
- Bookmark activation no longer reactivates the X11 toplevel after widget
  focus. It retries Return only while the modal window has not mapped, then
  waits for the modal entry's new idle-focus receipt without risking an empty
  submission.
- The orchestration contract now rejects removal of these acknowledgement and
  readiness guards.

No product component was replaced with a fake. The tests still drive the real
staged Rust/GTK/Ghostty product through physical X11 or Wayland input, real
shell working-directory reporting, real modal windows, and persisted files.

## Focused evidence

The repaired journeys passed individually on their applicable controlled
compositors: source UX X11, bookmark import/export X11, Open With X11 and
Wayland, shortcuts/settings X11 and Wayland, Task Runners X11 and Wayland, and
bookmark save/restore Wayland. A subsequent five-journey concurrent stress run
also passed source UX X11, bookmark import/export X11, Open With Wayland,
shortcuts/settings X11, and Task Runners Wayland together.

Full local qualification remains required before this record can claim the
implemented local suite passed. The declared matrix still contains BLOCKED,
XFAIL, and NOT_IMPLEMENTED cells, so even a clean executable run will not imply
release or full Linux qualification.

## First full rerun and follow-up

The first complete four-worker rerun passed every previously moving failure
except bookmark management X11, which again failed before mapping the name
dialog. Reproducing the management and import/export bookmark scenarios side by
side isolated the remaining distinction: management uses the intentional
no-window-manager X11 profile, while import/export starts controlled Openbox
for the native chooser. In the no-WM profile, modifier release sent to the
implicit active target was not reliable under load. Bookmark activation now
uses `xdotool --clearmodifiers` and explicitly targets the mapped Zentty window
only in the no-WM profile; the Openbox profile continues using its focused
active window without reactivation. A concurrent rerun of bookmark management,
bookmark import/export, source UX, and settings then passed all four journeys.

A second complete qualification run is required for the final machine receipt.

## Final qualification receipt

The second complete four-worker run passed every presently executable support
and matrix cell in 623.640 seconds. The machine summary now reports implemented
local suite passed, product boundary qualification passed, and qualification
host retired. Release and full Linux qualification remain not passed because
the authoritative declared totals are **126 PASS, 0 FAIL, 7 BLOCKED, 1 XFAIL,
and 22 NOT_IMPLEMENTED**.

Debug IBus-focus Valgrind is **PASS with reviewed suppressions**, not an
unsuppressed clean result. Its preserved raw receipt contains 427 errors in 427
contexts, 6,160 definite bytes, and 41,428 indirect bytes. Post-suppression
totals are zero errors/contexts and zero definite/indirect bytes, with all 427
errors/contexts accounted for by the reviewed effective suppression set. The
suppression-governance cell passed and the raw and suppressed receipts retain
independent hashes in the machine summary.
