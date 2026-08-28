# Durable Codex alias restore intent

## Dogfood failure

After a real restart, a saved worklane that had previously run a named Codex
resume opened as an ordinary shell. The saved workspace still contained the
worklane, pane, working directory, and `lastRunCommand`, but its restore-draft
array had no entry for that pane. Other panes contained UUID-backed Codex drafts.

Codex's own session index still mapped the name to a valid session. The product
had not attempted and failed the resume; serialization had already reduced the
pane to shell history.

## Root cause

The old model gave a named resume two unequal representations:

- authenticated shell integration persisted `codex resume <name>` as command
  history; but
- only a later authenticated Codex event made the session a structured,
  automatically restorable draft.

Accepted UUID drafts could survive unvisited relaunches after the preceding
repair, but the initial named launch had no durable intent until post-launch
canonicalization succeeded. Eager worklane startup reduced exposure without
repairing this boundary.

## Repair

An authenticated shell pre-exec signal for exactly three structurally parsed
arguments—`codex`, `resume`, and one safe target—now creates a provisional
`PaneRestoreDraft`. The draft stores:

- the target in `sessionID` until a canonical identity is known;
- the exact original argv in `agentLaunchSnapshot.arguments`;
- the pane's working directory; and
- no environment, task state, transcript, prompt, or secret.

If Codex later authenticates with a canonical session ID, the live draft uses
that ID while retaining the original alias argv. Restore command generation
prefers the effective `sessionID`; therefore an unresolved draft resumes by
name, while a reconciled draft resumes by canonical ID. A failed launch still
uses the existing persistent recovery UI. Zentty does not retry an alias after a
known canonical-ID launch fails because a renamed or reused alias could target a
different session.

The parser uses the already reviewed and locked `shlex 2.0.1` dependency. It
accepts quoted names with safe spaces, rejects additional commands/arguments,
environment prefixes, option-like targets, shell metacharacters, controls, and
oversized targets, and renders the target with shell quoting.

An ordinary command still requires the host-observed physical submission marker
before it becomes shell history. The narrow Codex promotion additionally accepts
the authenticated Bash pre-exec signal alone: that signal proves the exact
command is executing in the capability-bound pane, while host key-release
observation can be absent under valid compositor/input paths.

## Test-first evidence and discovery

The first focused model test failed with zero drafts after a named resume. After
the initial implementation, the complete Core suite caught an option-injection
regression (`-starts-with-option`); restoring the leading-alphanumeric rule fixed
it without removing safe spaces.

The first real X11 actor extension then failed even though the terminal visibly
executed the command. Logs showed the authenticated shell pre-exec signal but no
host `InputSubmitted` gesture under synthetic input. Separating narrow
authenticated Codex promotion from generic physical shell-history capture made
the real boundary deterministic without weakening arbitrary command handling.

Final focused receipts:

```text
rust-session-restore: PASS codex-alias provisional=durable canonical=optional unvisited-relaunches=2 argv=structural
rust-session-restore: PASS unvisited-agent drafts=preserved relaunches=2 authentication=real
```

The complete `zentty-core` suite passed, including 75 workspace-state tests and
the existing restore-command injection suite. The staged actor uses real GTK,
Ghostty surfaces, PTYs, shell integration, the packaged managed Codex wrapper,
clean snapshots, two full relaunches, and physical navigation. The controlled
Codex process intentionally delays authentication so the alias fallback—not a
hook-provided UUID—must carry the intent.

## Architecture and limitations

- No second store, status map, restore coordinator, timer, or test harness was
  introduced.
- The provisional state uses the existing pending restore-draft authority.
- This repair intentionally covers exact managed Codex resume commands only.
  Other agents require their own source-backed invocation and identity rules.
- Supported provisional names begin with an alphanumeric character and contain
  only letters, numbers, spaces, underscore, hyphen, or period. Unsupported
  names remain ordinary command history rather than being silently broadened.
