# Linux testing-autonomy dogfood report — 2026-09-01

This report tracks the work initiated by the external critique of TornadoTTY's
Linux test architecture. It is an active report: a green focused check is not
an assertion that the full suite, release qualification, or autonomy gate has
passed.

## Baseline discoveries

- `zentty-linux` contains 365 inline Rust test functions, while
  `crates/zentty-linux/tests` contains one integration-test source file.
  `zentty-core/tests` contains 26 integration-test source files. The critique
  that the GTK-facing middle layer is thin is therefore valid.
- The pane-divider test checks formatter-produced names and labels directly.
  Those assertions can detect an accidental string change but do not establish
  divider topology or resize semantics. GH-147 owns its replacement with a
  semantic integration cohort.
- Focused mutation receipts exist and several repaired cohorts report zero
  missed mutants, but there is no current-source, scope-aware mutation policy
  whose result gates unattended maintenance. Mutation strength is therefore
  not established for the unit layer as a whole. GH-146 owns that gate.
- The repository contains 45 test/support files over 300 lines. GUI journeys
  use many literal product-log assertions. The receipt environment variable
  `ZENTTY_TEST_RECEIPTS` is limited to the sidebar-management journey, but the
  broader dependence on human-oriented `zentty-linux:` log text is real.
  GH-148 owns a typed receipt contract; GH-149 owns the single Rust journey
  driver and incremental migration.
- The two ReleaseSafe Valgrind XFAIL cells were not missing explanations in the
  current matrix. Both already named tracking record
  `DOGFOOD-2026-08-02-RELEASESAFE-VALGRIND` and described the unsuppressed
  findings. The real defect was weaker: the validator accepted whitespace-only
  metadata, and BLOCKED did not require a tracking identifier.

## Planning record

- GH-145 is the testing-autonomy epic.
- GH-150: deferred-status explanation governance.
- GH-147: semantic GTK-adjacent middle layer, beginning with pane dividers.
- GH-146: current-source mutation autonomy gate.
- GH-148: typed product receipt boundary.
- GH-149: Rust GUI journey driver and staged Bash migration.

The implementation order is deliberate: first make the matrix honest, then
construct one semantic cohort, use that cohort to prove the mutation gate, and
only then replace the journey protocol and driver. This avoids blessing weak
tests with a mutation badge or creating a second journey system without a
migration path.

## GH-150 repair

### Failure

Matrix validation used string length to validate `tracking`, `defect`, and
`prerequisite`. A value containing only spaces or tabs therefore passed.
XFAIL required tracking, but BLOCKED did not.

### Repair

- BLOCKED and XFAIL now require a non-whitespace tracking ID.
- Each also requires a non-whitespace defect or prerequisite.
- FAIL and NOT_IMPLEMENTED defects receive the same whitespace-safe check.
- The JSON schema now rejects whitespace-only policy strings.
- A focused `qualification-gap` helper is the single formatter for both cell
  result metadata and the concise human report. Deferred evidence is preserved
  under `qualification_gap` in the machine result and printed as a `Deferred:`
  line in the human report.

### Regression evidence

The focused runner tests now reject missing, empty, and whitespace-only
tracking and reasons for each real ReleaseSafe Valgrind XFAIL cell. A synthetic
BLOCKED cell proves both rejection and the valid prerequisite form. The helper
test checks exact JSON and human renderings without running the product matrix.

Commands run after the repair:

```text
linux/tests/lib/qualification-gap-test
  Qualification gap focused tests passed

linux/tests/qualification-matrix-test
  Qualification matrix focused tests passed

linux/tests/qualification-matrix --validate-only
  Qualification matrix schema and coverage passed
```

No qualification cell was executed and no full-qualification claim is made.
The ReleaseSafe Valgrind cells remain XFAIL; no suppression was added or
broadened.

## Remaining uncertainty

- No current mutation kill-rate yet gates autonomous work.
- The pane-divider assertions have not yet been replaced by semantic tests.
- Product receipts remain fragmented across log strings.
- Large Bash journeys remain in production until the Rust driver proves real
  equivalence and individual migrations remove their predecessors.

## GH-147 pane-divider semantic cohort

### Discovery

The original `pane_dividers.rs` mixed four responsibilities: topology identity,
axis selection, physical-event interpretation, and GTK widget construction.
Its only local test asserted the strings produced by `name()` and `label()`.
The actual bounded resize lived behind callbacks and was not represented at the
public `zentty-linux` crate boundary.

### Repair

- Added a display-independent `pane_divider_model` public module. It owns the
  typed column/pane boundary, axis, axis-specific pointer coordinate, physical
  key eligibility and sign, nonzero resize request, callback payload, and
  bounded visible-margin calculation.
- Kept GTK as an adapter: GDK keys are translated once, and both drag and key
  paths now submit the same typed request to the existing workspace callback.
  No second resize or topology state was added.
- Removed the formatter-only inline test and added
  `tests/pane_divider_semantics.rs`, which crosses the crate boundary without a
  display. Six tests cover horizontal/vertical exclusivity, both keyboard
  directions, delimiter-like and deliberately confusable IDs, signed callback
  payloads, callback suppression at zero, rounding, and both integer bounds.

### Failures and evidence

- The first mutation invocation stopped at its unmutated baseline because the
  safe `gitignore=true` policy correctly omitted the ignored multi-gigabyte
  `build/` tree, including Ghostty's native library. The rerun supplied the
  existing library through an absolute `GHOSTTY_LIB_DIR`; no ignored tree was
  copied and no isolation rule was weakened.
- Final scoped mutation result: **24 generated, 21 caught, 3 compiler-unviable,
  0 missed, 0 timed out**. The unviable replacements required `Default` for
  semantic enums/types that intentionally have no meaningless default. All
  viable mutations—including sign deletion, callback replacement, zero-bound
  inversion, coordinate replacement, and bound arithmetic replacement—were
  killed.
- `cargo test -p zentty-linux --test pane_divider_semantics`: 6 passed.
- `cargo check -p zentty-linux --bin zentty-linux`: passed, proving the GTK
  adapter consumes the new model.
- Focused `cargo clippy --no-deps ... -D warnings`: passed. The first clippy
  invocation without `--no-deps` exposed two pre-existing `zentty-core`
  pedantic failures (`assigning_clones` and `too_many_lines`); they were not
  changed or hidden by this slice.

### Mutation operators cargo-mutants could not synthesize

The generated axis replacement was compiler-unviable because `DividerAxis`
intentionally has no arbitrary `Default`; cargo-mutants also generated no
same-typed topology-field swap. Each operator was therefore applied manually
and reverted with a reviewable patch:

- Swapping horizontal and vertical axis results failed three semantic tests,
  including both axis-specific cohorts.
- Swapping the pane request's `column_id` and `after_pane_id` failed
  `callback_receives_typed_unswapped_target_and_signed_delta`.
- After reverting both mutants, all six tests passed again and the model had no
  source diff from the committed implementation.

This is evidence that the assertions kill those named operators; it does not
misreport compiler-unviable generated mutants as caught.

### Inline-test classification audit

All 364 remaining inline tests are now classified by primary intent in
`zentty-linux-inline-test-audit-2026-09-01.md`: 331 behavioral, 29
contract/snapshot, zero remaining confirmed formatter-mirroring tests, and four
GTK widget-smoke tests. The zero is not a kill-rate claim: exact presentation
contracts remain legitimate snapshots, and GH-146 must still mutate them.

A focused audit check extracts the real inline test identities from Rust
source, validates every explicitly named contract/smoke test, rejects duplicate
classification, and pins the category arithmetic. It passed directly and as
part of `qualification-matrix-test`. The audit prioritizes source-text parity
snapshots, the six largest inline cohorts, widget-smoke migration boundaries,
and presentation-contract mutation.

## Public tracker reconciliation

The open tracker was reviewed against issue acceptance criteria, commits, and
recorded real-system receipts rather than age or unchecked boxes.

- Rebrand epic GH-144 closed after current `rebrand-identity-policy-test` and
  `rebrand-identity-policy` passed. The latter governs 12 identity boundaries.
  The withdrawn release remains absent; release-tag qualification is separate.
- Visible-pane notification GH-140 closed on its existing real two-window GTK
  and private-D-Bus evidence. It no longer waits solely for subjective human
  review; later dogfood may open a concrete defect.
- AT-SPI terminal-role GH-141 remains open because its required external,
  exact-PID installed-product receipt has not succeeded. An in-process GTK
  assertion was not promoted to equivalent evidence.
- False paired Codex notification GH-143 previously had an empty body. It now
  records the observed timestamp/pairing, separates agent-state correctness
  from GH-140 delivery policy, and has deterministic acceptance criteria that
  reject timer/debounce masking.
- Enhancement ledger GH-137 remains open for the fresh upstream comparison and
  explicitly authorized communication batch; no upstream contact occurred.
- Stable-Cargo migration GH-96 remains deferred. On 2026-09-01 the official
  Cargo tracking issue remained open and the Cargo Book still classified
  `min-publish-age` as unstable; the reviewed nightly enforcement therefore
  remains necessary.
- Testing epic GH-145 now explicitly records GH-147 and GH-150 closed, GH-146
  active, and GH-148/GH-149 genuinely pending.

The resulting open list contains three explicitly deferred enhancements, the
stable-Cargo follow-up, the enhancement register, two real product bugs, and
the testing epic with its three unfinished children. No issue was closed by
weakening its acceptance criteria.

## 2026-09-04 — qualification schema authority (GH-167)

### Discovery

- The checked-in Draft 2020-12 matrix schema described itself as authoritative,
  but rejected the live matrix. It omitted the entire `execution_graph` and
  the cell-level dependency, resource, scheduling, always-run, prerequisite-
  bypass, and display-blocking fields that the Bash runner validated itself.
- Consequently, changing an enum or a structural field required maintaining
  parallel validators, while direct schema validation could not protect the
  repository artifact consumed by the runner.
- Enabling the schema in the real runner exposed a contradictory focused-test
  fixture: it converted a PASS cell to BLOCKED but retained the PASS-only
  `environment_profile`. The fixture had passed only because the authoritative
  schema was never executed.
- The same focused test run honestly stopped on a separate baseline drift:
  five inline tests had landed after the classification audit. Four are
  behavioral tests and one is an ignored GTK widget-smoke test; none is being
  represented as mutation-proven merely because its category is recorded.

### Repair

- Extended the existing JSON Schema rather than introducing Zod, TypeScript,
  code generation, or another validation layer. It now closes the live root,
  execution-graph, profile-prerequisite, resource, dependency, and scheduler
  shapes and rejects unknown properties.
- Made `linux/tests/qualification-matrix` invoke that schema before semantic
  policy validation. Removed the duplicate Bash shape, primitive-type, enum,
  ID-pattern, order, scheduler-priority, and collection-uniqueness checks now
  owned by JSON Schema.
- Retained semantic checks for referenced-cell/resource membership, cycles,
  resource ownership, environment compatibility, deferred-status governance,
  required capability coverage, and qualification claims.
- Added focused negative fixtures for a missing execution graph, a missing
  required cell field, a negative scheduler priority, duplicate dependencies,
  an unknown scheduler property, an invalid status, and a mistyped display
  blocker. The fixtures assert rejection through the schema boundary without
  depending on validator-version-specific prose.
- Reconciled the inline-test audit to 369 tests: 335 behavioral, 29
  contract/snapshot, zero formatter-mirroring, and five widget smoke.

### Evidence and limitations

- Direct Draft 2020-12 validation of the live matrix: passed.
- `linux/tests/qualification-matrix --validate-only`: passed, including all
  retained semantic and coverage policy.
- `linux/tests/qualification-matrix-test`: passed.
- `linux/tests/lib/inline-test-audit-test`: passed.
- ShellCheck at warning severity and `git diff --check`: passed.
- This slice does not claim stronger unit assertions or a mutation kill rate.
  Those remain GH-146. Source/input identity in full-run receipts remains
  GH-168, and aggregate/retry simplification remains GH-169.

## 2026-09-04 — source-bound qualification receipts (GH-168)

### Discovery

- The full matrix summary recorded result and evidence-file hashes but no Git
  revision, dirty state, matrix identity, runner identity, lockfile identities,
  or actual tool versions. Copying the receipt away from its working tree lost
  the answer to “what was qualified?”
- The PR-subset summary carried flat source and matrix hashes, but the full
  summary did not. The two paths therefore represented the same concepts with
  different strength.
- Matrix results named a cell and outcome but did not bind the result to the
  exact command text. Agent and Docker cells also did not identify the
  executable versions or pinned image digest they exercised.

### Repair

- Added one shared `qualification-provenance` contract consumed by the full
  matrix and PR-subset runners. It produces one aggregate Git-tree identity,
  not a per-source-file manifest. The identity changes independently for HEAD,
  index, tracked-worktree, or non-ignored untracked-file changes and reports
  whether the tree is dirty.
- The envelope records SHA-256 identities for the matrix, authoritative schema,
  active runner, Cargo lock, Rust toolchain file, Ghostty lock, controlled CI
  environment manifest, and pnpm agent-tools lock. It also records the actual
  rustc, Cargo, Zig, Node, and pnpm version output plus executable identity.
- Ordinary dirty local trees remain allowed and are represented honestly.
  Ignored build products are excluded using Git's own ignore contract.
- Every matrix result now carries the exact command and its SHA-256 identity.
  Only agent lifecycle cells receive Codex/Gemini path, version, and executable
  hashes; only the Docker dev-server cell receives its pinned image reference
  and digest. Other cells receive no placeholder metadata.
- Both runners recapture and compare the complete provenance before summary
  creation and again immediately before atomic publication. The full runner's
  existing evidence lease/check now also covers agent executables. The PR
  policy hash is independently held stable across its run.
- The PR subset embeds the same provenance envelope and its legacy flat source
  and matrix fields are required to equal that envelope. Both generated
  summary formats are now schema version 2 because this is a machine-contract
  change.

### Focused negative evidence

- A temporary real Git repository proved clean verification and rejection of a
  wrong revision, wrong aggregate tree identity, inconsistent dirty state,
  wrong matrix hash, missing tool identity, and changed matrix, schema, runner,
  Cargo lock, Rust toolchain, Ghostty lock, CI manifest, or pnpm lock.
- Separate staged, unstaged, and untracked changes each produced a distinct
  aggregate tree identity and the correct dirty-state component.
- An external matrix file was changed without changing the Git tree and was
  still rejected, proving the explicit input identity is effective.
- A captured per-cell Gemini identity was rejected after its executable
  changed. Agent and container metadata was absent from an irrelevant cell and
  present with exact versions/digests on relevant fixtures.
- PR summary negatives reject a conflicting flat source revision, conflicting
  matrix hash, or missing provenance tool. Existing matrix negatives continue
  to reject false full-qualification claims.

### Executed checks and limits

- `linux/tests/lib/qualification-provenance-test`: passed.
- `linux/ci/run-pr-subset-test`: passed.
- `linux/tests/qualification-matrix-test`: passed.
- `linux/tests/qualification-matrix --validate-only`: passed.
- Warning-level ShellCheck and `git diff --check`: passed.
- No product matrix or aggregate local qualification was run. This slice proves
  the capture/verification/publication contracts with focused fixtures; it
  does not manufacture a new qualification result for code that was not run.

## 2026-09-04 — aggregate and retry simplification (GH-169)

### Before: actual execution graph

- `linux/tests/qualify-local` was the intended complete local entry point, but
  its expanded support list contained 45 invocations representing only 44
  unique paths. `qualification-failure-ledger-test` appeared in both the PR
  support manifest and the local literal list.
- `qualification-matrix-test`, itself a local support test, synchronously ran
  eight other focused support tests. Two of those,
  `qualification-capacity-test` and `valgrind-evidence-lock-test`, also appeared
  directly in the local list. The effective local support graph therefore made
  53 invocations for 50 unique paths and executed three paths twice.
- The PR subset declared 12 support tests and 20 selected product/dependency
  cells. One support entry was `run-pr-subset-test`, a self-test of the same
  aggregate runner currently executing the list. Its fixture did not recurse
  infinitely, but it nested another runner validation and obscured the graph.
- The authoritative product matrix contained 207 cells: 203 PASS, two XFAIL,
  and two NOT_IMPLEMENTED. Of those, 205 have executable commands. No matrix
  cell invoked an aggregate qualification command.
- A baseline focused `qualification-matrix-test` run took 14.49 seconds. The
  important defect was not that one timing alone, but that independently useful
  tests were hidden inside it and could not be scheduled or reported honestly
  by the local aggregate.

### Matrix-runner self-test classification

The retained negative fixtures protect externally meaningful correctness:

- schema shape, status vocabulary, required cells, and graph dependencies;
- invalid job/resource limits, unknown resources, and dependency cycles;
- unexpected skips and deferred-status tracking/reason governance;
- installed-package and agent executable/version closure;
- unique Valgrind evidence paths and suppression-review dependencies;
- stale scheduler scratch safety and evidence ownership; and
- rejection of a false full-Linux qualification claim.

The following checks were implementation-detail assertions and were removed:

- source greps for the exact scheduler sort expression, duration field,
  classifier/helper function names, worker counters, trap text, subshell shape,
  JSON-lines append expression, and final `jq` command;
- source-line ordering that required support work to overlap matrix work;
- source greps for Ghostty timing-field names and display-worker internals; and
- a negative file-existence check for an already retired package runner.

The eight nested focused tests were not deleted. They are now explicit,
independently runnable local support entries. This retains their real negative
coverage while removing the hidden aggregate-within-aggregate call graph.

### Retry inventory and decision

- `qualification-matrix`, `qualify-local`, and `run-pr-subset` invoke every
  selected cell/support command once. The shared bounded-batch focused test now
  asserts that successful and failing callbacks each start exactly once, as
  well as asserting that a failure survives the batch.
- Most shell loops repeatedly inspect a receipt, process, window, mount, or
  file-size transition. They are bounded asynchronous readiness polling and do
  not execute the failed test command again.
- The visual helper samples frames until two adjacent captures are stable. Its
  focused negative still rejects permanently alternating frames; it does not
  rerun a failed visual assertion.
- Bookmark and command-palette setup can repeat a harmless physical key action
  only while the required focus/mapping receipt is absent. Bookmark modal
  activation explicitly stops being repeatable once the dialog is mapped, so a
  repeated Return cannot submit an empty dialog. The orchestration contract's
  description was corrected to say that it guards this boundary rather than
  the previous inverted wording.
- Diagnostic-upload and pane-recovery “Retry” actions are user-visible product
  behavior being tested after a deliberate first failure. They are not harness
  recovery. A rollback test separately asserts that the product does not retry
  a failed agent update automatically.
- `apt-get` acquisition retries recover downloads before tests, and the nested
  X11 teardown retry removes only the wrapper-owned transient FUSE mount/root.
  Neither changes a test outcome.

No broad test retry capable of laundering an intermittent failed invocation
into PASS was found. The policy now forbids adding one without a tracked defect
and requires the original failure to remain in the result.

### Repair and after graph

- The local support graph is flat: 50 declared invocations, 50 unique paths,
  zero nested focused-test calls from `qualification-matrix-test`, and a runtime
  duplicate-list rejection before any support work begins.
- The PR subset is 11 support tests plus the same 20 selected cells. Its runner
  self-test remains independently executable and is scheduled once by
  `qualify-local`, not by the runner under test.
- The product matrix remains unchanged at 207 cells/205 executable commands.
  No product journey, assertion, real compositor, PTY, GTK process, or Ghostty
  boundary was removed or replaced by a mock.
- `qualification-matrix-test` decreased from 465 to 378 lines and took 14.20
  seconds after the repair. The 0.29-second difference is measurement noise,
  not a claimed speedup. The gain is an honest flat graph and independent
  scheduling/reporting; product coverage and cell count were deliberately held
  constant.
- The staged-shell journey had an unrelated inline fake Codex program that the
  existing orchestration contract correctly rejected. It now symlinks the one
  reviewed real controlled-agent actor used by the other agent journeys. Bash,
  zsh, fish, and Nushell staged-product cases all passed with that actor.

### Focused evidence and remaining limits

- Passed: matrix schema/semantic validation, `qualification-matrix-test`,
  `qualification-boundary-contract-test`, outcome/capacity/gap/provenance,
  bounded-batch, inline-audit, qualified-package-source, Valgrind evidence-lock,
  PR-policy validation and negatives, PR-runner self-test, orchestration
  contract, and all four staged-shell cases.
- Warning-level ShellCheck and `git diff --check` passed.
- No complete local/product matrix run was performed for this harness-only
  audit, and this record makes no new local, release, or full-Linux
  qualification claim.
- Real GUI journeys still contain many bounded receipt polls because GTK,
  compositors, PTYs, portals, and child processes are asynchronous. This audit
  classifies rather than deletes those waits. Any future intermittent failure
  must become a concrete defect; a blanket rerun is not an acceptable repair.

## 2026-09-04 — current-source mutation autonomy gate (GH-146)

### Existing state and policy boundary

- The repository already had a safe `linux/tests/mutate-rust` wrapper and many
  historical ad hoc `mutants.out` directories. Those receipts covered different
  hand-selected source files and did not prove the current protected scope.
- `.cargo/mutants.toml` already contained `gitignore = true` and
  `copy_target = false`, but a caller could override either on the command line
  or disable/replace the configuration. The wrapper now rejects `--no-config`,
  every alternate `--config`, `--gitignore=false`, `--copy-target=true`, and
  unsafe split-argument forms before entering a mutation scope. It also rejects
  VCS copying, in-place mutation, and leaked scratch directories. Its existing
  systemd/prlimit memory isolation remains the only execution wrapper.
- One machine policy now owns the mutation contract. It names the protected
  module, focused integration test, included mutation genres, explicit empty
  exclusion list, minimum generated count, cargo-mutants 27.1.0, four-worker
  and jobserver bounds, per-command deadline, receipt lifetime, and exact
  compiler-unviable dispositions.
- The first protected cohort is the display-independent pane-divider semantics
  extracted under GH-147. Its owning command is the real Rust integration test
  `cargo test -p zentty-linux --locked --test pane_divider_semantics`, which
  exercises the same model called by the GTK divider adapter.

### Gate and negative contracts

- `linux/tests/mutation-autonomy run` lists and executes the policy-selected
  inventory through the existing wrapper. Cargo-mutants' own unmutated baseline
  must pass; zero generated or viable mutants, list/execution drift, undeclared genres,
  nonzero tool exit, survivors, and timeouts all prevent PASS.
- The receipt records caught, missed, timeout, compiler-unviable, excluded, and
  total counts separately. Every mutant carries its name, file, function,
  genre, outcome, focused owning command, and hashed raw log/diff. The receipt
  also binds the complete raw inventory/outcome documents.
- Verification recaptures the aggregate Git revision/tree identity, policy
  identity, cargo-mutants executable/version, every raw evidence hash, and
  freshness. The checked-in policy requires a clean tree and a receipt no more
  than seven days old.
- Focused negative fixtures reject missing policy scope, zero mutants, a missed
  mutant, timeout, failed baseline, stale tree, stale receipt, stale policy,
  stale tool, wrong owning command, unreviewed unviable, incomplete totals, and
  unsafe copy/worker policy. The resource-isolation test separately proves the
  wrapper rejects unsafe CLI overrides and retains its cgroup/prlimit ceiling.
- `qualify-local` schedules policy/negative/isolation checks and the default
  receipt verifier. It does not execute the mutation campaign. Thus a release
  candidate or unattended protected change must deliberately generate a fresh
  receipt, while ordinary focused bug fixes and advisory PR CI do not pay the
  mutation cost.

### First real campaign and discoveries

- A diagnostic run against the exact dirty implementation tree independently
  listed and tested 24 pane-divider mutants in about two minutes. Cargo-mutants'
  baseline passed in 45 seconds. Results were **21 caught, zero missed, zero
  timeout, and three compiler-unviable**.
- The policy draft anticipated only two compiler-unviable replacements. The
  real run exposed a third: `keyboard_request -> Some(Default::default())`.
  Source review confirmed all three fail because `DividerAxis` or
  `DividerResize` deliberately has no arbitrary `Default`; fabricating one
  would erase the typed target/axis/delta invariant. Each exact generated
  mutant now has its own GH-146 disposition. They are counted as UNVIABLE, not
  as kills.
- The same diagnostic run exposed a receipt-assembler defect after mutation
  execution: the already captured cargo-mutants exit code was omitted from one
  jq argument list. The gate failed closed and published no passing receipt.
  The argument and the receipt field are now present, and a nonzero tool exit
  independently prevents the PASS claim.

The final current-source receipt is intentionally generated after this
implementation is committed: committing a receipt or a post-run hash would
change the very tree identity it attests. The ignored machine receipt and raw
evidence remain local release artifacts; GH-146 records their final hash and
counts after the clean-tree run. No full product qualification is claimed by
the diagnostic mutation campaign.

## 2026-09-04 — typed product observability boundary (GH-148)

### Discovery and design decision

- Thirty-three Linux test files still consumed `zentty-linux:` human diagnostic
  prose. Representative journeys asserted terminal readiness, exact pane-column
  formatting, split completion, settings selection, settings focus, and test
  notification completion through those strings. A diagnostic rename was
  therefore an undeclared test-API break.
- Existing `*RECEIPT*` environment variables were not one system. Most are
  controlled child processes or boundary probes writing what they observed;
  others belong to packaging, supply-chain, mutation, display, or visual
  evidence. Treating them all as product instrumentation would erase useful
  ownership boundaries.
- `ZENTTY_TEST_RECEIPTS` in the sidebar journey was an ambiguously named
  directory passed only to a controlled child shell. It was renamed to
  `TORNADOTTY_CONTROLLED_CHILD_RECEIPT_DIR`; it is not part of the new product
  contract.
- The repair uses one small Rust authority, not another Bash/JSON-schema pair.
  `ReceiptEvent` is the schema consumed by both the product writer and staged
  Rust journey driver. A machine policy inventories ownership and migration,
  but does not redefine event fields.

### Safety and negative evidence

- Activation is explicit and absent in normal launches. A receipt target must
  be a new absolute normalized file in a canonical mode-0700 directory owned by
  the current UID; symlinks, permissive directories, existing targets, and path
  escapes fail closed. Created evidence is mode 0600.
- The format is bounded to 8 MiB, 8,192 records, 8 KiB per record, and 96-byte
  safe identifiers. The finite schema has no arbitrary user-content, terminal,
  command, path, token, or error-detail field.
- Parser tests reject unknown versions/events/fields, malformed and truncated
  records, oversized records, duplicate terminal events, child exit before
  readiness, an invalid first event, unsafe paths, and secret-shaped IDs.
- Review found that the first writer draft enforced the per-record limit while
  only the parser enforced aggregate bytes and record count. That could create
  a stream the authority rejected itself. The writer now refuses an append
  before either the 8 MiB or 8,192-record boundary is crossed; focused tests
  fill each bound, assert the specific aggregate failure, and parse the bytes
  that were safely retained.
- The driver originally recognized a not-yet-created file and partial record by
  matching English error strings. Review caught this before commit. It now uses
  typed `NotCreated`/`PartialRecord` states; every malformed, unsafe, or
  oversized stream fails immediately rather than being polled until timeout.
- The source contract compares the policy to every receipt-named environment
  token and every test containing `zentty-linux:`. Its negative fixtures reject
  undeclared channels/consumers, unknown owners, duplicates, untracked partial
  migrations, and false completion.

### Representative migration

- `desktop-window-identity` now obtains process and pane readiness from typed
  lifecycle events while retaining Wayland/X11 protocol inspection for the
  native application ID.
- `rust-workspace-pane-settings` uses typed action completion for split-right
  and typed pane-layout geometry for exact columns. The layout assertion no
  longer mirrors a diagnostic formatter.
- `rust-session-restore` uses a fresh stream for every launched product process
  and typed per-pane terminal-ready events for the representative restored
  agent panes. Duplicate readiness invalidates the stream.
- `rust-notifications-settings` uses typed settings selection, settings-window
  focus, and send-test completion events.
- Literal assertions for those migrated facts were removed. Remaining human-log
  dependencies in partially migrated files are visible in the policy and owned
  by GH-149, rather than being misrepresented as complete.

### Environment discoveries and current limits

- A first nested-X11 smoke never reached TornadoTTY because the host's
  `/tmp/.X11-unix` is owned by `nobody:nogroup`; Xvfb refused to create its
  socket. This is an environmental blocker, not a product pass or failure, and
  the directory was not changed.
- A sandboxed nested-Wayland launch was denied a required bind operation. The
  same staged binary under an approved real Weston environment emitted valid
  `process_started`, `terminal_ready(pane-1)`, and exact one-column pane-layout
  events. The first smoke then deliberately sent SIGTERM; its `set -e` wrapper
  stopped at status 143 before final validation, so that run proves live events
  but not clean lifecycle completion.
- Final evidence must use the real pane/PTY exit path and require
  `process_stopped`; a test-only shutdown endpoint will not be added merely to
  make the receipt green.
- The corrected focused smoke reused the real `rust-product-smoke` child-exit
  path under private headless Weston. The staged ReleaseSafe product exited 0
  and the shared driver observed, in order, process start, typed topology and
  pane layout, `terminal_ready(pane-1)`, 1000x700 window geometry, the real
  `child_exited(pane-1)`, and `process_stopped`. Final `--complete` validation
  accepted all nine records. This supersedes the earlier SIGTERM wrapper error;
  no test-only shutdown action or signal was involved.
- This focused work does not claim a complete matrix, release qualification, or
  full Linux qualification. The remaining legacy migrations are explicitly
  tracked under GH-149.

### Focused journey failure and product repair

- The first migrated notification/settings run reached typed settings
  selection and activation, then failed its real visible-pane switch. The log
  showed the Alt mnemonic focused the switch, followed by a stale map-time idle
  callback moving focus back to the settings search field; Space was inserted
  as a search query instead of toggling the preference. This was a real product
  focus race exposed by the journey, not a receipt-parser failure.
- `shortcut_settings` now reasserts its deferred initial focus only when the
  window has no current focus or the intended initial widget still owns focus.
  It cannot overwrite focus that the user moved in the meantime.
- After rebuilding, the same single notification/settings journey passed under
  controlled Wayland input and a private D-Bus session. It crossed the real GTK
  switch, freedesktop notification daemon, CLI notification path, native audio
  chooser, `aplay`/ALSA preview, persistent custom asset/restart, removal, and
  missing-service error path. No failed invocation was discarded or relabeled;
  the failure above remains the discovery receipt and the post-repair run is a
  separate pass.

### Final focused QA

- `cargo test -p tornadotty-test-receipts`: 11 tests passed (eight contract,
  three external-driver), zero failed/ignored. Strict all-target Clippy passed.
- The product binary compiled through `cargo check`; the ReleaseSafe staged
  product and driver rebuilt successfully with the seven-day Cargo publication
  age audit at 91 packages and zero exceptions.
- The compact real-product lifecycle passed under private headless Weston with
  nine records and complete validation. Wayland and X11 desktop identity both
  passed against the staged product and native protocol state.
- The full workspace/pane-settings representative passed twice during the
  slice, including after completion was moved behind render/focus projection.
  Both its original and restart processes closed through Ctrl+Q and produced
  complete streams.
- The focused eager session-restore journey passed: two restored agent panes
  emitted typed readiness, eager ordinary surfaces stayed real, focus/topology
  were preserved, both normal closes were complete, and the second launch
  exposed its deliberately recoverable agent failure.
- The first notification/settings run failed on the product focus race recorded
  above. After repair, its separate post-fix invocation passed all real GTK,
  D-Bus, audio, persistence, and unavailable-service checks.
- The 360-test GTK binary invocation produced 354 passes, four already-declared
  display-gated ignores, and two environment errors: one test initialized GTK
  without a display, and the sandbox denied the other test's real loopback
  listener. Each exact failed test was then run once in its required boundary:
  the GTK test passed in private Xvfb and the kernel-listener/proc-correlation
  test passed outside the network-restricted sandbox. The other 354 tests were
  not rerun.
- Running Cargo *inside* the isolated Xvfb wrapper replaced HOME and caused an
  unnecessary fresh locked-registry/toolchain build. It passed, but GH-149 must
  compile its eventual Rust GUI driver outside the controlled desktop and run
  only the built artifact inside it. Repeating isolated Cargo builds is not an
  acceptable journey architecture.
- Strict Clippy for the new authority is clean. Product-wide pedantic Clippy is
  not clean on the current base: it reports unrelated existing findings in
  `attention_inbox`, `workspace_recipe`, `application`, `agent_events`,
  `pane_runtime`, `application_shell`, and `window_chrome`. The sole new adapter
  finding (needless ownership of `ReceiptError`) was repaired. This slice does
  not falsely claim product-wide Clippy cleanliness or expand into those
  refactors.
- Product-observability policy negatives, PR-subset policy/negative validation,
  the existing orchestration contract, warning-level ShellCheck, Bash parsing,
  and `git diff --check` passed. No aggregate matrix, `qualify-local`, release
  qualification, or full-Linux qualification was run or claimed.

## 2026-09-04 — single Rust GUI journey driver (GH-149, in progress)

### Process, input, and evidence ownership

- A single `tornadotty-journey-driver` now owns staged product process groups,
  phase deadlines, exclusive display-resource leases, stop requests, TERM/KILL
  escalation, descendant detection, input target verification, and durable
  machine evidence. This extends the GH-148 receipt executable; it does not add
  a second harness.
- The first stop implementation let a short-lived controller signal the
  product. Review rejected that as false ownership. The controller now writes
  a bounded owner-only stop request and the original long-lived supervisor is
  the only process that signals its product group and journals the action.
- Session state binds every PID to `/proc` start ticks. A reused or edited PID
  is rejected as stale. X11 input additionally proves the target window's PID;
  native Wayland input requires the controlled compositor attestation, and
  outer-X11 input proves the attested compositor owns the target.
- Journal validation was initially write-only, which would have made JSON
  strings in tests the de facto contract. A typed reader now rejects partial
  records, unknown variants/fields, wrong versions, sequence gaps, duplicate
  terminal lifecycle events, and invalid order. Every normal Bash-adapter
  teardown invokes that validator.
- The first validator accepted only a product-start/product-exit lifecycle, so
  a correctly rejected resource conflict produced evidence it could not
  validate. Pre-spawn resource and spawn failures now end with a typed
  `session_completed` record and validate as coherent failed journeys without
  being mistaken for successful product runs.

### Real failures found during migration

- The first converted X11 launch left a `dbus-launch` descendant in the product
  group. The supervisor reported and reaped it instead of calling the run
  clean. The journey now establishes a private D-Bus session explicitly; this
  fixed the environment rather than suppressing the leak.
- The workspace/settings restart path retained the first product's X11 window
  ID. Reacquiring and verifying the restarted product window removed that stale
  input target.
- Native Wayland under a private D-Bus session can autoactivate noisy host XDG
  portal backends. These messages are environmental diagnostics; they are not
  relabeled as product failures or silently counted as product evidence.

### First complete scenario conversion

- `desktop-window-identity` is now an environment-only Bash wrapper. Its named
  Rust scenario owns launch, typed readiness, real X11 `WM_CLASS` or Wayland
  protocol app-ID observation, real PTY-controlled shutdown, receipt and
  journal validation, diagnostics, and cleanup.
- The converted scenario passed once in private Xvfb and once in private
  headless Weston against the staged ReleaseSafe product. Both processes exited
  zero and both evidence streams validated. This did not run or claim aggregate
  qualification.
- Driver tests currently pass **18/18**: eight typed receipt-contract tests,
  three receipt-driver tests, and seven session/input/journal tests. The focused
  session group itself completes in about **0.11 seconds**. Strict all-target
  Clippy passes after correcting a `filter().next_back()` finding before the
  real runs.

### Remaining honest scope

- Divider/layout, session restore, and notification/settings now use the Rust
  supervisor, typed receipts, verified input, and resource declarations, and
  focused real-system runs have passed during the migration. Their scenario
  decisions and some human-log assertions still live in Bash, so the policy
  keeps them `PARTIAL` and GH-149 remains open.
- No assertion has been removed merely to make a port smaller. Each remaining
  literal must either become a typed product expectation, remain native
  boundary evidence, or be explicitly shown redundant before its Bash scenario
  is retired.
