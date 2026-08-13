# Zentty Linux custom-sound dogfood — 2026-08-13

This append-only record covers GitHub issue #41. The implementation contract is
`docs/design/linux-custom-notification-sounds-feature-plan.md`.

## Initial audit and decisions

- Epic #20 was stale after GH-36 and GH-39 closed. Both were moved into its
  delivered scope, the remaining list now contains only GH-37/38/40/41, and
  custom audio is explicitly the next execution batch.
- The macOS source converts selected audio to a private standardized CAF,
  persists the new internal/display names transactionally, rolls back if config
  persistence fails, and only then prunes prior owned files. Its dynamic picker
  choice and preview are product behavior; AppKit, AudioToolbox, `afconvert`,
  `NSSound`, and `~/Library/Sounds` are platform mechanics and will not be
  ported.
- Linux already has one notification service, one Notifications settings page,
  one `NotificationsConfig`, and one `ConfigStore` transaction. GH-41 must
  extend those authorities. A separate audio manager is permitted only as the
  focused XDG asset/conversion boundary; it may not route notifications or own
  configuration.
- The portable installed format is mono PCM WAV. `ffprobe`/`ffmpeg` validate and
  convert source audio, while real playback is `aplay`; the freedesktop delivery
  hint is `sound-file`. This avoids claiming arbitrary theme-engine support and
  lets qualification use an actual ALSA null device rather than a fake settings
  callback.
- The existing Notifications journey will be extended. No additional actor,
  duplicate integration layer, or prose-only pass is allowed.

## Implementation and security discoveries

- The Linux store now owns only private XDG asset lifecycle. It opens selected
  input with `O_NOFOLLOW`, copies through that descriptor into a mode-`0700`
  transaction directory, bounds source size, duration, decoder time, receipt
  size, and installed size, and publishes a mode-`0600` mono PCM WAV. Stable
  names use the first 128 bits of the converted bytes' SHA-256 digest.
- A mode-`0600` advisory lock serializes import, rollback, replacement, and
  removal. Persistence happens before prune. Rollback deletes a newly published
  asset but deliberately preserves an already committed deduplicated asset.
  Pruning recognizes only strict owned names and ignores symlinks, directories,
  malformed names, and unrelated files.
- Custom delivery resolves the strict owned name back to a bounded regular file
  and uses the freedesktop `sound-file` hint. Custom preview invokes fixed
  `/usr/bin/aplay` arguments without a shell. Theme names remain on the existing
  `sound-name`/Canberra path; no second notification or configuration authority
  was introduced.
- The GTK page exposes native Import Audio, Preview, and Remove Custom controls,
  persists the display name, previews after installation, reports environmental
  playback absence visibly, and reconstructs its dynamic custom choice after a
  real process restart.

## Harness failures and repairs

- **Failure:** the first controlled X11 chooser attempt had no window manager,
  so the native dialog could not be activated through `_NET_ACTIVE_WINDOW`.
  **Repair:** the existing notification journey starts real Openbox inside its
  private Xvfb session. This is test environment control, not a product shim.
- **Failure:** adding Openbox changed physical key routing and the command
  palette query appeared without executing. **Repair:** X11 input explicitly
  focuses the discovered Zentty toplevel before sending ordinary physical keys;
  settings input retains its separate toplevel boundary.
- **Failure:** a second chooser import was an unreliable attempt to prove both
  deduplication and unavailable playback in one UI sequence. **Repair:** the UI
  journey uses the actual Preview mnemonic for the unavailable-device case,
  while a real-ffmpeg store test proves deterministic deduplication and rollback
  without adding another integration harness.
- **Failure:** the first Wayland Alt+I chord did not activate Import Audio under
  nested Cage. Treating that absence as a pass was rejected. A first two-Tab
  workaround also failed because the dropdown/popover focus chain was not the
  assumed sibling chain. **Repair:** the journey starts from the already
  qualified Settings search shortcut and physically traverses the real GTK
  focus chain until the Import Audio focus receipt appears, then activates the
  native chooser. The temporary focus and chooser receipts remain as product
  observability rather than a test-only action.
- **Failure:** the first restart extension reused one cumulative product log,
  allowing old activation text to satisfy new waits. **Repair:** every real
  product launch gets a distinct log; the second settings page must emit the
  persisted strict custom name before removal proceeds.
- **Strict-lint findings:** pedantic Clippy rejected implicit transaction-value
  consumption, digest `format!` collection, unspecified lock-file truncation,
  clone assignments, and an oversized settings constructor. The repairs make
  transaction consumption explicit, write the digest directly, document
  non-truncating lock semantics, use clone-in-place, and extract sound-selection
  behavior into a focused function.

## Current evidence

- Focused store suite: six passing tests covering XDG resolution, strict names,
  modes, no-follow and size bounds, duration/WAV validation, safe prune,
  corrupt-input rejection, real conversion/dedup/rollback, and concurrent lock
  serialization.
- Controlled X11 real-system journey: PASS, nested session
  `2830b2396430d36848778f53a4fb4c1a52fc0b7afab4f223a000c3a0c3031006`.
- Controlled Wayland real-system journey: PASS, nested session
  `9af243ef586ec11cd4079a67394cad6e1b6e21488aaf15509bd2acaac3f47eed`
  (outer private-X11 session
  `1602a9170a42e7658f80ea7578d043fa094e28304994c2ca37d23181dce984fd`).
- Both journeys cross the real GTK chooser, ffmpeg/ffprobe, aplay with a real
  ALSA null PCM, ConfigStore persistence and product restart, Ubuntu
  notification-daemon on a private D-Bus, exact `sound-file` inspection,
  explicit unavailable playback, and physical removal. Portal warnings from
  Ubuntu's GNOME backend under nested Cage remain visible and are not converted
  into a pass or suppression.

## Mutation findings and repairs

- The first governed run correctly used the repository's `gitignore=true` and
  `copy_target=false` policy, but its unmutated package baseline failed inside
  network isolation when an existing real `/proc` listener test could not open
  a socket. The baseline was rerun with the required real localhost capability;
  this was not treated as a mutant result.
- Format mutants showed that `is_err()` assertions could not distinguish the
  intended early incomplete/non-file WAV rejection from a later header-read
  failure. Exact errors plus RIFF-only, WAVE-only, exactly-44-byte,
  exactly-maximum, and maximum-plus-one fixtures now pin each decision.
- Source-copy mutants exposed the same ambiguity for empty/directory input and
  exact size boundaries. Metadata validation, descriptor copy limits, and
  copied-byte validation now have focused pure decision functions with zero,
  exact-limit, and limit-plus-one assertions; the real no-follow copy remains
  independently exercised.
- `O_NOFOLLOW | O_CLOEXEC` produced an equivalent XOR mutant because the flags
  are disjoint. Rust already opens `File` descriptors close-on-exec; the custom
  flag now contains only the security-relevant `O_NOFOLLOW`, eliminating the
  equivalent rule rather than suppressing a miss.
- Lock mutation found both the immediate-timeout and infinite-wait cases. A
  short injected timeout makes the former deterministic; the latter is caught
  as a governed timeout. Only strict-versus-inclusive comparison at one exact
  `Instant` is excluded with a scoped cargo-mutants annotation because no
  observer can distinguish those states. Wakeup, timeout, lock-result, and
  deadline-arithmetic mutants remain in scope.
- Final governed result across sharded receipts: 52 caught mutants, two
  compile-unviable lock-result mutants, one intentional infinite-wait timeout,
  zero missed mutants, and the one scoped exact-`Instant` exclusion described
  above. Sharding was required because the execution supervisor limits an
  individual command window; every shard reused the ignored Cargo target and
  retained `gitignore=true`, `copy_target=false`, and the focused
  `custom_sound_store` test filter.

## Aggregate qualification findings

- The final post-review aggregate completed in `524720ms`. All 126 declared
  PASS cells passed, the one reviewed ABI XFAIL remained XFAIL, and all 7
  BLOCKED plus 22 NOT_IMPLEMENTED gaps remained explicit. The implemented local
  suite passed; release and full Linux qualification correctly remained not
  passed. Suppression governance was ACCEPTED, so the Valgrind result is **PASS
  with reviewed suppressions**, not an unsuppressed-clean claim. The immutable
  machine receipt is `build/linux/qualification-summary.json`.

- The first complete post-feature run executed every presently executable
  matrix cell and retained reviewed Valgrind suppression evidence, but it did
  not pass: `staged-wayland` hit the established tmux-product child deadline,
  and the X11 bookmark import/export journey failed to map its bookmark-name
  dialog after starting Openbox for native choosers. The staged Wayland command
  passed unchanged on an immediate exact controlled rerun, so no product or
  harness change was made for that non-repeating deadline.
- The bookmark failure repeated. Openbox had moved X11 activation from direct
  focus to `_NET_ACTIVE_WINDOW`; focus traversal reached Save Bookmark, but
  Return remained routed to the wrong surface. The shared journey now releases
  modifiers and reactivates the known Zentty toplevel before activating that
  focused control. The exact import/export cell then passed with nested session
  `29a102845825d32c0975c0fc9eb53d3c1cfee71a947770f366989df9174e7041`.
  This repaired an existing real-system harness boundary rather than weakening
  or skipping the unrelated cell.
- Subsequent aggregate starts exposed runner defects rather than product
  failures. First, an unrestricted `wait -n` could reap an unrelated child;
  bounded pools now wait only for their owned PIDs and a focused negative test
  keeps an unrelated child outside the pool. The remaining missing results had
  a different cause: background worker functions inherited the scheduler's
  `EXIT` trap and the first completed worker could delete the shared result
  directory. Each worker now clears that inherited trap while retaining the
  per-cell subshell that isolates shell state. A temporary attempt to remove
  that subshell caused `ghostty-regression` to exit the worker before publishing
  a result and was reverted rather than normalized as a failure. Focused runner
  assertions cover both lifecycle requirements. Every incomplete evidence
  directory was preserved under `build/linux/` rather than overwritten or
  represented as a qualification pass.
- The first direct rerun then failed immediately because the qualification
  runner used the new owned-PID helper without sourcing its library; the
  `qualify-local` wrapper happened to source that library for its separate
  support-test pool, hiding the missing direct-run dependency. The matrix runner
  now sources `lib/bounded-batch` itself, and its focused test pins that import so
  direct and wrapped qualification use the same scheduler implementation.
- Final code review found that an imported `PreparedSound` relied on every UI
  callback reaching explicit `finish` or `rollback`. If the GTK receiver closed
  after conversion, dropping the channel payload could leave a published but
  unreferenced file and transaction directory. `PreparedSound` now owns rollback
  through `Drop`; `finish` disarms deletion only after configuration has already
  committed the asset. A real second ffmpeg conversion proves that dropping an
  uncommitted transaction removes both its installed file and scratch directory
  while preserving the committed deduplicated asset.
- The same review moved mode-`0600` assignment and file sync before publication,
  removes a renamed asset if directory sync fails, bounds collision reads, kills
  a converter/prober after a wait error, and makes playback/delivery path lookup
  revalidate the private owned directory chain. These were repaired before the
  final qualification receipt rather than accepted as cleanup debt.
- A post-review mutation shard initially exposed six surviving boundary mutants.
  Installed read limits, installed-size decisions, and destination-error
  classification were extracted into focused decisions with exact zero/limit/
  limit-plus-one and `NotFound`/`PermissionDenied` assertions. A real ffmpeg
  transaction against a regular-file store proves non-`NotFound` inspection
  errors cannot enter the publication branch. The repaired final-review shard
  caught all 15/15 affected mutants using the governed repository copy policy
  and the `custom_sound_store` test filter. An earlier unfiltered attempt failed
  its unmutated baseline because unrelated config lock-stress tests contended
  across mutation workers; it was retained as evidence and not counted.
