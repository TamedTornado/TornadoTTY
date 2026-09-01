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

## Repository and package identity

- Jason renamed the downstream repository to
  `https://github.com/TamedTornado/TornadoTTY`. The local `origin`, active
  source links, package metadata, release publisher, and operator instructions
  now target that repository; `upstream` remains `dedene/zentty`.
- Debian and Arch package policy now advertise `tornadotty`. Their primary
  entry points are `/usr/bin/tornadotty` and `/usr/bin/tornadotty-cli`; the old
  `/usr/bin/zentty-linux` and `/usr/bin/zentty` names remain compatibility
  aliases to the same private binaries.
- Desktop, AppStream, icon, and launcher identity is
  `com.tamedtornado.tornadotty`. Package documentation moved to
  `/usr/share/doc/tornadotty`; internal binaries, libraries, resources, and
  runtime contracts remain under their established `zentty` paths.
- New releases use `tornadotty-v<version>` tags and TornadoTTY artifact names.
  The release workflow remains explicitly dispatched and does not qualify or
  publish ordinary commits.

## Focused verification receipts

- PASS: identity policy and all negative policy fixtures, including attempted
  internal state-namespace migration.
- PASS: all `zentty-core` unit/integration tests, including the new public
  identity and attribution contract.
- PASS: focused Linux identity tests for help/version/error output, About
  resource discovery, pane-notification identity, and tray presentation.
- PASS: Debian and Arch packaging-policy validators and their negative tests;
  Debian builder negative contracts; Arch artifact-auditor fixtures; isolated
  installed-product-root fixture; desktop-file and AppStream validators;
  release-version and release-workflow contracts.
- A fast, non-display `zentty-linux` crate run reached **350 PASS, 3 FAIL,
  3 ignored**. The failures were not accepted as rebrand passes: one GTK test
  lacked a display, one real `/proc` listener test was denied by the command
  sandbox, and the pre-existing exact-maximum config rewrite test exceeded its
  output bound even in isolation. The focused rebrand tests passed; these
  unrelated cells remain explicit and do not establish release qualification.
- Native TornadoTTY packages have not yet been built from the committed clean
  tree. Package publication remains blocked until those artifacts and their
  real installed-product journeys pass.
