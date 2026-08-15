# Zentty Linux dogfood: platform contracts and clipboard qualification

Date: 2026-08-15

## Trigger

The authoritative matrix had remained at 22 `NOT_IMPLEMENTED` cells while
substantial feature work landed. The operator correctly identified that the
delivery order was no longer retiring explicit qualification debt. This slice
therefore targets the coherent Linux platform boundary rather than another
unrelated feature: XDG paths, URL/file opening, detached process launch,
aggregate settings coverage, and real X11/Wayland clipboard semantics.

## Discovery: duplicated platform mechanics

The product already had strong feature-specific implementations, but no
single reviewed platform contract. Configuration, bookmarks, themes, custom
sounds, tmux compatibility, and restoration each repeated part of the XDG
fallback logic. Open With and development-server browsers each spawned direct
executables independently, while project branch/PR opening bypassed GIO and
called `xdg-open` directly. Those implementations were not all incorrect, but
the duplication made the four platform matrix cells honestly
`NOT_IMPLEMENTED` and allowed their behavior to drift.

### Repair

`zentty_linux::platform` now owns only Linux mechanics:

- absolute XDG user-directory resolution, empty-value behavior, standard HOME
  fallbacks, runtime-directory validation, and traversal-safe owned paths;
- shell-free process spawning with exact OS-string argv, CWD, environment,
  stdio, failure, detached reaping, and explicit child ownership; and
- GIO default-application dispatch for validated URIs and absolute files.

Product policy remains with existing authorities. The shared path resolver is
consumed by configuration, restoration, bookmarks, themes, custom sounds, and
tmux state. Open With and server-browser executable plans use the shared
process boundary. Project branch/PR opening and the default server browser use
the shared GIO boundary. No second config, process, or launcher authority was
introduced.

An independent `platform_contract` actor exercises the public boundary in the
real isolated-session harness. It verifies all private XDG roots and traversal
rejection; launches itself with hostile argument boundaries, exact environment
changes and a CWD containing spaces; observes spawn failure; signals and reaps
a real child; registers a real GIO default application; and receives exact
hostile URI and spaced file targets in a separate process.

## Discovery: primary selection depends on compositor capability

The existing Clean Copy journey already used real Ghostty selection, the
standard compositor clipboard, an independent `xclip`/`wl-paste` reader, and a
physical paste back through the real PTY. It did not inspect the platform
primary-selection channel, so the two clipboard matrix cells correctly
remained open.

The first new Wayland run used the established input-capable Cage profile. It
failed explicitly: `wl-paste --primary` reported that the compositor does not
support primary selection. This was not converted to PASS. The next attempt
used nested Weston, which advertises data-device support, but the focused
command-palette keyboard journey did not receive the outer-seat chord even
after aligning input dispatch with the shared helper. Weston therefore was not
used as evidence.

The controlled labwc profile supplies a real nested Wayland seat,
`xdg-activation-v1`, and primary-selection support. Under that profile the
same staged product journey passed: Ghostty Select All published the exact raw
selection to the primary channel; Clean/Raw/Markdown commands published exact
standard clipboard bytes; and the cleaned bytes pasted back through the real
PTY. Controlled X11 passed the same assertions through Xvfb, `xclip`, physical
input, real Ghostty, and the real PTY.

The focused clipboard path now exits after completing these clipboard
assertions so its dedicated cells do not rerun the unrelated global-search
journey.

## Discovery: broad local test invocation needs network namespace permission

A non-elevated `cargo test -p zentty-linux --all-targets` ran 244 binary tests
successfully and then failed the existing real `/proc` listener-correlation
test when the sandbox denied its live kernel-listener operation with
`Operation not permitted`. This is an environmental denial, not a platform
code failure and not a PASS. The authoritative rerun must execute that real
test with the permission already used by qualification rather than weakening
or replacing it.

## Matrix reconciliation

Six cells are changed from `NOT_IMPLEMENTED` to executable `PASS`
declarations, contingent on their commands succeeding in the final matrix
run:

- `platform-xdg-paths-contract`;
- `platform-open-url-file-contract`;
- `platform-process-launch-contract`;
- `platform-settings-contract`;
- `platform-clipboard-x11`;
- `platform-clipboard-wayland`.

The settings aggregate now depends on all 19 existing page/config/display
cells rather than treating unavailable optional agent binaries as a failure
of truthful capability presentation. It does not fabricate agent
availability: it requires the real page journeys, source-exact nine-section
registry, configuration tests, feature inventory, and truthful unavailable-
state behavior already owned by the product.

Before final qualification, the declared matrix delta is therefore expected
to be `PASS 132 -> 138` and `NOT_IMPLEMENTED 22 -> 16`, with total cells
unchanged at 162. This expectation is not a result receipt; final totals will
be recorded only after the authoritative runner completes.

## Mutation testing discovery and repair

The first focused `cargo-mutants` baseline correctly honored the repository's
permanent `gitignore = true` and `copy_target = false` policy. Because the
ignored 14 GiB `build/linux-deps` tree was no longer copied, the scratch build
could not find Ghostty's prepared shared library. This was a clean baseline
failure with zero mutants executed, not a product failure and not a reason to
disable the disk-safety policy.

The rerun supplied the already reviewed Ghostty library through the build
script's supported absolute `GHOSTTY_LIB_DIR` input. The first mutation result
was 18 caught, 7 missed, and 3 unviable. The missed mutations exposed real
test weaknesses: exact XDG error identity, positive safe-path acceptance,
successful URI/file conversion, and detached-spawn failure were not asserted
by the focused library tests even though the external actor covered their
larger journeys.

Those focused seams were strengthened without replacing the real actor. The
final result is **29 mutants: 26 caught, 3 unviable, 0 missed, 0 timed out**.
The run retained `gitignore = true`, did not copy the build tree or Cargo
target, and left no cargo-mutants scratch directory behind.

## First authoritative rerun: failures retained, then repaired

The first 162-cell rerun completed in 582.920 seconds and did **not** qualify:
six declared-PASS cells were not green. The receipt correctly reported every
failure rather than converting environmental or dependency absence to PASS.

- `architecture-contract-v1` found that the architecture mirror still carried
  the six old `NOT_IMPLEMENTED` declarations. Both the mirror and its axis
  validator now follow the authoritative matrix; the validator's negative
  self-tests pass.
- `git-review-context-x11` and `git-review-context-wayland` exposed a genuine
  harness drift after project-context opening moved from `xdg-open` to GIO.
  The real-product test had continued faking an executable named `xdg-open`.
  It now registers a private desktop application as the default HTTP/HTTPS
  handler and observes the exact branch and pull-request URI in that separate
  process. Controlled X11 and Wayland reruns both pass.
- `rust-global-find-product-usage-wayland` inherited the new primary-selection
  assertion while running under Cage, which deliberately lacks that protocol.
  The primary assertion is now scoped to the two dedicated clipboard cells;
  the broader real-product search journey retains its independent standard-
  clipboard and PTY round trip. Its controlled Wayland rerun passes.
- `config-live-reload-wayland` reported an interrupted sibling write as
  published during the heavily parallel full run. An immediate isolated rerun
  passed the real interrupted-write, partial-write, symlink-retarget,
  last-good, and external-writer journey. Inspection showed that the harness
  captured its baseline before the preceding accepted transaction's final
  coalesced notification was necessarily quiescent under host load. It now
  requires ten consecutive stable samples before introducing the killed
  sibling writer. The repaired controlled Wayland journey passes; the final
  authoritative rerun must still prove it under qualification load.
- `platform-settings-contract` was blocked by the failed config dependency and
  therefore made no aggregate claim.

A manual attempt to run the settings aggregate through an ad-hoc private HOME
also caused rustup to seek a toolchain over the network. That was an invocation
error: the authoritative isolated-session profile preserves the installed
toolchain environment while isolating product HOME/XDG state. No product cell
was weakened to accommodate the ad-hoc command.

## Final pre-commit qualification receipt

After the repairs above, the authoritative runner executed all presently
executable support and matrix cells in 633.540 seconds. Every declared-PASS
cell passed. The exact declared totals are:

- `PASS`: 138;
- `FAIL`: 0;
- `BLOCKED`: 7;
- `XFAIL`: 1;
- `NOT_IMPLEMENTED`: 16;
- total: 162.

The machine claims are deliberately narrower than an exhaustive release:
`implemented_local_suite_passed=true` and
`product_boundary_qualification_passed=true`, while
`release_qualification_passed=false` and
`full_linux_qualification_passed=false` because the 24 explicit non-PASS
cells remain visible. This is not a claim of exhaustive QA.

Debug Valgrind is **PASS with reviewed suppressions**, not an unsuppressed
clean result. Its preserved raw receipt reports 427 errors/contexts, 6,080
definite bytes, and 41,363 indirect bytes. The reviewed effective suppression
set reduces the post-suppression totals to zero errors/contexts and zero
definite/indirect bytes; suppression governance is `ACCEPTED`. ReleaseSafe
Valgrind remains `XFAIL` as required.
