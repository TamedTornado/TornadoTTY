# Zentty Linux dogfood: Amp parity (GH-127)

Date: 2026-08-27

## Scope and source audit

The authoritative inventory marked `agent.amp` `NOT_IMPLEMENTED`, although
Linux already recognized Amp and shipped substantial plumbing. The audit found:

- `AgentLaunchTool`, wrapper lookup, persistent-integration consent, PID export,
  marked plugin installation/removal, staging, and settings UI already exist.
- `linux/tests/staged-bundle` proves the plugin file is copied and a controlled
  Amp executable receives its original arguments plus basic tool/PID variables.
- No Linux test executes the bundled TypeScript plugin.
- Linux uses the generic persistent launch plan, so it does not set the source
  `PLUGINS=all` activation flag, publish sanitized launch arguments, or emit the
  source's ordered pre-launch `session.start` and `agent.running` events.
- `PaneRestoreDraft::resume_command` has no Amp case. Linux therefore persists
  Amp session identity but cannot safely restore it.

The current source authority is `AmpPluginInstaller.swift`,
`AmpResumeArgumentSanitizer.swift`, the Amp branch of
`AgentLaunchBootstrap.swift`, `AgentResumeCommandBuilder`, and the bundled
`zentty-amp-zentty.ts`. Source preserves only approved resume options, rejects
execute/management modes, validates `T-[A-Za-z0-9_-]+` thread IDs, and builds
`amp threads continue ... <thread-id>`. The plugin sends session, running, and
idle events and strips `AMP_API_KEY` from the helper environment.

## Design constraints

- Extend the existing `AgentLaunchPlan`, `PaneRestoreDraft`, canonical event
  protocol/status store, persistent installer, and `rust-agent-ipc` actor.
- Do not add another launcher, restore builder, status map, socket protocol, or
  product test harness.
- Amp is the controlled external/model boundary. The staged wrapper, CLI,
  installer, actual bundled plugin module, process, PTY, socket, app, Ghostty,
  and sidebar projection must remain real.
- Do not install or restart the operator's active Zentty build.

## Test-first plan

1. Add red table-driven sanitizer and exact launch-plan tests.
2. Add red source-compatible restore-command and hostile-input tests.
3. Implement shared sanitizer semantics once, source-compatible Amp planning,
   and safe restore construction in their canonical Rust owners.
4. Extend `rust-agent-ipc` with a focused scenario that executes the actual
   staged TypeScript plugin under the reviewed Bun runtime and observes the
   real product boundary.
5. Move the inventory row only after focused evidence passes.

## Discoveries and receipts

This section is append-only during GH-127.

- Red baseline: focused tests failed to compile because the shared Amp
  sanitizer export did not exist. This proved launch snapshots and safe restore
  were absent rather than merely undocumented.
- First implementation compile found two local `&str`/`String` mistakes in
  option matching. The repair uses the borrowed `&str` directly; it does not
  enable an unstable feature or weaken matching.
- The first nested-X11 plugin journey missed the expected session event, but
  direct `set -e` propagation bypassed the actor's `fail` helper and cleanup
  erased the product log. The focused Amp waits now route through the existing
  failure reporter, and its audit receipt is included in ordinary trap cleanup;
  this preserves actionable evidence without adding another harness.
- Redacted child evidence then proved the wrapper executed Amp with no
  `ZENTTY_AGENT_TOOL`, no plugin audit, and no disable flag. Directly invoking
  `<stage>/bin/zentty launch amp` succeeded, which isolated a real installed
  path defect: `integration_resource` assumed the CLI was exactly two parents
  below the stage prefix. Pane wrappers actually invoke
  `<stage>/libexec/zentty/agent-wrappers/shared/zentty`, so the installer could
  not find `share/zentty/amp/...` and silently launched without integration.
  Resource discovery now searches executable ancestors, matching the existing
  resource rule used by ephemeral agent launch assets. The real pane journey,
  not the direct CLI test, is the regression proof.
- After resource repair, the real plugin exposed a stale provisional session:
  source-compatible pre-launch events have no thread ID and created
  `pane-default`; later plugin events used the real thread ID, leaving the
  provisional Running status to outrank the real thread's final Idle. The
  canonical store now rekeys only a provisional session whose tool and tracked
  PID match an explicit `session.start`. A mismatched-PID test proves unrelated
  sessions cannot claim it.
- The next real journey reached the correct final Idle state and all five
  credential audits passed, but the harness compared the established
  `launch agent:amp` receipt against a nonexistent `agent:<amp>` format. The
  assertion now checks the actual stable receipt rather than changing product
  behavior.
- Extending `staged-bundle` brought that script under the issue's shell gate;
  ShellCheck rejected an older standalone `! grep` that implicitly depended on
  `errexit`. It is now an explicit failure branch with the same uninstall
  policy and an actionable message.
- Final design review moved the tool-specific launch plan and sanitizer out of
  shared `agent_launch.rs` into focused `agent_launch/amp.rs`. The shared module
  retains only dispatch and the public re-export, avoiding another monolithic
  adapter accumulation while preserving one launch authority.

## Final evidence

- Focused launch/restore/status suites: 24 + 6 + 34 PASS, 0 FAIL.
- Full `zentty-core`, `zentty-agent-ipc`, and `zentty-linux` crate gates: PASS.
- Clippy across all three affected crates with warnings denied: PASS. The first
  Clippy run rejected embedded policy tables, an opaque option expression, and
  an assigning clone; policy tables now live at module scope, matching is an
  explicit branch, and rekeying uses `clone_into`.
- Staged ReleaseSafe build: PASS; dependency publication-age audit reported 91
  packages and 0 exceptions; notices completed. No installed file was changed.
- `staged-bundle` under private X11 session
  `758c0273ed92b7caa93206e318aaaf65abbc28a0fb493895140b7ebaa9a9a2eb`:
  PASS, including owned Amp install, conflict-safe fallback, product smoke,
  existing full agent journey, and tmux compatibility. The tmux actor printed
  an expected cleanup `Killed` line while its owning journey still completed
  and reported PASS; no qualification cell was converted or skipped.
- Final focused real-plugin journey under private X11 session
  `9bea0cd8d35489094400884c29f36b8494b2750284d6f7e23f2e82ddbeab909d`:
  PASS. It exercised the staged pane wrapper, installed-helper resource lookup,
  marked plugin installer, actual TypeScript plugin under Bun, controlled Amp
  process, PTY, five audited helper execs, authenticated Unix socket, product,
  Ghostty, canonical status store, provisional-session rekey, and sidebar
  Running -> Idle projection. Every helper audit reported the API credential
  absent, and the sentinel appeared in no product or test receipt.
- Inventory runner and tests: PASS. Totals are now 63 entries, 53
  `IMPLEMENTED`, 0 `PARTIAL`, and 10 `NOT_IMPLEMENTED`.
- ShellCheck for every modified actor/runner, Bun compilation of the plugin
  driver, and diff hygiene: PASS.

This is focused feature evidence, not a claim of full Linux qualification. The
operator's active installed Zentty remained untouched throughout GH-127.

## Remaining limitation

No live Amp account or model request was made. The controlled executable
replaced only that external boundary; the actual shipped plugin and every
Zentty/terminal boundary were real. Future Amp API changes can still require a
plugin update, but the checked-in driver compiles and executes the current
source plugin contract and will fail if its registered event surface drifts.
