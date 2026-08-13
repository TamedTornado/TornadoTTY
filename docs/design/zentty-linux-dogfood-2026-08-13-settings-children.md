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

## GH-37 Worklanes & Panes completion

- The page formerly mutated its in-memory projection before persistence and
  could therefore display a value that the live configuration authority had
  rejected. Every available control now proposes a complete typed snapshot,
  accepts it only after `ConfigStore` succeeds, and presents persistence errors
  without inventing a second settings authority.
- The focused journey was expanded from one configured action to two real
  application windows, three retained shell processes, real Ghostty surfaces,
  every available page control, exact persisted values, restart, surface/PID
  continuity, and topology non-mutation. It also proves focus-follow through a
  physical compositor pointer and proves that another window, the command
  palette, and Settings prevent pointer-driven terminal focus theft.
- The first physical X11 attempts exposed two harness defects rather than
  product defects. Xvfb has no window manager, so untargeted focus assumptions
  could leave input on the wrong toplevel; native `GtkDropDown` also owns a
  transient popup and keyboard grab rather than a child X11 window. The final
  journey targets the proven Settings toplevel, explicitly opens each native
  dropdown, and selects the rendered popup row through XTEST pointer input.
- The original controlled X11 display was only 2560 pixels wide. That made the
  source's largest 2560-pixel *content* threshold physically impossible once
  Zentty's 280-pixel sidebar was included. The private display is now
  3200x1800, and the journey waits for actual resize receipts before exercising
  both sides of all five boundaries: 1200, 1440, 1680, 1920, and 2560 pixels.
  Core policy tests independently cover the exact below/at boundary and both
  non-adaptive modes.
- The nested Cage profile has a scoped virtual keyboard but no settings-window
  pointer injector. GTK's non-active Settings toplevel does not accept a
  subsequent virtual-keyboard selection after mnemonic focus, so environmental
  absence is not represented as physical-control success. The Wayland cell
  qualifies real presentation, exact projection, value preservation, deep
  linking, Ghostty surfaces, and restart; the companion controlled X11 cell
  owns every physical settings-control mutation and continuity invariant. This
  division matches the established Open With and Dev Servers qualification
  policy and remains explicit in the PASS receipt.
- Smooth scrolling and inactive opacity remain visibly unavailable with their
  reasons, and their original values survive all writes. Broadening these
  controls without a qualified Ghostty smoothing path or without reintroducing
  the rejected washed-out pane treatment would misrepresent capability.
- Final focused evidence: the controlled X11 journey passes every physical
  control, all threshold boundaries, multi-window projection, pointer guards,
  PID/surface continuity, and restart; the nested Wayland companion journey
  passes presentation/projection and restart. Strict workspace Clippy,
  formatting, ShellCheck, focused core tests (5/5), focused page test (1/1),
  architecture contracts and negative tests all pass. The governed mutation
  shard tested two adaptive-policy mutants: **1 caught, 1 unviable, 0 missed**.

## GH-40 Agents settings completion boundary

- The initial page counted three wrapper names as available even though the
  application runtime can only stage a wrapper when both its packaged executable
  and the corresponding real agent executable are present. The page now receives
  the observed wrapper inventory from the one existing `AgentRuntime`; config
  alone never produces an installed/ready claim.
- The source inventory was re-read rather than inferred from the scaffold. All
  eight persistent and eight ephemeral tools remain visible. Persistent defaults
  remain `ask`, ephemeral defaults remain `on`, stored overrides win, and every
  unavailable row retains and describes its requested state. In particular, a
  requested persistent `on` state explicitly says that no installed hooks are
  claimed. No Linux hook installer was fabricated inside Settings.
- Available Claude/Codex/Gemini switches use the existing authenticated-wrapper
  runtime and authoritative ConfigStore. Teams use the existing tmux-compatible
  agent runtime. Writes are transactional: rejected persistence restores the
  last accepted switch state and presents a visible error instead of leaving the
  page ahead of the runtime.
- The controlled X11 journey first exposed that testing directory permissions was
  not a deterministic write denial for an atomic-replace store. It now uses a
  malformed externally edited TOML document: the watcher retains last-good state,
  the physical switch write fails without replacing operator content, and the
  rendered control rolls back. Restoring valid TOML rearms the same watcher.
- A subsequent run exposed a harness race: the `settings-live-refresh` receipt is
  emitted before the replacement page finishes construction, so the actor could
  search for a transient toplevel too early. It now waits for a strictly newer
  `agent-settings loaded` receipt before operating the refreshed page. This is a
  product-state barrier, not a delay accepted as proof.
- The completed X11 path uses a real staged ReleaseSafe app, native Settings
  toplevel and physical pointer switches. It proves disabled and enabled wrapper
  launch state, real Codex executable selection, tmux-team environment, malformed
  write rollback, comment preservation, live external reconciliation, teams
  disablement, and a fresh real Ghostty/PTY pane receiving the externally restored
  wrapper state without a tmux session. The nested Wayland companion proves real
  page/deep-link presentation and state preservation; its scoped virtual keyboard
  cannot inject pointer input into a separate Settings toplevel, so it does not
  falsely claim physical switch mutation.
- Status-item/fleet presentation, sleep inhibition, and persistent-agent hook
  installation are not silently treated as GH-40 product capabilities. The page
  presents them as unavailable and preserves config. Their actual runtime
  authorities and real-system qualification remain owned by GH-21 and GH-7;
  closing this settings child must not close or promote those feature-inventory
  entries.
- The ownership audit found that `agents_settings.rs`, introduced with the first
  page scaffold, had not been included in the architecture hash set. The
  validator, its negative suite, and the settings-coordinator inventory now cover
  it, preventing this projection from accreting an unreviewed authority.
- Focused evidence before aggregate qualification: five page policy tests pass;
  strict package Clippy, formatting, ShellCheck, and architecture positive and
  negative contracts pass. The governed decision-policy mutation shard tested
  six mutants: **5 caught, 1 compiler-unviable, 0 missed**. Controlled X11 and
  nested Wayland journeys pass. Aggregate qualification is recorded below only
  after every final diff is staged and rerun.
- The first mnemonic run found two final-state defects. GTK activates a switch's
  mnemonic directly rather than necessarily emitting the focus-controller
  receipt, so the journey now proves a strictly newer persisted teams action
  instead of requiring an implementation-specific focus event. More importantly,
  the asynchronous notification caused a rollback to emit a second rejected
  write after the updating guard had cleared. Both team and integration handlers
  now ignore a notification equal to the last accepted state. The real malformed
  write path asserts exactly one failure, and the final X11 and Wayland journeys
  pass with the corrected build.
- The final workspace test command was first started inside the execution
  sandbox. Eight real Unix-socket CLI tests failed at socket creation with
  `Operation not permitted`; pure tests continued to pass. The identical locked
  workspace command was rerun with the required local IPC capability and all
  tests passed, followed by strict all-target Clippy. The sandbox receipt is not
  represented as a product failure or silently discarded.

## First aggregate rerun findings

- The first final aggregate did **not** pass and is not the final receipt. It ran
  in 527.18 seconds: 124 runtime PASS cells passed, while
  `architecture-contract-v1` and `workspace-pane-settings-wayland` failed; the
  declared matrix remained PASS=126, FAIL=0, BLOCKED=7, XFAIL=1, and
  NOT_IMPLEMENTED=22. Suppression governance was accepted, but no implemented-
  local, release, or full-qualification claim is made from that run.
- The architecture failure was correct: updating the authoritative settings gap
  without its deliberately duplicated architecture mirror caused the validator
  to reject drift. The mirror now contains the identical GH-7/GH-21 dependency
  statement, and both architecture positive and negative suites pass.
- The Wayland page failure was an activation race under aggregate load: Settings
  opened on window 2, while the actor expected its multi-window projection
  receipt for window 1 before continuing. The unchanged exact controlled
  Wayland journey passed immediately on rerun; no assertion or product behavior
  was weakened.
- The support suite also uncovered a deterministic stale fixture from GH-37:
  `nested-x11` had correctly grown its private display to 3200x1800 to exercise
  the 2560-pixel content threshold, but its wrapper self-test still required the
  former 2560x1440 argv and exited 89. The self-test now requires and reports the
  actual controlled dimensions, and it passes. This is precisely why support
  tests remain part of aggregate qualification rather than being treated as
  incidental harness code.
- The second aggregate also remained failed, with 125 runtime PASS cells and one
  unrelated Debug/X11 lifecycle failure. The real restored pane and prefill were
  both present, but Ghostty's native Debug logger interleaved bytes between the
  pane and text fields of one Rust stderr receipt. The actor had incorrectly
  assumed cross-runtime writes to a shared redirected descriptor were atomic.
  It now requires both unique receipt fields independently; it still requires
  the fresh pane identity, exact action CWD/prefill, three real PTYs, persisted
  launch context, and quiescent disposed callbacks. No product assertion was
  removed. The failed 533.11-second aggregate is retained as non-passing
  evidence and cannot authorize a completion claim.
- The third aggregate again retained 125 runtime passes and failed the same
  Debug/X11 lifecycle journey under the `io_uring` cell. This time native bytes
  split immediately after `pane=`, proving that even the first repair still
  assumed field-value adjacency. The final assertion requires the unique receipt
  prefix and the independently unique `<fresh-id> text=cargo test` suffix; the
  earlier action assertion independently binds that fresh ID to the exact CWD and
  prefill. This handles arbitrary one-record native insertion without accepting a
  missing pane, wrong command, fake PTY, or stale identity. The 533.89-second run
  remains failed evidence.

## Final aggregate qualification receipt

- The fourth aggregate ran every presently executable support and matrix cell
  against the final product and harness diff and passed in 534.39 seconds. The
  authoritative machine summary is `build/linux/qualification-summary.json`
  with SHA-256
  `be1c720343c424d75214fd01b770185d1abec19fb436b18cffa19d643cf9fe26`.
- The implemented local suite and product-boundary qualification passed. Release
  qualification and full Linux qualification did **not** pass because the
  authoritative matrix still contains explicit non-PASS cells; this receipt is
  not an exhaustive-QA claim. Declared totals are **PASS=126, FAIL=0,
  BLOCKED=7, XFAIL=1, NOT_IMPLEMENTED=22**.
- Debug IBus-focus Valgrind is **PASS with reviewed suppressions**, not an
  unsuppressed-clean result. The preserved raw receipt reports 427 errors in 427
  contexts, 6,240 definitely lost bytes, and 41,461 indirectly lost bytes. The
  paired suppressed receipt reports zero post-suppression errors, contexts, or
  leak bytes, with all 427 contexts explicitly accounted for by the reviewed
  effective suppression set. Suppression governance was accepted and includes
  the inherited Ghostty files. The retired C-host ReleaseSafe XFAIL was not
  reused as product evidence: the staged-product ReleaseSafe Valgrind cells
  remain explicitly NOT_IMPLEMENTED rather than being broadened into a false
  pass.
- The slowest real-system cells remained the upstream Ghostty regression
  (363.75 seconds), callback drop-order journey (82.58 seconds), ReleaseSafe
  build (75.78 seconds), Debug IBus-focus Valgrind (66.79 seconds), and X11
  agent integration (62.15 seconds). No timeout or environmental absence was
  converted into success.
