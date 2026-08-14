# Zentty Linux dogfood: post-settings human QA

Date: 2026-08-14
Issues: follow-up to GH-20, GH-37, and the Appearance/Shortcuts settings work

This record is chronological. It begins with operator-visible failures found
after the completed settings-child batch was launched against the operator's
real persisted workspace. A repaired build is not accepted until the failures
have focused regression coverage and real staged-product compositor journeys.

## Operator discoveries

1. Selecting any ordinary bundled theme from Appearance opened Ghostty's
   **Configuration Errors** dialog. `Abernathy` was representative: Zentty's
   catalog correctly discovered it under the staged
   `share/zentty/ghostty/themes` directory, but the generated Ghostty config
   stored only `theme = Abernathy`. Embedded Ghostty had no application resource
   directory and therefore searched only the user's Ghostty theme directory.
   Earlier real journeys exercised the one privately installed fallback theme
   and a user-owned override, so they never selected an ordinary bundled theme.
2. At the Settings window's real default width, the Shortcuts page allocated a
   fixed 330-pixel command browser beside a keyboard preview whose eleven-key
   rows imposed 38-pixel minimum key widths plus padding. The right-hand keys
   were painted outside the available detail viewport and clipped. Existing
   tests proved shortcut behavior but did not assert the rendered default-width
   boundary.
3. Linux projected the discovered project icon beside every sidebar pane. The
   source application uses project icons in window chrome and Worklane Peek,
   not as a repeated prefix for each pane row. The repeated icon was an invented
   Linux projection, visually displaced the pane marker/title, and had been
   encoded into the first Linux icon journey instead of being challenged
   against the source.

## Repair constraints

- Keep source-compatible theme names in Zentty's own config. The generated
  Linux Ghostty config may use an absolute staged path for a bundled theme,
  because Ghostty explicitly supports absolute theme paths and the embedded
  runtime currently has no resource-directory authority.
- User-installed themes retain precedence and name-based Ghostty resolution.
  Path resolution must not allow a configured name to escape the staged theme
  directory or turn a non-file into trusted product content.
- Shortcuts must remain usable at the documented default window size and retain
  a bounded scrolling fallback at smaller sizes. This is a layout repair, not a
  reason to remove the keyboard feature.
- Remove only the invented sidebar icon projection. Preserve the existing
  resolver, cache, window-chrome projection, Worklane Peek projection, and
  `show_project_icons` setting.

## Architecture-governance discovery and repair

While preparing these QA repairs, the ownership validator required SHA-256
updates for every touched source file. Operator review correctly challenged
what those hashes established. They established only byte-for-byte identity;
they did not prove behavior, ownership, integrity beyond Git, or architectural
quality. A presentation-only change therefore required manually blessing a new
checksum, while the checksum could not explain whether the change introduced a
second authority. This was brittle test-harness accretion.

The whole-file hashes and all hash comparisons have been removed from the
machine-readable ownership contract and validator. The useful checks remain:
complete and unique field, method, action, and owned-function inventories;
declared owners; single-authority declarations; lifecycle ordering; and the
qualification-cell linkage. Focused source constraints now detect the concrete
duplicate-authority risks formerly hidden behind the hashes: additional GLib
main loops, Ghostty runtimes, authenticated-agent receivers, enrichment
generations, settings writers, persistence stores, tmux state/stores, and pane
surface registries.

The validator self-test now proves both sides of the policy. An unrelated source
comment passes without changing the contract, while injected duplicate
authorities fail with responsibility-specific diagnostics. It also rejects any
attempt to reintroduce `source_sha256` or `owned_source_sha256` into the
contract. Positive validator, negative/self-test suite, ShellCheck, JSON parse,
and diff-whitespace validation all pass after the repair.

## Repair evidence

- Focused real staged-product journeys passed under controlled X11 and Wayland
  for both settings/theme behavior and project icons. The settings journey now
  selects an ordinary bundled `Abernathy` theme, observes the exact canonical
  staged path in Ghostty's live config, restarts the product, and proves that
  startup projection happens before Ghostty initialization. The icon journey
  proves real image decoding remains visible in Window Chrome and Worklane Peek
  while the pane-row prefix is absent.
- The first keyboard-layout receipt ran from an idle callback before GTK had
  allocated the widget and falsely reported `detail=0 keyboard=0 fits=false`.
  The journey rejected that evidence rather than accepting an environmental
  absence. Moving the receipt to the first frame callback produced real
  allocations and both controlled compositor journeys passed with `fits=true`.
- The first mutation run inside the filesystem sandbox failed its unmutated
  baseline because the existing real `/proc` listener correlation test could
  not create the required kernel listener. It was rerun with the required test
  permission; environmental absence was not converted into a pass.
- The first executable mutation run found three surviving boundary mutations:
  two unsafe theme-path condition combinations and the distinction between a
  missing bundled file and another filesystem inspection error. Tests were
  strengthened with existing nested content, `..`, and a deterministic symlink
  loop. The focused rerun tested 19 mutants: 17 caught and 2 compiler-rejected
  as unviable, with no missed or timed-out mutants. Every run used
  `--gitignore=true`; the ignored multi-gigabyte build tree was never copied.
- `cargo fmt --check`, workspace all-target check, strict workspace Clippy,
  the complete workspace test suite, both architecture validators, ShellCheck,
  and diff-whitespace validation passed.

The final local qualifier ran every presently executable support and matrix
cell in 534.820 seconds. Declared totals remain **126 PASS, 0 FAIL, 7 BLOCKED,
1 XFAIL, and 22 NOT_IMPLEMENTED**. The implemented local suite and product
boundary qualification passed; release and full Linux qualification remain
not passed, so this record does not claim exhaustive QA. Debug IBus-focus
Valgrind is **PASS with reviewed suppressions**, not an unsuppressed clean run:
the preserved raw receipt contains 427 errors/contexts, 6,240 definite bytes,
and 41,461 indirect bytes; post-suppression totals are zero errors/contexts and
zero definite/indirect bytes, with all 427 errors/contexts accounted for by the
reviewed effective suppression set. The suppression-governance cell passed and
the summary retained independently hashed raw and suppressed receipts.

## Theme picker follow-up

Human QA then found that the repaired catalog still presented a text-only
“preview” and made the independent **Dark Theme** and **Light Theme** saved
slots look like active appearance switches. Selecting the light slot while the
behavior remained **Always Dark** correctly saved a light preference without
changing the terminal, but the Linux projection did not explain that model.
This was not a Ghostty reload failure; it was an incomplete port of the source
Appearance UX.

The source audit showed that macOS has three separate behavior cards, two
theme-slot cards with mini-terminal previews, two-row ANSI palette previews in
the catalog, and explanatory inactive-slot text. A real `Aizen Dark` Ghostty
theme also disproved the two-color presentation: it contains background,
foreground, sixteen ANSI colors, cursor/cursor-text, and
selection-background/selection-foreground values.

Linux now parses those cursor and selection fields, renders real compact
two-row palette previews in every catalog row and saved-slot card, and renders
a terminal-like detail preview with themed prompt/output, selection, cursor,
and all sixteen ANSI colors. The slot cards show their saved theme names. The
detail panel explicitly states whether the edited slot is currently active,
saved for another fixed behavior, or selected by Follow System. A focused real
compositor journey exercises the inactive light slot while Always Dark is
active and requires the live Ghostty theme to remain unchanged.

The expanded journey initially passed on nested X11 but failed on nested
Wayland while trying to reach the Light Theme card with reverse focus
traversal. This was treated as a harness failure, not an environmental pass.
The deterministic route now uses bounded, paced forward physical `Tab` events
from the real theme search field, reaches and activates the actual Light Theme
card on both compositors, and proves that saving Aizen Light updates Zentty's
inactive light slot without changing the live Always Dark Ghostty
configuration. The focused X11 and Wayland receipts both report
`terminal-palette-preview` and `explicit-theme-slots` as passed.

Focused mutation testing used the governed `linux/tests/mutate-rust` entry
point (`gitignore = true`, `copy_target = false`) over the theme parser and
slot-status logic. It tested 101 mutants in four minutes: 95 were caught and
six were compiler-unviable; none were missed or timed out.

The first full local qualification rerun did not pass: the controlled Wayland
Open With journey missed persistence after its physical primary-target
selection, and the controlled Wayland Task Runners journey did not observe the
VS Code task before opening the palette. Their X11 twins passed in the same
run. Neither absence was accepted as a pass. Immediate isolated reruns through
the same nested Wayland wrapper both passed their complete real-product
journeys, identifying intermittent readiness/input ordering under the parallel
matrix rather than a stable product regression. A fresh complete qualification
run was therefore required before finalizing the slice.

That fresh four-worker run passed both earlier cells but exposed the same
physical-input readiness class in `product-source-ux-x11` (a repeated Tab did
not advance Worklane Peek); the exact cell immediately passed alone. A further
full run with only two workers passed that cell and every changed theme journey
but intermittently missed the bookmark name dialog and the Appearance opacity
focus/action in two unrelated X11 journeys. This is now three complete matrix
runs with different isolated GUI timing misses, not one reproducible product
failure. The final run's machine summary records those two executable failures;
declared matrix totals remain **126 PASS, 0 FAIL, 7 BLOCKED, 1 XFAIL, and 22
NOT_IMPLEMENTED**. Accordingly, this slice does **not** claim that implemented
local, release, or full Linux qualification passed. Debug IBus-focus Valgrind
remains **PASS with reviewed suppressions**; it is not described as an
unsuppressed clean result. The theme-specific real X11 and Wayland journeys,
workspace tests, strict Clippy, architecture validators, and focused mutation
suite all passed.
