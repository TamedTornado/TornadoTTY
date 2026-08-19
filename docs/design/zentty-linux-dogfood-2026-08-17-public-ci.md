# Zentty Linux public CI dogfood — 2026-08-17

> **Operator correction — 2026-08-19:** CI is advisory regression checking,
> not qualification or release authority. [ADR 0005](../architecture/0005-ci-is-advisory.md)
> supersedes the hosted-evidence and release-gate policy described below.
> Historical environment and failure discoveries remain useful.

This append-only report covers GH-10 and children GH-56 through GH-59. The
authoritative local product runner remains `linux/tests/qualify-local`; public
CI must consume it rather than grow a parallel product-test architecture.

## Initial plan and discoveries

GH-10 was too broad for one reviewable implementation. It is decomposed into
GH-56 (environment/bootstrap), GH-57 (pull-request controlled subset), GH-58
(full public qualification), and GH-59 (security/evidence closure). The order
separates tool provisioning from workflow behavior and final matrix claims.

The repository had no public workflow and no checked-in CI toolchain contract.
Local qualification implicitly depended on operator installations: Rust
1.97.1, Zig and Blueprint compiler 0.16.0, Gemini 0.53.0, native Claude Code
2.1.201, and Codex CLI 0.147.0. `qualify-local` already pins the prepared Gemini
directory, while Claude and Codex tests reject version drift. Public execution
therefore needs reviewed provisioning, not removal of those guards.

The existing operator Gemini tool directory already used pnpm 10.32.1 with a
lockfile and `minimum-release-age=10080`, but it lived outside the repository.
The CI-owned node-tool package now declares exact Gemini and Codex versions and
enforces the same seven-day policy. No scope exclusion is authorized.

Anthropic's official installer currently downloads a release manifest and
verifies the native binary checksum. The reviewed 2.1.201 Linux x64 manifest
identifies SHA-256
`a34809a6839fdefff21b9347d7fb5b6b58e6a9cc208a5e62853f29c83eb107a3`
and size 251,300,664 bytes, matching the binary used by local qualification.
CI will download that exact release directly and verify both values; it will
not execute a mutable `curl | bash` installer.

Zig's official download index identifies the 0.16.0 x86_64 Linux archive as
SHA-256 `70e49664a74374b48b51e6f3fdfbf437f6395d42509050588bd49abe52ba3d00`.
Blueprint compiler 0.16.0 is built from upstream revision
`04ef0944db56ab01307a29aaa7303df6067cb3c0`. These identities and the Ubuntu
24.04 package/environment contract are now centralized in
`linux/ci/environment-v1.json`; validation cross-checks Rust and Ghostty against
their existing authoritative lock files.

GitHub-hosted `ubuntu-24.04` is a fresh four-core, 16-GB x64 VM for public
repositories, but the label is an evolving image rather than an immutable
filesystem digest. The contract therefore pins the OS baseline and records the
run's GitHub image version as evidence. Tool downloads remain content-pinned;
apt package versions are captured from the runner rather than falsely claimed
immutable across security updates.

The first real content-pinned bootstrap successfully downloaded and verified
Zig, checked out the exact Blueprint compiler commit, built and installed it
under the ignored CI tool root, confirmed Rust, installed the locked pnpm graph,
and matched the existing Claude binary checksum. Its immediate idempotence run
then failed only because it unconditionally attempted to chmod the already
correct, executable Claude binary in the operator home, which this controlled
coding environment exposes read-only. Bootstrap now changes its mode only when
the reviewed binary is not already executable. This removes an unnecessary
host write while preserving clean-run installation behavior.

Corepack could enforce pnpm's exact version but did not compare the downloaded
package with the integrity recorded by Zentty. Bootstrap no longer delegates
that trust decision: it downloads the exact pnpm 10.32.1 tarball, verifies the
reviewed SHA-512, extracts the self-contained CLI below the ignored tool root,
and uses that executable for the frozen lockfile install. The second real run
completed idempotently without reconstructing Zig, Blueprint, Rust, pnpm,
Claude, or the node dependency graph.

Preflight now deletes its output before checking anything, validates all 53 apt
packages and records their actual Ubuntu versions, requires the deterministic
environment, proves user namespaces and Bubblewrap, checks clean exact Zentty
and Ghostty trees, and verifies every pinned tool and agent CLI. Its pure
receipt validator rejects stale time, wrong run/source, missing apt packages,
wrong tools, false prerequisites, and credential-shaped content. The negative
runner additionally proved wrong OS, a dirty Zentty tree, wrong Ghostty origin,
wrong Zig, and stale receipt cleanup.

The local Ubuntu baseline lacked only `fish` from the declared apt set. It was
installed from Noble (`3.7.0-1`); apt also reported an unrelated pre-existing
Unity repository missing-key warning, which does not exist on a fresh hosted
runner and was not treated as qualification evidence. The exact preflight of
Zentty `6bb98eef63b6d1d406e09305369b7ee4be7fcf2f` and Ghostty
`281d7d7dbeab24c1a2d04f6d3c720c34dbfac645` passed with all 53 packages,
namespaces, both controlled-display capabilities, and reviewed tools. Receipt
SHA-256: `5e881fa5418e24ebf8523b2318625f96dac9115e1405404f2a77a9f16a5ff4b7`.

Using only the new bootstrap environment, the exact current product was rebuilt
and passed real single-terminal Ghostty/GTK/PTY smoke journeys in nested
Wayland session
`aeaedd65ae4aa023cb05820a28ea2b93109d4ea9477da45476989aa5240114a0`
and nested X11 session
`1b0053ffe1a3c308d7a18454ff5198611dc246dfca5347a30b4daedbc59cb417`.
The checked-in pnpm Gemini/Codex directory then passed the real Gemini agent
journey through nested input-capable Wayland session
`5052ecc72b7c2f4382d03c2362f97432850a3c84130298a714e5e8f95d46ef94`.
This closes the local bootstrap proof, not the required fresh public-run proof.

The first public foundation workflow uses only official GitHub actions, pinned
to full reviewed release commits: checkout v7.0.1, setup-node v7.0.0, and
upload-artifact v7.0.1. It has read-only contents permission, disables persisted
checkout credentials, binds preflight to the exact event SHA, cancels
superseded ref runs, and uploads bounded-retention logs/receipts even after a
failure. Node is pinned to 24.14.0 rather than the mutable Node 24 major.

The workflow runs the same bootstrap, Ghostty lock preparation, preflight,
matrix validator, product build, and real nested X11/Wayland smoke commands used
locally. It does not add a CI-specific product simulator. Static negative tests
reject mutable actions/runners/Node, write permissions, persisted credentials,
missing always-upload, wrong event binding, and altered action manifests. The
workflow passed the checksum-pinned official actionlint 1.7.12 parser. The next
evidence must come from its fresh public `ubuntu-24.04` run; local success does
not substitute for that prerequisite.

Public run [32066079879](https://github.com/TamedTornado/zentty/actions/runs/32066079879)
completed provisioning and exact Ghostty preparation, then correctly failed
preflight because GitHub's Ubuntu 24.04 image restricts unprivileged user
namespaces through AppArmor. Failure evidence uploaded successfully and no
stale preflight receipt survived. Bubblewrap and the clean package lifecycle
cannot operate under that default.

The runner is a fresh disposable VM dedicated to this job, so the workflow now
explicitly sets `kernel.apparmor_restrict_unprivileged_userns=0` only when that
Ubuntu policy knob exists and is enabled. Preflight still proves that both
`unshare` and Bubblewrap actually work afterward; it does not convert their
absence into PASS. The policy change is logged and retained with the run.

The foundation acceptance criteria also require positive proof that bootstrap
or a future source cache cannot leave a product binary that satisfies the job.
Before the product build, CI now removes only the known Zentty product bundles,
metadata, and final Rust executables, while preserving Ghostty sources and
compiler dependency caches. The cleanup is idempotently tested, rejects an
unsafe root, and proves dependency directories survive. The workflow then
requires both rebuilt executables before running either real display journey.
The first cleanup-runner test expected seven removals but its fixture correctly
created all eight declared output paths, including empty `libexec` and `share`
directories. The test failed rather than masking the discrepancy; its exact
expectation was repaired to eight, with the protected dependency assertions
unchanged.

Public run [32066390972](https://github.com/TamedTornado/zentty/actions/runs/32066390972)
proved the AppArmor repair: bootstrap, exact-source preflight, the full fresh
ReleaseSafe build, and the controlled Wayland product journey passed. The X11
journey then exited 77 because `xdpyinfo` was absent. This was correctly a job
failure rather than an environmental PASS, and failure evidence uploaded.

The immediate missing package was `x11-utils`, but auditing every external
program owned by the controlled X11 wrapper exposed four more dependencies the
runner image happened to supply without our manifest: `mesa-utils` (`glxinfo`),
`fuse3` (`fusermount3`), `util-linux` (`findmnt`/`setsid`), and `procps` (`ps`).
All five are now explicit manifest packages, and preflight names the critical
display and cleanup commands so this class of failure occurs before a costly
product build. The reviewed apt contract therefore has 58 packages, not 53.

Public run [32067900394](https://github.com/TamedTornado/zentty/actions/runs/32067900394)
passed the final GH-56 foundation workflow at exact Zentty revision
`2e785499c4d5124160a34dec25575339262eb0f0` and Ghostty revision
`281d7d7dbeab24c1a2d04f6d3c720c34dbfac645`. The fresh hosted image was
`20260810.271.1`; the 58-package manifest identity was
`c6a8d877a5b77ee06224acdade1363bd242007508ee55cfc02f2e9d4ebe8766d`.
The downloaded preflight receipt revalidated locally and has SHA-256
`cd0b9136f3e1099fe5505da156d4f88ed47cf60070ef96ae1b3149a393bfbd4e`.
A credential-pattern scan of every downloaded artifact file passed.

The fresh job took 11 minutes 54 seconds. Apt provisioning took 1 minute 46
seconds; content-pinned tool bootstrap took 29 seconds; exact Ghostty source
preparation took 7 seconds; preflight took 5 seconds; and the uncached
ReleaseSafe product build took 9 minutes 4 seconds. The controlled Wayland and
X11 journeys then passed in 9 and 3 seconds, respectively. Their environment
receipt SHA-256 identities are
`e270a958a0c6e57f132b77aa61f1ad01a6ce4cf6e0e7667ab2abf4ec85b07187`
and `3d527a524b896b77da778109a59a61eec52f77305adb482c77bc899438855e8b`.
The cleanup receipt reported zero pre-existing product outputs on the fresh
runner, after which both required executables were reconstructed and exercised.

An attempted full local qualification before committing this evidence failed
immediately and correctly: the package builder and `prepare-ghostty` reject a
dirty Zentty checkout. That prerequisite failure also invalidated a packaging
negative test whose intended Ghostty fault could not be reached. Concurrent
wrapper self-tests subsequently observed fixture-only socket startup failures,
but none can be treated as product evidence because the root prerequisite had
already failed. The evidence must be committed first, then the entire local
qualification rerun from the clean exact revision.

The first clean rerun inside the restricted coding sandbox was not valid
qualification: Ghostty preparation could not resolve GitHub, fixture Unix
socket binds were denied, and Xvfb found `/tmp/.X11-unix` owned by the
user-namespace overflow identity rather than root. The standard X11 socket
directory was restored to `root:root` mode `1777`, and preflight now rejects a
missing, linked, wrongly owned, or wrongly permissioned directory before any
build. The aggregate was rerun with the network, namespace, Unix-socket, and
compositor access its real harnesses require.

That real aggregate exercised all 181 cells and exposed two CI-foundation
integration defects. First, bootstrap placed Zig below repository `build/`.
The network-disabled clean-package mount intentionally replaces the repository
root, so the otherwise valid compiler path disappeared and package
reproducibility failed. Versioned tools now use Zentty's established sibling
`.tools` boundary; only the generated environment file remains below ignored
`build/`. Bootstrap and preflight both reject compiler tools that fall back
inside the checkout. This is also safer for future mutation source-tree copies.

Second, installing Ubuntu's Fish 3.7 made the shell prerequisite resolver pick
the unsupported distribution binary before the already reviewed sibling Fish
4.8.1. The missing-tool unit case also incorrectly assumed `/usr/bin` lacked
Fish. Controlled versioned Fish and Nushell now precede PATH unless an explicit
operator override is supplied, while the negative test uses a genuinely empty
PATH. This preserves ordinary PATH fallback on hosts without controlled tools
and makes the supported-version gate deterministic.

The next aggregate proved both repairs: clean package reconstruction and all
five Fish cells passed. It then exposed an independent concurrency bug in the
installed-product harness. X11 and Wayland installed-package cells correctly
run in parallel, but the desktop-entry phase searched globally for *any*
`/usr/bin/zentty-linux`. Wayland selected a withdrawing process from the X11
cell; `/proc/PID/exe` was consequently empty and the audit failed. The selector
now requires the live executable plus the exact backend-private installed
receipt root inherited through the real desktop launch. Direct launches retain
their stricter process-ancestry check. No product assertion was weakened, and
the failed public-package evidence remains recorded.

The first focused concurrency rerun command accidentally backgrounded the
entire package-build AND-list rather than only the X11 journey. Wayland started
before the exact package existed and correctly failed its provenance
prerequisite; this was orchestration error, not product evidence. With explicit
subshell grouping, the exact package was then exercised concurrently by real
X11 and real nested Wayland sessions. Both installed-product journeys passed,
including direct launch, injected crash/restart, desktop entry, PTYs, panes,
clipboard, Open With, shell integration, and exact process audits. The selector
repair therefore has the real colliding-workload coverage that exposed the bug;
a final clean aggregate remains required.

The final clean aggregate at revision
`c879cfa37e640592716648f85bfdab3fd46b40b2` passed every presently executable
support test and matrix cell. The machine summary SHA-256 is
`b1bebc6b8976fb81f1e08f91b61232cf76ba879e913b6ca61acebc08e1615586`.
Declared totals are **161 PASS, 0 FAIL, 5 BLOCKED, 1 XFAIL, and 14
NOT_IMPLEMENTED**. Implemented-local and product-boundary qualification passed;
release and full-Linux qualification did not pass because the declared gaps
remain. Valgrind is **PASS with reviewed suppressions**; ReleaseSafe Valgrind
remains the expected tracked XFAIL.

The aggregate took 1,047.930 seconds. Its critical path is now explicit:
current pinned Ghostty regression took 487.040 seconds, network-disabled clean
package reconstruction 315.460 seconds, and install/upgrade/uninstall lifecycle
227.770 seconds. This is appropriate for complete/release qualification, not a
pull-request subset; GH-57 must select a real representative gate without
mislabeling it as full qualification.

Final public run [32077398464](https://github.com/TamedTornado/zentty/actions/runs/32077398464)
passed from exact evidence revision
`695b4c8342ab2f6c60da8f4f75cc5d1e392457db` in 11 minutes 53 seconds.
The downloaded artifact revalidated against the current manifest and exact
source identities, and a credential-pattern scan passed. Receipt SHA-256
identities are preflight
`1a114e5c29361de756a13f57b762ff2af9df5f4b0eb717f1096accefb5027323`,
Wayland
`2e4a69f4479d831df46ebf19170bae3170a7ecd88025eaa7ac10044f35785470`,
and X11
`e8fe6227345c0c6ba6989de9c364b21d769b3cf158b77030383ba2c02f6dd0be`.
Both real product journeys passed after a clean output reset and uncached
ReleaseSafe reconstruction. GH-56's public foundation prerequisite is closed;
this is not a claim of release or full-Linux qualification.
