# Zentty Linux public CI dogfood — 2026-08-17

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
