# Zentty Linux dogfood: deterministic Debian builder

This record covers GH-52. The ratified policy and closed install manifest from
GH-51 remain authoritative; the builder may consume them but may not invent a
second package layout.

## Test-first construction order

1. Validate checked-in desktop, AppStream, icon, license, and manifest inputs.
2. Build the existing ReleaseSafe staged product from exact clean revisions.
3. Expand the closed manifest into a private package root and machine receipt.
4. Normalize the private Ghostty RUNPATH, ownership, modes, ordering, and all
   timestamps from the source commit epoch.
5. Derive Debian dependencies from the final ELF files with
   `dpkg-shlibdeps`, then audit every NEEDED library and bundled exception.
6. Reject source/build paths, credentials, receipts, logs, test fixtures,
   static archives, undeclared payload paths, and unsafe permissions.
7. Build twice and compare the `.deb`, manifest, metadata, and provenance
   hashes. Finally repeat from a clean isolated checkout without reading the
   developer checkout.

## Initial discoveries

- The staged Ghostty embedding library contained a Zig cache directory in its
  RUNPATH alongside `$ORIGIN` and a system directory. This is acceptable only
  for the developer stage and is forbidden in the package. The package copy
  must be normalized to exactly `$ORIGIN`; the staged runtime remains
  untouched.
- The ratified manifest named a scalable SVG icon, but the actual reviewed
  Zentty source asset is a 256-by-256 RGBA PNG. Rather than fabricate a vector
  asset during packaging, the policy now uses the standard hicolor
  `256x256/apps` PNG location and validates that exact source file.
- Ghostty's GTK embedding build did not emit terminfo into its install prefix.
  The existing Ghostty resource build supports deterministic terminfo source
  and database generation; the shared build entry must enable that output.
- `patchelf` 0.18.0 was not installed on the Ubuntu 24.04 development host. It
  was installed as an explicit package-build prerequisite, not vendored or
  hidden behind a fallback.
- The first negative invocation ran from the intentionally dirty development
  tree and stopped immediately with `Zentty checkout must be clean`, before
  building or publishing anything. Static policy negatives, ShellCheck,
  desktop-file validation, pedantic offline AppStream validation, and diff
  hygiene passed before the first implementation commit.
- The first real build completed the pinned Ghostty and ReleaseSafe Rust build,
  including terminfo generation, then failed before package publication. A
  single tab-separated loop tried to represent file and symlink variants with
  empty columns; Bash whitespace-field collapsing shifted the symlink target
  into the destination column. File and symlink manifest variants now use
  separate typed loops with no optional fields.
- The second build proved that `-Demit-terminfo=true` alone was insufficient:
  Ghostty's custom `gtk-embed-lib` step installed only its library and header,
  while resource steps were attached only to the full application install.
  The minimal Ghostty change `7f0b21e3f` makes the standalone embedding build
  depend on Ghostty's existing resource installation steps. A targeted clean
  prefix build produced the real compiled `x/xterm-ghostty` entry and relative
  `g/ghostty` alias; no Zentty-owned terminfo generator was introduced.
- The first lock edit expanded the displayed short hash by hand and therefore
  wrote a nonexistent 40-character object. Immediate `rev-parse` comparison
  caught it before any build; the lock now contains the exact Git-produced
  revision `7f0b21e3f594a6821512e4532a7eda76828400c0`.
- The first build with the corrected pin stopped before compilation because
  `build-deb` always exported its computed default as `GHOSTTY_SOURCE_DIR`.
  That accidentally selected the caller-owned-checkout contract, under which
  `prepare-ghostty-source` correctly refuses to change revisions. The builder
  now leaves the variable unset for its managed default and exports it only
  when the caller explicitly supplied an external checkout.
- The next build reached Debian metadata generation and exposed a path-basis
  bug in `DEBIAN/md5sums`: GNU `find`'s `%P` removed the starting `usr/`
  component, so every checksum command addressed a nonexistent root-relative
  path. The checksum list now retains `usr/` while remaining NUL-delimited and
  bytewise sorted, including theme names containing spaces. `dpkg-shlibdeps`
  also emitted pages of advisory warnings for the deliberately private
  unversioned libraries and their `$ORIGIN` lookup; its warning bitmask is now
  disabled while fatal dependency resolution remains enforced and the
  separate ELF/RUNPATH audit remains authoritative.
- With checksums repaired, expanded-manifest generation found a jq scoping
  error: piping `$path` into `startswith` changed `.` from the manifest entry
  to the path string, so `.destination` attempted to index a string. The
  query now binds each entry explicitly before comparing exact and descendant
  destinations, preserving longest-prefix ownership for generated trees.
- The first complete payload audit then rejected real source paths retained
  by C/C++ `__FILE__` strings in Ghostty's bundled shader translators, ImGui,
  stb, HarfBuzz, Fontconfig, and libxml2 code. Stripping debug sections cannot
  remove runtime assertion strings, and broadening the audit would hide a real
  reproducibility and privacy defect. Ghostty commit
  `4ceacd74b00da6f84c7986291954c81c3d9b733e` applies compiler-supported
  `-ffile-prefix-map` flags to the bundled code that Ghostty must retain. The
  Ubuntu package build uses the baseline system Fontconfig and HarfBuzz,
  consistent with the ratified system-library policy, rather than bundling
  second copies merely to control their source paths. A real stripped embed
  build proved that the remaining bundled paths map under `/usr/src/ghostty`;
  no binary string rewriting or audit exception was introduced.
- Ubuntu 24.04's system HarfBuzz 8.3 lacks
  `HB_BUFFER_CLUSTER_LEVEL_GRAPHEMES`, which the pinned Ghostty Zig binding
  requires, so the first system-library build failed at compile time rather
  than silently weakening the API. Ghostty commit
  `831a92dd27ef811af94d505d2a870b6603fe904c` extends the same compiler mapping
  to its required bundled HarfBuzz. A real ReleaseSafe build with system
  Fontconfig, bundled HarfBuzz, and external debug stripping then contained
  zero developer checkout or build paths. The package therefore uses system
  Fontconfig but retains the compatibility-required HarfBuzz bundle.
- The subsequent package audit found the same runtime-path class independently
  in the separately packaged gtk4-layer-shell library. Ghostty commit
  `281d7d7dbeab24c1a2d04f6d3c720c34dbfac645` maps that dependency's source
  root as well. Both stripped shared objects were inspected directly and now
  contain no developer checkout or build path; this remains a build repair,
  not a package-audit exclusion.
- The next clean build completed end to end: the audit accepted exactly 530
  manifest-declared payload files, and the real `.deb` passed extraction,
  control/archive inspection, strict `DEBIAN/md5sums` verification, and the
  undeclared-file, missing-license, wrong-mode, source-path, and bad-RUNPATH
  negative mutations. This is the first structurally valid artifact, not yet
  a reproducibility or isolated-checkout qualification claim.
- Review of the first artifact's legal payload showed that the Debian
  `copyright` file covers Zentty, Ghostty, and gtk4-layer-shell, while the
  binaries also statically incorporate Cargo and Ghostty/Zig dependencies.
  That is not an adequate completed license audit. The release builder must
  add deterministic dependency notices rather than treating the top-level
  project licenses as transitive coverage.
- The committed two-build reproducibility harness built from two independent
  package work/output roots and byte-compared the `.deb`, expanded manifest,
  provenance JSON, and `SHA256SUMS`. All four outputs were identical for
  version `0.1.0+gitbd7f13576cb3`; the compressed Debian container needs no
  variance exception.
- The first masked-checkout isolation run failed immediately because a plain
  bubblewrap root bind did not preserve writable device semantics for
  `/dev/null`; even the prerequisite probe could not redirect output. The
  harness now explicitly device-binds `/dev` while continuing to replace the
  developer checkout with an empty read-only mount.
- The next isolated run found a previously hidden bootstrap bug in
  `prepare-ghostty-source`: immediately after `git clone --no-checkout`, Git
  correctly reports the default branch's entire index as deleted because no
  worktree has been checked out yet. The script mistook that initial state for
  caller modifications and refused every truly fresh managed clone. It now
  skips only the pre-checkout dirtiness test for a clone it created in that
  invocation, then performs the existing exact-revision and clean-tree checks
  after checkout. Existing managed or caller-owned checkouts receive no
  relaxation.
- With that repair, a full independent clone built and passed the package
  mutation audit while bubblewrap replaced the original developer checkout
  with an empty read-only mount. Provenance recorded exact Zentty revision
  `975381237a7a8de44e22b394174c2cb61ba8e216`; neither build nor audit could
  read the developer source tree.
- The dependency-notice audit enumerated 77 locked external Cargo packages;
  every package has both Cargo license metadata and at least one source-shipped
  license/notice file. Ghostty's embedded graph required a separate reviewed
  27-component notice manifest. The pinned zig-gobject release archive omits
  its repository LICENSE, so the exact release-tag MIT text (Git blob
  `2e486f241150be06138ff29dfd903f692f67f514`) is now a reviewed package source.
  stb, glad, and z2d carry source-level SPDX/notices rather than standalone
  files, so their evidence and the applicable Apache/MPL texts are included
  explicitly. Notice collection runs Cargo metadata offline after compilation;
  missing locked sources or notice files are failures, not network fallbacks.
- The first notice-bearing artifact passed with 690 closed-manifest files,
  77 Cargo dependency records, and 27 reviewed Ghostty/Zig components. Its
  extracted notices survived the full structural audit, and deleting either
  the main copyright or a single Ghostty third-party notice caused the mutated
  payload to fail qualification.
- Final notice-bearing reproducibility passed for all four release outputs at
  `0.1.0+git7b40b65be420`. A second clean independent clone at exact revision
  `7b40b65be420b9842e6f802d256ea88405ee2b89` also built and passed with the
  developer checkout masked. Dependency-notice collection is part of routine
  qualification support; the expensive double-build and clean-clone journeys
  remain explicit release-builder tests rather than silently inflating every
  local matrix run.
- Builder-contract negatives use independent real Git repositories: a dirty
  Zentty clone, a clean commit with the required icon removed, and a clean but
  wrong Ghostty repository. Payload mutations now also recompute the expanded
  manifest digest before proving credential-shaped content and static archives
  are rejected by their specific policies rather than merely by the generic
  closed-world file-set comparison.
- Adding 160 dependency-notice files exposed an audit scaling flaw: every
  payload file launched several jq processes to re-scan the full expanded
  manifest, multiplying each negative mutation's cost. The audit now loads
  receipt type/mode/digest/target maps once using a non-whitespace unit
  separator (TSV would collapse empty symlink fields). A real 690-file positive
  audit fell to about 5.6 seconds without removing any content checks.
- Provenance review found that the first receipt pinned source/toolchain
  revisions but did not expose dependency versions directly. The receipt now
  records the exact derived Debian runtime relationship string, counts and
  hashes for the packaged 77-Cargo/27-Ghostty component inventories, and
  hashes of Cargo.lock, Ghostty build.zig.zon, packaging policy, and the closed
  install manifest. The isolated test rejects absent or malformed dependency
  provenance.
- The first explicit timestamp assertion caught the package root directory
  itself: creating `DEBIAN` after the payload normalization pass advanced its
  mtime even though every file and child directory was normalized. The builder
  now performs a final whole-tree timestamp pass after all package-root writes
  and before receipt/audit/archive construction; the assertion remains in
  place to prevent recurrence.
