# Linux-originated enhancements

This register tracks behavior developed in the Linux port that is not merely a
translation of the reviewed macOS Zentty source. It prevents useful downstream
work from disappearing inside the port and gives the macOS maintainer a concise
set of ideas to evaluate independently.

This document is descriptive, not permission to contact upstream. Any comment,
issue, or pull request against `dedene/zentty` requires Jason's explicit review
and authorization. Most entries are better shared first as product logic and
test evidence rather than as a platform-specific code patch.

## Status vocabulary

- **DELIVERED** — implemented and covered by focused and real-product evidence.
- **IN_PROGRESS** — accepted design with implementation or evidence still open.
- **CANDIDATE** — possibly Linux-originated, but source comparison is incomplete.
- **SHARED** — presented to the macOS maintainer with Jason's authorization.
- **DECLINED** — intentionally retained only in this fork.

## Confirmed enhancements

| ID | Status | Behavior | Primary implementation and evidence | macOS applicability | Upstream disposition |
| --- | --- | --- | --- | --- | --- |
| `linux.agent-effective-cwd` | DELIVERED | Authenticated Codex status can replace the shell's stale directory as the pane/worklane project context after `codex resume` changes to the session's original working directory. The agent-owned directory also feeds Git, task, server, bookmark, sidebar, and restore projections through one canonical selector. | `WorkspaceState::effective_working_directory_for_pane`, the Codex adapter, `application_shell/agent_events.rs`, and [`zentty-linux-dogfood-2026-08-27-codex-session-cwd.md`](design/zentty-linux-dogfood-2026-08-27-codex-session-cwd.md) | High. The state ownership rule and authenticated event fields are platform-neutral even if the UI wiring differs. | Not shared. Prefer a concise behavior description and fixtures before proposing code. |
| `linux.restore-eager-worklanes` | DELIVERED | An opt-in setting initializes every pane in every restored worklane at launch, so unvisited agent sessions and shells start immediately instead of waiting for first navigation. Lazy restoration remains the default. | [GH-136](https://github.com/TamedTornado/zentty/issues/136); `RestoreConfig::start_restored_sessions_in_background`; General Settings; `linux/tests/rust-session-restore`; and [`zentty-linux-dogfood-2026-08-28-eager-restored-worklanes.md`](design/zentty-linux-dogfood-2026-08-28-eager-restored-worklanes.md) | High. This is a workspace lifecycle preference rather than a Linux constraint. | Not shared. Prefer a concise behavior description and real-product receipt before proposing code. |
| `linux.automation-api-cli` | DELIVERED | A stable authenticated local command API is the single automation boundary, and the packaged `zentty` CLI is a client of that API rather than a second command implementation. | [`docs/cli.md`](cli.md), `crates/zentty-core/src/application_api.rs`, `crates/zentty-linux/src/application_api_runtime.rs`, and `crates/zentty-cli` | Medium to high. The command vocabulary and API-first layering are portable; transport and process activation are platform-specific. | Not shared. Surface the contract and architecture, not a Rust/GTK patch. |
| `linux.remote-transfer-scp-first` | DELIVERED | Remote file drop prefers verified `scp` transfer and falls back to the source-compatible remote `cat` stream when `scp` is unavailable, preserving quoted destination handling and visible failure. | `crates/zentty-core/src/remote_paste.rs`, Linux remote-paste runtime, and [`zentty-linux-dogfood-2026-08-10-remote-paste.md`](design/zentty-linux-dogfood-2026-08-10-remote-paste.md) | Medium. The safer transfer policy is portable, while credential and process integration differ. | Not shared. Provide the threat model and fallback behavior if Jason elects to surface it. |

## Candidates requiring source reconciliation

Do not call these unique features until the current macOS baseline has been
checked and the result recorded here.

| Candidate | Question to resolve |
| --- | --- |
| Worklane-grouped Task Manager presentation | Is the grouping and Linux process-tree treatment materially different from the current macOS Task Manager, or only a platform rendering choice? |
| Reorder-then-focus behavior | Is focusing a successfully moved worklane a Linux desktop convention we intentionally added, or has current macOS Zentty adopted it? |
| Linux notification activation routing | Does current macOS activation already route directly to the notifying pane with no intermediate readiness notification? |

## Maintenance rule

When a Linux change introduces behavior not supported by current source evidence:

1. add or update an entry here in the same issue-sized commit;
2. link the owning issue, implementation, focused tests, and real-product receipt;
3. state whether the idea is portable or inherently Linux-specific;
4. never silently relabel a parity deviation as an enhancement; and
5. update the disposition after any Jason-authorized upstream communication.
