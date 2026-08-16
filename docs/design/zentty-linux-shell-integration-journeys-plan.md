# Linux shell-integration journeys plan

- **Tracking:** GH-45, child of GH-22
- **Status:** complete; commit-ready qualification passed
- **Source authority:** `WorklaneSessionEnvironment`, the source shell files
  under `ZenttyResources/shell-integration`, and their retained Swift tests

## Outcome

Qualify Bash, Zsh, Fish, and Nushell as real processes inside a staged Zentty
Ghostty pane. Each shell must receive the authenticated pane environment,
autoload the source-owned integration without changing user files, report real
prompt/activity/CWD state through the staged CLI, preserve ordinary shell
startup behavior, and remain usable when integration prerequisites are absent
or too old.

## One-system boundaries

1. `AgentRuntime::environment_for_pane` remains the only product authority for
   pane identity, socket capability, CLI path, hooks, wrapper paths, initial
   CWD, and shell-discovery variables.
2. The staged files under `share/zentty/shell-integration` remain the shell
   implementation. Linux does not fork or translate them into Rust.
3. Extend the existing `rust-cli-contract` real-product actor with a bounded
   shell scenario. Do not add another application launcher, fake socket,
   synthetic PTY model, or shell-integration test layer.
4. The existing standalone `staged-shell-integration` runner remains the fast
   resource/startup contract. It will use real shell executables and explicit
   prerequisite failure, while the product actor owns end-to-end behavior.
5. Shell executables are test prerequisites, never product dependencies.
   Missing or unsupported versions produce BLOCKED/failed qualification—not a
   skip or pass. Test-tool locations may be supplied explicitly without being
   persisted into product configuration.

## Source semantics to preserve

- Bash chains the inherited `PROMPT_COMMAND` and loads idempotently.
- Zsh temporarily owns `ZDOTDIR`, restores the user's original directory,
  sources the user's `.zshenv` once, and installs hooks only interactively.
- Fish and Nushell discover through the injected XDG data root and remove only
  that root before children inherit the environment.
- All shells preserve the real executable search path, put only valid managed
  wrappers and the opted-in tmux shim ahead of it, apply initial local CWD once,
  report pane root PID, prompt/activity/CWD context, and never wrap Zentty's own
  management commands.
- Nested, login, non-login, interactive, and non-interactive launches have
  explicit behavior and do not recursively load or leak discovery variables.

## Test-first order

1. Inventory retained Swift expectations and every integration entry point.
2. Tighten the standalone runner around explicit shell paths/versions, exact
   stdout/stderr, clean/custom homes, byte-identical user files, noninteractive
   and nested behavior, hostile paths, Unicode, and management-command bypass.
3. Add the `rust-cli-contract` shell scenario. Launch each actual shell in the
   real staged pane, use its native startup discovery, drive commands through
   the real PTY, and retain argv-boundary receipts from the real staged CLI and
   product event authority.
4. Assert prompt readiness, startup bytes, CWD/activity transitions, root PID,
   nested-shell behavior, management and agent-wrapper passthrough, clean exit,
   pane continuity, and exact before/after hashes for all user configuration.
5. Run all four shells in controlled X11 and Wayland representative profiles.
6. Add mutation coverage for shell selection, version gates, XDG/ZDOTDIR/PATH
   restoration, idempotence, and management-command exclusion. Keep
   `gitignore = true` and `copy_target = false`.
7. Reconcile the matrix, architecture contract, CLI documentation, and dogfood
   report; review the diff; then run every presently executable cell.

## Closure criteria

- Named real-product receipts satisfy every GH-45 acceptance criterion for all
  four shells and report their exact versions.
- Startup assertions distinguish expected prompt/title/keyboard protocol bytes
  from errors, source echo, and arbitrary noise.
- User configuration trees are byte-identical and no integration path leaks to
  nested child environments.
- Missing/old-shell negative fixtures prove an explicit diagnostic and usable
  fallback.
- Matrix totals and the three qualification claims remain honest; GH-45 does
  not turn unrelated BLOCKED, XFAIL, or NOT_IMPLEMENTED cells green.
