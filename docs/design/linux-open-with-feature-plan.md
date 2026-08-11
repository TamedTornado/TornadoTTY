# Linux Open With feature plan

Date: 2026-08-11
Issue: GH-18 (`project.open-with` subset)

## Outcome

The focused local pane can be opened in an installed Linux editor, file
manager, or terminal from the source-owned chrome and command palette. Zentty
preserves the source application's catalog-ordered enabled/primary semantics,
adds Linux-native desktop application discovery, and never evaluates a target
as shell text.

This is one feature slice. Bookmarks/presets and project icons remain separate
GH-18 work. Opening a directory in another tool does not make Zentty an IDE.

## Source and Linux contract

- The focused pane's resolved local working directory is the input. SSH/remote
  panes, missing directories, symlinks that cannot be canonicalized, and stale
  pane identity are unavailable rather than guessed.
- Preserve source preference fields: `primary_target_id`,
  `enabled_target_ids`, and custom applications with stable ID, display name,
  and executable path. Invalid/duplicate entries are normalized
  deterministically without changing unrelated configuration. As in the source,
  enabled built-ins retain catalog order rather than preference-array order;
  custom applications follow in their configured order.
- Linux built-ins cover editors, file managers, and terminals. Availability is
  resolved from executable files and desktop applications, never from display
  name alone. The configured primary target falls back to the first available
  enabled target in preference order.
- Desktop applications are launched with a canonical local file URI through
  GIO's desktop application contract; GLib may intentionally materialize that
  `file:` URI as a canonical path for a local desktop handler. Custom
  executables receive one canonical path argv.
  No shell, command-line string splitting, environment interpolation, or
  untrusted URI scheme is allowed.
- The proposed XDG default-terminal launcher receives its specified monolithic
  `--dir=<canonical path>` option; it is not treated like a custom executable
  whose final argv is a bare directory.
- Missing targets remain diagnostically absent; they are not silently treated
  as success. Launch is bounded to one child request and reports exact stable
  target ID without logging user file contents.

## Test-first construction order

1. Pure Rust tests pin config normalization, catalog ordering, primary fallback,
   duplicate IDs/paths, executable availability, URI encoding, local-directory
   validation, and launch-plan argv.
2. Real-system tests create executable custom targets and controlled `.desktop`
   applications under private XDG roots, then exercise real discovery and
   process launch without a product fixture route.
3. Staged X11 and Wayland journeys start the actual Zentty/Ghostty product in a
   canonical temporary directory, invoke primary and selected Open With actions
   through physical compositor input, and assert exact path/URI receipts from
   independent disposable applications.
4. Cover missing executables, hostile paths/desktop entries, remote panes,
   config order/primary fallback, and concurrent launch attempts. Mutation-test
   normalization, availability, and launch-plan policy with the governed
   disk-safe runner.
5. Update the feature inventory, ApplicationShell ownership contract, and
   authoritative qualification matrix; run every presently executable cell.

## Acceptance criteria

- [x] Linux editor, file-manager, terminal, and custom targets are explicit.
- [x] Catalog ordering and primary fallback match the source semantics.
- [x] The focused canonical local directory is used; remote/stale context is
      unavailable.
- [x] Custom executables receive one exact path argument with no shell.
- [x] GIO receives one exact canonical file URI and the controlled desktop
      handler receives its platform-resolved canonical local target.
- [x] Missing, non-executable, duplicate, malformed, and hostile targets are
      rejected or skipped diagnostically.
- [x] Chrome and command-palette routes expose primary and individual targets.
- [x] Controlled X11 and Wayland journeys exercise the staged real product.
- [x] Every discovery, failure, repair, and limitation is recorded while work
      proceeds.

## Qualification language

Passing this slice means only that its implemented local cells pass. It is not
release or full Linux qualification while the matrix contains BLOCKED, XFAIL,
or NOT_IMPLEMENTED entries.
