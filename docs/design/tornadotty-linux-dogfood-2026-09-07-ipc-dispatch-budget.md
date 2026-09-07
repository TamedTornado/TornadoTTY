# IPC dispatch budget — 2026-09-07

Issue: #163, parent #160. Follows the
[registry-lock repair](tornadotty-linux-dogfood-2026-09-07-ipc-lock-isolation.md).

## Change and boundary

The existing 10 ms GTK tick previously dequeued all four route batches before
doing their work. Count caps alone did not limit elapsed dispatch time. It now
checks a 4 ms cooperative budget before each dequeue and rotates between agent,
tmux, server and product routes. The cursor persists across ticks; existing
32/4/4/4 count caps and per-pane fair queues remain. Accepted work stays in those
queues when the tick yields, with no new pending queue or silent discard.

Agent state still applies each accepted event in order. Sidebar/project
invalidations accumulate and publish once per window per tick, preserving
coalescing. Budget yields have rate-limited, pane/route-attributed diagnostics.

This is **not** a hard 4 ms bound on the whole GTK callback: it cannot preempt
one expensive operation. Target synchronization, final rendering, lifecycle
sweeps and other tick work remain outside this dispatch budget. #163 stays open
for that remaining work, sustained saturation/rate evidence, and PTY overload
content correctness. No terminal stream is intentionally dropped.

## Focused evidence

- Four scheduling tests PASS, including repeated exhausted turns over the real
  ingress queues: messages retained, all routes serviced, quiet-pane fairness
  and per-pane FIFO preserved. These use injected elapsed durations, not sleep.
- Four targeted mutations caught: deadline inversion, cursor modulo corruption,
  count eligibility inversion, and removing empty-route exhaustion. Baseline
  PASS; no missed or unviable mutants. Required scratch-safe/resource-isolated
  wrapper, two workers; approximately two minutes total.
- Existing private-X11 event-coalescing journey PASS with 5,000 events of
  60,188 bytes over 26,976 ms. Extended its quiet child to perform real product
  discovery before acknowledging input; three probes spaced three seconds apart
  all completed while traffic continued, maximum 126 ms. Final GTK task state
  barrier 4 ms, meaningful transition persisted once. Duplicate side effects
  remain coalesced. Anonymous RSS +324 KiB, PSS +899 KiB, heap unchanged;
  no FD/child growth, one fewer thread. Existing 16 MiB / 2-second limits unchanged.
  This producer is sequential: it proves sustained traffic responsiveness, not
  indefinitely saturated multi-client GTK service or that the budget activated.
- Initial transport suite FAIL: mixed-auth test expected eight concurrent
  authorized requests to succeed with only four per-pane ingress slots.
  Its isolated fixture now reserves eight slots; production capacity and
  separate saturation tests are unchanged. Added actual error diagnostics.
  Subsequent full focused transport set PASS: ingress 9, Unix 9, product 17,
  tmux 6, server 2. No retry loop added.
- Existing real GTK tmux/product journey PASS, including split/select/layout,
  replies, wait channels and persisted topology recovery. Initial run and the
  unchanged previous `gh181-compaction` build both FAILED the same stale
  `zentty:` timeout prefix assertion. The check now requires the actual timeout
  reason and channel name, not executable branding; nonzero status remains
  required. The journey's intentional SIGKILL targets its private crash-restore
  fixture, not Jason's client.
- Build/package notices/dependency-age audit PASS. New large-enum Clippy finding
  repaired by boxing the event variant. Five pre-existing GTK Clippy findings
  remain (map/unwrap, pane callback length, two float casts, unused self); the
  old agent-event function-length finding disappeared with this decomposition.
  Dependency linting also reports the two previously recorded core findings.
  No new lint suppressions. Bash syntax, touched Rust formatting and diff checks
  PASS. No full qualification.

Reproduce the GUI check:

```sh
ZENTTY_LINUX_BINARY="$PWD/build/gh163-dispatch/bin/zentty-linux" \
ZENTTY_AGENT_IPC_SCENARIO=event-coalescing \
ZENTTY_AGENT_EVENT_LARGE_PAYLOAD=1 \
ZENTTY_AGENT_EVENT_BURST_COUNT=5000 \
ZENTTY_AGENT_EVENT_PROBE_GAP_SECONDS=3 \
linux/tests/nested-x11 linux/tests/rust-agent-ipc
```

Staged in `build/gh163-dispatch`; not installed. Jason's live client and sessions
were not restarted or modified. Live QA remains pending.
