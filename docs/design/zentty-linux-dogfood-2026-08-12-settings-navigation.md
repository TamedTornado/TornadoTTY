# Zentty Linux settings navigation dogfood — 2026-08-12

Tracking: GH-20

## Feature slice and acceptance criteria

This slice ports the source application's settings information architecture
without claiming that every section's controls are already implemented.

1. The single settings window owns the exact nine source sections, exact source
   titles/subtitles, pinned and Workspace group ordering, and source keyword
   vocabulary.
2. Open Settings starts at General. The command palette exposes one unique deep
   link for every section. Reopening an existing settings window selects the
   requested section instead of constructing a second settings system.
3. Sidebar search filters sections by title, identifier, subtitle vocabulary,
   and source keywords. `Ctrl+F` routes to the active page's local search when it
   has one, otherwise to settings search.
4. Navigation has browser semantics: duplicate visits do not grow history,
   navigating after Back truncates Forward, and `Ctrl+[` / `Ctrl+]` plus the
   visible toolbar buttons replay without creating new entries.
5. Unported section controls remain visible and explicitly tracked rather than
   disappearing or masquerading as implemented. This slice does not close GH-20.
6. Pure navigation policy receives unit and focused mutation coverage. The real
   staged product must prove General default, keyword search, Shortcuts
   navigation, Appearance palette deep-link, Back/Forward, and the existing
   settings/persistence/reload journey under controlled X11 and Wayland.

## Implementation and dogfood record

- **Source audit:** `SettingsSection`, `SettingsSidebarLayout`,
  `SettingsNavigationHistory`, and `CommandPaletteItem.buildSettingsItems()` are
  the source authorities. Linux now preserves their nine identities, titles,
  subtitles, two groups, keyword intent, default General destination, deep-link
  shape, and browser-history semantics.
- **No parallel persistence system:** `settings_navigation.rs` is pure identity,
  filtering, and history policy. `settings_shell.rs` remains a GTK projection.
  Appearance and Shortcuts keep their existing focused implementations, while
  all writes still cross the existing ConfigStore/ApplicationShell boundaries.
- **First staged build failures:** an earlier command had yielded before the
  binary was actually rebuilt, leaving the old two-section strings in the staged
  executable. The explicit rerun exposed two compile errors (a GTK focus return
  type and callback borrow lifetime); both were repaired before any product
  claim.
- **Real crash discovery:** the first rebuilt X11 journey reached General, then
  `Ctrl+2` aborted the process. `apply_selection` held a mutable `RefCell` borrow
  while `ToggleButton.set_active` synchronously emitted `toggled`, whose callback
  borrowed the same state. Selection now snapshots GTK objects and history flags,
  releases the model borrow, then updates widgets. The real X11 journey no longer
  panics.
- **Physical-key harness discovery:** `xdotool` rejects the display spelling
  `ctrl+[` even though GTK calls the key `bracketleft`. The journey now sends the
  real X11/Wayland key names `ctrl+bracketleft` and `ctrl+bracketright`; no direct
  model call replaced physical input.
- **Controlled compositor receipts:** after those repairs, the consolidated real
  settings journey passed under private Xvfb and nested Cage. It proves default
  General, navigation to Shortcuts, palette deep-link to Appearance, browser
  Back/Forward, real user-theme precedence, persistence, native Ghostty reload,
  and an uninterrupted PTY. Keyword-search evidence is added before final
  qualification below.
- **Authoritative registry baseline failure:** the first focused mutation run
  correctly refused to mutate because the unmodified tree failed its action
  registry test: the newly registered string action increased the total from 107
  to 108 and was absent from the exact ordered string-action receipt. The test was
  updated to name the new action rather than merely weakening its count. The
  complete 180-test Linux binary then passed this contract.
- **Mutation discoveries:** the first valid 47-mutant run reported seven misses:
  the exact section titles/subtitles and a successful Forward traversal were not
  independently asserted. Tests now pin every source ID, title, and subtitle,
  exercise subtitle search, and traverse Forward before testing history
  truncation. The final focused run tested 48 mutants: 44 caught, 4 unviable,
  zero missed, zero timed out.
- **Final controlled feature receipts:** after adding real sidebar keyword search
  to the journey and rebuilding the staged binary,
  `rust-shortcuts-settings-x11` and `rust-shortcuts-settings-wayland` both passed.
  The latter ran inside the repository's private Cage Wayland environment; neither
  compositor's absence or desktop state was converted into a pass.
- **Ownership reconciliation:** navigation policy is now an explicit owned module
  under the existing settings coordinator. The architecture contract validates
  its source hash/function inventory, the changed settings sources, the one new
  typed deep-link action, and the new ApplicationShell delegation method. Its
  positive contract and negative self-tests pass.
- **Strict lint decomposition:** strict Clippy rejected the first shell constructor
  at 175 lines. Rather than suppressing the design signal, widget construction,
  callback connection, typed action creation, and physical-key routing were
  split into focused helpers under the same settings owner. The strict workspace
  Clippy gate then passed.
- **First full-matrix failures:** the first complete rerun exposed three harness
  assumptions rather than hiding them. Adding General Settings' source keyword
  `copy` changed the existing pane-search palette result count from four to five;
  both X11 and Wayland receipts correctly rejected the stale exact count. The
  Copy action remains the exact-title top result, so its real clipboard journey
  now pins five results. Under concurrent qualification load, Wayland delivered
  only the first `A` of `Appearance Settings` at the prior 20 ms physical typing
  cadence. The controlled Wayland helper now uses a conservative 50 ms cadence;
  it still sends real compositor key events and does not invoke the action model
  directly. Exact failed cells are rerun before the complete matrix is repeated.

The complete presently executable matrix receipt, including raw and reviewed
Valgrind totals, is appended only after the final qualification run below.

## Final qualification receipt

- The repaired X11 pane-search, Wayland pane-search, and Wayland settings cells
  each passed in isolation. The complete matrix was then rerun from preparation
  through packaging and finished in 447,190 ms with every presently executable
  support and product/dependency cell passing.
- Declared matrix totals: **PASS=115, FAIL=0, BLOCKED=7, XFAIL=1,
  NOT_IMPLEMENTED=23**. The implemented local suite and product-boundary
  qualification passed. Release qualification and full Linux qualification did
  **not** pass because the declared non-PASS cells remain visible.
- Debug Valgrind is **PASS with reviewed suppressions**, not an unsuppressed-clean
  claim. The preserved raw receipt reports 427 errors/contexts, 6,160 definitely
  lost bytes, and 41,396 indirectly lost bytes. The reviewed post-suppression
  receipt reports zero errors/contexts and zero definite/indirect bytes, with 427
  errors/contexts suppressed. Suppression governance passed and the effective set
  includes both inherited Ghostty suppression files and Zentty's project file.
- ReleaseSafe Valgrind remains visibly NOT_IMPLEMENTED for the staged Rust
  product; the retired C-host XFAIL is not reused as evidence and no suppression
  was broadened to manufacture a pass. The async-backend ABI representation cell
  remains the matrix's one XFAIL.
- Workspace tests, strict all-target Clippy, formatting/diff checks, architecture
  positive and negative contracts, the final 48-mutant focused run, private X11,
  and nested Wayland settings journeys all pass for this commit candidate.

## Explicit remaining scope

General, Notifications, Updates & Privacy, Worklanes & Panes, Open With, Dev
Servers, and Agents are present in the source navigation map but their Linux
settings controls are still explicitly incomplete. Their pages say so. This is
preferable to silently omitting source features, but it is not acceptance of
those GH-20 criteria. Subsequent feature slices must replace each status page
with real controls and real-system persistence/effect journeys.
