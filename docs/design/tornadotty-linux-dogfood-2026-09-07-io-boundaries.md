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

## Follow-up batch: project freshness and Open With off GTK

Baseline: `53c0fa3e`. Still staged-only for Jason's larger testing batch.

The call-site audit confirmed that project inspection moved Git into a worker
but left `/proc` cwd reads, canonicalization of observed/current paths, and icon
cache invalidation on GTK. Open With additionally probed SSH/process state and
canonicalized paths while refreshing controls, then validated and launched
external applications synchronously.

Repairs:

- Capture pane/worklane identity, topology generation, known cwd, foreground
  PID, and remote status from memory. Share this small value snapshot between
  project inspection and Open With; it owns no product state or event system.
- Resolve and validate project filesystem paths on the existing worker. Compare
  its captured identity to current in-memory state before accepting results.
  Deleted/retargeted directories are checked on the worker, not GTK.
- Move icon-cache invalidations into that worker. Invalidations arriving while
  it runs remain pending and reject its obsolete result. Closed panes no longer
  get reinserted into the forced-refresh set by late results.
- Open With availability uses only the app catalog and known pane state. This
  is a presentation hint, not permission to launch a stale path. Actual SSH,
  `/proc`, filesystem validation, desktop app discovery at launch, and external
  launch run on workers. Revalidate the originating pane snapshot before launch;
  never redirect a delayed command to the newly focused pane.
- One process-wide Open With request can be validating/launching at a time;
  additional requests are explicitly rejected, not queued without limit. Its
  RAII permit follows the actual worker through completion/panic/detachment and
  survives window/config replacement. No timeout claims for an uninterruptible
  filesystem syscall; if stuck, this action remains busy rather than multiplying
  workers or freezing GTK.

### Regression evidence

The existing `linux/tests/rust-project-icons` journey now gates one actual Git
invocation using a FIFO and then delegates to real Git; it does not fake Git
output. While Git waits, physical GTK input requests a newer icon refresh. The
older build **failed** because it subsequently published the obsolete cached
miss. The repair **passed**, keeping the newer request authoritative. The same
journey still checks real icon containment/decoding, pane operations, and normal
and reduced-motion behavior. Its gated writer is cleaned up on failure.

The unchanged `linux/tests/rust-open-with` journey also **passed** under private
X11: real desktop entry/custom executable/XDG terminal launches received exact
canonical path arguments; a real foreground SSH client was rejected. No test
requirement was relaxed to accommodate asynchronous execution.

Focused Rust checks: eight project-runtime, four Open With, and one pane-context
test passed. The GTK strict Clippy result remains the same six pre-existing
findings; no new findings or blanket allowances. No full qualification run.

Commands (with the staged `ZENTTY_LINUX_BINARY` override as above):

```sh
cargo test -p zentty-linux --bin zentty-linux application_shell::project_context_runtime
cargo test -p zentty-linux --bin zentty-linux application_shell::open_with_runtime
cargo test -p zentty-linux --bin zentty-linux application_shell::pane_context
linux/tests/nested-x11 linux/tests/rust-project-icons
linux/tests/nested-x11 linux/tests/rust-open-with
```

Targeted staleness mutations: seven caught, zero missed. Initial Open With
mutation pass: ten caught, four missed, one unviable. The four misses were the
catalog-plus-context availability decision, because the initial tests exercised
only its inner context rule. The decision now takes the real catalog and value
snapshot directly rather than an entire GTK shell, with additional assertions
for empty catalogs, absent context, and PID-only fallback. The same mutation
scope is rerun; results are recorded with the issue update.

### Audit corrections and unfinished scope

Server termination's `/bin/kill` call is already inside `gio::spawn_blocking`;
listing that call alone as a GTK violation was incorrect. The grid failure-file
read is a test-only opt-in path, not an ordinary user operation.

Still to address under #163: task-runner manifest discovery in palette opening
and revalidation before launch; close-confirmation `/proc` inspection;
bookmark capture/storage; Open With catalog discovery on startup/settings
refresh; GIO file-monitor registration and other synchronous metadata/decoding
paths. Close-confirmation behavior is unchanged in this batch. Kernel-level
slow filesystem hangs were not reproduced here; the controlled slow Git and
stale-result failure were reproduced. Sustained PTY-output proof remains open.

Separate confirmed finding: `ApplicationShell::report_action_error` calls
`main_loop.quit()` for errors from pane/worklane/task actions. Tracked as
[#179](https://github.com/TamedTornado/TornadoTTY/issues/179), under #160: review
partial-operation rollback and recoverable UI before removing the fatal policy.
This is not evidence that it caused any previously reported production crash.
