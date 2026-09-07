# 2026-09-07: bounded IO ownership

Issue: [#163](https://github.com/TamedTornado/TornadoTTY/issues/163), under #160.
Baseline: `355f17d7`. This report accompanies the implementation commit.
Previous incident: [goal attention](tornadotty-linux-dogfood-2026-09-07-goal-attention.md).
Jason requested a batch for later QA: no installation or live-client restart.

## Confirmed defects and repairs

- Four connection readers could all wait for GUI replies, starving unrelated
  clients for seconds. A real-socket regression held four requests and failed
  to admit a quiet pane within 500ms on the baseline. Readers now hand replies
  to one bounded, nonblocking transport worker. No second product-state owner.
- Reply reservations cover admission through completed socket writes: 32 total,
  eight per pane. Saturation rejects **before** dispatching a non-idempotent
  command. An initial four-per-pane limit broke the existing eight-client burst
  regression; the limit was raised, not the test relaxed. Partial-write offsets
  preserve bytes; a client not reading does not block other pending replies.
- Per-read timeouts could be extended indefinitely by trickled bytes. The real
  50ms-per-byte regression took about 601ms on old code and failed its 500ms
  bound. A frame now has one absolute 250ms read deadline and the existing byte
  cap. This is an IO deadline, not an application-state delay.
- Enrichment workers were bounded per window, allowing extra windows or detached
  workers to multiply actual concurrency. A process-wide four-slot budget now
  follows the actual thread through exit, including panic. Dropping a window or
  thread handle cannot release a still-running worker's slot. Tests cover two
  production owners and detached threads, not merely a counter formatter.
- Transcript discovery limited selected candidates but not directory traversal.
  It now examines at most 4096 entries plus exhaustion detection, counting noise
  and errors before filtering. Exactly-at-limit discovery succeeds; one extra
  entry fails explicitly, without an arbitrary partial match. Enrichment does
  not retry a known exhausted scan. Existing bounded pressure logging reports
  discovery limits without paths or question text.

No wire protocol change, dependency, extra harness, or Ghostty change.

## Focused verification

Commands from repository root:

```sh
cargo test -p zentty-agent-ipc --lib --tests
cargo test -p zentty-core --test codex_transcript
cargo test -p zentty-linux --bin zentty-linux codex_enrichment
cargo clippy -p zentty-agent-ipc --lib --tests --no-deps -- -D warnings
ZENTTY_BUILD_SCOPE=product ZENTTY_BUILD_OUTPUT_DIR="$PWD/build/gh163" linux/scripts/build-local
ZENTTY_LINUX_BINARY="$PWD/build/gh163/bin/zentty-linux" ZENTTY_AGENT_IPC_SCENARIO=event-coalescing linux/tests/nested-x11 linux/tests/rust-agent-ipc
ZENTTY_LINUX_BINARY="$PWD/build/gh163/bin/zentty-linux" ZENTTY_AGENT_IPC_SCENARIO=codex-enrichment linux/tests/nested-x11 linux/tests/rust-agent-ipc
```

- PASS: 127 IPC tests, six transcript tests, 16 enrichment tests; scoped IPC
  strict Clippy. Socket tests include saturation before dispatch, shutdown with
  pending GUI requests, a filled real socket buffer, exact reply bytes, deadline
  equality, premature timeout prevention, and disconnected peers during writes.
- PASS: private-X11 real GTK/Ghostty/PTY/IPC traffic journey, including sibling
  input during traffic and bounded growth. PASS: real-file Codex enrichment and
  shutdown journey. These are **not** proof of sustained high-output PTY fairness.
- PASS: initial product build (41.79s); dependency-age audit: 91 packages,
  zero exceptions. The clean committed version is staged through the same command.
- FAIL, outside changed code: GTK strict Clippy still reports six findings in
  application.rs, agent_events.rs, pane_runtime.rs, application_shell.rs (two),
  and window_chrome.rs. A dependency-inclusive lint attempt also exposed existing
  core findings in attention_inbox.rs and workspace_recipe.rs. No blanket allows
  or unrelated cleanup were added. Scoped core lint completed successfully.
- A broader IPC run caught a stale launcher assertion left by `355f17d7`:
  the expected Codex notification list omitted `agent-turn-complete`. Updated
  that assertion to the already-implemented #143 policy; no new policy change.

Targeted mutations used `linux/tests/mutate-rust` with gitignore/copy-target
policy and resource isolation, four workers per invocation:

```sh
linux/tests/mutate-rust -p zentty-agent-ipc -f crates/zentty-agent-ipc/src/replies.rs --re 'reserve|advance' -j4 --cargo-test-arg=--lib --cargo-test-arg=replies --timeout 15
linux/tests/mutate-rust -p zentty-linux -f crates/zentty-linux/src/codex_enrichment/worker_budget.rs -j4 --cargo-test-arg=--bin=zentty-linux --cargo-test-arg=codex_enrichment --timeout 15
```

Initial reply run: 17 caught, four missed, two unviable. Misses exposed absent
assertions for premature/exact-boundary timeouts and retaining disconnected
writers. Added those behavioral checks, passing explicit monotonic time into the
existing reply advancement function. Final: **21 caught, zero missed, two
unviable** (51s). Worker budget: **seven caught, zero missed, one unviable** (2m).
This is scoped mutation evidence, not a claim about the whole unit suite.

No full qualification/matrix run. User-level QA remains pending for the batch.

## Remaining #163 acceptance gaps

Keep #163 open. GTK-thread proc/filesystem/command calls still need bounded
background ownership; count-limited callback batches are not wall-clock
preemption. Sustained high-output PTY ordering, latency, and resource behavior
need direct verification. Other background owners and diagnostic retention need
review. The worker cap cannot interrupt a blocked kernel filesystem syscall;
cross-window scheduling fairness is not proved merely by its global bound.
Process/cgroup containment remains the separate #162 concern.
