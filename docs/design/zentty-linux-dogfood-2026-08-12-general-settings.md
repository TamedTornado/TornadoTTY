# Zentty Linux General settings dogfood — 2026-08-12

Tracking: GH-20

## Planned feature

The authoritative implementation and test order is
`docs/design/linux-general-settings-feature-plan.md`. This record is append-only
for discoveries, failed receipts, repairs, and remaining uncertainty.

## Initial audit

- The source General page has thirteen distinct settings, not a generic handful
  of preferences: four lifecycle policies, eight clipboard switches, and one
  three-level aggressiveness selector.
- Linux already parses and uses all nine clipboard values through the real
  `ClipboardConfig` and `CleanCopyOptions`, but exposes no settings controls or
  general write transaction for them.
- Linux does not yet model the source confirmations or restore preference in
  `AppConfig`. Session restore currently defaults on unless the process receives
  `--no-session-restore`; destructive pane/window/quit requests currently execute
  without consulting source confirmation policy.
- Therefore replacing General with switches before adding these model/effect
  owners would create decorative controls. Tests and config ownership come first.
- This slice reuses the existing `ConfigStore`, `ApplicationShell`, persistence
  coordinator, action registry, and clean-copy pipeline. No parallel settings,
  clipboard, restore, or confirmation system is authorized.

## First implementation receipts

- `AppConfig` now models the exact source confirmation and restore keys alongside
  the pre-existing complete clipboard model. The focused core suite passes 17
  tests, including defaults, all false values, unknown-key compatibility, and
  rejection of each wrong known type.
- The first General write transaction preserves a real final config symlink,
  comments, root/clipboard unknown keys, and an unrelated Appearance table; it
  writes all thirteen values, reparses to the exact requested model, and leaves a
  `0600` target. Malformed TOML and a non-table owned section are rejected without
  replacement. Both focused persistence tests pass.
- Strict Clippy rejected `MessageDialog` because GTK 4.14 deprecates it. The
  confirmation route now uses GTK's current `AlertDialog` async choose API rather
  than suppressing the warning.
- The first staged X11 settings journey passes with the real General page in
  place, followed by the existing Shortcuts and Appearance journeys and live PTY
  reload checks. This is a compatibility receipt, not yet acceptance of the new
  controls: dedicated physical control, confirmation cancel/accept, and restore
  two-launch journeys are still required.
- Current unit receipts: all 183 Linux binary tests and all 17 focused core config
  tests pass; strict package Clippy passes. No commit or qualification claim has
  been made while the real General journeys and architecture reconciliation are
  incomplete.

## Physical-input and lifecycle discoveries

- The first physical General journey sent `Tab` directly to a Ghostty-focused
  window and never entered the settings focus chain. The repaired journey first
  focuses the real sidebar search entry with `Ctrl+F`, then traverses the actual
  GTK widgets. A second failure showed that 20 ms synthetic key pacing could
  queue one extra `Tab`: the log reached `always-clean-copies` but focus moved to
  the next switch before Space. Pacing is now 150 ms and both X11 and the private
  Cage/Wayland journey physically toggle Always clean copied content on and back
  off, checking the action receipt and the real persisted TOML after each edge.
- The first pane-confirmation journey exposed a product bug, not a harness bug:
  pane-local Close Pane and Task Manager End Task called `close_pane` directly,
  bypassing the new focused-pane confirmation route. All three pane-close entry
  points now converge on `request_close_pane`; there is still one close owner and
  one surface-disposal implementation.
- After accepting an `AlertDialog`, GTK invokes the response callback before the
  modal is completely removed. Closing and immediately restoring Ghostty focus
  inside that callback left later physical input outside the new pane. Accepted
  actions now run on the next GLib main-loop turn. The source UX journey proves
  Escape preserves the same real pane-4 PTY by sending text through it and
  observing a fresh OSC title, then accepts the second dialog with Return and
  completes every real pane teardown. The corrected X11 source UX journey passes.
- The consolidated multi-window journey now begins with confirmations enabled.
  It cancels and then accepts Close Window, proving the second PTY survives the
  cancellation and the first PTY survives the accepted close; it then cancels
  and accepts Quit, again proving live PTY input between the two dialogs. The
  corrected X11 journey passes. Unrelated lifecycle journeys explicitly persist
  all three confirmation values as false rather than silently assuming old
  defaults.
- The consolidated restore journey now preserves a qualified clean snapshot,
  persists `restore_workspace_on_launch = false`, launches a fresh real pane
  without mounting saved panes or background agents, restores the exact snapshot
  bytes, persists the value true, and resumes the ordinary source-compatible
  restore journey. The corrected X11 journey passes, including its existing real
  loopback SSH, agent, crash, corrupt-state, and physical-input coverage.

## Mutation and boundary receipts

- The first Linux mutation attempt failed its unmutated baseline because the
  sandbox forbade the pre-existing real `/proc` listener test from opening a
  socket. The rerun used the real-kernel environment rather than weakening that
  test. The General widget/model file initially had four surviving mutants: a
  negated control-update branch and the three dropdown-index arms. Boundary
  conversions were factored into pure functions and tested exhaustively. The
  repaired focused result is 42 mutants: 36 caught, 6 unviable, 0 missed, 0
  timeout.
- The first focused General persistence mutation run found six survivors: the
  missing-file read guard and both sides of the exact input-size comparison.
  New tests distinguish a missing file from a real permission-denied read,
  accept an exact 1 MiB source whose complete owned tables do not grow on write,
  and reject one byte more without replacement. The repaired exact-function run
  is 8 mutants: 7 caught, 1 unviable, 0 missed, 0 timeout.
- The broader `app_config.rs` mutation receipt caught every new confirmation,
  restore, clipboard, and aggressiveness mutation. Four older surviving mutants
  remain in server-detection normalization, outside this feature's functions;
  they were not mislabeled as General coverage or hidden by narrowing tests.
- The first full-workspace test command was mistakenly run inside the restricted
  sandbox. Eight real Unix-socket CLI tests failed with `Operation not
  permitted`; the same suite passed in the real-kernel environment. The later
  all-target strict Clippy gate then found four test-code issues (an unnecessary
  raw-string hash, default-field reassignment, two unchecked `u64` conversions,
  and a boolean equality assertion). All were repaired rather than allowed, and
  the strict workspace Clippy, ShellCheck, formatting, and architecture gates
  subsequently pass.

## First full-matrix run and repairs

- The first complete matrix run executed every presently executable cell but
  correctly failed the implemented-local claim: 109 execution outcomes passed
  and six failed. Three failures (`rust-pane-search` under both product/API
  routes and both Task Manager backends) were existing journeys that exercised
  quit or End Task while silently assuming the former no-confirmation default.
  They now explicitly persist disabled lifecycle confirmations because their
  target behavior is search and task routing; the dedicated lifecycle journey
  remains the authority for enabled confirmation behavior. Both corrected Task
  Manager backends pass, and the pane-search cells consume the same helper/config
  policy on the matrix rerun.
- The source UX cancellation proof was timing-sensitive under matrix load: GTK
  logged the modal cancellation and restored pane-4 focus, but the harness typed
  before that focus receipt settled. It now synchronizes on the latest real
  `focus-pane pane=pane-4` receipt before sending the PTY marker. The corrected
  real X11 journey passes.
- The controlled Wayland agent cell failed once before its first authenticated
  event; an immediate isolated rerun passed its complete Codex/Claude/Gemini
  real-PTY and real-Unix-socket journey. This is recorded as a transient, not
  converted into a pass or hidden. The complete matrix must rerun cleanly before
  commit.
- The second matrix rerun passed the Wayland agent and both Task Manager cells,
  but exposed two incomplete repairs. `rust-pane-search` wrote its clipboard-only
  fixture after the new confirmation fixture and therefore erased the latter;
  the one authoritative fixture now contains both sections. The source UX focus
  wait could match the cancellation callback's pre-existing focus receipt; it
  now requires a strictly newer pane-4 focus receipt after the physical click
  and sends input through the focused XTest keyboard. The strengthened standalone
  real X11 journey passes. Neither failure was accepted as environmental.
- The third matrix rerun cleared both pane-search cells, but matrix concurrency
  exposed two further missing synchronization edges. The General journey waited
  for its action log but read TOML before the atomic write completed; it now
  waits for the requested bytes. The arranged pane journey waited for
  `terminal-ready` but not post-dialog terminal focus; it now physically clicks
  pane-5 and requires a new pane-specific focus receipt before typing. Both
  corrected controlled journeys pass standalone. A final complete matrix rerun
  remains mandatory.
- The fourth matrix rerun passed every cell except source UX. Its evidence showed
  pane-5 `terminal-ready` preceded the scheduled horizontal scroll; the physical
  click therefore still struck pane-2 under load. The journey now waits for the
  real scroll adjustment receipt and verifies pane-5 is the hover target before
  clicking it. Three consecutive controlled X11 runs pass this strengthened
  sequence. The complete matrix still must be green before commit.
- The fifth matrix rerun again isolated source UX. The wait for the exact scroll
  receipt was correct in value but not in generation: it matched pane-4's older
  identical `460/460` receipt. The journey now captures the receipt count before
  Add Pane Right and requires a strictly newer receipt before proving pane-5's
  hover target. This is counted synchronization in the existing journey, not an
  added test layer.
- The sixth matrix rerun proved pane-5 was initialized, scrolled into view, and
  the real hover target, but clicking the already-selected surface did not
  necessarily emit a new focus edge. A proposed initialized-callback product
  focus reconciliation made unrelated agent, multi-window, Open With, and
  settings journeys fail and was fully reverted; it would have changed product
  semantics to satisfy one journey. The source UX proof now physically focuses
  the adjacent visible pane-2, requires its new pane-specific focus receipt, and
  uses Zentty's real Ctrl+Tab traversal contract to return to pane-5 before PTY
  input. This tests a user-reachable focus route without adding product behavior.
- The seventh complete run returned all feature cells to PASS except the source
  UX focus route and one unrelated Bookmark dialog startup timeout. The Bookmark
  cell had passed every preceding complete run; it remains a recorded transient,
  not a qualification pass. The final source UX route then passed five
  consecutive controlled X11 runs. The checked-in machine summary therefore
  still correctly says the implemented local suite is failed; this slice does
  not claim exhaustive or release qualification from those follow-up receipts.
