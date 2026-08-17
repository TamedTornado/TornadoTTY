# Zentty Linux package lifecycle dogfood — 2026-08-17

This record covers GH-53. It is append-only for discoveries, failures,
repairs, receipts, and remaining limitations from the real Debian package
lifecycle journey.

## Ratified test design before implementation

- Package transitions use the host's real `dpkg` and package database format,
  but only below a fresh `/tmp/zentty-dpkg-lifecycle.*` root.
- The journey runs inside a Bubblewrap user and PID namespace. The host root is
  read-only; only the disposable install root and receipt directory are bound
  writable. Namespace UID/GID 0 lets `dpkg` exercise real root ownership while
  mapping those writes back to the invoking developer account outside the
  namespace.
- The harness passes an explicit root and log path to every `dpkg` invocation.
  A probe showed that `dpkg --root` relocates its database but does not relocate
  the default `/var/log/dpkg.log`; leaving the log implicit correctly failed
  against the read-only host root. The production journey must never rely on
  that failure as isolation and instead writes the log inside the disposable
  root.
- Dependency configuration uses real `dpkg --force-depends` rather than a fake
  package database populated with invented system packages. Dependency and ELF
  correctness remain owned by the GH-52 package audit; GH-53 owns transaction,
  path, and package-database behavior.
- Exact package ownership is the union of the expanded manifest's files and
  symlinks plus the directory entries actually declared by the Debian archive.
  Every archive directory must be root:root mode 0755 and an ancestor of a
  manifest path. `dpkg-query -L` must equal that archive path set.
- The committed `upgrade-fixture-v1.json` defines the oldest supported
  lifecycle fixture. It is deterministically repacked from the candidate
  payload at Debian version `0.1.0~fixture1` and adds one package-owned legacy
  file that the candidate upgrade must remove. It is deliberately not
  represented as a previously published release.
- A failed-upgrade package is a test-only repack with a higher version and an
  injected `preinst` that exits 42. Actual Zentty packages contain no
  maintainer scripts. This proves the previous package/database state remains
  usable when Debian aborts before unpack; it does not claim general rollback
  after arbitrary maintainer-script or unpack failures.
- User fixtures cover configuration, workspace/state, custom theme, custom
  sound, cache, and runtime data. Their exact snapshot and unrelated system
  sentinels must survive every install, reinstall, upgrade, failed upgrade,
  remove, purge, and repeated cycle.
- The machine receipt records every transition, exact package versions,
  archive/package-database ownership hashes, payload fingerprints, user and
  sentinel hashes, injected failure status, and the precise rollback boundary.
  Missing Bubblewrap/user-namespace support is a prerequisite failure, never a
  pass.

## Initial probes

- A disposable real install succeeded with no maintainer scripts. Viewed from
  inside the user namespace, installed payload ownership is root:root; viewed
  from the host, namespace root maps back to the invoking developer UID/GID as
  expected.
- In a root containing one unrelated `/usr/share` sentinel, real `dpkg
  --remove` removed all Zentty files and empty package directories while
  preserving the shared directory and sentinel. Because Zentty declares no
  conffiles or maintainer scripts, the package database retained no residual
  `config-files` record after removal.
- The first complete isolated journey failed before installation because
  `dpkg-deb` creates a temporary control-member file while reading metadata.
  The inherited `/tmp` was intentionally read-only with the host root, so that
  implicit write was denied. The wrapper now sets `TMPDIR` to the disposable
  root's private `var/tmp` (and `HOME` to its private root home) rather than
  making ambient host temporary storage writable.
- The first end-to-end transaction sequence then passed all nine transitions:
  fresh install, same-version reinstall, default remove, oldest-fixture
  install, supported upgrade, injected failed upgrade, explicit purge, and a
  second install/remove cycle. The candidate archive declares 829 dpkg-owned
  paths: 690 manifest files/symlinks plus 139 reviewed ancestor directories.
  `dpkg-query -L`, archive ownership, root:root ownership, modes, symlink
  targets, SHA-256 content, and `dpkg --verify` all agreed.
- The initial correct audit took about 99 seconds because it launched `stat`,
  `sha256sum`, and `jq` separately for hundreds of paths during every
  transition. That repeated-process design was unnecessary test overhead. The
  audit now batches filesystem metadata with one `find`, joins it against the
  archive path set using an unambiguous unit separator, and verifies all file
  hashes with one `sha256sum -c` invocation. The complete nine-transition
  real-dpkg journey fell to about 41 seconds without dropping a contract.
- During that optimization, two focused failures were retained rather than
  hidden: the expected-directory stream was initially not connected to its
  loop, and the archive-key join initially inserted an empty column. Unified
  metadata diagnostics exposed both. The final batched audit compares the
  exact 829-path package database/archive set and exact 690-path expanded
  manifest set before checking metadata and content.
- The injected `preinst` exits 42 before unpack. The raw dpkg log shows Debian
  briefly entering half-configured/half-installed states and restoring the
  prior candidate version to `installed`; the candidate payload fingerprint
  remains identical. This evidence supports only the explicit
  `injected-preinst-before-unpack` boundary. The machine receipt sets
  `general_rollback_claim` to false.
- Default remove and explicit purge both pass with no residual package database
  record because the candidate has no conffiles or maintainer scripts. Every
  user XDG fixture and unrelated system sentinel retained its original exact
  snapshot. The receipt now includes before-state hashes and after-state user,
  system-path, package-status, and payload fingerprints for each transition,
  plus raw dpkg and failed-upgrade log digests.

## Qualification notes

- The first post-commit focused-suite command used the nonexistent historical
  names `debian-package-policy-test`, `qualification-matrix-validate`, and
  `architecture-validate`. It stopped immediately before executing a test.
  The repository's authoritative entry points are `packaging-policy-test`,
  `qualification-matrix --validate-only`, and `architecture-contract`; the
  corrected command passed those suites, the lifecycle runner negatives, and
  ShellCheck for every changed shell script.
- The clean-revision wrapper built commit `120f4a110ec448eb19f41d52e60468b6c537be5a`
  into `zentty_0.1.0+git120f4a110ec4_amd64.deb` and the resulting real-dpkg
  nine-transition journey passed. The builder boundary suite also passed its
  dirty-tree, missing-resource, wrong-Ghostty, and prepared-wrong-Ghostty
  negative cases.
- The first full matrix run exposed a clean-environment packaging defect that
  ambient developer caches had hidden. The Linux build downloaded and compiled
  its complete active dependency graph, but notice collection then asked Cargo
  for the unfiltered all-platform graph in offline mode. That graph included
  `windows-link` and `windows-sys`, so the package cell failed after the product
  build when the isolated Cargo cache correctly lacked an unused Windows-only
  crate. Notice collection now uses Cargo's host `--filter-platform` graph,
  matching what the Linux artifact can contain rather than downloading foreign
  dependencies to silence the test. The focused notice test compares the
  generated inventory with independently filtered metadata and rejects any
  `windows-*` package in Linux notices.
- The next full matrix run proved the package repair: `install-uninstall`
  passed its real build and nine dpkg transitions in the isolated root. It then
  exposed a separate application-transport limit in both X11 and Wayland CLI
  cells. Four real software-rendered GTK/compositor journeys were scheduled at
  once; the product accepted and logged each CLI request, but its main-loop
  dispatch did not return within the transport's generic two-second reply
  budget. Both CLI journeys passed unchanged when run concurrently without the
  other two desktops, and a package-build-plus-both-CLI reproduction also
  passed, ruling out the new package cell as the shared resource.
- This was not papered over by reducing matrix workers or inventing a scheduler
  resource: a user's CLI should not fail merely because the real GTK main loop
  is busy for two seconds. Application commands now have a bounded five-second
  end-to-end response budget on both client and server, while tmux, development
  server, raw-frame, and connection deadlines retain their existing tighter
  bounds. A real Unix-socket regression deliberately holds an authenticated
  application request beyond the old two-second ceiling and requires the
  structured reply to arrive successfully.
- The complete `zentty-agent-ipc` all-target suite passed with 93 tests,
  including the new delayed real-socket case; strict package Clippy, formatting,
  API schema, CLI inventory, and architecture contracts also passed. The
  focused timeout-selection mutation campaign tested five generated mutants:
  four were caught and one did not compile, with zero viable survivors.
- The first hand-launched four-journey stress command captured background PIDs
  through command substitutions, making them children of subshells that the
  supervisor could not `wait` for; it failed immediately and no processes
  remained. The corrected supervisor launches all four jobs directly and owns
  every PID. It reproduced the exact failed-matrix overlap—workspace/pane
  settings X11, agent settings X11, CLI X11, and CLI Wayland—with real staged
  ReleaseSafe Zentty, Ghostty PTYs, and controlled compositors. All four passed
  concurrently after the bounded application-response repair.
- The following full run passed both repaired CLI cells and the package cell,
  then exposed an older shortcut/settings X11 focus race while its Wayland twin
  and three other real GUI journeys ran concurrently. The log proved the
  opacity control received focus, but the harness's 50 ms polling loop could
  observe that historical marker only after it had already queued another Tab;
  its subsequent Home events therefore targeted the wrong widget. The exact
  failed X11 cell passed unchanged when rerun alone, confirming a stimulus/
  acknowledgement race rather than an opacity assertion mismatch.
- Shortcut/settings traversal now sends one physical Tab, waits for the
  product's next completed GTK focus-change receipt, and only then decides
  whether to advance. The same helper governs General, opacity, and Light Theme
  traversal so the stale-marker pattern cannot silently survive elsewhere in
  that journey. A missing compositor delivery remains a hard failure; the
  feature action itself is neither retried nor mocked.
- The repaired X11 and Wayland shortcut/settings journeys then passed
  concurrently with a second real Wayland Task Manager journey and the real
  custom-sound Cargo integration suite, matching the matrix's four-worker load.
