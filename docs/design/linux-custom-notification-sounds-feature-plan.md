# Linux custom notification sounds feature plan

Status: active implementation plan for GitHub issue #41
Date: 2026-08-13

## Source contract

The authority is `Zentty/AppState/NotificationSoundManager.swift`,
`Zentty/UI/Settings/NotificationsSettingsSectionViewController.swift`, and
`ZenttyLogicTests/NotificationSoundManagerTests.swift`. The portable behavior is
transactional import, a fresh or collision-safe internal name, retained display
name, preview, persistence before pruning, rollback on persistence failure, and
safe removal of owned obsolete sounds. Linux uses native GTK and XDG/audio
mechanics rather than copying AppKit, CAF, `afconvert`, or `NSSound`.

## Linux design

1. Add one focused `custom_sound_store` module. It owns
   `$XDG_DATA_HOME/zentty/sounds` (falling back to
   `$HOME/.local/share/zentty/sounds`), secure directory/file modes, bounded
   no-follow source ingestion, conversion, installed-name validation,
   transaction locking, rollback, and containment-safe pruning.
2. Standardize installed assets as bounded mono PCM WAV. `ffprobe` establishes a
   finite duration no greater than 30 seconds; `ffmpeg` performs conversion; the
   actual Linux playback boundary is `aplay` with an argv vector and no shell.
   Stable names are derived from the converted bytes' SHA-256 digest, so equal
   audio deduplicates and distinct content does not collide in ordinary use.
3. Extend the existing `NotificationService`, Notifications page, `AppConfig`,
   and `ConfigStore` authority. Do not add another notification router, settings
   store, watcher, or product test harness.
4. A custom notification uses the freedesktop `sound-file` hint. A named theme
   sound continues to use `sound-name`. Preview and delivery resolve only strict
   owned custom names; arbitrary configured paths never become playback argv.
5. GTK exposes native Import Audio and Remove Custom controls, an installed
   custom choice, visible status/error text, and disables preview when the fixed
   playback executable is unavailable. Persistence succeeds before obsolete
   files are pruned; failure rolls the prepared file back.

## Security and failure contract

- Input is opened with `O_NOFOLLOW`, must be a nonempty regular file, and is
  copied through that descriptor into a private transaction directory before an
  external decoder sees it. Input, output, stderr, duration, process time, and
  installed-file size are bounded.
- The owned data and sounds directories reject symlinks and use mode `0700`;
  installed assets and the transaction lock use `0600`.
- Pruning accepts only `zentty-custom-<32 lowercase hex>.wav`, uses
  `symlink_metadata`, and never follows or removes a symlink, directory, or path
  outside the owned sounds directory.
- Unsupported/corrupt/oversized/inaccessible/vanished sources, decoder timeout,
  persistence failure, missing installed assets, and unavailable playback are
  explicit content-safe failures. Environmental absence is never a pass.

## Test construction order

1. Focused unit tests first: XDG fallback, strict names, no-follow/containment,
   modes, bounds, deterministic naming, collision/deduplication, rollback,
   referenced-file retention, pruning, conversion failure, and process timeout.
2. Extend `linux/tests/rust-notifications-settings`, not a new harness. Under
   isolated XDG and private D-Bus it must drive a real GTK chooser on controlled
   X11, import a real generated WAV, persist/restart, preview through real
   `aplay` using an ALSA null device, deliver a real freedesktop `sound-file`
   hint, remove physically, and cover missing backend/device behavior. Nested
   Wayland exercises the same real chooser, installed/persisted playback, and
   notification boundary through controlled GTK focus traversal.
3. Governed mutation tests cover containment, name validation, duration/size
   decisions, persistence rollback, and prune/reference decisions.
4. Reconcile the matrix, feature inventory, architecture mirror, user docs,
   issue state, and append-only dogfood report. Run strict gates and one complete
   qualification receipt only after the coherent feature is complete.

## Completion rule

GH-41 closes only when every acceptance item is implemented or explicitly
resolved by the Linux design above, all presently executable cells pass, and no
custom-audio NOT_IMPLEMENTED cell remains. ReleaseSafe Valgrind remains XFAIL;
no suppression may be broadened to affect that outcome.
