# Linux bookmarks and presets feature plan

Date: 2026-08-11
Issue: GH-18 (`workspace.bookmarks-presets` subset)

## Outcome

Linux users can capture the focused worklane as a bookmark or preset, inspect
and edit what will reopen, persist it under the XDG configuration directory,
restore it as a new worklane, update or unlink a linked bookmark, and exchange
portable `.zenttypreset` files. This ports the source feature rather than
creating a second session-restore mechanism.

## Source contract

- A bookmark retains per-pane local working directories and derives a common
  project root. A preset retains the topology and commands but strips the
  project root and all working directories.
- Capture retains worklane title/color, column widths, pane-height proportions,
  focused and last-focused identities, custom pane titles, remembered titles,
  safe environment overrides, and detected live commands. Shell processes are
  not captured as commands; a running non-shell child may be.
- Restore allocates fresh worklane, column, and pane identities and remaps all
  focus references. It links the new worklane to the template ID.
- A missing directory falls back explicitly to the chosen local directory or
  home and produces a restore warning. A missing command is inserted as
  unsubmitted terminal text; it is never auto-executed.
- Multi-column widths scale from the captured readable width. A single column
  fills the current readable width. Pane-height proportions remain normalized.
- Store operations include create/upsert, rename, pin, duplicate with a unique
  name, use recency, delete, update from the linked worklane, edit, convert,
  unlink, search, and deterministic pinned/recency/name ordering.
- Export always produces a preset, strips working directories and reserved
  runtime environment keys, and resets imported identity, pin, and recency.
  Future envelope schema versions are rejected rather than downgraded.
- Persistence uses the existing `AtomicFileStore` transaction boundary and one
  `bookmarks.json` owner. Writes are bounded, locked, durable, and atomic;
  corrupt input is preserved once and replaced only by an explicit mutation.
  The source's dotfile-symlink behavior must be preserved only if the final
  target can be resolved without weakening ancestor, ownership, or replacement
  safety. Otherwise the UI must report the unsupported link rather than write
  through it silently.

## Ownership and anti-accretion rules

- `zentty-core` owns the source-compatible template model, capture/import/export
  policy, ordering, and transactional store. It reuses `WorkspaceRecipe`,
  `WorkspaceState`, and `AtomicFileStore`; it does not create another live
  workspace or persistence abstraction.
- One focused Linux runtime module owns XDG path resolution, live foreground
  process/CWD capture, command availability, identity allocation, and UI action
  coordination. `ApplicationShell` delegates to it.
- Existing Ghostty pane creation, workspace rendering, state snapshots, and
  restore-draft systems remain the only owners of those lifecycles. No Ghostty
  change is expected.

## Test-first construction order

1. Port source fixtures and pure model tests: JSON shape, bookmark/preset
   stripping, common-root behavior, environment filtering, capture semantics,
   fresh-ID remapping, width/height healing, missing CWD/command fallbacks,
   ordering, duplication, conversion, and import/export.
2. Extend the existing atomic-store boundary with focused tests for concurrent
   updates, size limits, corruption preservation, future schemas, symlink
   behavior, stale-reader writes, and failure atomicity. Mutation-test the
   policy modules with the governed disk-safe runner.
3. Add source-owned GTK chrome/sidebar/popover and edit/save routes. Exercise
   the staged product with physical input to save a real multi-pane worklane,
   inspect the actual JSON, quit, restart, restore as a new worklane, and verify
   real Ghostty PTYs, topology, CWDs, commands, linkage, update, and unlink.
4. Add controlled missing-directory and missing-command journeys, real
   import/export files, corrupt/concurrent store cases, and X11 plus Wayland
   compositor coverage. Environmental absence is not success.
5. Update the feature inventory, ApplicationShell ownership contract, and
   authoritative qualification matrix; run every presently executable cell.

## Acceptance criteria

- [x] Source schema and capture fields are ported without lossy aliases.
- [x] Bookmark and preset semantics differ exactly as documented.
- [x] Store mutation and ordering behavior is deterministic and durable.
- [x] Import/export policy is portable, bounded, and strips unsafe context.
- [x] Restore allocates fresh identities and reports every fallback.
- [x] The GTK UI exposes save, activate, update, edit, duplicate, pin, convert,
      delete, import/export, and unlink with source terminology.
- [x] A standard keyboard context-menu route physically proves rename, edit,
      duplicate, pin, convert, delete, linked update, and unlink against the
      staged product in controlled X11.
- [ ] The identical management journey passes under controlled Wayland. Cage's
      second-transient input defect is an explicit GH-18 XFAIL.
- [ ] Real GTK chooser journeys export an actual `.zenttypreset`, delete the
      live record, import the exported file, and prove the portable persisted
      result in controlled X11 and Wayland. Both portal-backed journeys are
      explicit GH-18 XFAILs rather than environmental passes.
- [x] One real staged-product journey crosses save, quit, restart, restore, and
      terminal readiness with no product-only fixture route.
- [x] X11 and Wayland controlled journeys pass, focused policy mutation passes,
      and every presently executable matrix cell is rerun.

The authoritative matrix promotes controlled X11 management to PASS and keeps
the three reproducible compositor/portal defects executable as XFAILs. UI
exposure alone is not end-to-end evidence, and GH-18 remains open until those
tracked journeys pass.

## Native GTK closeout — 2026-08-12

The remaining import/export blockers are specific to `GtkFileDialog`'s portal
delegation, not to Zentty's portable-file model. Linux will use one in-process,
transient-for-parent GTK file chooser for this application-owned file format.
This is the native GTK platform implementation of the source macOS save/open
panel contract; it is not a product-only test route and does not invent a file
browser. The existing real-file journey stays authoritative.

Construction order:

1. Keep the controlled X11 and Wayland import/export cells RED. Remove their
   private D-Bus/portal assumption and require a real mapped GTK chooser owned
   by the Zentty window.
2. Replace only the chooser coordination boundary; retain the existing bounded
   import/export envelope, atomic store, extension, diagnostics, and action
   receipts.
3. Drive Save/Open with compositor input in both environments and prove the
   exact exported bytes are deleted, re-imported, and persisted as a portable
   preset.
4. Close the independent second-transient Wayland management XFAIL without
   adding sleeps, retries, or product test modes.
5. Promote cells only after focused wrapper tests and a complete local
   qualification run.

## Qualification language

Passing this slice means only that its implemented local cells pass. It is not
release or full Linux qualification while the matrix contains BLOCKED, XFAIL,
or NOT_IMPLEMENTED entries.
