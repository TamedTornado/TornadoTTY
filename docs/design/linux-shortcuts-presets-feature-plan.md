# Linux Shortcuts and Presets Feature Plan

Status: implementation plan for GitHub issue #20
Source authority: `Zentty/Input/KeyboardShortcut.swift`,
`Zentty/Input/KeyboardShortcutResolver.swift`, `Zentty/Input/ShortcutPreset.swift`, and
`Zentty/UI/Settings/ShortcutsSettingsSectionViewController.swift`

## Outcome

Port Zentty's configurable application-shortcut system to Linux without placing a
second command vocabulary beside the existing `workspace.*` action router. A bound
application shortcut must activate its registered GTK action; an unbound key event
must continue into the real embedded Ghostty terminal unchanged.

This slice includes the searchable/category settings browser, physical shortcut
recording, binding conflict handling, clear/reset, left- and right-hand presets, and
portable import/export. It does not silently turn every GTK-local interaction key
(for example Escape inside a dialog) into a user-configurable application command.

## Architecture constraints

1. `application_shell/action_router.rs` remains the sole executable application
   action vocabulary. Shortcut metadata may reference only parameterless registered
   actions; it may not introduce callbacks or duplicate handlers.
2. Shortcut value types, parsing, conflict resolution, sanitization, preset
   resolution, and import/export belong in a focused, GTK-independent Rust module.
3. GTK/GDK physical event translation and the settings window belong in focused
   Linux modules. `application_shell.rs` should lose—not gain—hard-coded shortcut
   policy as bindings migrate.
4. TOML writes use the existing bounded, symlink-preserving, atomic config store and
   preserve comments and unknown settings.
5. Linux maps the source application's Command-oriented defaults to Control,
   source Control to Super, and Option to Alt. Super is not substituted for
   Command because it is compositor owned on common Linux desktops; it is retained
   only where the source chord needs Command and Control to remain distinct.
6. The Control+Tab hold/peek interaction remains a dedicated state machine. Its
   semantics are richer than a one-shot command and cannot be represented honestly
   by an ordinary binding.

## Test-first order

1. Add failing pure tests for storage parsing, physical keys, modifier identity,
   conflict replacement, duplicate/stale command rejection, presets, and
   import/export round trips.
2. Add failing config-store tests for atomic shortcut updates, comment/unknown-key
   preservation, symlink preservation, malformed external edits, and reload.
3. Add a real staged-app product journey that records and changes a shortcut,
   proves the new binding activates the action, proves the displaced/unbound key is
   delivered to the child PTY byte-for-byte, restarts the app, and proves the
   binding persists. Run it in the controlled X11 and Wayland environments already
   owned by `linux/tests/product-input`.
4. Implement the pure model and config persistence.
5. Implement one capture-phase dispatcher that resolves active bindings to the
   existing `workspace.*` actions, then remove migrated hard-coded handlers.
6. Implement the focused settings UI: searchable categories, command detail,
   physical recorder, conflict resolution, clear, presets with previews, reset, and
   import/export.
7. Run focused unit/integration tests, architecture contracts, mutation tests with
   the repository's guarded cargo-mutants configuration, and every presently
   executable qualification cell. Review the diff before commit and push.

## Acceptance evidence

- The settings window exposes every bindable parameterless command in the action
  router, grouped and searchable; registry drift is a failing test.
- Recorder accepts printable physical keys plus Space, Delete, Return, Tab, and the
  four arrows, and records left/right modifier provenance while resolving bindings
  to portable modifier classes.
- Conflicts are visible and resolved explicitly; clearing creates an explicit
  unbind; reset removes overrides.
- Both source presets are adapted for Linux and previews show their resolved keys.
- Exported data is versioned and deterministic; import rejects unknown command IDs,
  malformed shortcuts, duplicate command IDs, and conflicts instead of silently
  dropping them.
- Real X11 and Wayland receipts distinguish app-consumed shortcuts from terminal
  input and prove persistence across a real process restart.
- No CSI-u assumption is introduced and no environmental absence is reported as a
  pass.
