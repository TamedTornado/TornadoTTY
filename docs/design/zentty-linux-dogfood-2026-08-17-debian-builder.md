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
