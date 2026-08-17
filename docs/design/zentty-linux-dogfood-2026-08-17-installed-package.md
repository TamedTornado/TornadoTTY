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
qualification receipts because the runner repairs landed later and the
remaining installed clipboard, launcher, and agent-wrapper behavior had not
yet been promoted into matrix cells.

The next test-first extension closes those remaining representative paths
without importing another journey: a pane writes OSC 52 through the real
installed Ghostty surface and the outer controlled compositor reads it; the
exact installed Codex wrapper and CLI exec a controlled agent binary; and the
existing physical-input helper selects an installed Open With target whose
real process receipt proves the canonical working directory. X11 therefore
requires `xclip`, while Wayland requires the existing input-capable compositor
profile and `wl-paste`; absence is a failed prerequisite, never a pass.

The first extended X11 run failed an over-specific test assertion: the wrapper
correctly preferred its colocated dpkg-owned CLI at
`/usr/lib/zentty/libexec/zentty/agent-wrappers/shared/zentty`, while the test
expected the general `/usr/lib/zentty/bin/zentty` entry point. The colocated
copy is deliberate so hooks remain bound to the same installed payload even
when PATH changes. The temporary diagnostic proved the full Codex hook
arguments and original probe argument reached the subprocess, but it was
subsequently replaced by the reviewed actor boundary described below.

The orchestration contract then correctly rejected an inline fake Codex
program in the installed runner. The runner now copies the single reviewed
`controlled-agent` actor into its source-hidden namespace, selects its
`codex-permission` profile, and requires both successful wrapper return and the
authenticated application event. It deliberately does not enable the actor's
verbose start receipt because that diagnostic contains the pane capability;
no installed qualification receipt may record that token.

The first Wayland extension then failed the OSC 52 clipboard assertion. Moving
the write among panes did not repair it reliably, and relying on the focused
pane's OSC timing would test a narrower Ghostty protocol path than Zentty's
user-facing Copy feature. The journey now reuses the existing physical-input
helper and real command palette to invoke Select All and Copy on the focused
installed surface. An independent `xclip`/`wl-paste` compositor client must see
the pane's sentinel. This is both more representative and removes the faulty
leader/focus assumption rather than weakening the assertion.

The first physical-copy retry correctly executed Select All but rejected the
Copy query because five legitimate palette entries match the word “Copy.” The
existing source journey deliberately specifies that count and activates the
already selected first result. The installed journey now uses the same closed
expectation (five), records pre-action counts to reject stale log matches, and
presses Return without the erroneous Down navigation.

The first full four-worker matrix exposed one installed-Wayland deadline bug:
the desktop receipt shell's 20-second safety deadline expired while the heavily
loaded host was scheduling the `/proc` audit, so the executable disappeared
between discovery and inspection. Standalone X11 and Wayland runs had passed,
which confirms this was a bounded-runner scheduling defect rather than a
desktop product failure. The desktop child and its supervisor now share a
60-second bounded release window; the audit still fails if identity is absent
or wrong and environmental absence is not converted into success.

That same overloaded run also reported pre-existing physical-input readiness
failures in source UX, remote Wayland drag, and shortcut settings. Their logs
showed input/deadline loss rather than product assertions. They remain failures
until rerun; qualification is not claimed from the partial receipt.

All four failed cells passed immediately when rerun sequentially against the
same build: installed Wayland session
`64b4b07ecbb50b3c07626431b8994292e49c4427c7e0a6c39b975347bc7ab17e`,
source UX X11 session
`5bfba1e10eabcce75a09382abdaa20507cefccb0f21f3d2f821b7bf26e2d89b1`,
remote-drop Weston session
`35630b8405ba8bbd9b511fd05743f321da9ad4bb3036a3de66551f2282fa000a`,
and shortcut-settings Wayland session
`c6bcc283dd61726051357b5a53218fc4791b5fe493f417dd3fcaf57f4c3c19c9`.
The package SHA-256 in that diagnostic run was
`7e59b34a59e0e0601960ff10ced70c4c259a134ef6091c1e62860b7c7bc9dd7f`.
These results justify the deadline repair but do not replace a complete clean
matrix receipt, which must still be rerun with bounded concurrency.

The two-worker complete rerun passed both new installed-product cells and every
previously failed input cell, but exposed a separate pre-existing 10-second
Openbox startup deadline in `development-servers-docker-x11`. Its standalone
rerun passed unchanged in session
`c0d13818c94def52848dbf627c26176bb8fd6cdec06ea7fc04185812fccb5316`.
The harness now allows 30 bounded seconds for the real window manager to publish
`_NET_SUPPORTING_WM_CHECK`; it still requires the exact root-window property and
does not turn missing Openbox into a pass. This receipt also demonstrated that
two matrix workers are counterproductive here: wall time grew from 761 seconds
to 1,465 seconds while one startup deadline still failed. Final qualification
therefore returns to four workers after repairing the observed bounded waits.

The next four-worker run passed the package cells and the repaired Docker cell,
then exposed two more pre-existing time assumptions. The platform clipboard
cell allowed only eight seconds for a real Ghostty surface to initialize; the
log stopped immediately after OpenGL load. Its shared exact-receipt waits now
allow 20 bounded seconds. Source UX used a fixed 300 ms sleep between the
Ctrl+Tab keydown and pointer selection; under load GTK had armed the hold timer
but had not displayed Peek before Tab was released, so the test clicked an
absent card. The first repair mistakenly waited for a nonexistent
`worklane-peek=shown` receipt; a standalone run rejected that harness defect
rather than silently passing. Worklane Peek now publishes a receipt from GTK's
real widget `map` signal, and the journey waits for a fresh exact
`worklane-peek=mapped` receipt before releasing Tab and clicking. Neither repair
retries a product operation or accepts absence; both replace wall-clock
assumptions with observed real-system state.

The corrected four-worker qualification completed cleanly against implementation
commit `e3b0953459bf755ab26854fa2382e4d04117d2ea`. All 155 PASS-declared matrix
cells passed, the one defect-linked XFAIL had its expected outcome, and every
support/negative runner test passed. The run took 821,490 ms; its machine
summary SHA-256 is
`21633ff04af2d1c75cb379d61cc9cef7f7a7e1937a56aa4165c1799d3a87e0d1`.
The exact installed Debian artifact was
`zentty_0.1.0+gite3b0953459bf_amd64.deb`, SHA-256
`4d7ebf2a52da26e70d7cc78b8bb7eaa6d447cc6af671adb1ba4155b61ccde843`,
containing Zentty commit `e3b0953459bf755ab26854fa2382e4d04117d2ea` and Ghostty
commit `281d7d7dbeab24c1a2d04f6d3c720c34dbfac645`. Its installed manifest
SHA-256 was
`1da71f9f96b94bf727decd656a94a9674e9ebe828807366b974d6a43a7a0b288`.
The controlled installed-product display sessions were
`e74988f59dc0d171a86a1d2bdb84c5330cffed25253fbf091659a874762d9c99`
for X11 and
`7315c65feae451f63d0cb6e86b3b906030dd6ab7ebbdd829ce0e99182eede201`
for Wayland. Both receipts prove direct, restart, and desktop-entry launches;
real single/multi-pane PTYs; CLI read/mutation; persistence; installed tmux and
agent paths; clipboard and Open With actions; shell integration and terminfo;
desktop identity; clean `/usr/bin:/bin`; and no source/build leakage or recorded
credentials across all audited processes.

Debug Valgrind is **PASS with reviewed suppressions**, never described as an
unsuppressed clean result. The preserved unsuppressed receipt contains 427
errors/contexts, 6,160 definitely lost bytes, and 41,428 indirectly lost bytes.
The paired reviewed-suppression receipt contains zero post-suppression errors,
contexts, definite bytes, or indirect bytes and exactly 427 suppressed
errors/contexts. Suppression governance passed and the summary records both
receipt identities. ReleaseSafe Valgrind remains the declared XFAIL; no rule
was broadened to make it green.

## Qualification boundary

This feature may make the installed X11/Wayland cells pass. It does not imply
release qualification or full Linux qualification while any matrix cell is
`BLOCKED`, `XFAIL`, or `NOT_IMPLEMENTED`. Any Valgrind success is described only
as **PASS with reviewed suppressions**.
