# Zentty Linux external AT-SPI dogfood

Date: 2026-08-23
Tracking: GH-86, parent GH-82

This append-only record starts before GH-86 implementation. The ratified plan
is `docs/design/linux-external-atspi-plan.md`.

## Starting state

- Controlled X11 and Wayland product actors pass real PTY, input, persistence,
  and in-process `GTK_A11Y=test` widget semantics.
- Earlier investigation created a valid private accessibility service and
  proved it with a stock GTK application, but the staged Zentty process did not
  publish an application root.
- Experiments that registered a second application or activated Ghostty's
  private application did not produce an external root and were reverted. No
  unproven lifecycle change remains in product code.
- Ghostty currently creates its internal custom `GtkApplication` before the
  Rust host initializes GTK. The embedding ABI does not expose a host-owned
  application or an explicit registration lifecycle.

## Decision before changes

Reproduce the boundary with a durable separate Rust inspector inside the
existing `rust-worklane-accessibility` journey. Preserve a stock GTK control,
then make the smallest owning-project repair. Passing in-process metadata is
not external evidence, and absence of the private bus/registry is not a pass.

## Discovery: the current staged product exports a real tree

- The new inspector uses `libatspi` from a separate Rust process and filters by
  both application identity and exact process ID. It cannot accept a supplied
  JSON tree or call Zentty internals.
- A stock Zenity dialog first proved the private session bus, accessibility
  bus, registry, GTK bridge, and compositor environment. The inspector then
  found the current staged Zentty window, worklane, pane row, pane controls,
  named GTK actions, selected states, and focused terminal descendant.
- Therefore the old conclusion that the embed boundary prevented all external
  export is no longer true of the current product. The old raw binary no longer
  exists, so this record does not invent which intervening shell change made
  the window exportable. The durable current reproducer replaces that prose as
  authoritative evidence.
- The remaining real defect was identity: the AT-SPI application root was
  named `ghostty`, even though its child frame and complete hierarchy belonged
  to Zentty. Matching only the `Zentty` frame title would have hidden this
  defect and allowed the wrong application root to pass.

## Ownership decision: no native Ghostty patch is required

- Ghostty's embedded runtime owns and installs the process-default
  `GApplication` before the Rust host may initialize GTK. GTK's real AT-SPI
  bridge exports that application and every Zentty-owned child widget.
- The host product owns the public desktop identity. Before GTK initialization
  and before the private application is registered, the existing standard GIO
  object permits its application ID to be changed. Zentty's safe Ghostty
  adapter now performs that narrow handoff and restores the GLib program and
  application names.
- This produces an external application root named `zentty` with the exact
  staged-product PID. It does not register a competing application, run
  Ghostty's standalone lifecycle, add a shadow accessibility model, or change
  Ghostty native code. If a future Ghostty version stops installing that
  object, startup fails explicitly rather than silently reverting identity.
- Because the owning defect is host identity policy rather than terminal
  behavior, a Ghostty pull request would add API surface without fixing a
  Ghostty-owned problem. The minimal upstream-reviewable Ghostty change for
  GH-86 is therefore no change.

## Harness failures and repairs

- The first inspector revision initialized and exited `libatspi` on every poll.
  Reinitializing the process-global client corrupted its cache and emitted GLib
  criticals. The inspector now initializes exactly once and balances every
  full-transfer desktop, accessible, state-set, and action reference.
- The first tree search only traversed fields named `children`; it printed the
  correct Zenity node but failed to see it below the receipt's `applications`
  array. Search now recursively visits JSON objects and arrays, with unit tests
  proving unrelated scalar hints cannot impersonate accessible names.
- The first runner assertions used invalid jq recursive-descent syntax and then
  counted the compound Pane Actions control as one node. GTK correctly exports
  its outer push-button and inner toggle-button roles. The assertions now pin
  that real two-node representation before and four-node representation after
  adding a second pane.
- Headless Weston has no input seat, so it correctly exposed no active/focused
  states. The Wayland cell now uses the already-maintained nested Cage/X11 seat
  harness rather than weakening focus assertions or turning environmental
  absence into PASS.
- GNOME's portal backend repeatedly crashed when auto-activated inside nested
  Cage. The GTK portal fallback remained available and both the control and
  product completed. This is noisy environment-owned behavior, not used as
  accessibility evidence and not suppressed from raw output.
- The first external receipt exposed a drag label but did not make the real
  drop destinations externally distinguishable. The existing GTK drag source
  and drop controllers remain authoritative; their owning widgets now also
  expose exact source/destination descriptions and group roles. The separate
  inspector checks the terminal source, worklane destination, and pane-canvas
  destination rather than asserting against a shadow drag model.
- One Wayland run observed the application root before GTK had published the
  selected pane row. A one-shot assertion made normal asynchronous tree
  construction look like a product failure. The actor now polls the external
  registry for the entire required contract with a fixed bound; it does not
  accept a root-only or partially constructed tree.
- Running the complete worklane cell after `rust-agent-ipc` exposed an ordering
  defect that an isolated actor run did not: the earlier journey could leave a
  desktop portal attached to the controlled display's runtime, and the private
  accessibility control then raced that unrelated service. `private-atspi`
  now owns a child runtime directory, proves the AT-SPI socket there, preserves
  the real Wayland socket by absolute path, unmounts only its exact portal mount,
  and removes only its owned directory. The exact X11 and Wayland cell commands
  both passed after this repair.
- The architecture validator also found responsibility-contract drift left by
  the already-landed GH-87 shell changes: new sidebar motion, live peek, pane
  sizing, topology, and drag helper members were absent from the closed-world
  ownership manifest. The manifest was reconciled to the existing code; no
  second implementation or ownership layer was added. A full-workspace Clippy
  probe separately reported pre-existing `too_many_lines` in
  `worklane_peek::render` and exact floating-point comparisons in
  `workspace_state`; neither is introduced or hidden by GH-86. Focused changed-
  package Clippy is clean (the repository's existing explicit allowance for
  the shell binary's `too_many_lines` remains).

## Focused real-system result

- Controlled X11 passed: stock GTK control, exact Zentty application/PID,
  active frame, selected worklane and pane, externally focused descendant,
  named pane controls/actions, an AT-SPI-invoked **Add Pane Right**, and the
  resulting live two-pane external topology.
- Controlled input-capable Wayland passed the same assertions through a real
  nested seat.
- Both journeys reject a real but mismatched application identity, reject the
  old PID after process exit, and retain the in-process GTK widget contract as
  supporting—not substitutive—coverage.
- Runner tests reject an unknown/mock input mode, a fabricated receipt path, a
  missing accessibility socket, missing required node, missing selected state,
  false application identity claim, and an uncontrolled developer display.
- The authoritative product-worklane matrix cells now execute this external
  evidence. Matrix status counts are unchanged because two existing PASS cells
  gained the previously missing external contract; no new PASS was fabricated.
- Final focused validation passed: ReleaseSafe rebuild; inspector unit tests;
  shell checks; negative runner; architecture, matrix, and feature-inventory
  validators; changed-package Clippy; artifact preparation; and the exact
  controlled X11 and input-capable Wayland worklane cell commands (including
  their preceding real agent IPC journey).
- Authoritative matrix totals at this commit are **199 PASS, 0 FAIL, 0 BLOCKED,
  3 XFAIL, and 2 NOT_IMPLEMENTED** (204 cells). These are declared statuses,
  not a claim that the entire matrix was rerun for this issue-sized change.
- Full Linux qualification is not claimed. The matrix still contains its
  separately tracked XFAIL and NOT_IMPLEMENTED cells.
