# Zentty Linux dogfood: first public Linux release

- **Tracking:** GH-142
- **Scope:** GitHub Release artifacts for Ubuntu/Debian and Omarchy/Arch.
- **Qualification boundary:** GitHub Actions builds and checks artifacts but is
  not the release-qualification authority. Publication remains an explicit
  operator action after local qualification.

## Initial audit

- The repository had no GitHub Releases and no release workflow.
- The existing deterministic Debian builder targets Ubuntu 24.04 LTS amd64 and
  already owns expanded-manifest, provenance, checksum, payload, disposable
  lifecycle, and installed-product evidence.
- No native Arch package existed. Omarchy is Arch-based, so renaming the Debian
  artifact or repackaging Ubuntu-built ELF files would not establish Omarchy
  support.
- The real validation host `omarchy-laptop` reports Omarchy `4.0.2-1`, x86_64,
  Linux `7.1.9-arch1-2`, GTK `4.22.4`, libadwaita `1.9.3`, Hyprland/Wayland,
  23 GiB RAM, and 431 GiB free disk. It has `base-devel`, `makepkg`, `pacman`,
  Bubblewrap, and working unprivileged user namespaces. Rust, Zig, and nested
  display tools are not preinstalled.
- The package-notice collector assumes Debian's
  `/usr/share/common-licenses/{Apache-2.0,MPL-2.0}` paths. That is a real
  cross-distribution packaging defect and must not be bypassed on Arch.
- Inherited macOS/upstream tags already occupy the ordinary `v*` namespace.
  Linux releases therefore use `linux-v<application-semver>` and validate that
  the tag points at the exact packaged commit.

## Decisions

1. Publish native `.deb` and `.pkg.tar.zst` artifacts.
2. Build the Arch artifact against a reviewed Arch/Omarchy userspace and prove
   it on the real Omarchy host.
3. The first GitHub entry is a prerelease while the repository continues to
   state its explicit qualification gaps.
4. Do not implement automatic in-app updates as part of this work; GH-75
   remains separate.

The real Omarchy host is also the authoritative Arch build host. Treating it
only as a post-build install target added a container boundary without adding
evidence. Ubuntu builds the Debian family artifact; Omarchy builds the Arch
family artifact, and each package is then exercised on its native family.

## Portability repair

- The reviewed Apache-2.0 and MPL-2.0 texts are now package inputs rather than
  paths borrowed from the build host. The collector and its negative contract
  reject any remaining `system_source` entry. Debian output retains the same
  license text bytes while Arch no longer depends on Debian's filesystem
  convention.

## Native Arch construction boundary

- The build environment is the immutable official Arch `base-devel` image
  digest `sha256:84cd9ef000b3cff245ec028e87965b84724f4bf1cc63fc2741ba927b88515ed6`.
  System packages come from the official Arch Linux Archive snapshot dated
  2026-08-31 rather than a moving rolling mirror.
- The same content-pinned Zig 0.16.0 archive, dated Rust nightly, Cargo lock,
  pinned Ghostty revision, source-path remapping, publish-age audit, and notice
  collector used by the Debian path remain in force.
- The generated pacman package has a closed expanded payload manifest and
  provenance receipt. Its audit has negative fixtures for undeclared payload,
  dependency drift, and stale provenance.

## First native-build failure and repair

- The pinned Arch snapshot uses GCC 16 startup objects with `.sframe`
  `R_X86_64_PC64` relocations. Zig 0.16's linker rejected `crt1.o` while linking
  both Ghostty build-data helpers. Product compilation had not started; the
  repeated error was an exact toolchain boundary rather than a product test
  failure.
- An attempted explicit `x86_64-linux-gnu` target did not repair the build.
  Ghostty's two generators remain host-native, while the cross-target library
  also loses native GTK discovery. The workaround was removed rather than
  retained as dead configuration.
- Ghostty commit `bab8c088f45e47a00ce3bfe2c142d6cb51ecd200` instead selects
  its already-supported LLVM linker for exactly the two failing host helpers.
  This is a two-line, product-neutral build repair; the product and its GTK
  dependencies remain native. The authoritative Arch artifact will be built
  on the real Omarchy host, not relabeled from Ubuntu or treated as passing
  because a container happened to compile it.
- Node and pnpm were removed from the Arch build environment after the first
  package transaction proved that no JavaScript tool participates in native
  construction. This reduces the release build closure rather than installing
  unused tooling defensively.
- The first real-Omarchy build passed both formerly failing Ghostty host-helper
  links, then stopped because `blueprint-compiler` was absent. The dependency
  was already in the reviewed build manifest but missing from the native
  builder's command preflight. The preflight now names it explicitly; the
  laptop uses Omarchy's signed `blueprint-compiler` package from a user-local
  extraction rather than silently altering the host system.
- After Ghostty compiled, the Cargo publication-age audit correctly refused to
  run without a crates.io sparse-index cache. The container wrapper had fetched
  the lockfile before invoking the native package builder, making the builder
  accidentally dependent on its caller. The native builder now performs its
  own `cargo fetch --locked` before the age audit; the audit itself remains
  unchanged and cannot turn missing registry evidence into a pass.
- The first warm-cache packaging pass stopped without its own diagnostic while
  resolving the first ELF dependency. An execution trace showed `awk` exiting
  as soon as it found a match in `ldd` output; under `pipefail`, the resulting
  SIGPIPE from `ldd` terminated the builder. The resolver now consumes the full
  output and emits only its first match, preserving strict pipeline failure
  handling instead of disabling it.
- With the pipeline fixed, the scan reached the ELF interpreter and rejected
  it because `readelf` reports `ld-linux-x86-64.so.2` while Omarchy's `ldd`
  prints `/lib64/ld-linux-x86-64.so.2 => /usr/lib64/ld-linux-x86-64.so.2`.
  The resolver now compares the basename of absolute left-hand paths, handles
  both redirected and direct loader rows, and still asks pacman which installed
  package owns the resolved path.
- The completed native ELF scan found ten direct package owners rather than the
  fifteen initially reviewed. Current Omarchy owns `libgcc_s.so.1` in `libgcc`,
  not the older aggregate `gcc-libs`; `gdk-pixbuf2`, `harfbuzz`, `libepoxy`,
  `libxkbcommon`, and `pango` enter through GTK/libadwaita rather than a direct
  Zentty ELF edge. The ratified dependency list now matches the measured direct
  owners exactly; pacman still installs the transitive GTK closure normally.
- The real Omarchy host does allow unprivileged overlayfs for ordinary trees,
  but it rejects using or bind-mounting `/` as an unprivileged overlay lower
  layer. The lifecycle harness therefore uses real pacman with a disposable
  root and an explicit temporary copy of the host package database. Pacman
  performs dependency, integrity, conflict, install, query, and remove
  operations there; the installed application binary runs with the host's
  native libraries. A before/after fingerprint proves the host pacman database
  is unchanged. The harness also supports a distinct prior package for a real
  upgrade transition.
- The first scripted run tried to preserve host-root ownership while copying
  the package database inside a one-user namespace. Host UID 0 is deliberately
  unmapped there, so the copy failed before pacman ran. The disposable database
  now preserves content, modes, timestamps, and links but assigns its files to
  namespace root; ownership is not part of libalpm's local database semantics.

## Evidence

- Ghostty `bab8c088f45e47a00ce3bfe2c142d6cb51ecd200` built natively on
  Omarchy/GCC 16 after both host helpers selected LLVM. The product then built
  with Rust `1.100.0-nightly` and Zig `0.16.0`; the Cargo age audit reported 91
  packages, zero exceptions, and PASS.
- Native package `zentty-0.1.0+git1cf3619028b3-1-x86_64.pkg.tar.zst` passed the
  closed 1,297-file payload audit and a real pacman fresh-install, installed
  `zentty-linux --help`, and uninstall cycle.
- A second audited package from `be70cf5e77af` was installed over the first by
  real pacman. Receipt `zentty-upgrade.lifecycle.json` records the distinct
  versions, `install-previous`, `upgrade`, installed help, and uninstall, plus
  an unchanged host-package-database fingerprint. This is native Omarchy
  package evidence, not a container-only approximation.
- The Debian builder previously always appended `+git<revision>`, even when
  invoked for a validated release tag. It now shares the same `linux-v<version>`
  validator as the Arch builder and emits the plain application version for a
  tagged release. It also owns its locked Cargo fetch rather than assuming a
  caller populated the sparse index.
- Release publication is an explicit, manually dispatched workflow. The build
  job has read-only repository permission and produces both native package
  families plus manifests/provenance; only the conditional publisher job has
  `contents: write`. Every action is pinned to a full reviewed commit. The same
  bundle validator and publisher are usable locally, so GitHub Actions checks
  and transports artifacts but is not a release-qualification authority.
- Publication completed as the GitHub prerelease `linux-v0.1.1` from Zentty
  revision `c27e0b3505c30903eb96e563f47bfaab4744b8a8` and Ghostty revision
  `bab8c088f45e47a00ce3bfe2c142d6cb51ecd200`. The public release contains the
  native Ubuntu/Debian and Omarchy/Arch packages, both expanded manifests, both
  provenance receipts, aggregate checksums, and release metadata.
- The final Debian artifact passed its nine-transition disposable-root
  lifecycle. The final Omarchy artifact passed native pacman installation over
  retained dogfood version `0.1.0+gitbe70cf5e77af-1`, installed-product help,
  and uninstall; its receipt explicitly records `upgrade_direction_verified`
  and an unchanged host package-database fingerprint.
- Reconstructing the user-local Omarchy release environment over SSH exposed
  three caller preconditions before compilation: the isolated Rust paths,
  repository working directory, and explicit Zig path. Each attempt failed at
  preflight and was not treated as qualification. The manual build was rerun
  with the dated Rust toolchain, Cargo home, reviewed Zig 0.16 binary, and
  repository root explicit. The checked-in workflow already invokes its
  builder from the checkout root; making the native script independent of its
  caller's current directory remains a tooling-hardening opportunity rather
  than a product qualification claim.

## Release-version ordering repair

- The first tagged `0.1.0` artifacts built and passed their payload audits, but
  were withheld before publication. Both dpkg and pacman order the existing
  `0.1.0+git<revision>` dogfood versions after clean `0.1.0`, so upgrading a
  dogfood installation would be presented as a downgrade.
- The first public prerelease is therefore `0.1.1`. Both native package
  comparators order clean `0.1.1` after every retained `0.1.0+git...` package.
  The unpublished `linux-v0.1.0` tag is retired; a real upgrade from the
  retained Omarchy dogfood artifact to tagged `0.1.1` must pass before
  publication.
- Review of the upgrade harness found that it required distinct versions but
  did not require the candidate to sort newer; pacman can explicitly perform a
  downgrade. The harness now uses pacman's own `vercmp` and rejects a candidate
  unless it is strictly newer than the installed fixture. Receipts expose that
  ordering assertion rather than inferring upgrade direction from a command
  name.
- The final cross-host bundle audit rejected the native Arch binary because it
  contains the application's legitimate cross-platform scanner token
  `/home//Users/unsafe`. The Arch-host audit had incorrectly passed the same
  bytes: `grep -q` closed its `strings` pipeline early and, under `pipefail`,
  converted the match into a false negative when `strings` received SIGPIPE.
  The audit now matches concrete home/tmp build paths rather than a bare
  `/home/` token and consumes the complete pipeline. Positive and negative ELF
  fixtures prove both that the application token is allowed and a real source
  path remains rejected even with substantial data after the match.

## TornadoTTY republication repair (2026-09-02)

- After the public rebrand, the prior Zentty release was deliberately removed.
  The repository README linked to an empty Releases page until a new
  `tornadotty-v0.1.1` publication was requested.
- The first rebranded workflow run, `33597971738`, failed before artifact
  construction with `build-deb: error: prepared Zig package cache is missing`.
  This was not treated as a release or package pass. The workflow prepared the
  exact Ghostty checkout but omitted the immutable Zig dependency-fetch phase
  required by the reproducible Debian builder.
- A focused Zig 0.16 probe against the pinned Ghostty checkout established that
  `--fetch=needed` does not create the complete global `p` store, while
  `--fetch=all` creates it without compiling the product. The release workflow
  now fetches the exact Ghostty dependency tree into the declared repository
  cache with the same ReleaseSafe/fontconfig/GTK-embed configuration and
  explicitly requires the resulting `p` directory before package construction.
- `release-workflow-contract` now makes cache population and verification part
  of the release orchestration contract. Its negative suite removes the fetch
  edge and proves the mutation is rejected. No redundant pre-package product
  build or cache-bypass was added.
