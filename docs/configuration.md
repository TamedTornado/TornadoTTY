# Linux product configuration

Zentty's Linux product configuration is one TOML document at:

```text
$XDG_CONFIG_HOME/zentty/config.toml
```

When `XDG_CONFIG_HOME` is unset or empty, Zentty uses
`$HOME/.config/zentty/config.toml`. Startup fails with an actionable error when
neither location can be resolved. Tests and development journeys set an
isolated XDG home; they never fall back to the operator's real configuration.

## Ownership and permissions

`ConfigStore` is the sole bounded reader and writer. `ConfigReloadAuthority`
owns the process-wide last-known-good snapshot and directory watches, and the
application coordinator publishes accepted snapshots to every open window.
Settings pages are editable projections of that authority, not independent
stores.

Zentty creates or tightens its own `zentty` configuration directory to mode
`0700`. A regular logical config file is tightened to `0600` at startup;
product-created config files, sibling temporary files, and the shared advisory
lock are also mode `0600`. A symlinked `config.toml` is supported: Zentty
updates the resolved target without replacing the link or changing the
operator-managed target directory or target file's mode. A symlinked Zentty
configuration directory is rejected; use a file symlink instead.

## Validation and live reload

Startup parses the complete known schema. Invalid syntax, encoding, size, or a
known value produces a content-safe warning and source defaults; file contents
and values are not copied into diagnostics.

While Zentty is running, TOML syntax is the all-or-nothing boundary. Once syntax
is valid, known top-level sections are decoded independently:

- a valid section is normalized and published;
- an invalid known section retains its last-known-good value;
- an absent section deliberately returns to its source default;
- a syntax failure, vanished file, unreadable target, or oversized document
  retains the complete last-known-good runtime state.

Rapid replace-by-rename edits are coalesced, and the final complete document is
published to every open window without recreating Ghostty surfaces or PTYs.
Visible settings pages rebuild from the accepted snapshot while preserving the
selected section. Product self-writes do not repeatedly rebuild their initiating
page or enter a reload loop.

## Writes, external editors, and interruption

Every Zentty-owned read/modify/replace operation uses one advisory lock. This
prevents two product settings actions from losing independently owned sections.
Comments and unknown keys survive product changes wherever `toml_edit` can
preserve them.

Writes use a same-directory mode-`0600` temporary file, write and sync the full
document, atomically rename it over the target, and sync the containing
directory. A process interruption before rename can leave only an ignored
sibling temporary file; it cannot publish a partial config. Failure after rename
but before/during directory sync is reported as a failed durability operation,
even though the complete renamed document may already be visible.

External editors do not honor Zentty's advisory lock. Overlapping complete
atomic replacements therefore have a documented **last-writer-wins** outcome:

- if the external rename is last, its complete document wins;
- if Zentty writes last, it rereads the external document and preserves its
  comments/unknown keys while changing the owned setting.

Zentty does not claim cross-process compare-and-swap or merge semantics.

## Symlinks and includes

The logical config file may point to a relative or absolute regular-file target.
Zentty watches both the logical directory and the current target directory, so
target replacement and live retargeting continue to reload. A broken target is
retained as unavailable rather than silently publishing defaults or creating an
operator-managed target directory.

The source `AppConfig` schema is a single TOML document and has no include
directive. Unknown keys are preserved but do not create recursive watches.
Ghostty's separate `config-file` include stack belongs to Ghostty configuration
and is reloaded through the Ghostty appearance/configuration authority.

## Size and compatibility

The product config is bounded to 1 MiB. Exactly 1 MiB is accepted; a larger
document is rejected without replacing the accepted runtime state. Unknown
tables and keys are retained during product writes where practical so future or
external configuration can coexist with the current build.

## Custom notification sounds

Notifications settings can import a local audio file of at most 30 seconds.
Zentty validates and converts it to a private mono PCM WAV under
`$XDG_DATA_HOME/zentty/sounds` (or `$HOME/.local/share/zentty/sounds`), records
only its Zentty-owned content-derived name in `config.toml`, and sends that
absolute owned file through the freedesktop `sound-file` hint. The data
directories are mode `0700`; the installed audio and transaction lock are mode
`0600`.

Import requires `/usr/bin/ffprobe` and `/usr/bin/ffmpeg`. Preview requires
`/usr/bin/aplay` and a working ALSA output. Unsupported, corrupt, symlinked,
oversized, or inaccessible input is rejected visibly. A missing playback device
is reported as unavailable and does not undo an otherwise successful import.
Changing to a theme sound or selecting **Remove Custom** deletes obsolete
Zentty-owned assets only after the new configuration has persisted.

## Pane toolbar click-through

Hold **Ctrl** (either left or right) to temporarily hide the split/move/close
toolbar at the top right of terminal panes. While held, clicks reach the
terminal underneath instead of the toolbar. Release Ctrl to restore the
toolbar's normal hover/focus visibility, without moving the pointer.

The modifier is not consumed: terminal applications still receive normal
Ctrl-modified input, and existing keyboard shortcuts keep their behavior.
This is temporary window-local presentation state, not a saved layout setting.

## Local GUI stall evidence

The Linux client has a lightweight local progress observer independent of the
optional crash-report submission setting. It samples the main-loop heartbeat
and symbolic operation from a separate thread; it never calls GTK from that
thread. If progress stops, it reports at approximately 500 ms, 2 seconds and
10 seconds, capped at three stall samples and twelve total records per minute
(including recovery). Healthy updates produce no observer records.

The records distinguish layout/application ticks, server ranking, server
projection, tooltip/accessibility updates, menu construction and popover
replacement. They include a correlation sequence, elapsed times and available
Linux kernel wait name/syscall **number only**. They contain no terminal text,
agent messages, URLs, paths, syscall arguments or protocol payloads. State is
fixed-size, with no event queue. Nonblocking local journal delivery can drop
records if the journal is unavailable or full; startup reports installation
failure explicitly. Journal retention follows the system's existing limits.

Retrieve the bounded records with:

```sh
journalctl --user -t tornadotty-ui-diagnostics --since '30 minutes ago'
# Substitute the failed GUI's PID, not a newly relaunched process:
journalctl --user _PID=12345 --since '30 minutes ago'
```

A `stalled` record is evidence of nonprogress, not proof of a deadlock or its
root cause. `recovered` refers to the same `stall-sequence`. Kernel metadata may
be unavailable under system security policy. No stack unwinding, ptrace,
`WAYLAND_DEBUG`, network submission or payload tracing is enabled.

Existing Ghostty/GDK fatal Wayland diagnostics remain intact; a normal GDK
`display-closed` signal is also logged. A fatal disconnect can exit before that
signal is delivered, so the off-GUI records are intended to preserve context
**before** failure. Installing a new executable does not instrument an already
running process; the observer starts on the next launch of the new build.
