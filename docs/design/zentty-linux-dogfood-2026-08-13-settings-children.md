# Zentty Linux dogfood — remaining settings children

Date: 2026-08-13
Issues: GH-37, GH-38, GH-40

The authoritative construction and acceptance order is
[`linux-settings-children-completion-plan.md`](linux-settings-children-completion-plan.md).
This report is append-only while the batch is implemented.

## Starting audit

- GH-41 is implemented and pushed as `4a3b450`. Its final local qualification
  passed all 126 declared PASS cells with suppression governance accepted; it
  does not claim release or full Linux qualification. Public closure remains
  deferred until the requested cross-issue completion audit.
- GH-37 already has a source-shaped page, shared config fields, live placement,
  adaptive right-pane behavior, labels, borders, project icons, and guarded
  focus-follow behavior. Its journey proves only one initial configuration and
  one pointer path; it does not yet drive every available control, threshold
  boundary, restart, multiple windows, or continuity contract. Smooth scrolling
  and inactive opacity are deliberately visible but unavailable because their
  qualified Linux rendering paths do not exist.
- GH-38 already uses the #18 discovery/launch authority and its journey proves
  real desktop, executable, terminal, and SSH boundaries. The page has native
  add/remove and primary controls, but the existing journey drives only a
  checkbox. Application disappearance while the page is open, visible stale
  reconciliation, no-target state, native add/remove, primary selection, and
  write-failure rollback are not yet proven.
- GH-40 already projects teams and three authenticated ephemeral wrappers while
  preserving every unsupported source integration as unavailable. It does not
  yet distinguish requested versus observed active state, expose consent/error
  feedback, refresh external changes, or satisfy the complete isolated
  integration lifecycle journey. Status-item and sleep inhibition remain named
  platform gaps and must not be represented as implemented.

## GH-38 Open With completion

- The page previously reconciled only when it was first presented. A custom
  executable deleted while Settings remained open therefore remained visible
  until another application action happened. The page now owns only a refresh
  request; discovery, normalization, persistence, menu projection, and palette
  projection still cross the existing #18 runtime and ConfigStore authorities.
  Removed IDs are surfaced in page status instead of disappearing silently.
- Native add/remove, enable/disable, primary selection, external disappearance,
  explicit no-target presentation, restart, exact desktop/custom/terminal argv,
  and real SSH rejection now run in the existing `rust-open-with` staged-product
  journey. Its temporary XDG desktop catalog uses actual `.desktop` metadata,
  executable files, GTK's native file chooser, a real Ghostty pane, and the
  existing disposable OpenSSH actor. Nothing in the product recognizes a test
  scenario.
- The first expanded X11 run found that `Alt+P` focused the primary dropdown but
  did not open it. Searchable native dropdown behavior and explicit Home/End
  keyboard selection were added for accessible deterministic navigation. The
  real X11 path selects the rendered row through XTEST pointer input; nested
  Wayland reaches the same control through virtual-keyboard focus traversal and
  changes it with End.
- That selection exposed a real GTK lifecycle defect in Zentty: the
  `selected-notify` callback persisted the new primary and immediately rebuilt
  the dropdown model while GTK was still closing its popover. The product
  segfaulted in the real X11 journey. Primary-only changes now update the
  accepted config and runtime transactionally without rebuilding an unchanged
  catalog. Repeated selected notifications for the already accepted ID are also
  ignored. Both compositor journeys pass after this repair.
- Several early X11 harness attempts used `windowfocus` in a controlled Xvfb
  session that intentionally has no window manager. That created intermittent
  `BadMatch` failures unrelated to product behavior. The journey now sends
  targeted XTEST keyboard/pointer input to proven product toplevels and does not
  convert focus absence into a pass. It also resolves the largest real Settings
  toplevel because GTK transient child surfaces can share its title.
- The no-target exercise initially raced its second external edit against the
  config authority's first rebuild. The repaired journey waits for the full
  accepted live-projection receipt, not merely the page's early no-target log,
  before the next edit. This preserves real watcher behavior without sleeps as
  assertions.
- Final focused evidence: controlled X11 and nested Wayland journeys both pass;
  formatting, strict package Clippy, focused Rust tests (2/2), ShellCheck, and
  the architecture/negative contracts pass. The governed mutation shard kept
  `.cargo/mutants.toml` copy safety and tested four policy mutants: **3 caught,
  1 unviable, 0 missed**. The remaining unviable mutant cannot compile and is
  not a surviving behavior.
