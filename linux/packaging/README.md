# Linux packaging contract

Zentty's first supported Linux artifact is a native Debian package for Ubuntu
24.04 LTS on amd64. This directory is authoritative for package construction:

- [`policy-v1.json`](policy-v1.json) freezes the supported runtime, dependency,
  layout, versioning, XDG-data, lifecycle, ownership, and reproducibility rules.
- [`install-manifest-v1.json`](install-manifest-v1.json) is the closed package
  ownership manifest. Tree entries must be expanded to an exact file receipt
  when the package is built; undeclared output is an error.
- [`upgrade-fixture-v1.json`](upgrade-fixture-v1.json) defines the oldest
  supported transaction fixture used by the real dpkg lifecycle journey. The
  fixture is derived deterministically and is not represented as a previously
  published release.
- `linux/tests/packaging-policy` validates the live contract against the staged
  build boundary. `linux/tests/packaging-policy-test` supplies its negative
  fixtures.

The staged bundle is not an installer. GH-52 builds the native package and
GH-53 owns real package-manager lifecycle qualification; installed GUI launch
and public release qualification remain separate GH-54/GH-55 work.

## Filesystem model

The relocatable application tree is installed below `/usr/lib/zentty`. Relative
links in `/usr/bin` expose the GUI and CLI without breaking the application's
sibling-binary, private-library, and resource lookup. Desktop integration,
compiled terminfo, icons, and copyright material use standard `/usr/share`
locations. Ghostty's runtime shell-integration and theme resources live at
`/usr/share/ghostty`, adjacent to the compiled terminfo sentinel Ghostty uses
to discover that directory. Zentty's product-specific shell integration and
theme catalog remain below `/usr/lib/zentty/share/zentty`; the two resource
roots serve separate consumers and neither substitutes for the other.

Package-manager removal never deletes files below a user's XDG directories.
Zentty may later expose a separate, explicit per-user cleanup command; it must
not be implemented by enumerating home directories in Debian maintainer
scripts.

`linux/tests/debian-package-lifecycle-build` is the authoritative GH-53
qualification entry point. It builds the exact clean revision, then drives real
`dpkg` install, reinstall, upgrade, injected pre-unpack failure, remove, purge,
and repeat-cycle operations inside a disposable Bubblewrap root. Its validated
machine receipt and raw dpkg logs are written below
`build/linux/package-lifecycle/`. The host root is mounted read-only; missing
Bubblewrap/user-namespace support is a prerequisite failure rather than a
passing skip.

The injected failure proves only Debian's pre-installation-script boundary:
when `preinst` fails before unpack, the prior payload remains installed. Zentty
does not claim general automatic rollback after arbitrary unpack or maintainer
script failures. Production packages currently contain no maintainer scripts
or conffiles, so default remove and purge have identical system-payload results
and both preserve all user-owned XDG data.

AppImage, Flatpak, and RPM are deferred. Nothing in this contract implies that
those formats or Linux distributions other than the named baseline have been
qualified.
