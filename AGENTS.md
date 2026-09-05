# TornadoTTY contributor and agent instructions

## Ownership and scope

- This is Jason's **TornadoTTY** repository, a Linux-focused fork of ZenTTY.
  Jason sets product direction and approves upstream interactions. Inherited
  macOS instructions do not govern Linux development.
- Preserve upstream attribution. Existing `zentty` crate, module, environment,
  and persistence names are not a mandate for sweeping internal renames.
  Public branding and installed entry points should use TornadoTTY.
- Work against an issue with acceptance criteria for substantive changes.
  Prefer issue-sized features or repairs, not arbitrary tiny slices. Record
  newly discovered scope separately rather than silently expanding the issue.
- No broad rewrite or new framework merely to make the architecture look tidy.
  Prefer existing owners, integration boundaries, and test infrastructure.
- Do not generate or edit repository files using Python. Use `apply_patch` or
  transparent, narrowly scoped editor-safe shell commands.

## GitHub repository targeting

- Every mutating GitHub command must explicitly specify
  `--repo TamedTornado/TornadoTTY` (or `-R TamedTornado/TornadoTTY`).
  Never rely on remote inference: this checkout also has an upstream remote.
- Read-only upstream inspection must explicitly name `--repo dedene/zentty`.
- Do not comment on, review, open, close, or otherwise mutate upstream issues
  or pull requests without Jason's approval for that exact interaction.
- Review the diff and run relevant checks before committing. Report exact
  commits, push status, actual test results, and remaining acceptance gaps.
  Do not close an issue solely because code was written.

## Linux build and daily iteration

- Read the README and the relevant issue before changing behavior. Use
  `linux/scripts/build-local` to build the integrated product and
  `linux/scripts/run-local` when a local launch is requested. Inspect their
  options and prerequisites rather than improvising another launcher.
- Jason may be running this agent inside TornadoTTY. Do not terminate,
  restart, or launch his client without explicit coordination. Use isolated
  test instances and state; never overwrite his workspace with test fixtures.
- An authorized install may atomically replace installed executables while
  the old process continues running. Do not overwrite an executing file in
  place. Distinguish source, built, installed, and currently running versions;
  a new on-disk binary does not change a running GUI or agent's launch policy.
- Do not install unrelated system packages or modify partitions to complete
  a routine build/deploy. Explain genuinely missing prerequisites.
- Give progress updates at meaningful boundaries, especially on failures or
  unexpectedly long work. Do not claim work is underway or monitored after
  ending the turn.

## Testing: product behavior first

- For a bug, reproduce it with a focused failing regression before fixing it
  where feasible. If reproduction is unavailable, say so. Run focused checks
  for each repair before any aggregate run.
- **Do not run full qualification for every fix or commit.** Use targeted
  Rust tests and the affected existing integration journey. Broader local
  qualification (`linux/tests/qualify-local`) is for release or meaningful
  cross-cutting validation, not the default edit loop. GitHub CI checks our
  work; waiting for a public CI receipt is not a separate release approval gate.
- Exercise real GTK/Ghostty/PTY/IPC boundaries for integration claims, using
  the existing isolated journey driver. Do not create parallel harnesses,
  recursive aggregate suites, or extra evidence machinery without a concrete
  product need. No broad retries or relaxed requirements to turn failures green.
- Assert observable semantics, not formatter mirroring, source hashes, exact
  incidental log wording, or byte-identical help. Test the user-reported path,
  not just a nearby reducer. Test agents' automated behavior ourselves; Jason
  performs user-level QA, not manual execution of our integration fixtures.
- Use `linux/tests/mutate-rust` for targeted mutation checks. Preserve its
  resource isolation and `.cargo/mutants.toml` (`gitignore = true`,
  `copy_target = false`); never copy build trees into mutation workers.
  Mutation strength is required for unattended-maintenance claims, not test count.
- Use existing capacity controls; do not arbitrarily serialize all tests or
  launch unbounded workers. GUI tests must not steal focus from the live client.
- Report only results actually run. Keep PASS, FAIL, BLOCKED, XFAIL, and
  NOT_IMPLEMENTED distinct, with reasons and tracked gaps. Do not present an
  old matrix total as a new run. Preserve raw Valgrind receipts alongside
  suppressed runs, review the entire effective suppression set, and describe
  success as **PASS with reviewed suppressions**. Keep ReleaseSafe Valgrind
  XFAIL; do not broaden suppressions to make it green.

## Dependencies

- Respect `rust-toolchain.toml` and Cargo's checked-in minimum-publish-age
  policy. Do not bypass it with another toolchain or a courtesy wrapper.
  Track migration to stable Cargo in #96 once it supports the needed policy.
- For JavaScript tooling use pnpm, with `minimumReleaseAge: 10080` or
  `minimum-release-age=10080`. Exclusions require Jason's explicit approval.

## State, errors, and logs

- Keep canonical state ownership explicit. Agent lifecycle, human attention,
  and UI selection are different concepts; do not substitute one for another.
- Preserve saved worklane/pane topology and restore intent across lazy startup,
  failed agent resume, normal agent exit, and repeated save/restart cycles.
- Bound queues, retention, and GUI-thread work. Move blocking work off GTK;
  reject stale background results. Do not infer that payload limits bound a queue.
- CLI failures use actionable stderr and a nonzero exit. Recoverable in-app
  failures should be observable and contained, not crash the whole application.
- Log useful lifecycle transitions and bounded diagnostics. Do not log tokens,
  private terminal contents, or every unchanged status update by default.
  Separate hypotheses from confirmed causes and fixes from verified outcomes.

## Planning and field reports

- Use GitHub issues for substantive plans, acceptance criteria, and follow-ups.
  Add standalone design documents only when Jason requests them or an agreed
  issue requires one. No inherited third-party approval requirement applies.
- Follow [field-reporting guidance](docs/dogfood-field-reporting.md). The
  current report is
  [2026-09-05 ingress backpressure](docs/design/tornadotty-linux-dogfood-2026-09-05-ingress-backpressure.md).
  Keep this pointer current when rotating reports; start another at a natural
  boundary rather than growing one incident log indefinitely.
- Record meaningful observations, failed hypotheses, repairs, focused evidence,
  and uncertainty as work proceeds. Keep entries concise; don't commit raw
  dumps or private data. A pending user check stays pending, not falsely passed.
- Cross-link TornadoTTY and Ghostty commits when a repair changes both repos.

## macOS-only maintenance (when explicitly in scope)

- `project.yml` owns Xcode project generation. Change it and regenerate rather
  than hand-editing `Zentty.xcodeproj/project.pbxproj`.
- `ZenttyLogicTests` is the unhosted target; `ZenttyTests` needs an app host.
  Use `scripts/test-on-virtual-display -only-testing:ZenttyLogicTests` or
  `scripts/test-hosted-on-virtual-display` for window-creating tests. Close
  test windows in teardown; use XCTest expectations instead of run-loop sleeps.
- If invoking `xcodebuild` directly, prefix
  `TEST_RUNNER_SWIFT_BACKTRACE=enable=no` to avoid interactive crash prompts.
  Do not add per-agent `-derivedDataPath` trees or use `./runner`.
- For macOS Antigravity changes, consult `scripts/agent-bench/` and run the
  existing `scripts/test-agy-bench` scenarios; do not transplant that harness
  into the Linux implementation. Use `scripts/test-modern-shell-integrations`
  for relevant inherited shell integration release checks.
