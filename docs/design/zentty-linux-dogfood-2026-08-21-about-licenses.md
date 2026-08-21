# Zentty Linux dogfood — About and packaged licenses

Date: 2026-08-21

Issue: GH-74

Plan: `docs/design/linux-about-license-trust-plan.md`

## Initial state

The macOS source has a real About surface with version/build/commit, Docs,
GitHub, and Licenses actions. Linux had no About action or view. Linux already
had a stronger package-time notice collector covering the target-filtered Cargo
graph, Ghostty/Zig components, and embedded code, but the staged product did
not produce those resources and no product UI consumed them.

This slice will extend that one notice authority rather than copying the macOS
`ThirdPartyLicenses.json`, which describes Swift/macOS dependencies that are
not the Linux distribution graph.

## Discoveries and decisions

- `linux/scripts/build-local` already injects a Zentty revision into Cargo, but
  the Rust product did not consume it and the value was abbreviated. The About
  model must expose the full build revision and an explicit dirty-tree marker;
  packaged builds remain clean by existing package policy.
- Staged notice resources belong under the existing executable prefix at
  `share/zentty/package-notices`. Installed Debian notices intentionally live
  under `/usr/share/doc/zentty/third-party`, so discovery must model both
  layouts explicitly rather than relocating or duplicating package policy.
- Cargo metadata already contains exact version, registry source, license
  expression, optional license file, and optional repository. Ghostty's notice
  manifest and pinned revision are the distribution source authority for its
  bundled components. Full text remains in separate bounded local files.

## Running record

### Single catalog authority

- The existing notice collector covered the Linux Cargo, Ghostty/Zig, and
  embedded-source graph, but emitted only packaging manifests. It now emits
  `catalog-v1.json` from that same target-filtered graph. No second dependency
  scanner was introduced.
- The staged build now carries the same notice tree that the Debian builder
  consumes. The measured staged graph contains 75 Cargo packages, 27 Ghostty
  components, two embedded-source entries, and 104 catalog entries. Its total
  installed notice footprint is 1.5 MiB.
- Catalog loading rejects unknown fields, duplicate IDs, missing/empty/
  oversized/non-UTF-8 notice files, symlinks, path traversal, unsafe links,
  unsupported kinds/schema, an excessive aggregate, and a catalog whose
  Zentty revision differs from the executable's compiled revision.
- The parser deliberately reads full local notice text. Rendering never needs
  network access; links are passed to the existing platform opener only from
  explicit button activation.
- Search results retain reference-counted catalog entries rather than cloning
  every full license body on each keystroke.

### Product implementation and UX repairs

- Linux had no About command or window. The new named `workspace.show-about`
  action is present in the command palette and owns a dedicated GTK module,
  rather than expanding the application coordinator.
- The first visual run rendered the 256 px source icon at its natural size,
  overwhelming the metadata. Replacing `Picture` with a bounded 112 px
  `Image` preserved the packaged asset and corrected the hierarchy.
- The first physical-key journey found no deterministic initial focus in the
  secondary toplevel. About now focuses Docs after presentation.
- The shared product-input helper intentionally refocuses the primary window
  before every X11 key. That is correct for primary-window tests but invalid
  for a secondary About toplevel. This journey now establishes the real
  secondary-window focus once and sends subsequent compositor-visible input
  without stealing focus back.
- GTK initially focused the Back button when switching stack pages; the
  SearchEntry's internal text widget also made `has_focus` an inaccurate
  receipt. Disabling a decorative stack transition makes the page switch and
  `grab_focus` deterministic, and the product records the successful focus
  request before accepting search text.
- A test initially asserted the words `MIT License` in autocfg's MIT file,
  but that real upstream file begins with its copyright grant instead. The
  fixture assertion now uses the actual distributed text; no notice was
  rewritten or invented.

### Staged and installed receipts

- Controlled X11 and controlled nested Wayland both passed the same real
  staged-product journey: physical command-palette opening, accessible About
  keyboard order, exact 104-entry build/catalog identity, exact search and
  selection of `cargo/autocfg/1.5.1`, local full text, a controlled HTTPS
  handler, no silent link opening, close without terminal restart, and a
  second copied bundle whose stale revision renders a recoverable diagnostic.
- The first installed-package extension raced the About window mapping: the
  catalog-loaded log precedes `window.present()`. The harness now waits for
  `about-view state=shown` before locating the X11 toplevel.
- Candidate commit `e92386a8f767ef8e901ab2488e89df6f7ef1e087` produced a clean
  Debian artifact. Package audit passed 1,286 payload files, all 75 Cargo and
  27 Ghostty notices, exact catalog completeness, and exact Zentty/Ghostty
  revisions. Its installed-product X11 journey passed from an unprivileged
  namespace with package SHA-256
  `64319c7944218bdbf99364424fc56f34b307ab22f8b6705bcdc7050b57b09a87`.
- Building the clean candidate exposed that `build-local` honored
  `CARGO_TARGET_DIR` for Cargo itself but copied binaries from a hard-coded
  repository `target/`. It now resolves relative/absolute Cargo target roots
  once and copies from the same authority.

### Mutation and qualification discipline

- The governed mutation wrapper retained `gitignore = true`,
  `copy_target = false`, a 6 GiB process ceiling, and a 12 GiB isolated scope.
  An attempted eight-worker file-wide run exceeded that isolated scope after
  ten outcomes; it did not affect the desktop. The rerun used two workers.
- The trust-boundary set initially produced 39 caught, 10 missed, and three
  unviable mutants. Focused boundary, optional-homepage, file/directory,
  repository, and metadata tests repaired eight survivors. Refactoring total
  byte accounting into a small bounded helper avoided a 64 MiB unit fixture;
  all four mutations of that helper were caught. The targeted survivor rerun
  caught 11 of 13, and the two remaining aggregate-bound mutations were then
  removed by that helper/refactor and its exact-limit/overflow tests.
- I accidentally invoked `linux/tests/qualification-matrix`, which is the
  runner rather than the validator. It correctly refused to claim success:
  `prepare-ghostty` lacked sandbox DNS and current Fontconfig suppression bytes
  exceeded the reviewed range. No result was converted to PASS. The intended
  focused `qualification-matrix-test` passed, and no broad qualification rerun
  is part of this feature closeout.
- A broad `zentty-linux` crate test run had one unrelated real `/proc` listener
  test denied by the command sandbox (`Operation not permitted`); the isolated
  test passed immediately with the required elevated kernel access. The other
  276 binary tests and all nine library tests passed in the original run.
- The architecture contract rejected the new `about_window` field until its
  ownership was explicit. It is now recorded under the existing
  `application_shell` top-level-window authority; no new coordinator or second
  lifecycle system was created.
- The authoritative matrix gains two release cells, `about-licenses-x11` and
  `about-licenses-wayland`. Declared totals are now 188 PASS, 0 FAIL,
  0 BLOCKED, 3 XFAIL, and 4 NOT_IMPLEMENTED. This is not a claim of full Linux
  qualification.
- The source feature inventory moves the combined About/licenses/privacy entry
  from NOT_IMPLEMENTED to PARTIAL, reducing its inventory total from 13 to 12.
  It remains partial because GH-76, not this slice, owns privacy and crash
  transmission. Both inventory runner and negative runner tests pass with that
  explicit split.

### Remaining boundary

GH-74 completes About and license trust. It does not claim privacy/crash
reporting, updates, or performance diagnostics; those remain separate GH-76,
GH-75, and GH-77 work under GH-23.
