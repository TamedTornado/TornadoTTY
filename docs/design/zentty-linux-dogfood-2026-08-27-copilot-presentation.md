# Zentty Linux dogfood: Copilot presentation parity

Date: 2026-08-27
Tracker: GH-126
Parent: GH-7

## Discovery

The authoritative feature inventory retained one `PARTIAL` row after GH-125:
`agent.copilot`. Linux already has the source-compatible `copilot` managed
launcher, private configuration overlay, six hook groups, canonical event
adapter, PID/session correlation, resume command, and a real staged-pane hook
journey. The missing behavior is narrower than another integration: Linux does
not apply the source Copilot presentation reducers to the real Ghostty metadata
it already receives.

Source `CopilotEventAdapter` deliberately seeds `sessionStart` at Idle. Source
`CopilotOSCProgressReducer` promotes that existing idle Copilot session to
Running when OSC 9;4 reports activity. Linux receives OSC 9;4 through the
public Ghostty callback, but `AgentStatusStore::apply_terminal_progress` selects
only Codex sessions.

Source `CopilotTitleNeedsInputReducer` gives a Copilot-owned question title the
highest presentation priority. The classifier accepts asking, awaiting,
waiting, requesting, prompting, confirming, or needing as the first word, or a
standalone `question` token anywhere in the title. Linux receives the real
terminal title, but `WorkspaceState::reconcile_terminal_title` currently
delegates only to Codex title reconciliation.

## Ownership and precedence

- Extend the existing canonical `AgentStatusStore`; do not add GTK-local or
  presentation-only maps.
- Continue using `pane_runtime`'s existing Ghostty title and progress
  callbacks. No polling, process watcher, timer, or second metadata path.
- OSC activity may promote an existing idle Copilot session. It cannot create
  a session, erase human attention, alter task progress, or steal another
  agent's pane.
- A question-like title may promote only an already recognized Copilot session
  unless Linux already owns a trustworthy Copilot process identity. Title text
  alone is never sufficient.
- Copilot title needs-input wins over idle, running, unresolved-stop, and OSC
  activity and projects Question interaction through the existing sidebar,
  attention, fleet, notification, and persistence authorities.
- Codex interrupt/title/transcript ownership and Gemini notification behavior
  remain independent and unchanged.

## Test-first plan

1. Add canonical-store tests for every accepted verb, token boundaries,
   punctuation/case/whitespace, negative substrings, non-Copilot panes, and no
   session creation.
2. Add precedence tests for Idle + active OSC, Remove, existing attention,
   stale unresolved stop, title versus OSC, and preservation of durable agent
   identity fields.
3. Extend the existing `linux/tests/rust-agent-ipc` actor. Its controlled
   Copilot executable remains the external model boundary; wrapper, hook,
   socket, product, GTK, Ghostty, PTY, OSC, title, and visible projection are
   real.
4. Reconcile `agent.copilot` to `IMPLEMENTED` only after focused and real
   product evidence pass. No full qualification or operator deployment belongs
   to this issue.

## Initial limitation under review

The macOS source can recognize Copilot from terminal process metadata before a
hook arrives. Linux's current title callback supplies a title but not a process
name to the canonical store. GH-126 will not broaden title inference to every
shell pane to mimic that pre-hook case. The managed wrapper's authenticated
session-start hook is sufficient for the primary path; any safe pre-hook
alternative must reuse already-owned process identity rather than infer from
the title alone.

## Implementation and focused evidence

The test-first compile failed because `AgentStatusStore` had no terminal-title
entry point. After adding the method, two of three Copilot tests passed; the
remaining failure exposed invalid test evidence twice: `workingDirectory` was
first placed under `session` instead of canonical `context`, then used a
nonexistent `/tmp/copilot` path rejected by existing path validation. The test
now uses real `/tmp` and retains the preservation assertion.

The repair keeps one canonical store. `apply_terminal_progress` selects an
existing Codex or Copilot status and shares runtime promotion/visibility
cleanup, while Codex-only ownership and interrupt fields remain guarded by the
actual tool identity. `apply_terminal_title` orders Copilot title attention
before the existing Codex shell-return and title paths. The Copilot classifier
uses source-equivalent alphabetic first-word and standalone-word matching and
mutates only an existing recognized Copilot session.

Final review found that the first implementation used a boolean `||` chain for
that precedence. A repeated Copilot question title correctly produced no
visible change, but `false` also meant “unclaimed” and allowed the same title to
fall through to Codex reconciliation. The private Copilot reducer now returns
`Option<bool>` so `Some(false)` means “claimed and idempotent,” and the accepted
Copilot aliases are defined once for both OSC and title selection. A regression
assertion proves the repeated title leaves the complete stored status,
including its timestamp and durable identity, untouched.

A repository-wide `cargo fmt --check` also reported formatting drift in two
unrelated, already-tracked files (`workspace_state.rs` tests and
`agent_fleet.rs`). GH-126 did not absorb those changes; its three modified Rust
files pass a direct `rustfmt --check`.

The first final inventory gate rejected owner 126 because the authoritative
reviewed issue ledger ended at 124 even though live GitHub confirmed GH-126 was
open. The repair adds the real issue, its GH-7 dependency, and its sole
`agent.copilot` ownership to that ledger rather than bypassing owner validation.
Clippy also required idiomatic `?` propagation for the new optional Copilot
lookup; the implementation now uses it.

The next inventory run caught duplicate ownership because parent epic GH-7
still listed `agent.copilot` after the inventory moved it to GH-126. The ledger
now records GH-7 as the dependency and GH-126 as the single implementation
owner, matching the existing one-owner invariant.

With ownership reconciled, the summary assertion found that the inventory
runner omitted an enum key when its count reached zero. That made the first
fully implemented agent inventory impossible to represent explicitly. The
runner now initializes `IMPLEMENTED`, `PARTIAL`, and `NOT_IMPLEMENTED` to zero
before counting, so consumers receive `PARTIAL: 0` rather than a missing key.

Focused receipts:

- `agent_status`: 33 PASS, 0 FAIL, including three new Copilot reducer,
  precedence, identity, and negative suites.
- Copilot canonical adapter subset: 3 PASS, 0 FAIL.
- Existing Codex OSC regression: PASS.
- `shellcheck` for the existing actor and controlled external-agent fixture:
  PASS.
- Staged ReleaseSafe build: PASS; dependency publication-age audit reported 91
  packages and 0 exceptions; package notices completed.
- Existing `linux/tests/rust-agent-ipc`, focused scenario
  `copilot-presentation`, under private X11 session
  `8dc9969d9fca33d2c4f5fbc4684a0468a3d31bae23a3579fdb5a62d58700c0a4`:
  PASS. The wrapper, private hook overlay, authenticated Unix socket, app,
  Ghostty, PTY, OSC callback, title callback, and sidebar projection were real.
  The controlled process replaced only Copilot's external/model behavior. It
  proved Idle -> Running -> NeedsInput/Question, late-OSC precedence, and a
  non-Copilot negative title without inventing agent state.
- Final issue gate: `zentty-core` PASS; `zentty-linux` PASS; Clippy with warnings
  denied PASS; inventory contract PASS; all modified shell actors/runners PASS;
  diff hygiene PASS.
- Authoritative inventory summary: 63 entries, 52 `IMPLEMENTED`, 0 `PARTIAL`,
  and 11 `NOT_IMPLEMENTED`. This is feature-inventory completion, not a claim
  of full Linux qualification.
- Operator installation was deliberately untouched because the installed app
  was in active use. All product evidence used the staged ReleaseSafe binary.
