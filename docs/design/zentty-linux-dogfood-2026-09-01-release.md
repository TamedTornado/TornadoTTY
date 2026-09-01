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
- Updating or patching the pinned Zig compiler would make the two package
  families diverge. Instead, the Arch build now selects the explicit
  `x86_64-linux-gnu` Ghostty target. Zig supplies its pinned GNU CRT while the
  build continues to compile and link against the real Arch GTK stack.
  `build-local` exposes this only through a validated optional target argument;
  the existing Ubuntu build remains native and unchanged.
- Node and pnpm were removed from the Arch build environment after the first
  package transaction proved that no JavaScript tool participates in native
  construction. This reduces the release build closure rather than installing
  unused tooling defensively.

## Evidence

Evidence will be appended as package construction, lifecycle, and publication
work proceeds. A missing or environmentally blocked journey will remain
explicit and will not be converted into a pass.
