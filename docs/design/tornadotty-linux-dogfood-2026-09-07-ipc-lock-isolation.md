# IPC parsing / GTK registry isolation — 2026-09-07

Issue #163, parent #160. Previous revision `d59efc82`; Ghostty unchanged at
`bab8c088f45e47a00ce3bfe2c142d6cb51ecd200`.

## Reviewed boundary and concrete defect

Existing IPC already bounds transport frames (384 KiB), canonical events
(64 KiB), reader workers (4), pending connections (32), and pending replies
(32 global / 8 per pane). Existing fair ingress queues are 128 global / 16 per
pane for events and 32 / 4 for each command route. GTK drains at most 32 events
and 4 commands per route per tick. Full admission returns `ingress_full` rather
than acknowledging lost work. Pressure logging is aggregate and pane-attributed.
These were not missing, and this repair introduces no second queue/rate system.

However, `receive_request` held the shared pane-registry mutex while parsing
canonical event JSON. GTK uses the same mutex for topology/credential changes;
putting the parser on a worker did not isolate that shared critical section.
Parsing now precedes a short target-authentication lock. Event ownership and
failed-event destruction stay outside that lock too. Other routes take the
same lock only for their own authority lookup; token checks are not bypassed.

## Regression and validation

- Real Unix socket regression holds the registry like a GUI update and sends
  malformed canonical JSON in a valid envelope. Old code times out waiting for
  that lock; repaired code returns its parse rejection while the lock remains
  held. Cleanup releases the lock and joins on both paths. The initial sandbox
  attempt could not bind a socket; the actual RED/GREEN runs used escalation.
  A first assertion expected the wrong client error variant; corrected it to
  the existing event client's `Rejected` result, retaining the parse reason and
  no-admission checks.
- Strengthened existing saturation test: two concurrent busy producers receive
  32 explicit full-queue errors; the quiet pane is admitted. Including the
  initial rejection, diagnostics show 33 rejected / 3 queued / high-water 3.
  Accepted busy/quiet messages retain fair order, with recovery and shutdown.
- 43 focused transport/admission tests PASS: Unix (9), ingress (9), product (17),
  tmux (6), server (2). Unix/ingress rerun after strengthening saturation: PASS.
- Existing private-X11 event-coalescing journey extended with an optional
  near-limit payload, not a new harness. Final run: 500 events of 60,188 bytes,
  three sibling-PTY input probes during traffic, maximum 71 ms; producer
  2,644 ms, final GTK state barrier 4 ms. Anonymous RSS +196 KiB, PSS +786 KiB,
  main heap unchanged; no FD or child growth, one fewer thread. Existing 16 MiB
  growth and 2-second response limits unchanged. Duplicate side effects stay
  suppressed and the meaningful task transition is presented and persisted.
  This is a bounded burst result, not proof of indefinite hostile-load fairness.
- Build/package notices/age audit PASS; touched IPC library/tests Clippy,
  formatting, Bash syntax and diff checks PASS. No new suppressions.

Reproduce the GUI run with `ZENTTY_AGENT_IPC_SCENARIO=event-coalescing`,
`ZENTTY_AGENT_EVENT_LARGE_PAYLOAD=1`, `ZENTTY_LINUX_BINARY` pointing at
`build/gh163-ipc/bin/zentty-linux`, and
`linux/tests/nested-x11 linux/tests/rust-agent-ipc`.

The unrelated standalone `controlled-agent-test` still FAILS its old Codex
permission fixture contract: it supplies neither the now-required semantic
notification settings nor the required approval FIFO. A diagnostic addition of
the settings exposed the FIFO mismatch and was reverted; no weakened fixture
requirements committed. Track alongside the residual Bash work in #170.

No full qualification, installation or live-client interruption. Staged only in
`build/gh163-ipc`. #163 remains open: count limits do not prove a wall-clock GTK
budget or per-pane sustained-rate fairness, and PTY overload/content correctness
and remaining blocking monitor work still need their own acceptance evidence.

## #181 — compaction visible in the sidebar

Jason observed Bro compacting in the gear dropdown while its lane/pane still
said Working. The fleet recognized compaction text; the sidebar status preferred
task progress over that text, and pane titles/animation still used the generic
Codex live title. Shared the existing compaction predicate between fleet and
sidebar. Known generic Codex activity titles now get a presentation-only
Compacting prefix; the spinner renderer supports that overlay. Saved lane names,
custom pane names, source live titles and progress remain unchanged. Completion
removes the overlay naturally from current state; no extra timer or stored flag.

Regression RED before repair: sidebar still Working. GREEN after repair:
123 core integration tests (workspace state, agent status, fleet), plus a private
X11 GTK test feeding real parsed events into workspace state and updating the
actual sidebar widgets. It checks Compacting (2/5), the unchanged Bro name,
all ten spinner frames retaining Compacting, and return to Working/Running.
The first core test incorrectly tried to establish progress through a compaction
event; corrected the fixture to send the real task.progress event first.
Build/package notices/age audit and diff checks pass. Existing Clippy findings
remain outside this repair; no new suppressions. No full qualification.
Staged at `build/gh181-compaction`, not installed; live QA remains pending.
