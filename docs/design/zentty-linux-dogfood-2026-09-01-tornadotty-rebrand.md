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
- TornadoTTY owns the fork's conventional `v<version>` release namespace and
  uses TornadoTTY artifact names. Upstream Zentty tags are not mirrored into
  this fork; explicitly needed upstream tags must use a distinct local name.
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
  At that point both checks recognized the canonical public identities while
  retaining the old executable as a compatibility alias. The GH-166 decision
  below supersedes that installed-alias policy.
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

## 2026-09-04 installed-name correction (GH-166)

- Dogfooding exposed that the installed TornadoTTY launcher still resolved to
  `/usr/lib/zentty/bin/zentty-linux`. The earlier rebrand deliberately retained
  that path, but it is an observable installed-product name rather than a
  source-only implementation detail. GH-166 supersedes that boundary decision.
- The canonical private application root is now `/usr/lib/tornadotty`; its GUI
  and automation executables are `bin/tornadotty` and
  `bin/tornadotty-cli`. The only public command links are
  `/usr/bin/tornadotty` and `/usr/bin/tornadotty-cli`.
- Rust crate names, `ZENTTY_*` environment variables, XDG state directories,
  log prefixes, and internal `share/zentty` and `libexec/zentty` subtrees remain
  unchanged. They are compatibility or implementation contracts and were not
  swept into a cosmetic rename.
- `Provides`, `Conflicts`, and `Replaces` metadata still names the old `zentty`
  package so package managers can perform a clean upgrade. The lifecycle
  fixture now owns the two old command aliases and an old-root sentinel, and
  requires all three to disappear on upgrade without touching user data.
- Focused contract results: identity policy PASS; identity negative fixtures
  PASS; Debian packaging policy PASS; packaging negative fixtures PASS; Arch
  packaging policy PASS; Arch artifact-auditor fixtures PASS; isolated
  installed-product-root fixture PASS; `agent_runtime` path-resolution tests
  PASS.
- The first real Debian candidate passed structural audit (1,298 declared
  files) and all nine disposable dpkg lifecycle transitions, including removal
  of the old owned paths and preservation of user XDG data. Executing its
  private binaries then exposed one remaining public leak:
  `tornadotty-cli` printed a `zentty: usage: zentty ...` error. The CLI prefix,
  usage strings, launch diagnostics, server diagnostics, and tmux CLI
  expectations now use `tornadotty-cli`/`TornadoTTY`; the internal Cargo binary
  target remains `zentty`.
- PASS: focused public CLI identity process test and five real Unix-socket tmux
  CLI process tests. The package structural suite now executes both packaged
  binaries and rejects obsolete public names. A final current-revision Debian
  rebuild/lifecycle and a real Arch/Omarchy artifact remain required; none of
  these focused results is a release-qualification claim.
- A direct full IPC-crate run from a dogfooded Codex pane initially failed three
  “outside a managed agent” launch cases because the real pane correctly
  exported `ZENTTY_AGENT_TOOL=codex`. Serial execution reproduced the same
  failures, disproving an initial concurrency hypothesis. Re-running the full
  24-case launch process suite with only that ambient marker removed satisfied
  the cases' documented outside-agent precondition and passed. No assertion or
  production behavior was changed to hide the environmental mismatch.
- Final GH-166 reconciliation found one installed-layout contradiction outside
  the manifest: Debian copyright metadata still named the old
  `usr/lib/zentty` paths and identified the public upstream name as Zentty.
  The metadata now names TornadoTTY and the actual
  `usr/lib/tornadotty/lib` payload. The existing packaging-policy runner owns
  this contract, accepts an injectable copyright fixture, and has a negative
  case proving that either legacy library path is rejected.
- The first current-artifact installed-product run failed at bootstrap before
  creating its controlled namespace because the journey expanded an unset
  `TMPDIR` under `set -u`. Requiring callers to manufacture that optional
  variable would hide the portability defect. The journey now uses `/tmp` when
  `TMPDIR` is absent and continues to propagate an explicitly supplied value
  into its clean product environment.
- The next invocation omitted the matrix cell's declared `nested-x11-v1`
  environment and correctly failed instead of treating the developer desktop
  as controlled evidence. This was an operator invocation error; no code or
  prerequisite status changed to make it pass.
- The correctly wrapped X11 rerun proved the `.deb` archive was clean but the
  prepared runtime root was not: `installed-product-root` linked every missing
  host `/usr/bin` and `/usr/lib` child into the disposable namespace. Because
  the workstation still has the withdrawn package, this reintroduced
  `/usr/bin/zentty`, `/usr/bin/zentty-linux`, and `/usr/lib/zentty`; the same
  mechanism could let a host TornadoTTY command mask a missing package entry.
  Host fallback linking now excludes both canonical package-owned commands and
  roots and all legacy aliases while retaining unrelated runtime tools and
  libraries. A synthetic host-tree test proves both sides without depending on
  the CI host's installed packages.
- With the helper repaired, the focused installed-package X11 journey passed
  against Debian artifact SHA-256
  `ac7ff32dac1037fce0e66cdf22cf315f097f201e3e371e8435a01bed042c4d75`.
  It exercised the packaged GUI and CLI, real PTYs, controlled agent launch,
  clipboard, Open With, About/notices, injected crash and topology restore,
  canonical desktop-entry launch, and final diagnostics. The controlled X11
  session was `4988d13132b2e2d82846dec4565664c4582d98fdee5d50afeff8603e6a25309e`.
- After committing the helper repair, the final exact package source was
  `db2361b904f7d0710e1317c77357560144bffd5c`. Its Debian artifact again passed
  the 1,306-file structural audit and all nine disposable lifecycle
  transitions. The artifact SHA-256 is
  `c05bd54d4f1145f7998e5186928c2cbc5f56f54a14ec9cd0d75706cea9d277c2`;
  the lifecycle summary SHA-256 is
  `f19b255ed7abad82046538be66f88c778a3e4e4a29fac9fd1710555fe9942f1f`.
- The final exact-revision installed-product X11 journey also passed in
  controlled session
  `6003330521bd7e435772c47f771e3f0262b1c5fe84e9e9dc96193070bc5d7f3b`.
  Its receipt SHA-256 is
  `c88dd43d221c62e75f17e409c30da1c9890b199d2b38bd872941a85675b20836`.
- The configured real Omarchy host `jason@omarchy-laptop.local` was not
  reachable for the remaining Arch artifact/lifecycle run: mDNS returned
  `No appropriate name servers or networks for name found`. No Arch or
  Omarchy pass is claimed from environmental absence, and GH-166 remains open
  until that prerequisite is available.

### Real Omarchy closeout

- When the Omarchy laptop returned, its `.local` mDNS name still did not
  resolve from the development workstation. An active-neighbour candidate was
  rejected after inspection showed Ubuntu 22.04 on host `Jason-Plex`, not
  Omarchy; the original known-hosts file was restored before continuing. Local
  DNS then resolved `omarchy-laptop.lan` to the actual host, whose existing
  trusted SSH key matched and whose OS identified as Omarchy 4.0.2 on x86-64.
- The existing clean laptop checkout at
  `~/Projects/zentty-release` fast-forwarded from `c27e0b35` to exact source
  revision `4a02c9f3397c7474a6549c29edc4fe4730d32a2d`. The isolated release-tool
  prefix supplied Cargo 1.100.0-nightly, Rust 1.100.0-nightly, Zig 0.16.0,
  Blueprint Compiler 0.22.2, and patchelf 0.19.1; no system package was
  installed. The builder created a fresh managed Ghostty checkout at pinned
  revision `bab8c088f45e47a00ce3bfe2c142d6cb51ecd200`.
- PASS: the real native Omarchy package build completed with the Cargo publish
  age audit at 91 packages and zero exceptions, the notice catalog at 79 Cargo
  and 27 Ghostty inputs, and the structural Arch audit at 1,305 declared files.
  The package is
  `tornadotty-0.1.1+git4a02c9f3397c-1-x86_64.pkg.tar.zst`, SHA-256
  `44904a70f7f58711195667abcafe2a51de4e1e296faddfbe0cd567d55c87171c`.
  Its expanded manifest SHA-256 is
  `b8e25d36e301c6618e382b83d1c60b2f0fc984d151201b34b21721b57171180a`;
  its provenance SHA-256 is
  `d0853f1c85d5f25a727584cb382aca14d9368d67c89bfa7af598fb8e102e8235`;
  and the native build log SHA-256 is
  `48de9f192860dae477dbc3611ba0c7a21c34fde346e9372001fc46d07ab05915`.
- The first lifecycle invocation stopped at preflight because the separate SSH
  command did not export the isolated `patchelf` prefix. Correcting that
  operator environment exposed a real harness defect: the old `zentty` package
  installed, but pacman's `--noconfirm` retained the safe default of "no" for
  removing a conflicting package, so the replacement transaction failed. The
  lifecycle also redirected pacman's diagnostic to a temporary log that its
  failure trap deleted, initially producing an opaque exit status.
- A disposable namespace reproduction proved that pacman question mask `4`
  answers only the package-conflict removal question and successfully replaces
  `zentty` with `tornadotty`. The lifecycle now uses that mask only when a prior
  package is supplied, routes mutating pacman operations through one checked
  logging helper, and prints the retained pacman log before an explicit
  operation failure. The Arch policy test owns regression assertions for the
  conflict answer and failure diagnostics.
- PASS: the repaired real lifecycle installed retained package
  `zentty-0.1.1-1`, upgraded to
  `tornadotty-0.1.1+git4a02c9f3397c-1`, verified the installed payload and
  public help contract, removed legacy paths, uninstalled TornadoTTY, and
  proved the real host pacman database remained unchanged. Its receipt SHA-256
  is `d12922620e6d6c4114f97888604ff7fb7b7fc673805d642baefcf653cb92911f`;
  the captured lifecycle log SHA-256 is
  `bae29880d2e1470635bf315818aa857d8abd529a4b52b759989b928c9910356c`.
- The final focused package gate found one additional safety-ordering defect:
  the Debian private lifecycle inspected the candidate archive before rejecting
  a non-disposable root. That made the root-guard negative test fail on an
  unrelated read-only temporary-directory error. Root validation now precedes
  every package operation. PASS: rebrand identity policy and negative cases;
  common packaging policy and negative cases; Arch policy and negative cases;
  Arch artifact-auditor fixtures; Debian builder contracts; Debian lifecycle
  negative/receipt fixtures; installed-product-root isolation; Bash syntax;
  ShellCheck; and diff hygiene.
- This is GH-166's issue-sized package qualification, not a full Linux or
  release qualification run. The package payload remains bound to exact source
  `4a02c9f3`; the subsequent changes are lifecycle-test diagnostics and safety
  ordering only and do not change the built product.
