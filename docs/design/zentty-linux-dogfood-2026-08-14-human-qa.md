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
