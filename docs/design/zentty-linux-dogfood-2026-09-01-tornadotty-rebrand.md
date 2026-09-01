# TornadoTTY rebrand dogfood record

- **Tracking:** GH-144
- **Decision:** Rebrand the downstream Linux distribution as **TornadoTTY**,
  displayed as **Tornado TTY**, while preserving truthful attribution to the
  upstream Zentty project.

## Release withdrawal

- The public `linux-v0.1.1` GitHub prerelease distributed a modified build
  under the Zentty product identity. It and its remote tag were removed before
  rebrand implementation began. The stale local tag was also removed.
- The withdrawn binaries must not be restored or announced. A future release
  starts from a new TornadoTTY identity and package lifecycle.

## Collision search

- Exact searches found no `TornadoTTY` or `Tornado TTY` product in GitHub
  repositories/code/accounts, crates.io, npm, PyPI, RubyGems, AUR, Debian,
  Homebrew, Flathub, or Snapcraft.
- The intended `TamedTornado/TornadoTTY` repository path was unclaimed.
- Common TornadoTTY domain variants had no registration or DNS result at the
  time of the search. Availability is transient and was not treated as a
  reservation.
- One inactive 2019 repository named `tornado-terminal` is a small Python
  Tornado/Terminado browser-terminal demonstration, not an established product
  using the TornadoTTY name. Python's Tornado framework will nonetheless add
  search noise if the product is described only as “Tornado terminal.”

## Identity-boundary discovery

- The old name is not merely display copy. It also appears in the application
  ID, package and executable names, XDG config/state/runtime roots, IPC
  discovery, authenticated agent environment variables, Rust crates, shell
  integrations, desktop metadata, logs, and package payload paths.
- A global textual replacement would add risk without improving the public
  identity. In particular, renaming state and runtime boundaries could lose
  saved worklanes, settings, or installed agent-hook connectivity.
- **Decision:** rename public branding and distribution surfaces only. Preserve
  the existing `zentty` XDG config/state/runtime roots, `ZENTTY_*` environment
  contract, IPC discovery, Rust crate/module names, source paths, and private
  `/usr/lib/zentty` layout as deliberate internal compatibility interfaces.
  The old public executables may remain as compatibility aliases; they are not
  the advertised product identity.
- The machine-readable policy rejects an attempted internal namespace migration
  so future cleanup cannot silently turn this scoped rebrand into a state or API
  migration.

## New visual identity

- A new icon candidate uses an orange tornado vortex, cyan accent, and terminal
  prompt on a dark tile. It does not reuse the upstream blue interwoven-ribbon
  icon or its geometry. The generated source and reviewed 256px launcher asset
  are stored separately so packaging can consume the exact reviewed output.

## Public application identity implementation

- The shared Linux composition-root constants now expose `Tornado TTY`,
  `TornadoTTY`, and `com.tamedtornado.tornadotty`. GTK host identity, window and
  settings titles, notifications, diagnostics, About, command-palette copy,
  version output, and accessibility labels consume that public identity.
- The About view includes explicit unofficial-fork and non-endorsement text.
- Local build staging installs the new icon under the new application ID while
  leaving the executable, resource, hook, state, and runtime internals named
  `zentty`.
- Focused Rust coverage verifies the public constants, attribution, pane
  notification identity, version output, and staged/installed icon discovery.
