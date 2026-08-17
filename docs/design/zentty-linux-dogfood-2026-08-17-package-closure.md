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
