# Zentty Linux dogfood: remaining managed-agent launchers

Date: 2026-08-16
Issue: GH-47
Parent epics: GH-22, GH-14

This record is started before implementation. It freezes the source launcher
contracts for Copilot, OpenCode, Pi, OMP, and Small Harness so the port does not
invent aliases, bypass rules, resume behavior, configuration ownership, or a
second launch architecture.

## Existing Linux boundary

Linux already has one managed path:

1. pane-local PATH resolves a staged wrapper;
2. the wrapper invokes `zentty launch <tool>`;
3. `zentty-agent-ipc::launch_agent` resolves the real executable while excluding
   wrapper directories;
4. `zentty_core::build_agent_launch_plan` creates the source-specific ephemeral
   plan;
5. the wrapper process is replaced with the real executable by `exec`.

The authenticated pane socket, canonical `AgentEvent` protocol, one status
reducer, existing workspace recipe, and existing integration installers remain
the only authorities. GH-47 may extend the launch plan with source-required
ephemeral files/actions, but must not add another launcher, IPC transport,
state store, or integration harness.

## Source inventory

### GitHub Copilot CLI

- Executable and wrapper name: `copilot`; no alias.
- Disable switch: `ZENTTY_COPILOT_HOOKS_DISABLED=1`.
- No source management-command or early-exit bypass list.
- Source `--config-dir PATH` and `--config-dir=PATH` are consumed by the wrapper
  to select the source config directory, not forwarded to the real process.
- Otherwise the source home is `COPILOT_HOME`, falling back to `$HOME/.copilot`.
- A private per-pane overlay symlinks source-home entries except `config.json`,
  copies/merges the config, preserves existing fields/hooks, and adds exactly
  the six source hook groups: session start/end, user prompt, pre/post tool, and
  error.
- Managed environment: `ZENTTY_AGENT_TOOL=copilot`, overlay `COPILOT_HOME`, and
  wrapper PID in `ZENTTY_COPILOT_PID`.
- Restore command: `copilot --resume=<validated-session-id>`.
- Event behavior: attach plus idle on session start, running on user prompt,
  question-only needs-input/idle around AskUserQuestion, no-op error, and
  session end plus PID clear.

### OpenCode

- Executable and wrapper name: `opencode`; no alias.
- No source disable switch, management-command bypass, or early-exit bypass.
- If an executable named `opencode` has an executable sibling `.opencode`, the
  source launches that sibling to avoid recursing into OpenCode's own shim.
- Source config precedence: `ZENTTY_OPENCODE_BASE_CONFIG_DIR`, then
  `OPENCODE_CONFIG_DIR`, then `$XDG_CONFIG_HOME/opencode`, then
  `$HOME/.config/opencode`.
- The source copies the config into a private per-pane overlay and installs
  `zentty-opencode-zentty.js` under its `plugins` directory. It sets
  `OPENCODE_CONFIG_DIR` and `ZENTTY_OPENCODE_BASE_CONFIG_DIR`; the optional
  macOS theme/state synchronization is presentation-specific and is not silently
  claimed by this launcher slice.
- A canonical OpenCode session-start event is sent before exec because the
  plugin owns subsequent events.
- Restore command: `opencode --session <validated-session-id>`.

### Pi

- Executable and wrapper name: `pi`; no alias.
- Disable switch: `ZENTTY_PI_HOOKS_DISABLED=1`.
- Passthrough subcommands: `install`, `remove`, `uninstall`, `update`, `list`,
  `config`.
- Early-exit flags: `--help`, `-h`, `--version`, `-v`, `--list-models`,
  `--export`, including `--flag=value` forms.
- Management subcommand detection skips values belonging to leading
  `--profile`, `--cwd`, and `--config` scope flags, in separate or `=` form.
- Managed launches prepend `-e <staged zentty-pi-zentty.js>`, set
  `ZENTTY_AGENT_TOOL=pi` and `ZENTTY_AGENT_CANONICAL_NAME=Pi`, and send one
  canonical session-start event before exec.
- Missing extension is a direct launch with no false status claim.
- Restore is project-local and requires a working directory: `pi -c`.

### OMP / Oh My Pi

- Executable and wrapper name: `omp`; no alias.
- Disable switch: `ZENTTY_OMP_HOOKS_DISABLED=1`.
- Early-exit flags are Pi's set plus `--alias`.
- Source passthrough subcommands are the explicit v16.3.6 snapshot in
  `PiFamilyLaunchPolicy`: `acp`, `agents`, `auth-broker`, `auth-gateway`,
  `bench`, `commit`, `completions`, `config`, `dry-balance`, `gallery`, `gc`,
  `grep`, `grievances`, `install`, `join`, `models`, `plugin`, `read`, `say`,
  `search`, `setup`, `shell`, `ssh`, `stats`, `tiny-models`, `token`, `ttsr`,
  `update`, `usage`, and `worktree`.
- It uses the same leading scope-flag parser as Pi.
- Managed launches prepend `-e <staged zentty-omp-zentty.js>`, set
  `ZENTTY_AGENT_TOOL=omp` and `ZENTTY_AGENT_CANONICAL_NAME=OMP`, and send one
  canonical session-start event before exec.
- Missing extension is a direct launch with no false status claim.
- Restore is project-local and requires a working directory: `omp -c`.

### Small Harness

- Executable and wrapper name: `small-harness`; no alias.
- Disable switch: `ZENTTY_SMALL_HARNESS_HOOKS_DISABLED=1`.
- Passthrough subcommand: `completions`.
- Early-exit flags: `--help`, `-h`, `--version`, `-V`, including `=value`
  normalization where applicable.
- A private per-pane `managed-hooks.json` contains the source marker, twelve
  hook groups, the `--adapter=small-harness` command, exact forwarded pane
  environment allowlist, ten-second timeouts except one second for SessionEnd,
  and mode 0600 under a private directory.
- Managed environment sets `ZENTTY_AGENT_TOOL=small-harness` and
  `SMALL_HARNESS_MANAGED_HOOKS_FILE`, unsets
  `SMALL_HARNESS_MANAGED_HOOKS_JSON`, and supplies wrapper PID as
  `ZENTTY_SMALL_HARNESS_PID`.
- Direct/bypassed launch must unset both managed-hook variables so stale pane
  state cannot activate deleted files.
- Restore is project-local and requires a working directory:
  `small-harness --continue`.

## Local prerequisite audit

At implementation start, `command -v` found none of `copilot`, `opencode`,
`pi`, `omp`, or `small-harness` on this Linux host. Real-installed-binary cells
therefore remain BLOCKED until an executable is installed and version-pinned;
absence will not be converted into PASS. The external-agent boundary will use
the existing deterministic executable fixture to qualify exact argv,
environment, cwd, stdout/stderr, signals, and exit status, while all Zentty,
Ghostty, PTY, socket, wrapper, overlay, and compositor components remain real.

## Test-first order

1. Extend the existing `agent_launch` characterization suite with the exact
   inventory above, including hostile paths and disabled/outside-pane behavior.
2. Add filesystem tests for private overlays, symlink rejection, ownership,
   rollback, and idempotency before implementing overlay writers.
3. Add Copilot adapter golden/negative cases before its implementation.
4. Implement one source family at a time through the existing plan and wrapper.
5. Extend the existing staged bundle and `rust-agent-ipc` journeys rather than
   adding a new integration layer.
6. Run focused mutation tests, relevant real matrix cells, and document every
   mismatch or repair here before commit.

## Dogfood discoveries and repairs

- The first staged Wayland journey passed Copilot, OpenCode, and Small Harness
  but Pi launched directly and emitted no authenticated event. The controlled
  child receipt proved that it received no managed agent environment or
  extension arguments even though all pane-routing variables were present.
- Root cause: staged wrappers execute their bundled helper from
  `libexec/zentty/agent-wrappers/shared/zentty`; resource lookup had assumed the
  helper always lived in `bin/zentty`, so Pi/OMP could not find their staged
  extensions. Resource discovery now walks installation ancestors for the
  exact packaged `share/zentty/<resource>` file, while tests may still provide
  an explicit isolated `ZENTTY_RESOURCE_ROOT`.
- A source comparison caught two incorrect Copilot transitions before commit:
  `postToolUse` must return the source adapter to idle, and `errorOccurred` is a
  no-op. The initial implementation had invented input-resolved and
  needs-input transitions; both code and characterization tests were repaired.
- The same comparison showed that OpenCode always sends its canonical
  pre-launch session event inside a routed pane, even when its optional plugin
  resource is absent. The plan now preserves that source behavior without
  claiming an `OPENCODE_CONFIG_DIR` overlay that was not created.
- Direct invocation outside a routed pane was initially enforced only by the
  Pi-family and Small Harness helpers. A common eligibility guard now covers
  all five GH-47 tools, with an explicit direct/non-mutating characterization
  cell.
- The first complete staged-bundle attempt failed in the pre-existing tmux
  compatibility journey at `__tmux-compat kill-window`. That receipt was not
  treated as an agent-launch pass. During diagnosis, two overlapping
  `build-local` invocations raced over the ignored staging directory and one
  failed while copying wrappers. After allowing the authoritative one-worker
  build to finish, the isolated controlled-Wayland tmux journey passed and the
  complete staged-bundle journey passed. No tmux product code or test was
  changed for GH-47.

## Qualification receipts

- `cargo test -p zentty-core`: PASS, including 24 adapter cases, 22 launch-plan
  cases, five workspace-recipe cases, and 56 workspace-state cases.
- `cargo test -p zentty-agent-ipc`: PASS, including 18 launch/exec boundary
  cases and the real authenticated Unix-socket suites.
- The launch boundary preserves hostile argv as distinct arguments, current
  working directory, stdout, stderr, exit status 37, and signal/process
  identity across the final `exec`.
- `cargo clippy -p zentty-core -p zentty-agent-ipc --lib -- -D warnings`:
  PASS. Whole-workspace/all-target clippy continues to expose unrelated
  pre-existing warnings outside this issue; they were not repaired or hidden
  in this feature change.
- `linux/tests/feature-inventory`: PASS with 60 entries. GH-47 moves five
  agent entries out of `NOT_IMPLEMENTED`; Pi, OMP, and Small Harness are
  `IMPLEMENTED`, while Copilot and OpenCode are honestly `PARTIAL` because
  their source-specific OSC presentation and live theme synchronization are
  outside this launcher issue. Totals are now 17 `IMPLEMENTED`, 28 `PARTIAL`,
  one `MODEL_ONLY`, and 14 `NOT_IMPLEMENTED`.
- `linux/tests/cli-source-contract-test`, `cargo fmt --check`, and
  `git diff --check`: PASS.
- Official ReleaseSafe one-worker build: PASS. The staged helper, product,
  wrappers, and resources were rebuilt together before product qualification.
- Controlled nested Wayland (private Weston headless compositor, Pixman
  renderer) isolated tmux product journey: PASS.
- Controlled nested Wayland complete `staged-bundle`: PASS. It exercised the
  real GTK/Ghostty product, PTY child, packaged helper, every packaged wrapper,
  private authenticated Unix socket, agent status reducer, and tmux journey.
  The managed-agent receipt named Copilot, OpenCode, Pi, OMP, and Small Harness
  alongside the previously supported agents.
- Locally installed real binaries for `copilot`, `opencode`, `pi`, `omp`, and
  `small-harness` remain unavailable. Those external-product cells are
  **BLOCKED**, not passed. Deterministic executables replace only that external
  agent/model boundary; Zentty, Ghostty, compositor, wrappers, PTY, filesystem,
  and IPC remain real.

## Mutation receipts

- Core launch/Copilot-adapter target: initial run produced 30 caught, six
  unviable, and four missed mutants. The misses exposed insufficiently
  observable extension and hook-environment assertions. After strengthening
  those tests, the 16 affected decision mutants were all caught. Combined,
  every viable targeted core mutant is caught.
- IPC resource/prelaunch/exec-selection target: initial run produced five
  caught, one unviable, and two missed pre-launch delivery mutants. A real
  `AgentIpcServer` boundary test and an explicit debug-only failure receipt
  were added. The affected rerun caught both viable mutants; the remaining
  mutant is unviable. No surviving viable targeted IPC mutant remains.
- All mutation runs used `linux/tests/mutate-rust`, which enforces the governed
  `gitignore = true` and `copy_target = false` policy. No ignored build tree was
  copied into cargo-mutants scratch workers.

## Remaining uncertainty

- Source-version-pinned, installed-binary journeys cannot be claimed on this
  host until those five third-party CLIs are installed. Their absence is the
  only GH-47 real-agent prerequisite gap recorded here.
- OpenCode live theme synchronization and Copilot-specific OSC/title
  presentation are deliberately not claimed by this launcher issue. The
  feature inventory remains `PARTIAL` for those broader source behaviors so
  they cannot disappear behind a completed launcher claim.

## Final local qualification repair and receipt

- The first full local qualification correctly failed. The inventory runner
  test still asserted the old status totals, the Agents settings product test
  still expected five newly supported ephemeral tools to be unavailable, and
  the Rust Agents settings unit test imported the removed unavailable array.
  Those were stale test contracts caused by this issue, not environmental
  skips; each was updated to assert the new eight-tool managed inventory.
- Extending the shared controlled-agent start receipt had also broken the
  pre-existing session-restore cancellation parser by appending diagnostics to
  its exact `started pid:... profile:...` line. The fixture now preserves that
  stable line and emits the new launch diagnostics on a separate line. The
  complete real session-restore journey then passed under both controlled X11
  and input-capable controlled Wayland.
- The first parallel matrix run also observed independent fleet window timing
  failures. The fleet journey passed in isolation, and it passed again in the
  authoritative complete rerun; no fleet code or acceptance threshold was
  changed.
- Authoritative `linux/tests/qualify-local` rerun: all presently executable
  support and matrix cells passed. Declared totals are **152 PASS, 0 FAIL,
  5 BLOCKED, 1 XFAIL, and 15 NOT_IMPLEMENTED**. The implemented local suite and
  product-boundary qualification passed. Release and full Linux qualification
  remain not passed, as required while declared gaps remain.
- Debug Valgrind is **PASS with reviewed suppressions**, never described as an
  unsuppressed clean result. The preserved raw receipt reports 427 errors in
  427 contexts, 6,160 definite bytes, and 41,428 indirect bytes. The reviewed
  post-suppression receipt reports zero errors/contexts and zero
  definite/indirect bytes, with 427 errors/contexts recorded as suppressed.
  Suppression governance passed and the raw/suppressed receipt hashes are
  recorded in `build/linux/qualification-summary.json`.
