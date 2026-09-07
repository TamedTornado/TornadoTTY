# Recoverable pane actions — 2026-09-07

Issue: [#179](https://github.com/TamedTornado/TornadoTTY/issues/179), parent #160.
Previous report: [IO boundaries](tornadotty-linux-dogfood-2026-09-07-io-boundaries.md).

## Confirmed failure and repair

The existing closed-pane-restore journey now replaces its private instance's
credential directory with a regular file. Real pane credential publication
fails. Against `b7f6042f` (`build/gh163`), Split Right logs the publication error
and destroys the application window. This confirms the fatal action policy,
not the cause of any earlier production crash.

Removing `quit()` alone was insufficient: ordinary close-based rollback could
change focus, layout, and Undo history. Synchronous creation now retains a
temporary workspace snapshot and restores it on failure, discarding fresh
credentials and transient launch intent. IDs remain monotonic. No persistent
parallel state owner or error framework was added. The existing dismissible
warning banner reports the action, pane, and filesystem context; terminal
focus returns after the originating palette closes.

Audited helper callers: new worklane, split/add/duplicate pane, Undo Close Pane,
task launch, pane close, and worklane close. Cleanup after a committed close
continues across all panes and renders the resulting workspace before reporting
errors. Explicit final-pane close closes that window, not sibling windows.
Native disposal failure means an invalid native handle in the pinned Ghostty
implementation; this was source-reviewed, not fault-injected. Already-disposed
processes cannot safely be resurrected by rolling back topology.

Other child-exit/fallback shutdown paths remain outside this user-action repair;
#160/#163 are not closed or claimed to provide complete process isolation.
No Ghostty source change was required (pin `bab8c088f45e47a00ce3bfe2c142d6cb51ecd200`).

## Focused proof

Build: `ZENTTY_BUILD_OUTPUT_DIR=build/gh179 linux/scripts/build-local`.
Publish-age audit: 91 packages, zero exceptions. Build and package notices PASS.

Journey:

```sh
ZENTTY_LINUX_BINARY="$PWD/build/gh179/bin/zentty-linux" \
  linux/tests/nested-x11 linux/tests/rust-closed-pane-restore
```

PASS: eight failed creation/replacement actions preserve topology and focus,
with real surviving PTY input after each; no phantom Undo entries. Failed Undo
retains the actual closed pane, then repaired credentials permit a successful
retry with controlled Codex resume, real scrollback, working-directory fallback,
exact PTY ownership, quiescent callbacks, and persisted restore intent.
The optional `ZENTTY_PANE_ACTION_ERROR_SCREENSHOT` capture was visually reviewed:
readable, wrapping error with a dismiss control; both original panes remain.

Earlier fixture attempts failed: a worklane query matched two commands; the
Makefile needed its documented `.PHONY` declaration; repeated Undo searches
matched historical log entries and executed the wrong command. Unique queries,
a valid task fixture, and the existing count-based wait corrected these without
retries or relaxed product assertions. Failure cleanup now preserves diagnostic
output before deleting private fixtures.

Rust binary-target tests: pane_runtime **13 PASS, 1 existing IGNORE** (GTK
accessibility requires a controlled display); close_runtime **3 PASS**.
An initial `--lib` invocation selected zero relevant tests and is not evidence.
Strict binary-target Clippy **FAIL: six existing findings**, no suppressions:
application map/unwrap, two long callbacks, two geometry casts, unused self.
An all-targets invocation also stops at two existing core lint findings.
The new collapsible-if finding was fixed. Shell syntax and diff checks PASS.
No mutation run or kill-rate claim. No full qualification, Wayland run, or new
matrix totals. This is real failure/recovery integration proof, not an
unattended-maintenance qualification claim.

Build staged only. Jason's live terminal was not installed over, launched,
restarted, or stopped; user QA remains for the agreed later batch.
