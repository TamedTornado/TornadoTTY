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

- Persistence still has an unbounded worker-request/result channel and
  synchronous waits in explicit save/clean-exit paths. Coalescing latest live
  snapshots must preserve durable topology and clean-exit ordering.
- Transcript enrichment still creates replacement threads and uses an
  unbounded result channel. Generation/cancellation checks exist, but alone do
  not impose a hard concurrency/retention bound.
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
