# Zentty Linux dogfood — privacy and local diagnostics

Date: 2026-08-21

Issue: GH-76

Plan: `docs/design/linux-privacy-diagnostics-plan.md`

## Initial state and source boundary

The macOS source owns an opt-in Sentry client and a Settings privacy row. Linux
had copied the configuration shape but defaulted `error_reporting.enabled` to
true while rendering the row unavailable. It had no crash capture, report
store, payload review, retention policy, or transport. This was misleading even
though no Linux transport existed.

The Linux outcome is intentionally stricter than the source implementation:
local capture defaults off, no production endpoint is present by default, and
no report can be sent until its complete redacted JSON has been opened and a
second safe-default confirmation has been accepted. Terminal output, commands,
agent prompts/results, complete environments, workspaces, and IPC credentials
are not collected.

## Construction record

### One typed authority and one store

- `zentty-core::support_diagnostics` is the only payload and lifecycle model.
  It owns local, pending-review, sent, failed, and cleared states and rejects a
  transition directly from local to sent.
- Context is constructed from a small allowlist rather than collecting broad
  process state and attempting to scrub it later. Bounds apply to fields,
  values, report detail, on-disk report size, report count, and age.
- `zentty-linux::diagnostic_store` is the only persistence authority. It uses
  atomic replacement in the XDG state directory with directory mode 0700 and
  file mode 0600. Malformed, oversized, stale, and interrupted files cannot
  prevent product startup and are removed by bounded pruning.
- Store review found that ordinary metadata lookup would follow a same-user
  symlink. The store now rejects a symlinked root and symlinked report files;
  focused tests also publish five reports concurrently and prove every visible
  JSON document is complete.
- Production submission is absent unless a fixed reviewed HTTPS origin is
  compiled into the product. The runtime test origin is accepted only on
  loopback inside a wrapper-proven nested X11 or Wayland session. Transport is
  an argument-vector `curl` child with payload on stdin; no report or endpoint
  text reaches a shell.
- Every transport failure now transitions a reviewed report to `failed`,
  including child startup, stdin, wait, HTTP, and timeout failures. Successful
  transport is the only route to `sent`.

### Settings and review UX

- The existing Updates & Privacy page and its existing product journey were
  extended; no second settings surface, store, or harness was introduced.
- Local Crash Capture is a real persisted switch. Its UI states explicitly that
  the change applies after restart and that existing reports are neither sent
  nor removed on opt-out.
- Users can create/review a local support report, review the newest stored
  report, or delete all reports. Review shows the complete JSON, not a summary.
- Closing review returns a pending report to local. Submission is unavailable
  when no fixed endpoint exists.
- The first confirmation used `GtkAlertDialog`. Real nested Wayland input proved
  that its non-default button could not be selected reliably by standard focus
  traversal. It was replaced with a small modal GTK confirmation owned by the
  review window. Cancel is the default, Return demonstrably cancels, and
  explicit Alt+S submits only after compositor activation. This repaired the
  product accessibility boundary rather than teaching the test a private call.

## Failures found and repaired

- The first controlled hostile panic contained a different user's absolute
  `/home/...` path. Redacting only the running process's `$HOME` leaked it.
  Redaction now removes Linux and macOS user-home paths independently of the
  current account, with a focused regression test.
- Calling the inherited Rust panic hook after saving a redacted report would
  print the original panic payload to stderr. A stack receipt was therefore not
  a privacy receipt. The diagnostics hook now emits only sanitized report-state
  metadata; the real crash journey also rejects hostile secrets in the product
  log.
- A later payload review found a stronger version of the same boundary: a Rust
  panic payload is arbitrary application text and can itself contain a command,
  terminal output, or agent content that no pattern scrubber can classify
  reliably. Panic payloads are now dropped completely. The report retains only
  a basename/line/column crash location plus bounded build/platform context;
  the real hostile panic asserts that neither disk, log, nor captured HTTP
  contains the discarded payload.
- The X11 journey initially failed inside the command sandbox because Xvfb
  could not create its display socket. It passed in the required controlled GUI
  execution context; no environmental absence was converted to PASS.
- Immediate input after dismissing a GTK notice raced focus restoration and
  intermittently missed the Beta mnemonic. The journey now waits a bounded GTK
  turn after safe dismissal.
- Confirmation testing first used Tab/arrow assumptions against
  `GtkAlertDialog`; both selected Cancel under real Wayland. The accessible
  modal described above provides deterministic public keyboard controls and an
  activation receipt.
- An exploratory `strace` product wrapper was discarded before acceptance: it
  changed process ownership/lifecycle seen by the existing harness and hung
  teardown. No parallel tracing harness was retained. Default network silence
  is instead established by the architecture (the sole transport call exists
  only behind explicit reviewed submission), absence of any request at a live
  controlled origin during launch/review/cancel, and the request appearing only
  after the second explicit confirmation.
- The harness formerly dumped the entire verbose Ghostty log on failure. It now
  prints the final 200 lines, preserving the useful failure boundary without
  flooding receipts.
- The final architecture contract run exposed pre-existing GH-73 inventory
  drift: close-evidence fields, fullscreen/minimize methods and actions had
  shipped in source without being reconciled into the ownership contract. The
  contract now assigns those existing members to their actual authorities,
  updates its action cardinality, and also records this slice's sole settings
  callback and ConfigStore writer. Both the positive validator and its negative
  self-tests pass; no duplicate runtime was added to repair the documentation.
- Exact-package qualification exposed a clean-tree defect in the shared
  working-tree fixture: it always invoked `git apply`, even when `git diff`
  produced an empty patch. Dirty-tree tests therefore passed while qualification
  of the committed candidate failed with `No valid patches in input`. The helper
  now applies only a non-empty captured diff, and its contract test exercises
  both dirty and already-clean source trees.
- Final callback review found that two activation events delivered to the same
  confirmation button could enqueue the reviewed report twice before the window
  finished closing. The shared submission gate now uses an atomic-in-callback
  `Cell::replace` guard: only the first explicitly confirmed activation can start
  transport, while later events are ignored.
- The original interrupted-write test constructed a plausible temporary file,
  but did not prove the production write boundary. A child-process test now
  drives the real store through create, write, permission assignment, and
  `fsync`, exits immediately before rename without unwinding, then proves that
  startup pruning removes the orphan and never publishes a partial report.
- The exact installed Wayland journey exposed a pre-existing input/top-level
  category error. Labwc is itself hosted in private X11 so physical keys use
  XTest, but Zentty's inner windows remain Wayland surfaces and cannot be found
  as X11 toplevels. The About close assertion had conflated those facts. X11
  still locates the exact toplevel; Wayland now closes it through the existing
  compositor-input abstraction, which selects outer XTest or `wtype` without
  pretending the inner surface belongs to X11.
- The clean-tree repair then exposed a provenance flaw hidden by the same
  helper: it created an empty synthetic commit even when the reviewed tree was
  already committed, so package manifests named a content-equivalent but
  non-public revision. Fixtures now commit only when they actually materialize
  dirty source. The clean contract pins `HEAD`, allowing final package receipts
  to identify the reviewed public commit exactly.

## Real-system evidence

The same staged ReleaseSafe binary passed under private Xvfb and private
labwc/Wayland with physical compositor input. Each journey performs a real
settings opt-in, restart, real controlled panic, private-file and typed-JSON
inspection, hostile-secret checks in disk and product log, ordinary-launch
silence against a live loopback HTTP origin, complete review, safe-default
cancellation, second explicit submission, captured HTTP request/body
inspection, persisted sent state, stable/beta persistence, and opt-out.

The transmission origin first returns a real HTTP 500. The product persists
`failed`, then a restart/review retries the same report against a fresh 204
origin and persists `sent`; Delete All finally removes the local report. This
closed the initial journey's missing retry/failure/clear acceptance cells.

The controlled endpoint captured exactly `POST /v1/reports` only after explicit
review and confirmation. This is not a claim that production telemetry was
contacted: the default build intentionally contains no production endpoint.

The final installable candidate was the public commit
`a8d45b772d8e7d9433728f84c27aa6d21e918907`, packaged as
`zentty_0.1.0+gita8d45b772d8e_amd64.deb` with SHA-256
`09ddbf82722f2e97cd9a41bed059564ac390eaceda3f70171bb532b41fc61322`.
The disposable Debian lifecycle passed all nine install, reinstall, supported
upgrade, injected-failure, remove, purge, and repeated-cycle transitions.
Installed-product journeys then passed against that same package under X11
session `177a08500e40d7d492e247a98b9f0d33099e082c733d6d9353b40ca503a543a3`
and Wayland session
`9417acd1fda033bc990297ca11824169946612cf5e737b2dda3981faf8e261da`.
Both receipts include `diagnostics_network_silent: true` after direct launch,
real PTYs and panes, agent traffic, CLI mutation, clipboard, Open With, About,
an injected process kill, restore, and desktop-entry launch. This establishes
silence for the representative installed journey; it is not a claim that every
future product action has been enumerated.

### Mutation repair

- The first governed file-wide mutation pass found 99 mutants: 63 caught, 33
  missed, one unviable, and two non-terminating arithmetic mutants. The misses
  exposed under-specified helper boundaries rather than being accepted as a
  percentage score.
- Exact tests were added for foreign-home boundaries, URL terminators, multiple
  named credentials, UUID and token near-misses, invalid/overlapping/adjacent
  ranges, safe identifiers, and UTF-8 byte limits. The survivor-only rerun
  reduced the result to one equivalent loop-condition mutant, 30 caught, and
  four non-terminating arithmetic mutants.
- The equivalent loop condition was removed structurally. Mutatable arithmetic
  loop progress was replaced with saturating progress and iterator-based UTF-8
  boundary selection, eliminating the hang class instead of increasing test
  timeouts. The final focused set caught all 34 generated mutants with zero
  misses and zero timeouts. Every run used `linux/tests/mutate-rust`, retaining
  gitignore/copy policy plus the dedicated 12 GiB cgroup and 6 GiB process cap.
- The first 32-mutant store subset then found nine survivors: redundant
  regular-file/symlink predicates, an unisolated age/count interaction, missing
  filename distractors, and untested exact byte/identifier bounds. Splitting
  independent predicates, isolating the age case, and adding exact helpers and
  fixtures produced a final retention/path/size subset of 21 caught and one
  unviable with no misses. The production/test endpoint and controlled-session
  allowlists caught all 15 generated mutants.

## Remaining boundary

GH-76 completes local crash/support diagnostics and the no-silent-telemetry
promise. It does not implement in-app updates (deferred GH-75) or the terminal
performance overlay and bounded performance measurements (GH-77). Those remain
explicit separate inventory entries under GH-23.
