# Zentty Linux dogfood report — fcitx input-method qualification

## Scope and acceptance boundary

This report tracks TamedTornado/zentty issue 67, a child of the native-input
and scaling epic in issue 8. The slice is complete only when real fcitx5 GTK4
composition passes through staged Zentty, Ghostty, and a real PTY on controlled
X11 and Wayland. Daemon startup, module discovery, injected-event success, or a
non-Ghostty GTK reproducer cannot establish product correctness. Existing IBus
cells remain independent evidence and must remain green.

The implementation plan is:

1. own a private D-Bus session, private fcitx5 daemon, pinned Cangjie engine,
   and private configuration inside each controlled display;
2. prove the installed GTK4 module and exact daemon identity;
3. run the same exact preedit, cancel, commit, focus-transfer, and active-pane
   destruction journey already required of IBus, with an exact PTY receipt;
4. add explicit X11 and Wayland cells to the authoritative matrix and negative
   tests for the environment owner; and
5. rerun both fcitx cells, both IBus cells, affected contracts, and all presently
   executable qualification cells before closure.

## 2026-08-21 — initial controlled investigation

### Discovery and raw evidence

- The host originally lacked the fcitx runtime. The controlled prerequisite was
  installed from the Ubuntu repositories: fcitx5 5.1.7, the fcitx GTK4 frontend
  5.1.1, and the Cangjie 5 table engine. The module is
  `/usr/lib/x86_64-linux-gnu/gtk-4.0/4.0.0/immodules/libim-fcitx5.so`.
- A private profile selected `keyboard-us` and `cangjie5`. A foreground
  `fcitx5 -D --disable=notifications` instance owned `org.fcitx.Fcitx5`,
  `org.freedesktop.portal.Fcitx`, and its compatibility names on the private
  session bus. Starting a remote controller before daemon settlement can race
  D-Bus activation and create a second daemon, so readiness must be proven
  before any controller call.
- `GTK_IM_MODULE=fcitx5` was rejected as the support setting after fcitx's own
  diagnostics identified `fcitx` as the documented GTK module ID. Linker
  diagnostics independently proved that GTK loaded `libim-fcitx5.so`; module
  presence was not the missing behavior.
- Early staged-product runs received literal `a ` in the PTY and
  `fcitx5-remote` reported state 0 with no current input method. This was kept
  as a failure rather than relabeled an environmental pass.

### Independent GTK control and failed hypotheses

- A minimal real GTK4 `GtkEntry` control initially reproduced literal input.
  The first attempts focused its X11 window before GTK had settled; allowing
  the mapped client to settle produced state 2, engine `cangjie5`, and the
  exact committed receipt `日`. Private D-Bus monitoring showed
  `CreateInputContext`, `FocusIn`, `ProcessKeyEventBatch`, and `Reset`. This
  established that the daemon, table engine, GTK module, and controlled X11
  input path work, but did not excuse the product failure.
- Repeating the staged-product run with the corrected module ID, a bounded
  daemon wait, a second pane, and X11 toplevel focus loss/regain still produced
  literal input. The product did create two fcitx input contexts and sent
  `ProcessKeyEventBatch`, but the D-Bus receipt contained no `FocusIn` for
  either context. This disproved daemon startup, engine selection, module
  loading, and toplevel focus as root causes.

### Diagnosis and avoided Ghostty patch

Ghostty's GTK surface can receive its initial focus callback while the fcitx
delegate is still connecting. That first focus transition is not replayed by
fcitx after its asynchronous context creation. Several candidate Ghostty
changes were built and exercised: replay from a main-loop idle, replay after a
50ms or 500ms timer, binding the client widget to the focused GL area, and
explicit focus-out/focus-in replay. None made the artificial never-transitioned
toplevel a valid fcitx client, so none was retained or pushed.

A controlled second GTK toplevel then drove a real post-initialization focus
loss and regain, which is also part of issue 67's required behavior. With the
original pinned Ghostty revision `281d7d7dbeab24c1a2d04f6d3c720c34dbfac645`,
the private D-Bus receipt immediately gained `FocusIn`, `fcitx5-remote`
reported state 2 and engine `cangjie5`, and the real Zentty PTY received the
exact committed character `日`. This proves fcitx support on the existing
Ghostty contract and avoids shipping a timing workaround. The permanent
controlled runner must own the second toplevel and assert the focus transition;
it may not treat an unfocused initial context as a product failure or bypass
the real lifecycle.

### Complete X11 journey and focus-harness repairs

The staged ReleaseSafe product subsequently passed the complete X11 journey:
preedit creation, Escape cancellation, exact `日` commit, transfer to another
real Ghostty surface, return composition, destruction of a pane with active
preedit, survivor composition, and ordinary window shutdown. A staged Rust GTK4
focus helper owns the required second toplevel; it reports compositor-active
rather than merely mapped, and runs with GTK's simple IM context so it cannot
steal the private fcitx context. Earlier runner versions raced mapping, treated
the outgoing context as the newly focused context, or activated only the first
surface. Each produced a real red and was repaired with an active-window
barrier, per-surface engine activation, and bounded D-Bus/GLib settlement.

An attempted D-Bus `FocusIn` counter was removed. fcitx can reuse or replace
contexts without emitting one new provider-internal call for every successful
GTK focus transfer. Requiring that implementation detail rejected journeys
whose product focus callbacks and exact PTY routing were correct. The lasting
oracle is product-visible focus plus exact real PTY text, not a daemon-private
call count.

## 2026-08-21 — Wayland lifecycle failure, upstream diagnosis, and repair

### Raw failure

The first complete Cage/Wayland journey delivered all four exact PTY receipts
(`pane-2:日`, `pane-1:1`, a second `pane-2:日`, and `pane-1:日`) but the product
segfaulted during ordinary final window destruction. A privileged debugger was
attached immediately before close because this host has no `coredumpctl` and
routes disabled core dumps through Apport. The exact main-thread stack began:

1. `gdk_popup_get_parent` in GTK4;
2. the Ubuntu `libim-fcitx5.so` GTK4 module;
3. `gdk_surface_destroy` / `gtk_window_destroy`;
4. Zentty `ApplicationShell::detach_and_close` and application shutdown.

This was not relabeled a pass merely because all PTY text was correct. The host
module was `fcitx5-frontend-gtk4` 5.1.1. The official fcitx5-gtk history contains
the June 2026 fix `bc09262d0dad397990012b227b96c6bc41d369ba`, whose own description
says a parent-surface signal could remain connected to a dangling GTK4 input
window, and release 5.1.7 contains that fix. Official 5.1.7, pinned at
`3b18d2ab7401d4233daf38bbc5896f1703685a43`, was built into an ignored test-only
prefix. `/proc/<product-pid>/maps` proves the staged product loads that exact
module during the controlled cell. The identical Wayland journey then exited
cleanly. Zentty does not replace or suppress the defective system library and
does not describe 5.1.1 as supported Wayland evidence.

The investigation also exposed portal noise unrelated to Zentty: Ubuntu's
`xdg-desktop-portal-gnome` repeatedly segfaults when D-Bus-activated inside the
minimal private Cage session before the GTK portal backend takes over. Those
messages name a different executable and PID. They remain visible evidence and
were not confused with, or used to excuse, the separately debugger-proven
Zentty/fcitx module crash.

### Product and Ghostty repairs

- Zentty shutdown now follows the already-ratified architecture order: detach
  widgets, release terminal surfaces, then destroy the GTK window. The previous
  implementation destroyed the GDK surface before releasing terminal instances,
  contradicting ADR 0001 even though it was not sufficient by itself to repair
  fcitx5-gtk 5.1.1's upstream dangling callback. The ApplicationShell ownership
  inventory names the separate detach and close operations so a future refactor
  cannot silently collapse that lifecycle boundary again.
- A candidate safe-adapter change replayed focus once on the next GLib idle
  turn after reparenting. Existing IBus Wayland qualification caught the flaw:
  an older surface's queued replay could steal focus back from a later request.
  The candidate was removed rather than adding another focus authority.
- fcitx intentionally commits preedit on focus loss/reset. That produced the
  exact failure `a日` when a pane with pending `a` lost focus: the stale preedit
  was committed as literal text before the later independent `日`. Ghostty commit
  `c8eedc856a5960196538bb41f5e282a746165d67` adds one scoped cancellation flag,
  explicitly resets on focus loss, discards only the reset-generated commit,
  clears core preedit, and leaves ordinary commits untouched. The discarded
  focus-timing experiment was not pushed.

### Permanent controlled evidence

`linux/fcitx5-gtk-test.lock` pins the official 5.1.7 source and
`linux/scripts/prepare-fcitx5-gtk-test-module` builds only its GTK4 module in
`build/linux-deps`. Preparation is flocked, hash-receipted, and cached; it does
not install or replace a host library. `with-supported-fcitx-gtk` validates the
receipt, selects that module for both controlled backends, and the product test
verifies its exact mapped path. The authoritative matrix now has separate `ime-fcitx-x11` and
`ime-fcitx-wayland` PASS cells rather than hiding fcitx inside the IBus cells.
The environment-owner contract test covers invalid timeouts, private identity,
sanitized module variables, child execution, daemon cleanup, and rejection of
an ambient X11 desktop.

The exact final focused receipts are:

- `rust-ime-composition-x11: PASS real-fcitx preedit-update cancel commit focus-transfer active-preedit-destruction real-pty`
- `rust-ime-composition-wayland: PASS real-fcitx preedit-update cancel commit focus-transfer active-preedit-destruction real-pty`

The unchanged IBus provider was then rerun on both controlled backends after the
deferred-focus candidate was removed. Both X11/Xvfb and Wayland/Cage passed the
same real-PTY composition, cancellation, transfer, active-preedit destruction,
and survivor-composition journey. This matters because it proves the fcitx
repair did not make provider-specific behavior the new focus authority. Full
matrix totals and the final Zentty commit are recorded only after all presently
executable cells complete.

### Pre-qualification contract repairs

The first workspace-wide test invocation ran inside the Codex filesystem
sandbox. Six existing agent-IPC tests failed at `UnixListener::bind` with
`Operation not permitted`; the identical suite passed outside that sandbox.
This was recorded as an execution-environment rejection, not repaired in
product code and not converted into a pass without the real rerun.

The architecture suite then correctly rejected the shutdown refactor because
the checked-in ApplicationShell responsibility inventory still named the old
combined `detach_and_close` method. The inventory now names both
`detach_for_shutdown` and `close_detached_window`, and its negative validator
also passes. Controlled public Ubuntu package policy now explicitly includes
fcitx5, its GTK4 and Cangjie5 packages, CMake, and Extra CMake Modules; the
literal untrusted-PR workflow package list remains contract-equal to that
reviewed manifest. The controlled-fcitx runner self-test also rejects an
unsupported pinned GTK module version rather than accepting any library placed
under `GTK_PATH`.

### Full-qualification run rejected

The first full `linux/tests/qualify-local` run was not a pass. Its matrix phase
took 2,135,860 ms and declared 179 PASS, 0 FAIL, 1 BLOCKED, 3 XFAIL, and 5
NOT_IMPLEMENTED cells, but the executable run rejected these cells:

- `ime-fcitx-x11` failed before the final fcitx publication barrier repair.
- `install-uninstall` and clean-checkout negative fixtures correctly rejected
  the uncommitted working tree. A dirty checkout is not packaging evidence.
- `agent-sleep-inhibition-x11` missed one child-exit acquisition in the crowded
  full run. Its isolated real-systemd-logind journey subsequently passed; it
  remains subject to the final clean matrix rerun.
- Debug X11 single- and multi-terminal Valgrind reports were absent after their
  producers failed, so suppression governance also failed. Missing evidence
  was not retained or reported as a pass.

The public-PR subset validator also exposed a stale policy reference: it named
the already-passing physical-Wayland cell as a public gap while the actual
blocked gap is fractional Wayland scaling. The manifest, validator, and
negative tests now agree on `fractional-scale-wayland`. Public CI remains a
check of committed work, not a release-approval authority.

### Deterministic fcitx activation

Fixed sleeps, one-shot `fcitx5-remote` activation, and a second helper-owned
fcitx context all failed intermittently. The daemon can report ready before the
provider context has been published on its private D-Bus tree, and switching an
engine before that publication is not durable. A second input context also
changed the system under test instead of proving the product context.

The retained harness polls the real private fcitx D-Bus tree for the product
input context, then performs the real compositor focus transfer and repeatedly
applies the idempotent open/engine selection until fcitx reports active state 2
with the Cangjie engine. It never substitutes harness readiness for PTY text.
Both X11 and Wayland journeys passed twice consecutively after this repair.

### Valgrind qualification architecture repairs

The failed full run exposed three independent harness defects rather than a
reason to broaden suppressions:

1. Product Valgrind phases inherited no private session bus, so GIO attempted
   desktop bus autolaunch and could deadlock in synchronous initialization.
   Non-API product phases now run inside `dbus-run-session` within the already
   private compositor environment.
2. `ZENTTY_RUST_STARTUP_ATTEMPTS` was declared but ignored by the Rust product
   smoke journey. It is now passed into the shared readiness helper and the
   helper rejects malformed values.
3. The X11 helper waited for `focus-pane` after focusing the initially selected
   pane. That log is emitted only when product selection changes, so it was an
   invalid assertion. The helper now proves the focused X window belongs to the
   real product; the subsequent exact PTY acknowledgement remains the terminal
   child-focus proof in physical-input cells.

Debug embedding-library logging then dominated Memcheck execution. Ghostty's
executable and C library honor `GHOSTTY_LOG`, but the alternate GTK library root
had not selected that same `std_options` policy. Ghostty commit
`13bed0225266d3c6e54e499dfae268f36a2770fe` reuses the existing policy with a
four-line, separately audited change; `GHOSTTY_LOG=false` now removes diagnostic
serialization without changing application behavior or memory instrumentation.

Finally, memory-lifecycle cells had accidentally accumulated physical XTEST
typing and external-resize qualification already owned by dedicated matrix
cells. Under Memcheck that orthogonal journey could consume the entire
300-second bound. Memory safety now runs the real terminal, PTY title, child
exit, window, surface destruction, and shutdown lifecycle without duplicating
physical-input/resize concerns. The default X11 product smoke remains physical;
only the memory producer opts out explicitly. This reduced scope is a removal
of duplicate qualification, not a fake component.

The first passive-X11 attempt exposed two stale assumptions and was rejected:
its fallback title literal still said `wayland`, then the runner required an
820x640 resize receipt even though only the physical branch performs that
resize. The title now derives from the actual backend, and resize evidence is
conditioned on the single shared `physical_journey` decision. Both repaired
branches are exercised by the successful real X11 receipt rather than accepted
by inspection alone.

All four focused Debug lifecycle profiles now report **PASS with reviewed
suppressions**, never an unsuppressed clean result. Their preserved raw versus
post-suppression totals are:

| Profile | Raw errors/contexts | Raw definite/indirect bytes | Post-suppression errors | Post-suppression definite/indirect bytes |
|---|---:|---:|---:|---:|
| single Wayland | 220 / 132 | 58,664 / 118,520 | 0 | 0 / 0 |
| interaction Wayland | 340 / 252 | 83,264 / 176,495 | 0 | 0 / 0 |
| single X11 | 679 / 591 | 136,488 / 299,165 | 0 | 0 / 0 |
| interaction X11 | 639 / 551 | 75,328 / 171,247 | 0 | 0 / 0 |

The shutdown change altered which external Fontconfig/Pango cache descendants
remain in each complete process receipt. Governance rejected every new count
outside the prior reviewed ranges. The manifest ranges were widened only to
include the newly retained raw receipts, including per-process fork/exec
summaries; no suppression pattern was broadened. The focused governance runner
and its stale/increase/out-of-scenario/untracked-rule negative suite pass. Full
qualification is still pending a clean-checkout rerun and is not claimed here.

During the final controlled fcitx Wayland rerun,
`xdg-desktop-portal-gnome` repeatedly exited with signal 11 inside the nested
Cage-on-X11 session. The GTK portal backend activated, the real fcitx GTK module
loaded, and the exact composition/lifecycle/PTy journey passed. This is retained
as external-environment noise and uncertainty, not called a Zentty pass in its
own right and not hidden as infrastructure absence.

## AI disclosure

Investigation and implementation assistance were provided by OpenAI Codex
under Jason Maskell's direction. Any upstream Ghostty proposal must be reviewed,
understood, edited, and submitted by a human contributor in accordance with the
project's contribution policy.
