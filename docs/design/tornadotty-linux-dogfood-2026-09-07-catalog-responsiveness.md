# App/browser discovery responsiveness — 2026-09-07

Issue: #163, parent #160. Previous batch: `9aaeb6c3` (bookmarks).
Ghostty remains `bab8c088f45e47a00ce3bfe2c142d6cb51ecd200`.

## Problem and repair

Desktop-app enumeration, PATH lookup and custom executable canonicalization
ran on GTK during startup and settings actions. The existing Open With journey
now has a focused slow-discovery mode: strace delays a real custom executable's
`readlink` by five seconds, without substituting its result.

The previous build fails the window-creation deadline. The repaired build opens
the window and completes a full real PTY input gesture in under 1.5 seconds
while discovery remains pending. Changing both app and browser preferences
during that scan rejects its results and publishes the latest preferences.
The first attempt incorrectly timed terminal readiness rather than window
creation; renderer startup is a separate boundary. The corrected test was run
against both old and new builds, and separately waits for a shell prompt before
timing input. No timeout relaxation hides a product stall.

One background catalog worker per window coalesces to one latest generation.
Only plain catalog values return to GTK. Startup, config reload, settings and
Refresh Apps share that owner. Custom app/browser validation and the existing
browser-availability monitor also run their filesystem checks off GTK, with
one validation slot per page and stale-setting rejection. No new timer, pool,
dependency or harness was introduced. Configuration loading/persistence still
uses the existing authority; this does not claim all settings I/O is asynchronous.

The first normal journey caught a refresh bug introduced here: an older worker
was rejecting a refresh belonging to the queued newer scan. Keep its completion
pending for that scan instead. Refresh also checks authoritative disk settings
before reconciling, so a file-watcher delivery lag cannot overwrite an external
edit. Restart assertions now await catalog completion, not merely window creation.

The remote portion also exposed an outdated assertion: after SSH identity is
known, the palette disables Open With and blocks physical activation before
dispatch. Requiring a backend-launch rejection log in that state is incorrect.
The journey now requires the disabled result, an increased blocked-activation
count, and the unchanged launch receipt. It does not accept an enabled action
or a launched desktop process as success.

## Focused checks

- Normal Open With journey: PASS on private X11, including custom-app add,
  remove, refresh, external preferences, durable restart, exact desktop/custom/
  terminal launch arguments, Recent Actions and real SSH rejection.
- Slow discovery: PASS on private X11, including real PTY responsiveness and
  stale app/browser preferences. Reproduce with
  `ZENTTY_OPEN_WITH_SLOW_DISCOVERY=1 linux/tests/nested-x11 linux/tests/rust-open-with`
  and `ZENTTY_LINUX_BINARY` pointing at the staged binary.
- GTK targeted tests: 12 PASS (2 discovery, 7 Open With, 1 server runtime,
  2 dev-server settings).
- Scoped `Discovery::begin`/`finish` mutation check: 14 caught, zero missed.
  Run through `linux/tests/mutate-rust` with its existing bounded resource and
  gitignore-aware copy policy. This is not a whole-suite mutation claim.
- Development-server canonical journey: FAIL at the reviewed screenshot on
  both the unchanged previous build and this build; tracked in #180.
  A separate diagnostic run with only that screenshot comparison excluded via
  a disposable visual map passes the real listener, authenticated watcher,
  primary/preferred/explicit/system browser, exact argv, forged-target rejection,
  persisted ignore, safe-stop and restart assertions. That is semantic-only
  evidence, not a visual-parity or canonical-journey PASS. No baseline changed.
- Build, package notices and dependency age audit: PASS (91 packages, zero
  age exceptions). Touched-file formatting, Bash syntax and diff checks: PASS.
- Clippy still reports the six pre-existing GTK findings. Workspace-wide fmt
  check also flags four untouched files; touched-file check is clean. No new
  suppressions or unrelated formatting changes.

Staging: `build/gh163-catalogs`. No installation, live-client interruption,
full qualification, refreshed matrix totals or Wayland claim. User QA remains
pending for the later batch. #163 remains open for remaining blocking monitors
and the broader PTY/IPC bounds; #180 owns the independent screenshot failure.
