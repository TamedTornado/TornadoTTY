# Linux General settings feature plan

Status: active implementation plan for GitHub issue #20
Date: 2026-08-12

## Source authority

The feature authority is
`Zentty/UI/Settings/GeneralSettingsSectionViewController.swift` together with
`AppConfig.swift`, `AppConfigTOML.swift`, and the source clean-copy pipeline.
Linux must port the behavior and labels, not infer a smaller "typical settings"
page.

The source page contains exactly twelve controls:

1. Confirm before closing a pane.
2. Confirm before closing a window with running processes.
3. Confirm before quitting Zentty.
4. Restore worklanes on next launch.
5. Always clean copied content.
6. Flatten multi-line commands.
7. Command flatten aggressiveness: Low, Normal, or High.
8. Preserve blank lines when flattening.
9. Remove box-drawing characters.
10. Flatten slash-command selections.
11. Strip URL tracking parameters.
12. Quote paths with spaces.
13. Show Copy as Markdown command.

The numbered inventory is intentionally thirteen: the source's four lifecycle
controls plus nine clipboard controls must not be compressed into the twelve
switches merely because aggressiveness is a selector rather than a switch.

## Ownership and implementation order

1. Extend the existing source-compatible `AppConfig` with confirmations and
   restore policy. Reuse the existing `ClipboardConfig`/`CleanCopyOptions`; do
   not create a second clean-copy model.
2. Add one `ConfigStore` general-settings write transaction that preserves
   comments, unknown keys, unrelated tables, final symlinks, permissions, and
   concurrent writers through the existing lock/atomic-replace boundary.
3. Add a focused GTK General page module. Every control initializes from the one
   process config snapshot, persists through `ConfigStore`, and applies through
   `ApplicationShell`; no page-local durable state is authoritative.
4. Apply clipboard changes immediately to sidebar/menu visibility and the real
   clean-copy action. Apply restore policy on the next ordinary launch while
   retaining `--no-session-restore` as the explicit test/operator override.
5. Route pane/window/quit requests through source-named confirmation policy.
   Tests must distinguish enabled confirmation, cancel, confirm, and disabled
   direct execution; a dialog receipt alone is not the destructive-effect proof.

## Test-first acceptance

1. Core tests pin defaults, exact TOML names, invalid known values, and every
   clean-copy option.
2. ConfigStore tests cover a complete update, comments/unknowns/unrelated tables,
   symlink and `0600` preservation, malformed input, oversize input, and
   serialized concurrent distinct updates.
3. Focused mutation tests cover the pure config policy before GTK acceptance.
4. Controlled X11 and nested Wayland journeys open General through its real
   palette deep link and manipulate every representative control with physical
   input. They inspect persisted normalized TOML, restart the staged app, and
   assert the controls reload.
5. The real PTY journey proves clean-copy output changes immediately and that
   Copy as Markdown action visibility follows configuration. A two-launch
   persistence journey proves restore enabled and disabled behavior.
6. Real confirmation journeys cancel and accept pane/window/quit operations and
   prove the corresponding process/window topology. Environmental absence is
   never a pass.
7. Run strict lint/format/architecture gates and the complete presently
   executable matrix before commit. Keep all non-PASS declared cells visible.

## Scope boundary

This feature replaces the General status page only. Notifications, Updates &
Privacy, Worklanes & Panes, Open With, Dev Servers, and Agents remain separate
source-backed feature slices. External config watching/partial reload remains a
later issue #20 slice and is not implied by General's safe write transaction.
