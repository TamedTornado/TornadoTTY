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
