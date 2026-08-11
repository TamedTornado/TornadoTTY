# Zentty Linux dogfood — bookmarks and presets

Date: 2026-08-11
Issue: GH-18 (`workspace.bookmarks-presets` subset)

## Slice contract

The test-first contract is
[`linux-bookmarks-presets-feature-plan.md`](linux-bookmarks-presets-feature-plan.md).
This record is updated during construction, not reconstructed at the end.

## Source and architecture discoveries

- Source Zentty's bookmark is not a backup file and its preset is not a pane
  layout preset. Both are reusable single-worklane templates. A bookmark binds
  pane directories; a preset deliberately removes them.
- Source schema version 1 captures column and pane identities only so focus
  references can be remapped. Restore must allocate new runtime identities;
  reusing saved identities would collide with an already open template.
- The existing Rust `WorkspaceRecipe` already contains most topology fields and
  `WorkspaceState` is the sole live model. The template implementation must map
  through those types instead of introducing a parallel workspace model.
- The existing Rust `AtomicFileStore` already owns bounded locking, durable
  temporary replacement, corruption quarantine, and symlink rejection for
  persistent JSON. Bookmark persistence must reuse it rather than accrete a
  second atomic-write helper.
- The source store intentionally follows a final `bookmarks.json` symlink for
  dotfile setups. Linux's current shared store rejects symlinks. Preserving the
  source behavior safely requires a narrowly reviewed final-link resolution
  policy; silently weakening ancestor checks is not acceptable. This remains a
  design/test obligation, not an environmental pass.
- Source export converts bookmarks to presets, removes CWDs and project root,
  filters runtime-owned environment keys, and gives imports fresh identity and
  recency. Export is therefore a portability/security boundary, not a raw copy
  of `bookmarks.json`.
- Source restore never auto-runs a command that is no longer resolvable. It
  prefills the command for user review and reports the fallback alongside any
  stale working-directory fallback.

## Construction record

### Real capture exposed an ambient-environment secret leak

The first controlled staged-product save inspected `/proc/<pid>/environ` to
approximate source pane overrides. Its real JSON receipt immediately proved
that this was unsafe: the child inherited the operator session and the draft
store contained unrelated API credentials and other ambient values. The file
was confined to the disposable X11 environment and copied only to `/tmp` for
inspection; it was not committed. The implementation now captures only the
explicit environment attached to a Zentty pane launch, retained by the one
`PaneRuntimeCoordinator`; it never treats inherited process environment as
template configuration. Removal and cross-window transfer also move or clear
that metadata with the existing pane lifecycle. The controlled journey must
assert that a seeded ambient secret is absent from the persisted bookmark.

### Store no-ops must also be persistence no-ops

The initial transaction helper returned the correct `false`/`None` result for a
missing rename, pin, recency, duplicate, or delete target, but still rewrote the
unchanged JSON bundle. That diverged from the source store contract and created
unnecessary timestamp/disk churn. The shared locked mutation path now compares
the validated result with the locked input and returns `ReadOnly` when unchanged.
A regression test executes every missing-target operation and proves the store
bytes remain identical.

- The slice began only after Open With commit
  `e2fd1463d38e511ba63b12cb42efcbeadf56b66b` passed the full local entry point
  and was pushed. The bookmark plan was derived from the source
  `WorkspaceTemplate`, capture/import/export/store, popover, save sheet,
  sidebar action, and restore-banner implementations plus their Swift tests.
- The first Rust construction step added only behavior tests and failed to
  compile on the intentionally absent template API. The tests require capture,
  safe-environment filtering, bookmark-to-preset stripping, fresh identity and
  focus remapping, width scaling, stale CWD and missing-command fallbacks, and
  import through the existing `WorkspaceState`; implementation follows that
  boundary rather than a UI-first placeholder.
- The first implementation run passed capture and restore but failed the
  portable-environment assertion because the test treated `TERM` as generated.
  Source `templateSafeOverrides` deliberately strips `COLORTERM`, `PATH`,
  `ZDOTDIR`, `PROMPT_COMMAND`, `GHOSTTY_LOG`, `XDG_DATA_DIRS`, and every
  `ZENTTY_` key while retaining an explicit `TERM` override. The Linux test was
  corrected to the source boundary; filtering was not broadened to satisfy the
  mistaken assertion.
- Core persistence now uses `AtomicFileStore` transactions for every mutation,
  so two writers reload the latest locked bundle instead of overwriting a stale
  in-memory snapshot. Focused tests cover concurrent inserts, deterministic
  pinned/recency/name ordering, upsert, rename, delete, duplicate naming,
  record-use, one-megabyte bounds, corruption quarantine, future-schema
  preservation, and explicit final-symlink rejection. A new quarantine-only
  atomic action preserves malformed bytes without inventing an empty live file
  during read.
- Portable export/import uses the source envelope and always converts a
  bookmark to a location-free preset. Import resets identity, creation/update
  time, pin, and recency and refuses future or oversized payloads.
- Adding restored worklanes exposed that the live Rust `WorklaneState` had been
  relying on the separate recipe snapshot to remember `bookmarkOriginID`.
  Keeping that split would have made unlink/update behavior depend on parallel
  state. The origin now lives in the sole live worklane model and projects back
  into the recipe. A focused insertion test initially reused a pane ID from the
  existing lane and was correctly rejected as `DuplicatePane`; the fixture now
  supplies genuinely fresh runtime identities and proves insertion, activation,
  origin projection, and unlink through `WorkspaceState`.

### The first real capture fixture hid the child process it meant to prove

The first staged journey launched Zentty with `--command 'sleep 30'`. Ghostty
correctly exposed the wrapping `/bin/sh` as the foreground process, and the
shell filter correctly refused to persist it. Treating that as a capture bug
would have weakened the product. The journey now starts the ordinary real
shell and physically types and submits `sleep 30`; `/proc/<foreground-pid>/cmdline`
then proves the actual non-shell child is captured. A short wait was also
required after opening the GTK popover: physical input sent before GTK moved
focus could race the save control. The final harness waits on explicit focus
receipts rather than sleeping or assuming a fixed tab order.

### Synchronous popover replacement violated GTK lifecycle ownership

An initial successful save immediately rebuilt the sidebar while the activated
popover callback was still closing its widget tree. The real X11 log exposed a
GDK lifecycle critical. Store mutation remains synchronous, but GTK projection
is now refreshed once on the next default-main-context idle turn. This keeps
the existing sidebar renderer as the only projection owner without replacing
an active popover from inside its own signal callback.

### Source shortcut audit removed coordinate-dependent qualification

The source command registry defines “Show Bookmarks & Presets” as Command-
Shift-B. The first Linux journey had physically clicked a coordinate because
that source command was missed during the UI pass. Linux now maps the source
command to Control-Shift-B and the popover explicitly focuses its search field.
The same keyboard-only product journey therefore runs under private X11 and a
private input-capable Cage/Wayland compositor; it does not assume the operator
desktop, a compositor coordinate system, or an application-only fixture route.

### Stale restore warnings are product behavior, not log-only evidence

Core restore already returned distinct missing-directory and missing-command
fallbacks, but the first GTK integration merely printed them. A focused overlay
banner now names the missing directory and actual fallback and explains that an
unavailable command was inserted without execution. The controlled journey
mutates the legitimate persisted file between complete launches, physically
activates it, proves a real Ghostty PTY starts in the safe fallback directory,
proves the missing command is prefilled, and observes the visible warning
receipt. Environmental absence is not converted to success.

### Final bookmark-file symlinks can be supported without weakening the store

The shared atomic store continues to reject symlink paths. The bookmark entry
point alone resolves an already-existing final symlink once to a canonical
regular file, then applies the unchanged ancestor, lock, bounds, and atomic
replacement policy to that target. A focused test proves the link itself is
preserved and the target receives the transaction. Dangling links and links to
non-regular objects remain errors.

### Current real-product receipts

- X11 session `92d0c529af54008dd3048f4020e18117ffafbf2f90b61a4830bdb23271174cb4`
  passed physical save, private persistence, ambient-secret exclusion, full
  process restart, real PTY restore, recency, and both visible stale fallbacks.
- Wayland session `5011a2a38a9538689dbb742fa05117792c0b9f0c38435721b79e663ae018a963`
  passed the same journey under private Cage with virtual-keyboard input and a
  software renderer.
- Core model/store tests and all 159 Linux binary tests pass; strict workspace
  Clippy and the complete architecture validator pass.

The final clean `qualify-local` receipt SHA-256 is
`e1847641b8b505f9f9e57d44a7e4856b1449262c2607baf724a92de118a27309`.
Every presently executable support and matrix cell passed in 408.94 seconds.
Declared totals are PASS=107, FAIL=0, BLOCKED=7, XFAIL=1, and
NOT_IMPLEMENTED=25. This establishes implemented-local and product-boundary
qualification only; release and full Linux qualification correctly remain not
passed.

### Focused mutation exposed unasserted security and geometry boundaries

The first governed, disk-safe run (`gitignore = true`, `copy_target = false`)
tested 142 mutants: 97 were caught, 19 were unviable, 25 were missed, and one
duplicate-name mutation timed out by creating a non-terminating suffix loop.
The missed set was useful rather than waived. It exposed absent assertions for
final-symlink ancestor traversal and missing-path behavior, future nested
template schemas, exact import size, invalid IDs/names, error sources, duplicate
suffix progression, pane iterators, non-finite/zero capture widths, and every
invalid pane-height boundary. Tests now pin those behaviors. Duplicate naming
was changed from an unbounded incrementing loop to a finite search bounded by
the number of existing templates, eliminating the pathological mutant and a
real denial-of-service shape if naming assumptions were ever violated. A
redundant import schema branch and a semantically unused single-column scale
predicate were removed rather than adding tests that pretended equivalent
mutants were observable. The corrected governed rerun tested 139 mutants: 119
were caught, 19 were unviable, and one duplicate-name bound mutant survived.
That final arithmetic bound was replaced with checked addition, and a targeted
current-tree rerun caught all six duplicate-name mutants. No surviving or timed
out mutant remains in the focused policy set.

### Wayland repetition exposed dialog-focus timing, not capture loss

After the final UI rebuild, X11 passed but repeated Wayland journeys failed the
persisted-record assertion. The first diagnosis incorrectly inferred a
foreground-process race from the aggregate assertion and briefly added capture
polling. A field-level failure receipt disproved that theory: CWD and `sleep 30`
were exact, while the physically entered name was `rolled bookmark` instead of
`Controlled bookmark`. A first harness hypothesis blamed leaked synthetic
modifiers, but an explicit release did not repair the full qualifier. The real
cause was typing before the newly presented modal name entry acquired focus;
the last six letters arrived only after realization completed. The speculative
product polling and modifier workaround were both removed. The name entry now
emits the same real focus-state receipt already used by the popover, and the
physical journey waits for that lifecycle boundary instead of a 100 ms sleep.
The diagnostic receipt intentionally reports only the controlled persisted
fields, never environment values.

### Adding the source bookmark control exposed a narrow-sidebar regression

The whole qualifier also caught the sidebar allocating 227 px after a real
600 px external resize, violating the existing one-third clamp. The new fixed
bookmark button made the adjacent “New worklane” label's natural width a hard
minimum. That label now ellipsizes, preserving the source control while allowing
the existing sidebar-width owner to enforce 198 px. The established physical
resize journey remains the acceptance boundary; no bookmark-specific width
policy was added.

### The file-chooser journey was not allowed to become another brittle harness

An attempted expansion tried to drive export/import through the template row's
ellipsis menu using only Tab order. Real GTK correctly focused the template
activation button but did not include the sibling `MenuButton` in sequential
focus traversal; repeated Tab cycled through the row, save, and import controls.
Rather than retain guessed pointer coordinates or an application-only action
route, the expansion was removed. The implemented envelope and GTK actions stay
unit/typed-action covered, while four explicit X11/Wayland management and
import/export `NOT_IMPLEMENTED` cells now prevent the gap from disappearing.
The next slice must first establish an accessible, source-quality row-menu
keyboard route and then drive the real chooser.

### GH-18 closeout resumes with standard keyboard context menus

The remaining work is not a new bookmark subsystem. It closes the four
explicit matrix gaps using the existing store, action router, popover, and GTK
file dialogs. The row action menu will expose the conventional Menu/Shift+F10
route from the focused template row, with accessible focus receipts for robust
physical input. The same staged-product journey will then exercise management
mutations and real native chooser files under both controlled compositors. No
coordinate guessing or application-only test action is permitted.

### A second source audit caught incorrect conversion semantics before commit

The first Linux conversion action changed the selected record in place and, for
a preset, merely flipped its kind without binding any working directories.
That was not the source behavior. Source Zentty keeps the original: bookmark to
preset creates a fresh unpinned, location-free copy named “(preset)”; preset to
bookmark captures the currently focused live worklane into a fresh bound copy
named “(bookmark)” while carrying the preset color. The Linux coordinator now
does exactly that through the existing capture/store owners, and the row menu
uses the source verbs “Save as preset…” and “Bookmark in current worklane…”
instead of the lossy “Convert bookmark/preset” alias. Focused naming tests pin
the empty and non-empty source cases. This discovery is why the physical
management cells remain explicit rather than being inferred from action wiring.

The same audit found that the first GTK popover had invented persistent bottom
buttons labeled “Save bookmark” and “Save preset.” Source Zentty instead places
a compact plus menu beside search with “Bookmark Current Worklane…,” “Save
Current as Preset…,” and “Import Preset…,” and uses a richer empty state whose
primary action is “Bookmark current worklane.” The Linux projection now follows
that structure and wording. This was corrected before visual QA rather than
leaving an acknowledged source-UX deviation to harden into product behavior.

### The management journey found a real closed-dialog ownership cycle

The first complete management journey physically exercised the standard Menu
key route and caught two distinct defects instead of hiding them behind typed
actions. Search text survived long enough that appending a new filter could
leave a previously focused row selected; the harness now clears the real GTK
entry and asserts the exact focused template name, not merely any activation
row. The first edited command also exited immediately, invalidating the linked
update lifecycle, so the controlled command is now `sleep 30` and the journey
proves a live restored PTY before update and unlink.

More importantly, both bookmark dialogs held a strong reference to themselves
through their own button signal closures. Closing hid the window but retained
the object graph and native surface. Those captures now use GTK weak references
for both Cancel and successful Save. This is a product lifecycle repair, not a
test accommodation. X11 now passes physical save, rename, duplicate, pin, edit,
source-compatible conversion, duplicate deletion, real-PTY activation, linked
update, and unlink in one bounded staged-product journey. The final X11 receipt
is `Rust bookmark management passed: x11, context-menu=standard, ...` from the
2026-08-11 controlled rerun.

### Controlled Wayland exposed a compositor transient-input defect

The identical Cage journey reliably saves the first bookmark, opens the
standard keyboard context menu, presents the rename dialog, and reports GTK
focus on its prefilled entry. Modifier release, one-client replacement input,
modifier-free End/Backspace replacement, compositor-settle delays, weak dialog
ownership, and fresh application processes were each tested rather than
assumed. Cage still routes no replacement text to the second transient: the
submit receipt remains `changed=false chars=7`, the action is a safe no-op, and
the persisted store still contains `Managed`. Weston accepts input in the
first transient but does not reliably restore keyboard focus to the parent or
a replacement application surface in its nested X11 backend. This is now an
explicit GH-18 XFAIL, not an environmental pass or prose-only omission.

### Real chooser work requires both a portal and a focus-managing desktop

`GtkFileDialog` did not fall back to an in-process chooser in the sanitized
qualification session. With no session bus its future remained pending. A
fresh `dbus-run-session` and isolated portal preference activate the real GTK
portal backend. On X11, a controlled Openbox instance is also required; the
WM-free Xvfb profile cannot transfer keyboard focus to the out-of-process
chooser. With both present, the chooser maps as a distinct portal-owned window
titled “Export Zentty preset,” but the synthetic keyboard path still cannot
activate its Save control in the controlled session. The test therefore sees
no exported bytes and exits 1. Under Cage, the GTK portal explicitly reports
that it cannot associate its window with the Wayland parent, while the GNOME
portal backend crashes without a GNOME session. Both chooser cells are tracked
GH-18 XFAILs with real commands and expected failures; neither is claimed as a
pass. Temporary diagnostic screenshots were kept outside the repository and
all screenshot instrumentation was removed from the committed harness.

The first full-matrix rerun then exposed an orchestration boundary rather than
a product result: the X11 chooser XFAIL spent the entire 10-second generic
success wait proving that no export arrived, and its nested wrapper missed the
phase's evidence collection window before writing the environment receipt. The
chooser-specific wait is now four seconds; a focused rerun exits 1 and writes a
complete controlled-X11 report in eight seconds. The expected failure remains
the same and was not converted to success.

### Corrected qualification receipt

The corrected full local qualifier passed every presently executable support
and matrix cell in 418.35 seconds. The bookmark results are X11 management
PASS, Wayland management XFAIL, and both portal chooser journeys XFAIL. Declared
totals are PASS=110, FAIL=0, BLOCKED=7, XFAIL=4, and NOT_IMPLEMENTED=21. The
machine-summary SHA-256 is
`4371f94ca7019dab4f46d917be5e194539fa1d63014d6734af9a87c1c99e842a`.
Implemented-local qualification passed; release and full Linux qualification
correctly remain not passed. The Valgrind result remains PASS with reviewed
suppressions, never an unsuppressed-clean claim.

## Remaining work and uncertainty

- The GTK popover, save/edit/action surfaces, foreground-process capture,
  private XDG persistence, restart/restore, linkage, final-symlink policy, and
  stale fallback journeys are implemented. Controlled X11 management passes.
  Controlled Wayland transient input and physical `.zenttypreset` chooser
  completion remain explicit XFAILs before this inventory entry can move from
  `PARTIAL` to `IMPLEMENTED`.
- The save sheet currently captures first and offers pane editing afterward;
  source UX presents command disclosure during save. The persisted behavior is
  present, but this interaction difference remains a polish/parity review item.
- Bookmark initialization still treats an unreadable safe target as a window
  construction error. Corrupt content is quarantined safely, but the preferred
  user-facing recovery behavior for permissions failure remains uncertain.
- No release or exhaustive-QA claim is made. The authoritative matrix still
  contains its existing BLOCKED, XFAIL, and NOT_IMPLEMENTED cells.
