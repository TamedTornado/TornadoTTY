# Bookmark storage responsiveness — 2026-09-07

Scope: #163, following the close-responsiveness batch at `e42a7eed`.
Ghostty remains pinned to `bab8c088f45e47a00ce3bfe2c142d6cb51ecd200`.
No live-client install or restart; user QA is in the agreed later batch.

## Product change

- Bookmark loading, capture-time `/proc` reads, mutations, restore filesystem
  checks, and recency writes run away from GTK. Import uses bounded asynchronous
  Gio reads (the existing 1 MiB limit); parsing/persistence run on the worker.
  Export retains its asynchronous Gio write.
- One process-wide admission permit covers an operation through publication.
  Overlapping requests are explicitly refused, not silently queued or reported
  saved. Existing locked atomic storage remains the durable owner; each window
  has only a projection, refreshed together after completion.
- Restore checks its captured source pane/context and the current template
  before applying prepared work. Failed surface creation restores the previous
  topology and removes the attempted surfaces/launch registrations.
- Completion is logged only after durable work and refresh, not when GTK merely
  accepts the request. Storage errors use the existing visible action notice.
- Closing a non-final window does not cancel an accepted save. Application quit
  or final-window close is refused visibly while storage is busy; retry after
  completion. This uses the existing close coordinator, not a shutdown timer.
- Unchanged snapshots do not trigger another sidebar rebuild. No second store,
  unbounded mutation queue, new test framework, or Ghostty change was introduced.

## Regression work and failed attempts

The new `storage-responsiveness` scenario extends `rust-bookmarks-presets`, using
the existing private-X11 runner and strace delay injection already used by task
and close tests. It delays real reads, not their returned data. The old staged
`e42a7eed` build fails the terminal-startup deadline under that delay.

The first unelevated Xvfb attempt was sandbox-blocked; the approved isolated
runner was then used. The import/export actor still searched for pre-rebrand
`Zentty` chooser titles; its title lookup was corrected to the shipped branding.
No import/export behavior requirement was weakened.

Early stale-restore results were misleading: timing the new actor showed
`xdotool mousemove --sync` waiting approximately 15 seconds before typing. The
directory change then reached the already-restored pane. The unnecessary sync
wait was removed; XTest requests remain ordered and real PTY output is the
response barrier. Timing now covers the whole input gesture. Atomic replacement
breaking strace's path filter was investigated and disproved with disposable
files. Startup input also waits for the real shell's first prompt, while checking
that the delayed bookmark load is still pending. The close probe must actually
accept the normal quit confirmation before checking save protection.

## Focused validation

Staged bundle: `build/gh163-bookmarks`.

```sh
ZENTTY_BOOKMARK_SCENARIO=storage-responsiveness \
  ZENTTY_LINUX_BINARY="$PWD/build/gh163-bookmarks/bin/zentty-linux" \
  linux/tests/nested-x11 linux/tests/rust-bookmarks-presets
# Same existing journey: save-restore, management, import-export.
GHOSTTY_LIB_DIR="$PWD/build/linux-deps/ghostty/zig-out/lib" \
  cargo test --locked -p zentty-linux --bin zentty-linux bookmark_runtime
cargo test --locked -p zentty-core --test bookmark_store
linux/tests/mutate-rust -p zentty-linux \
  -f crates/zentty-linux/src/application_shell/bookmark_runtime/storage.rs \
  -F 'Permit::(acquire|drop)' -j4 --cargo-test-arg=--bin=zentty-linux \
  --cargo-test-arg=bookmark_runtime --timeout 15
```

- Slow-storage journey: PASS real PTY startup while loading, complete input
  gestures below 1.5 seconds during delayed reads/writes, explicit overlapping
  request rejection, successful retry, source-CWD change rejecting restore,
  unchanged recency for that rejected restore, visible failed storage without
  file modification, and save protection across confirmed final-window close.
  Closing normally after storage finishes also passes.
- Existing management, save/relaunch/restore and native import/export journeys:
  PASS (rename, edit, duplicate, pin, convert, linked update/unlink, delete). The
  recency assertion now waits for the actual asynchronous durable update,
  rather than assuming that creating the surface also completes its disk write.
- GTK bookmark tests: 7 PASS, including real-file empty/exact-limit/oversize/
  missing import reads and overlapping worker/panic admission cleanup.
- Core bookmark integration tests: 6 PASS (real locked atomic storage, mutation
  preservation, managed symlinks, corruption/future schema, portable import).
- Scoped admission mutations: 1 caught, 0 missed, 1 unviable. This is limited
  admission evidence, not a strength claim about the whole GTK suite. The test
  separately joins successful assertions and the deliberately panicking worker
  so an assertion failure cannot masquerade as the expected panic.
- Build, package notices, dependency publish-age audit (91 packages, no
  exceptions), touched-file formatting, shell syntax and diff checks: PASS.

No full qualification, refreshed matrix totals, or Wayland claim. The existing
six GTK Clippy findings remain; a dependency-inclusive invocation also encounters
two unchanged core findings. No new suppressions were added. #163 remains open
for remaining catalog/monitor I/O and broader PTY/IPC backpressure work.
