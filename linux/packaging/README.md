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

The staged bundle is not an installer. Native-package evidence is kept distinct
from staged evidence in the qualification matrix and the versioned package
qualification summary.

## Supported baseline and dependencies

The qualified package target is **Ubuntu 24.04 LTS (Noble), amd64** with glibc
2.39, GTK 4.14, libadwaita 1.5, Wayland 1.22, or X11 1.8.7 and newer compatible
versions. The exact runtime dependency relation is generated from the final ELF
payload with `dpkg-shlibdeps`, reconciled with the minimum baseline in
`policy-v1.json`, and recorded in the artifact provenance JSON. Ghostty and
gtk4-layer-shell are the only private shared libraries; all other dependencies
are system libraries. Other distributions, architectures, and older library
stacks are not qualified.

## Build, checksums, and provenance

From a clean checkout with the pinned Ghostty source prepared:

```sh
linux/scripts/prepare-ghostty-source
ZENTTY_GHOSTTY_PREPARED=true linux/scripts/build-deb
```

The output directory `build/linux-package/` contains the current revision's
`.deb`, expanded manifest JSON, provenance JSON, and `SHA256SUMS` (and may retain
older developer-build artifacts).
The provenance records the Zentty and Ghostty commits, Ubuntu baseline,
architecture, dependency-set hashes, source-input hashes, toolchain versions,
and artifact/manifest hashes. Release qualification additionally rebuilds from
a detached clean clone with the developer checkout masked and networking
disabled, then requires all four outputs to be byte-identical.

Verify an artifact set before installation:

```sh
cd build/linux-package
sha256sum --check --strict SHA256SUMS
dpkg-deb --info ./tornadotty_*_amd64.deb
dpkg-deb --contents ./tornadotty_*_amd64.deb
```

## Install, verify, upgrade, and remove

Install or perform a same-version reinstall with the system package manager:

```sh
sudo apt install ./build/linux-package/tornadotty_*_amd64.deb
zentty --help
tornadotty
```

`apt install ./newer-zentty.deb` is the supported upgrade route. The automated
lifecycle journey proves upgrade from the oldest supported fixture and proves
that an injected `preinst` failure before unpack preserves the previous
payload. This is deliberately **not** a claim of automatic rollback after an
arbitrary failure once unpack has begun. Preserve the previous `.deb` if manual
package-file rollback may be needed; compatibility of newer user state with an
older application is not guaranteed.

Remove package-owned system files with either:

```sh
sudo apt remove zentty
sudo apt purge zentty
```

The package has no maintainer scripts or conffiles, so remove and purge
currently have the same system-payload result. Neither command deletes user
data. Configuration, data, state, cache, and runtime files below the user's XDG
directories are retained by design. There is not yet a package-owned command
for explicit per-user cleanup.

## Qualification receipts

`linux/tests/debian-package-lifecycle-build` is the single real dpkg lifecycle
producer. It runs install, reinstall, upgrade, injected failure, remove, purge,
and a repeated cycle inside a disposable Bubblewrap root with the host root
read-only. `linux/tests/debian-package-installed-product` launches the installed
GUI and CLI under controlled nested X11 and Wayland using installed paths only.

The matrix exposes separate command-backed evidence cells for clean
build/reproducibility, payload audit, install/reinstall, upgrade/failure,
uninstall/purge/residue, installed X11, installed Wayland, and the final
summary. Their closed receipt graph is written below
`build/linux/package-qualification/`; missing or stale evidence is a failure,
not a skip. `linux/tests/qualify-local` remains the authoritative entry point.

## Filesystem model

The relocatable application tree is installed below `/usr/lib/tornadotty`.
Relative links expose only `/usr/bin/tornadotty` and
`/usr/bin/tornadotty-cli`; packages must not recreate `/usr/bin/zentty`,
`/usr/bin/zentty-linux`, or `/usr/lib/zentty`. This keeps the installed product
identity distinct without renaming source crates, environment variables, or
user-state namespaces that remain compatibility contracts. Desktop integration,
compiled terminfo, icons, and copyright material use standard `/usr/share`
locations. Ghostty's runtime shell-integration and theme resources live at
`/usr/share/ghostty`, adjacent to the compiled terminfo sentinel Ghostty uses
to discover that directory. Product-specific shell integration and the theme
catalog remain in the internal `share/zentty` subtree below
`/usr/lib/tornadotty`; the two resource roots serve separate consumers and
neither substitutes for the other.

Package-manager removal never deletes files below a user's XDG directories.
Zentty may later expose a separate, explicit per-user cleanup command; it must
not be implemented by enumerating home directories in Debian maintainer
scripts.

Its validated lifecycle receipt and raw dpkg logs are written below
`build/linux/package-lifecycle/`. Missing Bubblewrap/user-namespace support is
a prerequisite failure rather than a passing skip.

The injected failure proves only Debian's pre-installation-script boundary:
when `preinst` fails before unpack, the prior payload remains installed. Zentty
does not claim general automatic rollback after arbitrary unpack or maintainer
script failures. Production packages currently contain no maintainer scripts
or conffiles, so default remove and purge have identical system-payload results
and both preserve all user-owned XDG data.

AppImage, Flatpak, and RPM are deferred. Nothing in this contract implies that
those formats or Linux distributions other than the named baseline have been
qualified.
