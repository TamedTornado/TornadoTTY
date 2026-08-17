# Dogfood record: Linux package qualification closure

Issue: GH-55

## Objective

Close the Debian packaging epic with one auditable, versioned receipt graph.
The graph must connect the exact clean source revision to its reproducible
artifact, structural/security audit, real dpkg lifecycle, controlled installed
X11 and Wayland journeys, and public operator documentation. Staged-bundle
evidence remains separate from installed-package evidence.

## Starting audit

GH-51 through GH-54 are closed and their real-system journeys pass, but the
authoritative matrix exposes most lifecycle evidence through the single
`install-uninstall` producer cell. That producer already performs fresh install,
same-version reinstall, supported upgrade, injected pre-unpack failure, remove,
purge, and a repeated install/remove cycle. Re-running that expensive journey
once per label would add cost without increasing realism. The closure therefore
keeps one real lifecycle producer and adds command-backed evidence cells that
validate disjoint portions of its signed-by-digest machine receipt. A validator
may not invent an outcome, skip a missing receipt, or accept a receipt from a
different artifact/revision.

The repository also has a real clean-checkout build journey and a two-build
reproducibility journey, but neither is represented in the matrix. Running both
would build the package three additional times. The planned replacement builds
once from a clean detached clone while masking the developer checkout, compares
all four release outputs byte-for-byte with the already-built exact-revision
artifact set, and emits a machine receipt. This proves clean construction and
cross-path reproducibility with one additional build.

## Implementation plan

1. Extend the clean-checkout journey to consume the exact primary artifact set,
   compare every release output, and publish revision, package, manifest,
   provenance, checksum-file, environment, and digest evidence.
2. Add a lifecycle evidence validator with explicit scenarios for
   install/reinstall, upgrade/expected failure, and remove/purge/residue. Each
   scenario validates the complete base receipt first and then its own closed
   transition subset.
3. Add a package audit evidence validator that binds artifact, manifest,
   provenance, checksums, Debian metadata, dependency metadata, payload audit,
   and forbidden-content claims.
4. Generate one schema-versioned package qualification summary only after the
   clean-build, audit, lifecycle, and installed X11/Wayland cells pass. Embed
   source receipt hashes and exact claims rather than copying unaudited prose.
5. Add matrix cells for each evidence family and runner negative tests for
   missing cells, stale revision/artifact, checksum mismatch, missing installed
   backend, source leakage, unexpected outcome, and false release/full claims.
6. Publish baseline, dependency, install, verification, upgrade,
   uninstall/purge, retained user-data, provenance, checksum, and limitation
   documentation.
7. Run every presently executable matrix cell after the implementation. Do not
   call release or full Linux qualification passed while required BLOCKED,
   XFAIL, or NOT_IMPLEMENTED cells remain. Describe Debug Valgrind only as
   **PASS with reviewed suppressions** and leave ReleaseSafe Valgrind XFAIL.

## Non-goals and boundaries

- No host-root package installation.
- No claim of general transactional rollback after unpack begins.
- No AppImage, Flatpak, RPM, or non-Ubuntu baseline claim.
- No duplicate fake package manager or fake desktop product harness.
- No credentials, test actors, logs, mutation scratch trees, or build-tree
  paths in the shipped payload.

## Discoveries, failures, and repairs

This section is append-only during implementation. Every rejected assumption,
negative-test finding, harness failure, product defect, and final limitation is
recorded here before GH-55 closes.

The first implementation audit confirmed that `build-deb` already performs the
closed payload, dependency-floor, ELF hardening, RUNPATH, notice, credential,
source-path, static-archive, test-artifact, desktop, and AppStream audits before
creating an artifact. The closure does not add another package builder. It
binds those checks to an exact artifact/revision audit receipt and independently
replays the audit's negative fixtures.

The lifecycle journey likewise already has a strict nine-transition receipt
validator. Separate matrix cells now validate install/reinstall,
upgrade/expected failure, and remove/purge/residue subsets only after validating
the complete producer receipt and its raw log hashes. This makes the matrix
explicit without multiplying real dpkg runs or allowing partial evidence to
masquerade as a pass.

The old clean-checkout and two-build reproducibility scripts were overlapping
systems. The clean-checkout journey now performs the stronger combined proof:
it clones the exact Zentty and pinned Ghostty revisions before the build
boundary, masks the developer checkout, disables networking inside the build,
builds once at a different absolute path, validates the package, and requires
all four outputs to be byte-identical to the primary build. The unreferenced
standalone two-build script was removed, leaving one reproducibility authority.

The final summary validator is deliberately separate from its orchestrator. Its
negative suite proves rejection of missing evidence, changed artifact bytes,
false full-qualification claims, stale revisions, source/build leakage, an
installed receipt bound to the wrong package, and an unexpected lifecycle
outcome. Receipt hashes alone are insufficient: the validator also checks the
semantic contract of every source receipt.

The first real clean-build run failed before entering the network-disabled build
namespace. The prepared Ghostty checkout is an intentional partial clone;
`git clone --no-local` asked its promisor remote for unrelated missing history
and aborted when lazy fetching was unavailable. The journey only needs the
already checked-out pinned commit, not remote history. It now creates a
single-branch local clone with copied (not hard-linked) objects, verifies the
exact pinned revision and clean state, and then disables networking for the
actual build. A focused clone reproduced the repair without contacting a
remote. The failure was not converted into a pass.

The next run correctly entered the network-disabled namespace, then Zig tried
to download `uucode`: cloning Ghostty does not include its ignored local cache,
and `build-local` previously hard-coded the Zentty checkout's global Zig cache.
Copying the 821 MiB cache for every qualification would waste disk and time,
while reusing its compiled objects would weaken the different-path proof. The
build helper now accepts an explicit global-cache directory. The clean journey
creates a fresh writable cache and mounts only the prepared package-download
subtree read-only at `p`; compilation objects are rebuilt in the temporary
namespace. Network remains disabled, a missing prepared package remains a hard
failure, and the potentially large temporary build tree is removed on exit.

That repair reached the real Ghostty compile and exposed a second sandbox
mistake: the read-only host root also made `/tmp` read-only, while
`glib-compile-resources` legitimately creates private temporary files there.
The journey now supplies a private tmpfs at `/tmp` and mounts only the detached
source, detached Ghostty source, and fresh Zig cache below a private workspace.
The masked developer checkout remains at its original absolute path, so any
accidental source-tree fallback still fails. This permits tool-owned temporary
files without granting writes to host `/tmp` or the host root.

The first private-workspace attempt used `--dir /work` after mounting the host
root read-only; Bubblewrap correctly refused to create that mountpoint. The
controlled machine already provides an empty `/mnt` directory, so the journey
now binds its single temporary root there and uses `/mnt/source`, `/mnt/ghostty`,
and `/mnt/zig-global-cache`. This is a namespace-path correction only; the host
root remains read-only and the checkout mask remains in force.

The first complete different-path build reached comparison and rejected
`SHA256SUMS`. The comparator had sorted file names, so the checksum file was
checked before the three artifacts it describes and hid the causal mismatch.
It now validates the closed file set separately, then compares package,
manifest, provenance, and finally checksum file in causal order, printing only
the two mismatched SHA-256 values. Reproducibility remains failed until the
underlying first differing artifact is identified and repaired.

The causal rerun confirmed the Debian archives themselves differ, not merely
the checksum file. A failed comparison now preserves compact payload-manifest
and provenance diffs below the ignored qualification output rather than
retaining two large package trees. The archive is still rejected immediately;
the diagnostics exist only to locate the first differing packaged path.

The compact diff localized the divergence to the two Rust executables (and the
second installed copy of the CLI); both Ghostty shared libraries and all other
1,281 payload entries were identical. Cargo was compiling the same commit from
different absolute checkout roots. The Debian build now passes rustc an exact
`--remap-path-prefix=<checkout>=/usr/src/zentty`, giving primary and detached
builds one canonical source identity before debug sections are stripped. This
is build policy, not a post-build binary rewrite; the payload audit continues
to reject leaked host paths.

The next independent build passed: all four release outputs were byte-identical
between the primary checkout and the network-disabled detached clone at a
different canonical mount path. The diagnostic implementation commit was
`efcf02eeba3b41c3ddd5bc1209d42a56df330d00`; its package SHA-256 was
`baedc7242340a112302467ea9797ddc23be56b38ed01b9ab1b4df2479f41105b`.
This focused receipt proves the repair. Final GH-55 evidence must still come
from the subsequent all-cell qualification of the final implementation commit.

The first full matrix run then failed `install-uninstall` before construction.
Setting Cargo offline globally in `build-deb` was too broad: the matrix's
display-none isolation intentionally supplies a private empty home, so its Cargo
registry index is not the operator's prepared cache. Offline enforcement belongs
to the clean-build Bubblewrap network namespace, where dependencies are
deliberately mounted; it is not a portable property of every developer build
invocation. The global Cargo override was removed. The failed producer caused
all installed/package-summary dependents to report blocked-by-failed-dependency,
not false passes; all unrelated executable cells passed.

The second full run passed the real package lifecycle, both installed display
journeys, the payload audit, and all three lifecycle evidence cells, but the
clean-build cell failed before compilation. Its outer display-none environment
correctly supplies a private empty home; after entering the network-disabled
inner namespace, the rustup proxy tried to install the pinned toolchain into
that empty home. The clean journey now resolves the pinned toolchain from the
operator's prepared rustup store before isolation, places its real `cargo` and
`rustc` directory first on `PATH`, and mounts the prepared Cargo registry
read-only into a fresh `CARGO_HOME` with Cargo offline. Build outputs and Cargo
compilation state remain writable only in the temporary workspace. This makes
the clean proof independent of ambient `HOME` without permitting downloads.

The third full run progressed through the pinned Rust setup and then exposed
one remaining inherited-environment leak: the outer isolated session's
`TMPDIR` named its private host path, which intentionally does not exist behind
the inner `/tmp` tmpfs. GLib honored that stale path and failed closed. The
inner build now explicitly binds `TMPDIR=/tmp`, matching its private writable
tmpfs. The same run had one unrelated Wayland agent-hook deadline failure; no
package dependency relied on it, and it remains a failure pending rerun rather
than being reclassified.

The exact private-home reproduction then completed but correctly rejected the
two Rust executables as different. The prepared registry was mounted at
`/mnt/cargo-home`, while the primary build's registry lived under its ambient
Cargo home. Rust crate identity can incorporate dependency source paths, so
remapping only the Zentty checkout was incomplete. `build-deb` now derives the
effective Cargo home and remaps it to `/usr/src/cargo` alongside the Zentty
source remap. Primary, matrix-private-home, and clean-clone builds therefore
share canonical identities without requiring a fixed operator home path.

The focused rerun at `7a8a43226282f22488409131efdcef140fa5a6b1`
passed the complete clean-build proof: a detached exact-revision checkout,
different source path, private home and temporary filesystem, prepared
read-only dependency stores, and disabled network produced the same closed set
of four release artifacts byte for byte. The clean-build evidence validator
also accepted the resulting receipt. This is focused repair evidence only;
the final claim still requires an all-cell run against the final implementation
revision.

The separately rerun Wayland agent-integration cell failed before its earlier
hook-deadline path: the real Gemini executable on the controlled host now
reports `0.55.1`, while qualification pinned the previously reviewed `0.53.0`.
The exact-version guard did its job and rejected unreviewed environmental drift
rather than silently passing it. The matrix, its architecture mirror, its
orchestration contract, and the harness default now consistently pin `0.55.1`.
No agent behavior or product adapter was changed by this qualification repair.
The corrected focused cell then passed its full real-system chain under nested
Wayland: real Gemini `0.55.1`, PTYs and Unix socket; the representative tmux
topology; real Claude `2.1.201` against the controlled loopback model; and the
consolidated restore journey including transient loopback SSH, physical input,
remote paste, background agents, crash recovery, and corrupt-state recovery.

That focused agent conclusion was itself incomplete because the manual command
did not reproduce `qualify-local`'s reviewed-tool prefix. The all-cell run
proved the distinction: `qualify-local` deliberately places the vendored
`build/linux-deps/gemini-cli-0.53.0` first on `PATH`, and both X11 and Wayland
correctly executed `0.53.0`; changing only the expected receipt to the ambient
operator version made both cells fail. This was a test invocation error, not
dependency drift. The reviewed pin is restored consistently to `0.53.0`. Future
focused reruns of these cells must include the same pinned-tool prefix rather
than resolving the operator's mutable global executable.

Apart from that incorrect pin, the first final-candidate matrix run validated
the new package graph end to end: lifecycle construction and install/uninstall,
installed X11 and Wayland journeys, clean different-path reproducibility,
payload audit, install/reinstall, expected upgrade rejection, purge/residue,
and the authoritative summary all passed. The run remained failed because both
agent cells rejected the mismatched expected version, so none of those results
is promoted to final qualification evidence.
