# TornadoTTY — ingress backpressure, 2026-09-05

Issue: #163, under #160. Previous report:
[GUI runtime isolation](zentty-linux-dogfood-2026-09-04-gui-runtime-isolation.md).

## Confirmed boundary and repair

At `0ed7032f`, AgentRuntime created four unbounded Rust channels and collected
all available messages on each GUI tick. Socket frame/connection limits did
not bound those queues. This is a confirmed structural gap, not a newly proven
cause of the historical 8 GiB heap incident repaired in #161.

The four channels now use one focused ingress implementation, not forwarding
threads or a second event bus. It owns bounded per-pane queues, FIFO admission
order within each pane, and round-robin service across panes. Admission never
waits for the GUI to drain. Full queues return explicit rejection; they do not
acknowledge, discard, coalesce, or automatically retry the rejected command.
Pane identity comes from authenticated targets, not a new client field.

| Route | Global pending | Per-pane pending | Maximum dequeued per GUI tick |
|---|---:|---:|---:|
| Agent events | 128 | 16 | 32 |
| tmux commands | 32 | 4 | 4 |
| Development-server commands | 32 | 4 | 4 |
| Product/API commands | 32 | 4 | 4 |

The existing frame ceiling is 384 KiB. Combined route capacity is 224 messages,
so at most 84 MiB of serialized input can be represented by these queues
before accounting for parsed-object overhead. This is not a whole-process RSS
cap. Existing socket worker/read/connection limits remain separate. A normal
10 ms tick has capacity for up to 3,200 events/second when other work permits;
limits provide finite burst headroom rather than unlimited buffering. One pane
cannot consume an entire route. These are work-count bounds, not a claim that
a single expensive GUI operation can be preempted on a hard frame deadline.

Overload uses stable wire code `ingress_full` and ProductUnavailable category.
Normal-operation diagnostics aggregate rejected counts, current depth,
lifetime high-water mark, and last affected pane once per five seconds at
most; they retain no payload or credential. That interval is only log-rate
control, not an event correctness or approval heuristic.

## Focused proof and discoveries

- Tests were added before the new queue API; the initial red was a missing-API
  compile failure, not a reproduced GTK hang. Existing transport tests were
  migrated to the same bounded implementation used by the product.
- Queue tests cover global/per-pane limits, rejection retaining the original
  message, fair ordering, concurrent admission, batch remainder, disconnect,
  timed receiving, invalid limits, and aggregate diagnostics.
- Real Unix-socket tests pause consumption, fill a busy pane, prove explicit
  overload rejection, admit/service a quiet pane, and shut down a full queue.
  A separate product-route test proves a rejected request is not executed and
  the previously accepted request still receives its reply.
- The first mutation audit caught 20, missed 8, and found 21 compiler-unviable
  mutations. All misses were real-route pane-identity adapters: synthetic
  messages alone did not test them. Added independent-capacity tests for all
  four actual authenticated message types. The next audit caught 28 with
  21 compiler-unviable and no survivors. Later batch/diagnostic additions are
  included in the final audit below, rather than covered by that earlier count.
- The new route fixture initially used CLI verb `list` where the wire protocol
  requires `server-list`; corrected the fixture, not the protocol.
- Strict Clippy first encountered existing dependency lints in core; scoped
  `--no-deps` checks then identified missing must-use annotations in the new
  API. Those were fixed. No lint suppression or unrelated product edit was added.
- The first normal build needed escalation for its pinned Ghostty fetch;
  subsequent builds reused that verified source. No dependency was updated.

## Reproduction

```sh
cargo test -p zentty-agent-ipc --offline --locked
cargo clippy -p zentty-agent-ipc --lib --tests --no-deps --offline --locked -- -D warnings
linux/tests/mutate-rust -p zentty-agent-ipc \
  -f crates/zentty-agent-ipc/src/ingress.rs -j 4 \
  --cargo-test-arg=--lib --cargo-test-arg=--test=ingress --timeout 30
ZENTTY_BUILD_SCOPE=product ZENTTY_BUILD_OUTPUT_DIR="$PWD/build/gh163" linux/scripts/build-local
ZENTTY_LINUX_BINARY="$PWD/build/gh163/bin/zentty-linux" \
ZENTTY_AGENT_IPC_SCENARIO=event-coalescing ZENTTY_AGENT_EVENT_BURST_COUNT=5000 \
ZENTTY_AGENT_EVENT_COALESCING_RECEIPT=/tmp/gh163-gtk-final.json \
linux/tests/nested-x11 linux/tests/rust-agent-ipc
```

Socket/display tests and governed mutation scopes need host permissions outside
the restricted sandbox. GTK uses a private display, home and state directory.
No traffic targets Jason's client, and no installed binary is replaced.

## Remaining #163 scope — not complete

- Persistence was the next unbounded boundary; the September 6 entry below
  records its replacement and the distinction between startup and post-GUI
  shutdown waits.
- Transcript enrichment's per-window worker/result/cache bounds are recorded
  below. Whole-application lifetime bounds and discovery enumeration remain
  part of the unfinished background-work audit.
- Audit remaining blocking discovery and expensive GTK command paths. The
  four socket workers currently wait for product replies; several waiting
  commands can still delay other traffic despite bounded GUI ingress.
- Engine-owned PTY gathering already uses four 64 KiB buffers in the pinned
  Ghostty Exec.zig with backpressure. That source inspection is not a substitute
  for the high-output/byte-order/interactive-latency tests required by #163.
- Finish overload/stale-target/background cancellation evidence across these
  boundaries before closing #163. Cgroup containment remains #162 and native
  process-isolation investigation remains #164.

## Final ingress checkpoint results

- Complete affected IPC crate: **119 passed, 0 failed**.
- Final queue/batch/diagnostic mutation audit: **30 caught, 22 compiler-unviable,
  0 missed**, 52 total, 59 seconds. Compiler-unviable replacements attempted
  nonexistent constructors/defaults for generic queue types; they are not kills.
- IPC library and tests pass strict scoped Clippy; diff whitespace checks pass.
- Product-only ReleaseSafe build, dependency-age audit (91 packages, zero
  exceptions), and package notice collection pass.
- Final private-X11 event journey: **PASS**, 5,000 events/title frames over
  25.419 seconds; three sibling input probes, maximum **62 ms**; post-producer
  GTK drain **3 ms**. Main-heap growth **0 KiB**, total PSS growth **34 KiB**,
  anonymous RSS/PSS fell **544 KiB**, no descriptor/child growth. Duplicate
  side effects remain zero, with exactly one meaningful task transition.
- Existing private-X11 CLI contract journey: **PASS**, including actual pane
  operations and concurrent-instance isolation.

Local outputs: `/tmp/gh163-ipc-final.log`, `/tmp/gh163-clippy-complete.log`,
`/tmp/gh163-mutants-complete/mutants.out/outcomes.json`,
`/tmp/gh163-build-complete.log`, `/tmp/gh163-gtk-final.json`, and
`/tmp/gh163-cli-gtk.log`. Commands above recreate them. No full matrix was run;
no new aggregate matrix totals or Wayland result is claimed. The built product
is staged in `build/gh163`, not deployed into the running application.

## September 6 — bounded persistence worker (#163)

### Findings and decisions

- The old worker had unbounded request **and completion** channels. The
  existing 350 ms live-state debounce limited submission frequency, not queue
  retention when storage fell behind. Successful completion messages also
  accumulated until the GUI drained them.
- Correction to the earlier scope note: `save_clean_exit` runs **after** the
  GTK loop, window teardown, and Ghostty runtime drop. Its synchronous wait
  is a durability barrier, not an active-GUI freeze. Preserve it; do not add
  an asynchronous quit state machine or a timeout that claims unsaved state
  was saved.
- `complete_launch` did wait for the worker before entering the GTK loop.
  Launch now submits snapshot consumption asynchronously. A newer pending
  snapshot may supersede that deletion: replacing old state with current
  state is the intended equivalent, not deletion of the newer snapshot.
- Snapshot deletion itself does not acquire the adjacent storage lock. The
  first attempted contention fixture incorrectly assumed it did; corrected
  the fixture to enqueue a real locked **write** ahead of consumption. The
  old implementation then waited through the 250 ms lock deadline. The
  focused test requires launch submission below 200 ms, releases the lock,
  flushes the actual worker, and checks consumption on disk.

### Implementation bounds and ordering

- Replaced the old worker, rather than adding a forwarding system. Its
  focused module owns one pending latest-value snapshot, at most one
  in-flight operation, and one non-coalescible control barrier. There is
  still exactly one persistence thread. No disk I/O holds the mailbox lock.
- Pending ordinary snapshots are replaced only by a current generation.
  Superseded topology is freed outside the lock. This is a bound on retained
  snapshot **count**, not a claim that arbitrarily large workspace topology
  has a fixed byte size.
- A final save excludes further submissions, follows the pending live save
  so draft merging still works, and marks clean only after an accepted,
  successful final publication. Worker teardown drains accepted state;
  worker unwinding disconnects outstanding response waiters.
- Completion retention is one failure-count/latest-error summary. Successes
  do not allocate queued receipts or erase previous errors. Coalescing has a
  bounded counter logged when the GUI drains background diagnostics.
- Background failures now have an operation-neutral log prefix because
  they include launch consumption as well as live writes. No payloads or
  private terminal contents are added to logs.

### Focused validation checkpoint

- **23 persistence tests passed**, including real file-store publication,
  1,000-to-one pending-save coalescing, stale rejection, barrier ordering,
  ten storage failures followed by success, teardown draining, unvisited
  agent draft preservation, and two clean relaunches.
- Queue saturation tests deliberately park worker scheduling before starting
  it; they do not replace file writes with fake success. The lock-contention
  regression exercises the actual storage boundary.
- Strict GTK binary/test Clippy is **not green**: six diagnostics remain in
  unchanged application coordinator, agent-event, pane-runtime, shell, and
  window-chrome code. The newly introduced worker warning was corrected;
  no remaining diagnostic references the changed persistence code. These
  unrelated findings were not suppressed or swept into this repair.
- The first mutation pass exposed overlapping lifecycle booleans and a
  missing queued-final-save shutdown case: 14 caught, 4 missed, 2 unviable,
  4 timeouts. Replaced the booleans with explicit Open/Barrier/Closing/Stopped
  states, added queued-barrier shutdown coverage, and tested the pressure
  summary rather than incidental log formatting. The second pass left one
  missed mutant that panicked only at worker exit after successfully saving;
  the shutdown test now asserts the thread's join result as well as disk state.
- Final mutation audit: **19 caught, 0 missed, 2 compiler-unviable, 5
  timeouts**, 26 total in 3 minutes. The five mutations remove shutdown/work
  or break the condition-variable wake condition; each hangs and is stopped
  by the 20-second mutation test limit. The runner exits 3 for those timeouts,
  not a product-test failure. Do not count the two unviable mutations as kills
  (one requests an absent `Default` implementation, one creates an invalid
  let-chain). The ordinary 23-test baseline completes in about 0.08 seconds.
- Final product-only build **PASS**, dependency-age audit **PASS** (91
  packages, zero exceptions), notice collection **PASS**. Existing private-X11
  restore scenarios both **PASS** on that build: unvisited authenticated-agent
  restoration through two relaunches, and unresolved Codex alias preservation
  through two unvisited relaunches followed by launch on visit.
- No full qualification or new matrix total is claimed. Jason's
  installed/running client has not been replaced or stopped. The staged
  executable is `build/gh163/bin/zentty-linux`.

Reproduction commands (GUI and mutation commands need the existing host
permission boundary):

```sh
cargo test -p zentty-linux --bin zentty-linux persistence_coordinator --offline --locked
linux/tests/mutate-rust -p zentty-linux \
  -f crates/zentty-linux/src/persistence_coordinator/worker.rs -j 4 \
  --cargo-test-arg=--bin=zentty-linux --cargo-test-arg=persistence_coordinator \
  --timeout 20 --output /tmp/gh163-persistence-mutants-complete
ZENTTY_BUILD_SCOPE=product ZENTTY_BUILD_OUTPUT_DIR="$PWD/build/gh163" linux/scripts/build-local
ZENTTY_LINUX_BINARY="$PWD/build/gh163/bin/zentty-linux" \
  ZENTTY_UNVISITED_RESTORE_ONLY=true linux/tests/nested-x11 linux/tests/rust-session-restore
ZENTTY_LINUX_BINARY="$PWD/build/gh163/bin/zentty-linux" \
  ZENTTY_CODEX_ALIAS_RESTORE_ONLY=true linux/tests/nested-x11 linux/tests/rust-session-restore
```

Local logs are `/tmp/gh163-persistence-tests.log`,
`/tmp/gh163-persistence-mutants{,-final,-complete}.log`,
`/tmp/gh163-persistence-build-final.log`,
`/tmp/gh163-persistence-clippy-final.log`,
`/tmp/gh163-persistence-restore-final.log`, and
`/tmp/gh163-persistence-alias-final.log`.

## September 6 — transcript enrichment backpressure (#163)

### Findings and repair

- The old scheduler created a new thread for every changed candidate. Setting
  the previous cancellation flag did not mean that thread had exited. Its
  result channel, cached questions, and pane/session path hints were unbounded.
  The new admission regression failed against that implementation: candidate
  33 was accepted without a preceding GUI drain.
- The existing per-window owner now retains at most **4 worker handles**,
  including cancelled-but-still-running workers, **32 current pending panes**,
  **4 queued results**, **64 cached questions**, and **32 path hints**.
  Each pane can occupy only one actual worker slot. Queued replacements keep
  one latest candidate in their original queue position; dispatch skips a
  still-running pane so it cannot occupy the other worker slots.
- The GUI reads at most four results per drain and joins only workers already
  known to have exited. Shutdown cancels pending candidates and clears queued
  work without waiting for file I/O. Generation/candidate checks still reject
  stale results. A panicking worker removes only its own generation.
- Overload rejects optional enrichment for newly arriving panes. It does not
  clear or invent canonical agent attention; generic attention remains until
  a later successful enrichment. There is no automatic retry loop for rejected
  candidates. Existing pending panes may still replace their queued candidate
  at capacity. Aggregate rejection diagnostics are limited to once per five
  seconds; that interval controls logging only, not state correctness.
- Cache eviction is bounded and oldest-first. Only the latest cached file
  version and the current session hint for a pane are retained. Closing a pane
  removes its hint even when it has no pending job. Eviction causes rediscovery,
  not loss of canonical session/attention state.
- Related real read-bound defect: `read_transcript_tail` sought near the old
  EOF, then used `read_to_end`, which could follow concurrent appends beyond
  the claimed 256 KiB boundary. Reads now stop at the captured length. A real
  unlinked-file fixture appends between length capture and read, for both short
  and greater-than-256-KiB files. Restoring the old unbounded read makes the
  regression fail by including the appended text.

### Focused evidence

- **14 GTK-crate enrichment tests PASS**: real transcript retry/parse/cache,
  full admission, queue replacement/cancellation, 1,000 replacements with
  held cache access, quiet-pane service, single-pane worker quota, stale
  completion, worker panic, owner drop, cache eviction and diagnostic limits.
  The stall fixture locks the actual cache used by actual file workers; it
  does not replace the resolver or fabricate successful parse results.
- **Five core transcript integration tests and one captured-tail regression
  PASS**. Captured-tail mutation audit: **9 caught, 0 missed, 0 unviable, 0
  timeouts**, 52 seconds.
- Initial scheduler/cache mutation audit: **41 caught, 5 missed, 5 unviable,
  1 timeout**, 52 total. Added coverage for panic-generation cleanup, owner
  drop and diagnostic rate/count semantics; final audit recorded separately.
- Final scheduler/cache audit: **51 caught, 0 missed, 5 compiler-unviable,
  1 timeout**, 57 total in four minutes. The timeout mutation inverted the
  finished-thread check and joined a live worker while the test held its cache
  lock: it is the GUI-blocking defect this guard prevents. The runner exits 3
  for that 15-second timeout; ordinary tests have no hangs. Unviable mutations
  requested nonexistent `Default` implementations; they are not counted as kills.
- Final product build and dependency-age audit **PASS** (91 packages, zero
  exceptions). Final private-X11 enrichment **and** pending-shutdown journeys
  **PASS**. Build staged at `build/gh163/bin/zentty-linux`, not installed.
- Strict Clippy remains non-green due to **six unchanged GTK findings and
  two unchanged core findings**. The new scheduling ownership warning was
  fixed; no remaining diagnostic points at changed enrichment/transcript code.
- Reused the existing real GTK/PTY/IPC/transcript enrichment and pending-close
  journeys, exposing them through `ZENTTY_AGENT_IPC_SCENARIO=codex-enrichment`
  so this repair does not require the entire agent-adapter journey. No new
  driver, fixture protocol, or qualification layer was introduced.

### Remaining uncertainty and scope

These are **per-window-owner** bounds, not a claim of four threads across all
windows. Cancellation cannot interrupt an already blocked filesystem syscall.
Discovery still collects directory entries before selecting its newest bounded
candidate set; that allocation/scan boundary remains to audit. The broader
cross-window lifetime, blocking-discovery, socket-worker, GTK command-budget,
and PTY overload requirements in #163 are not closed by this change. No full
qualification was run and Jason's installed/running client was not touched.

Commands:

```sh
cargo test -p zentty-linux --bin zentty-linux codex_enrichment --offline --locked
cargo test -p zentty-core --test codex_transcript --offline --locked
cargo test -p zentty-core --lib append_after_length_capture --offline --locked
linux/tests/mutate-rust -p zentty-linux \
  -f crates/zentty-linux/src/codex_enrichment.rs \
  -f crates/zentty-linux/src/codex_enrichment/cache.rs -j 4 \
  --cargo-test-arg=--bin=zentty-linux --cargo-test-arg=codex_enrichment \
  --timeout 15 --output /tmp/gh163-enrichment-mutants-final
linux/tests/mutate-rust -p zentty-core -f crates/zentty-core/src/codex_transcript.rs \
  --re read_captured_tail -j 4 --cargo-test-arg=--lib \
  --cargo-test-arg=codex_transcript --timeout 15 --output /tmp/gh163-transcript-tail-mutants
ZENTTY_LINUX_BINARY="$PWD/build/gh163/bin/zentty-linux" \
  ZENTTY_AGENT_IPC_SCENARIO=codex-enrichment linux/tests/nested-x11 linux/tests/rust-agent-ipc
```

Local evidence: `/tmp/gh163-enrichment-{tests,clippy-final,build-final,gtk-final}.log`,
`/tmp/gh163-enrichment-mutants{,-final}.log`,
`/tmp/gh163-transcript-tail-red.log`, `/tmp/gh163-transcript-tail-mutants.log`,
and `/tmp/gh163-enrichment-core-clippy.log`.
