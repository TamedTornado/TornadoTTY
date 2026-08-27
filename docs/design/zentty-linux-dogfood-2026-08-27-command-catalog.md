# Zentty Linux dogfood: authoritative command catalog

Date: 2026-08-27
Tracker: GH-125
Parent: GH-7

## Dogfood discovery

GH-111 exposed an architectural defect rather than an isolated missing menu
item: Linux registered actions in `ActionRouter::ACTION_SPECS`, described
keyboard commands in `shortcut_registry::COMMANDS`, and manually rebuilt
ordinary command-palette items in
`ApplicationShell::command_palette_action_items`. Add Pane existed and was
routable, but disappeared from one duplicated presentation table. The source
application avoids this class of omission by building ordinary palette items
from `AppCommandRegistry.definitions` and resolving availability through the
same command/action path.

The GH-125 audit also found a separate shortcut registry. It owns stable
source-compatible command IDs, categories, and default bindings, while
validating every shortcut action reference against `ActionSpec`. GH-125 does
not move those shortcut-specific concerns. The repair instead makes
`ActionSpec` authoritative for registered action schema, live availability,
and ordinary command-palette presentation, removing the palette-only duplicate
that caused the omission.

## Ownership design

- `ActionSpec` remains the single registered-action authority.
- An action catalog entry is explicitly one of:
  - an ordinary palette command with title, description, keywords, recent
    eligibility, parameter schema, and availability;
  - a contextual action whose existing runtime owner supplies target-specific
    presentation and parameters; or
  - a reviewed non-palette action with a non-empty exclusion reason.
- Ordinary palette items are generated from the catalog and read enabled state
  from the installed `gio::Action`; they never recompute availability.
- Contextual panes, Settings sections, task runners, development servers, Open
  With targets, and fleet targets remain with their current owners. Each must
  reference a registered action with the exact parameter schema.
- Palette buttons continue to activate `workspace.<action>`; no direct handler
  or second dispatch path is introduced.
- The giant `ApplicationShell::command_palette_action_items` list is deleted.
- `shortcut_registry::COMMANDS` remains the authority for shortcut-only command
  IDs, categories, and defaults. Its action references remain closed-world
  validated against `ActionSpec`; consolidating its settings copy is not a
  prerequisite for fixing palette drift and is not claimed here.

## Test-first contract

Focused closed-world tests must fail for:

1. duplicate action names, command IDs, or palette titles;
2. an ordinary palette action with a parameter, missing presentation field, or
   missing registered GTK action;
3. an action without an ordinary, contextual, or reviewed-exclusion
   disposition;
4. a contextual producer whose target parameter shape differs from its
   registered schema;
5. a palette item's enabled state differing from the live `gio::Action`;
6. an ordinary palette activation that bypasses the named ActionRouter path;
7. a shortcut references an action absent from the authoritative action
   catalog;
8. loss of deterministic matching, grouping, recent-action behavior, or source
   Add/Split pane terminology.

The existing `linux/tests/rust-source-ux-x11` journey will be extended rather
than adding a harness. It must physically open the real GTK palette over a real
Ghostty PTY, search and invoke representative worklane, pane, settings,
task/server/Open-With or agent actions, and prove a disabled result cannot
execute. Only focused GH-125 checks are required; neither full qualification
nor operator deployment belongs to this issue.

## Claim boundary

Completing this catalog promotes `commands.palette-routing` from `PARTIAL` to
`IMPLEMENTED`. It does not complete required-later agent adapters, deferred
updates/diagnostics, sustained operator dogfooding, or full Linux
qualification.

## Implementation and evidence

The repair added an explicit `PaletteDisposition` to every registered
`ActionSpec`: ordinary metadata, a named contextual owner, or a non-empty
reviewed exclusion reason. `ApplicationShell` now projects ordinary rows from
the installed `ActionRouter`, appends its existing contextual producers, and
validates every resulting target against the registered parameter schema and
live `gio::Action` enabled state before presentation. The 71-entry
`command_palette_action_items` duplicate was deleted.

Focused evidence on 2026-08-27:

- `rustfmt` and `shellcheck linux/tests/rust-source-ux-x11`: PASS.
- ActionRouter closed-world suite: 8 PASS, 0 FAIL.
- Core deterministic palette suite: 9 PASS, 0 FAIL.
- The full `zentty-linux` unit binary reached 342 PASS before its real kernel
  listener test was denied by sandbox confinement. The exact unchanged test
  was rerun with permission to bind a loopback listener and inspect `/proc`: 1
  PASS, 0 FAIL. No test or requirement was weakened.
- After the key-routing repair, the final permission-complete binary run was
  343 PASS, 0 FAIL, and 2 controlled-display tests ignored by their existing
  gates. Clippy passed for `zentty-linux` and `zentty-core` with warnings denied.
- Staged ReleaseSafe build: PASS. The dependency publication-age audit reported
  91 packages and 0 exceptions; package-notice collection completed.
- `ZENTTY_SOURCE_UX_COMMAND_CATALOG_ONLY=true` through the existing
  `linux/tests/nested-x11` actor: PASS with private X11, real GTK, real Ghostty,
  real PTY, and physical keys. A previously omitted registered action (`Reload
  Configuration`) executed through its named action and applied to Ghostty. A
  live-disabled `Resize Pane Up` row rendered disabled; Return produced neither
  a resize nor an execution receipt nor a successful dismissal. Final nested
  session: `519e98dc3b91a1464ca569b3d1b3ab0b173d76d0ddbeda7a34c10440b8140acd`.

The first timer-free disabled-result run failed. Return on the insensitive row
did not execute or dismiss, but it escaped the search-entry-scoped key
controller and reached the real PTY. The deterministic shell title changed to
`focus:`, proving an empty command had been submitted. Moving the existing
controller to the palette overlay root was a failed repair: Ghostty and that
overlay are siblings, so the key still escaped. The final repair delegates
palette navigation from the existing window-level physical-key router and adds
an explicit disabled-activation receipt. Text keys continue to the search
entry, while Return, Down, and Escape stop before Ghostty. The regression now
also pins the PTY title across Return; no sleep, new timer, or parallel input
router is involved.

The first window-level implementation also failed under the real journey: it
held an immutable `ApplicationShell` `RefCell` borrow while the palette button
synchronously activated its GTK action, whose handler needs a mutable shell
borrow. Rust aborted on that reentrant borrow. The repaired router clones the
lightweight palette view while inspecting visibility, ends the shell borrow,
and only then dispatches the key/action.

## Discovery outside GH-125

The unmodified full `rust-source-ux-x11` path stopped before the palette at the
`empty-dark-x11` zero-tolerance pixel comparison. Inspection showed an honest
stale baseline: the current, already-landed product displays the pane working
directory and project-folder controls that are absent from the reviewed image
(11,903 changed pixels). GH-125 does not change those pixels. The baseline was
not silently accepted, bypassed, or rewritten. The focused mode above was added
to the same real-product actor so command routing can be exercised independently
while the visual-parity owner reviews that unrelated evidence.
