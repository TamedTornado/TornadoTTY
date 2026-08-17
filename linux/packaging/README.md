# Linux packaging contract

Zentty's first supported Linux artifact is a native Debian package for Ubuntu
24.04 LTS on amd64. This directory is authoritative for package construction:

- [`policy-v1.json`](policy-v1.json) freezes the supported runtime, dependency,
  layout, versioning, XDG-data, lifecycle, ownership, and reproducibility rules.
- [`install-manifest-v1.json`](install-manifest-v1.json) is the closed package
  ownership manifest. Tree entries must be expanded to an exact file receipt
  when the package is built; undeclared output is an error.
- `linux/tests/packaging-policy` validates the live contract against the staged
  build boundary. `linux/tests/packaging-policy-test` supplies its negative
  fixtures.

The current staged bundle is not an installer. Until GH-52 through GH-55 build
and qualify the real artifact, the authoritative `install-uninstall` matrix
cell remains `NOT_IMPLEMENTED`.

## Filesystem model

The relocatable application tree is installed below `/usr/lib/zentty`. Relative
links in `/usr/bin` expose the GUI and CLI without breaking the application's
sibling-binary, private-library, and resource lookup. Desktop integration,
compiled terminfo, icons, and copyright material use standard `/usr/share`
locations.

Package-manager removal never deletes files below a user's XDG directories.
Zentty may later expose a separate, explicit per-user cleanup command; it must
not be implemented by enumerating home directories in Debian maintainer
scripts.

AppImage, Flatpak, and RPM are deferred. Nothing in this contract implies that
those formats or Linux distributions other than the named baseline have been
qualified.
