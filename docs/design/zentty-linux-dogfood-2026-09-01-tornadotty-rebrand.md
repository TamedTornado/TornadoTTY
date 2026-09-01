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
- Release publication remains blocked pending release-tag qualification. The
  focused native package and X11 journey receipts below establish the current
  rebrand slice, not release qualification or exhaustive Linux QA.

## Real Debian package discovery and repair

- Clean-tree construction produced and audited
  `tornadotty_0.1.1+gitcd73112f44f7_amd64.deb` with 1,300 declared payload
  files. Its structural negative suite passed.
- The installed-product runner correctly rejected a direct arbitrary-artifact
  invocation because its only accepted input is the lifecycle-qualified
  locator. No bypass was added.
- The first lifecycle run then exposed a genuine stale contract: lifecycle
  queries, verification, removal, and purge still named the old public Debian
  package `zentty`. Fresh installation therefore succeeded while the audit
  looked up the wrong database record.
- Repair: derive the package name from the candidate control metadata, require
  it to equal `tornadotty`, and use that value for every dpkg lifecycle
  operation. Internal XDG and private install paths remain unchanged.
- PASS: the repaired real lifecycle completed all 9 transitions, including
  fresh install, reinstall, supported upgrade, injected failed upgrade,
  remove, purge, and a second install/remove cycle, while preserving user XDG
  data and unrelated system files.

## Installed-product rebrand discoveries and repair

- The lifecycle-qualified current-source build produced
  `tornadotty_0.1.1+gitfec6f65be297_amd64.deb`; its structural audit covered
  1,300 declared files and the real Debian lifecycle passed all 9 transitions.
- The first controlled X11 installed-product journey exposed two stale public
  UI expectations in the journey rather than the package: the command-palette
  query `Copy` now returns seven legitimate matches instead of five, and the
  About action/window is now `About Tornado TTY` rather than `About Zentty`.
  The Copy assertion still requires the exact-title action to execute and an
  independent compositor client to observe its clipboard result; it was not
  weakened into an arbitrary-match pass.
- A later desktop-entry phase appeared to hang because the PID locator only
  recognized the compatibility command `/usr/bin/zentty-linux`, while the
  canonical desktop entry launches `/usr/bin/tornadotty`. The final installed
  path inventory also still queried the obsolete package name `zentty`.
  Both checks now recognize/require the canonical public identities while
  retaining the old executable solely as a tested compatibility alias.
- The apparent hang was needlessly opaque: product output was redirected to a
  temporary file, PID discovery was silent, and durable evidence was copied
  only after the failure trap ran. The journey now emits timestamped
  `START`/`PASS`/`FAIL` phase lines, preserves them immediately in
  `build/linux/installed-package/progress-<backend>.log`, and reports the
  expected executable identities when PID discovery fails.
- PASS: the repaired real installed-product X11 journey completed in a
  controlled private Xvfb session. It verified package resources and metadata,
  real PTYs and CLI mutation, the controlled Codex adapter, compositor
  clipboard delivery, Open With, About/licenses, crash-and-restart topology,
  canonical desktop-entry launch, diagnostics silence, and a machine-readable
  final receipt at `build/linux/installed-package/x11.json`.

## Secondary public-copy and dynamic-title audit

- A targeted post-package audit found that the main identity constants had
  been changed but secondary public copy still named Zentty in settings
  descriptions, shortcut/preset dialogs, quit confirmation, agent settings,
  support-report text, executable/configuration errors, system sleep-inhibitor
  labels, and command-palette descriptions. These strings are public product
  identity, not retained internal namespaces, and now say Tornado TTY.
- The same audit found many real-system X11 journeys selecting windows by an
  exact `Zentty` title. Replacing that with an exact `Tornado TTY` title exposed
  the deeper flaw: the main window title is intentionally dynamic and can show
  active worklane/project context. Shared input now locates a mapped window by
  its owning PID; identity qualification separately requires the canonical
  `WM_CLASS`, and scenarios without the PID use the canonical application class
  rather than mutable title text.
- An initial identity rerun observed the old `com.zentty.zentty` class because
  `build/linux/bin/zentty-linux` was an old local bundle. The current tree was
  rebuilt with the documented build command; the rebuilt real X11 application
  then passed with `com.tamedtornado.tornadotty`. No source requirement was
  weakened to accommodate the stale artifact.
- The focused notification/settings journey reached the real freedesktop
  notification service but initially failed because its D-Bus receipt still
  expected application name `Zentty`. After correcting the public expectation
  to `Tornado TTY`, the journey passed through real GTK settings, physical
  input, private D-Bus, notification delivery, custom sound import/playback,
  persistence, deduplication, removal, and restart.
- PASS: product-input helper tests; TornadoTTY product-identity test; 7 core
  configuration tests; Linux binary compilation; 6 shortcut, 6 agent-settings,
  2 general-settings, 6 custom-sound, 3 sleep-inhibitor, 2 fleet, 20 tmux,
  8 action-router, 2 shortcut-registry, 5 bookmark, 3 close-runtime, and 2
  updates/privacy tests; real X11 window identity; real X11 About/licenses;
  real X11 notifications/settings.
- The focused config-store module initially remained **39 PASS, 1 FAIL** at the
  exact-maximum rewrite boundary. Inspection showed a stale fixture rather than
  a product-size exception: its 1 MiB source omitted the newer
  `start_restored_sessions_in_background` field, so a General-settings rewrite
  correctly added that owned field and refused to publish the resulting
  over-limit document. The fixture now includes every General-owned field,
  keeping the exact-limit rewrite size-neutral while retaining the one-byte-over
  rejection. PASS: the complete config-store module now runs **40/40**.
