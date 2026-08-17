# Installed-package qualification dogfood — 2026-08-17

Issue: GH-54  
Parent: GH-9

## Purpose and non-drift constraints

GH-53 proved package-manager ownership. GH-54 proves that the payload installed
by that package is the product users can actually run. A staged bundle, an
unpacked archive executed through build-tree paths, or a copied binary is not
acceptable evidence.

This work extends the existing controlled X11/Wayland environments and existing
Rust product journeys. It must not introduce a second compositor supervisor, a
second application-control protocol, a model-only terminal, or an installed-
product mock. The tested application, Ghostty library and PTYs, CLI subprocess,
desktop entry, package resources, XDG state, and restart are real. Test-only
code may supervise the disposable package root, controlled display, stimulus,
and receipts.

## Test-first construction order

1. Define one installed-root contract that accepts the exact `.deb`, expanded
   manifest, and disposable installation root produced by the existing package
   pipeline. Reject dirty-tree, stale-revision, hand-copied, host-root, missing
   package-database, and mismatched-manifest inputs.
2. Add negative runner tests before the successful journey: source/build path
   leakage; forged package identity; missing installed resources; desktop-entry
   drift; clean-PATH escape; missing controlled-session identity; and a missing
   portal/compositor prerequisite becoming PASS.
3. Reuse the existing nested X11 and Wayland wrappers and product input/control
   helpers. Add an installed-layout mode to existing journeys where practical;
   keep package installation/resolution orchestration in one focused wrapper.
4. Run direct executable and desktop-entry launches from the dpkg-owned root.
   Exercise real single/multi-pane Ghostty PTYs, one CLI read and one mutation,
   clipboard and URI/file platform paths, installed shell integration,
   terminfo, agent wrapper and tmux shim, then restart with the original build
   directory hidden from the product namespace.
5. Inspect `/proc` for the application and observed children: executable,
   mapped files, argv, selected non-secret environment path values, and open
   package resources. Fail on any path under the Zentty source or build tree.
   Receipts must never serialize credentials or pane capability values.
6. Emit one machine-readable receipt containing display-session IDs, artifact
   and manifest hashes, Zentty and Ghostty revisions, launch modes, exercised
   command families, installed resource proofs, restart proof, and leakage
   audit. Promote explicit X11 and Wayland matrix cells only after both real
   journeys pass.

## Initial audit discoveries

- The Debian payload has `/usr/bin/zentty` and `/usr/bin/zentty-linux`
  symlinks into `/usr/lib/zentty/bin`, colocated Ghostty and GTK layer-shell
  libraries, installed Ghostty themes, agent wrappers, the tmux shim, desktop
  metadata, icons, AppStream metadata, and system terminfo.
- The existing product smoke derives resources from a staged bundle and the
  existing CLI contract deliberately asserts staged/source paths. Calling
  either unchanged would produce a green staged-product receipt, not installed
  evidence. Their display, PTY, input, API, and cleanup mechanics remain the
  systems to reuse; layout assertions need an explicit installed-root branch.
- `dpkg --root` proves package database transitions without populating an
  entire distribution root. Executing the payload requires a namespace that
  presents dpkg-owned `/usr` paths while retaining only documented host runtime
  dependencies. Merely pointing `LD_LIBRARY_PATH` at the extracted package is
  insufficient because it does not prove absolute desktop/resource paths.
- The first real installed X11 probe launched successfully from
  `/usr/bin/zentty-linux` in the controlled package namespace, created a real
  Ghostty PTY, delivered its OSC title, and completed lifecycle cleanup. It
  also exposed a package defect: Ghostty discovered `/usr/share/ghostty` from
  the installed terminfo sentinel, but the package did not own that resource
  directory. The successful terminal did not make the missing directory a
  pass; it would leave Ghostty's shell integration and runtime themes dependent
  on unrelated host state.
- The package contract now owns the pinned Ghostty runtime resources at
  `/usr/share/ghostty`. `build-local` stages them from the exact Ghostty install
  prefix; the closed manifest, ratified policy, documentation, and policy
  negative tests all require the new tree. Zentty's separately consumed theme
  catalog remains in its private application tree.
- A focused installed-root resolver now validates the real dpkg database,
  artifact/manifest revision and hashes, and every installed payload checksum
  before execution. It constructs a disposable merged-runtime namespace from
  symlinks to documented host runtime dependencies without mounting the Zentty
  source or build tree. Its negative suite rejects the host root, an
  uninstalled root, wrong manifest checksums, execution outside a controlled
  session, stale helper identity, modified payload, and source visibility. The
  fixture and the real qualified package CLI both executed successfully inside
  the resulting Bubblewrap namespace.

## Installed journey discoveries and repairs

- The first complete runner assumed that the nested compositor had made the
  caller UID 0. That is not part of either display harness contract. The runner
  now enters its own unprivileged user namespace with `unshare -Ur`; package
  installation therefore remains disposable and never requires host root.
- The provenance validator initially read obsolete top-level hash names. The
  package provenance schema owns the hashes at `.artifact.sha256` and
  `.manifest.sha256`; the runner now validates those authoritative fields.
- A normal restored shell does not promise that Zentty's tmux compatibility
  shim is first on `PATH`. Qualification now invokes the exact dpkg-owned shim
  and proves that it observes all three real panes rather than accidentally
  accepting a host tmux installation.
- Initial `/proc` discovery could select a stale product from another journey,
  and `readlink -f` translated namespace paths through the host mount. Direct
  launches are now tied to the supervised descendant process tree, executable
  identity uses the raw `/proc/<pid>/exe` target, and desktop-entry launch is
  accepted only after the direct applications have exited. Environment and
  mappings remain hashed only after explicit source/build leakage checks.
- Cleanly releasing every PTY intentionally closes the panes, so it cannot
  prove crash recovery of a live topology. The journey now waits for the real
  debounced snapshot containing the CLI rename and three panes, deliberately
  kills the supervised namespace, and verifies that a new installed process
  restores that exact live topology before completing cleanly.
- `gtk-launch` follows the user's configured login shell rather than a caller's
  `SHELL` variable. The desktop-entry proof now uses a private, real Ghostty
  configuration whose `command` is the controlled receipt shell. This tests
  Ghostty's installed configuration path instead of relying on an ineffective
  environment override.
- Package building was accidentally started twice while a long-running command
  was being polled, producing a transient `Directory not empty` collision in
  the shared build tree. This was orchestration error, not a product failure;
  the build was rerun once, serially, and completed. Future polling must use the
  returned process session rather than launching or matching the command again.

## First controlled installed-product receipts

The real dpkg-owned payload now passes both controlled display journeys with a
clean `/usr/bin:/bin` path, no source/build mount, direct launch, desktop-entry
launch, three real Ghostty PTYs, CLI read and mutation, tmux observation, live
snapshot crash recovery, installed shell integration and terminfo, and three
process audits per backend:

- X11 session `fce05710555166b349f4057df1532d735e7e517873c68deaf9b1f0ab02ccc73d`
- Wayland session `8e9c2363470c7f49bcc58abfa9eaac946b9d659f26c959c555c5b63462b27cc0`
- package SHA-256 `c11b4dea05b4d997c19f4feae60be5c973c674ac7481af535ba19f371b188745`
- manifest SHA-256 `629e6093cb2da649c20eb47efb68abe6a0907e0a9ba0a505aff7c2696dd86e03`

These are development receipts for commit `8a9e7ff37a50`; they are not final
qualification receipts because the runner repairs below are not yet committed
and the remaining installed clipboard, launcher, and agent-wrapper behavior has
not yet been promoted into matrix cells.

## Qualification boundary

This feature may make the installed X11/Wayland cells pass. It does not imply
release qualification or full Linux qualification while any matrix cell is
`BLOCKED`, `XFAIL`, or `NOT_IMPLEMENTED`. Any Valgrind success is described only
as **PASS with reviewed suppressions**.
