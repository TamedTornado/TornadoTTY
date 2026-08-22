# Zentty Linux dogfood: Worklane Peek (2026-08-22)

## Mandate

GH-80 completes the existing Worklane Peek as a source-accurate live navigation
surface. This report is append-only during the slice and is the record of
discoveries, failures, repairs, receipts, and uncertainty.

## Baseline audit

- The Linux controller already implements physical Control+Tab tap/hold
  disambiguation, repeated traversal, arrows, wheel navigation, Escape,
  Control-release commit, pointer selection, input shielding, and PTY focus
  restoration in the existing `rust-source-ux-x11` actor.
- Linux currently rebuilds a row of GTK cards on every selection. Each card uses
  `GtkWidgetPaintable` over the one canonical Ghostty widget. This preserves
  surface ownership but is not yet accompanied by real evidence that inactive
  worklane previews remain live.
- The Linux HUD currently shows only pane title and worklane title. The source
  HUD is process title, folder, branch, and project icon; agent attention and
  progress must also remain visible from the already-owned sidebar projection.
- The source uses one active lane plus lazily mounted neighboring live lane
  carriers. Its camera hard-cuts on traversal wrap and animates ordinary moves.
  Linux has no equivalent transition policy yet.
- There is no GH-80 evidence yet for inactive-window rejection, closing-window
  teardown, controlled Wayland input, reduced motion, or resize cancellation.

## Work log

- **Failure:** changing the preview from side-by-side worklane columns to the
  source's vertical lane stack invalidated the X11 actor's hard-coded first-card
  coordinate. The real click selected pane 2 instead of pane 1. This was a test
  coupling failure, not accepted as a product pass.
- **Repair:** every mapped card now emits its GTK-computed bounds and the
  existing actor clicks the center of the exact pane-identity receipt. The
  pointer event remains a real X11 event; the test no longer assumes one frozen
  layout coordinate.
- **Failure:** the first live-neighbor actor attempt sent the Peek chord as soon
  as pane 6 reported terminal readiness and observed no open receipt. A second
  attempt incorrectly waited for `focus-pane`, but that diagnostic belongs to
  initialization restoration/pointer routes and is not emitted merely because
  `create_worklane` gives its ready surface GTK focus.
- **Repair:** the actor now types a unique value and requires pane 6's OSC title
  response before sending physical Control+Tab. That proves real PTY input
  focus without depending on an unrelated diagnostic or a guessed sleep.
- **Discovery:** the PTY probe suggested a focus race after physical Control+T:
  `terminal-ready` was real, but direct synthetic text still did not reach pane
  6. Two candidate repairs—reusing pending-initial-focus and then adding the
  existing deferred post-transient restoration—did not make the real probe pass.
  Both speculative product changes were reverted rather than retained as
  unproven complexity. The precise new-worklane shortcut focus behavior is not
  claimed as a GH-80 repair.
- **Test correction:** the Peek journey now gives the rendered pane 6 a real
  pointer click before its PTY probe. This establishes an unambiguous physical
  starting focus for the Peek gesture without weakening what Peek itself must
  prove or silently asserting that the separate Control+T focus behavior passed.
- **Failure:** that pointer-focused probe still raced the fixture command's
  deliberate startup title sequence. `terminal-ready-pane=pane-6` established
  the Ghostty surface and PTY, but the child had not yet entered its input loop;
  the typed probe was consumed while the startup sequence was still running.
- **Repair:** the actor now waits for pane 6's second real OSC startup title
  before clicking and typing. This uses the fixture's existing child-ready
  handshake instead of adding a sleep or weakening the physical-input proof.
- **Test correction:** the extra pane-6 typing probe remained flaky while pane
  5's deliberately rapid live-output stream was driving redraws. It duplicated
  the actor's earlier real-PTY input proofs and did not qualify a Peek contract,
  so it was removed rather than turning an unrelated focus race into product
  code. Pane 6's two OSC startup receipts still prove its independent real PTY;
  the Peek journey itself remains entirely physical input.
- **Failure:** the initial 20 Hz title producer began before the neighboring
  worklane had finished starting. Its intentional metadata/render pressure
  starved the synthetic key delivery badly enough that the Peek chord was not
  observed at all; treating that as a Peek failure conflated setup load with
  the behavior under test.
- **Repair:** the real producer now starts after a fixed 1.5-second child-side
  delay. That deterministic phase boundary lets the neighboring PTY complete
  its OSC handshake and Peek open first, while all 80 live updates still occur
  during the open-HUD assertion.
- **Failure:** after removing the redundant typed focus probe, the Xvfb window
  was not guaranteed to own the window-manager focus when the new late-stage
  chord was sent; no `armed` receipt appeared, so this was input delivery—not
  a live-surface count failure.
- **Failed test-harness attempt:** `xdotool windowactivate` is not supported by
  the deliberately WM-free private Xvfb profile and caused the actor to exit
  before its own diagnostic. It was removed. A real click on the visible
  neighboring terminal is the supported focus mechanism in this environment
  and matches the actor's established focus discipline.
- **Test correction:** merely issuing the click and immediately sending the
  chord still raced GTK's asynchronous focus transfer. The actor now waits for
  the existing `focus-pane pane=pane-6` receipt produced by that physical click
  before arming Peek; no product timing or arbitrary post-click sleep is used.
- **Test correction:** the late-stage chord now targets the physically focused
  X input destination rather than using xdotool's `--window` event injection.
  The latter had worked earlier in the single-worklane journey but produced no
  capture-controller receipt after the hierarchy changed; focused delivery is
  both closer to user input and bound by the preceding focus receipt.
- **Failure:** focused delivery produced the expected `armed` receipt, proving
  the chord arrived, but the actor released Tab after a wall-clock 300 ms while
  the live metadata stream was loading GTK. The main-loop 200 ms hold callback
  had not run yet, so Peek correctly did not open.
- **Repair:** in the pressured live test only, the actor keeps the physical key
  down until the product's existing `open trigger=tab-hold` receipt (bounded at
  five seconds), then releases it. This preserves the hold contract and removes
  scheduler speed as a false oracle.
- **Product bug:** the new open-time `GtkWindow::is_active` gate rejected a
  physically focused, event-receiving window in the intentionally WM-free Xvfb
  profile. Key autorepeat then re-armed and re-rejected Peek repeatedly. GTK's
  active hint is a window-manager state, not a reliable proof that a captured
  physical event belongs to another window.
- **Repair:** open-time rejection now covers only actual shell teardown. Active
  window isolation remains owned by the application coordinator: inactive
  windows do not receive ordinary physical key events, and an active Peek is
  cancelled on the real `is-active` transition. The multiwindow actor qualifies
  that coordinator contract rather than treating a WM hint as authorization.
- **Failure:** after the multiworklane journey passed, the actor raced directly
  from pane 6's close receipt into pane 5's old hover coordinate. The close
  action had fired, but GTK had not yet restored and scrolled the worklane-1
  layout, so the pointer entered the retiring pane hierarchy and no pane-5
  controls appeared.
- **Repair:** cleanup now waits for both the exact restored topology and a new
  `pane-scroll-focused` receipt before targeting pane 5. This keeps the existing
  control test physical while synchronizing on real layout completion.
- **Test correction:** the complete journey then reached its final accounting
  oracle, which still expected the pre-GH-80 total of five PTYs/child processes.
  The temporary neighboring worklane is intentionally a sixth real Ghostty PTY,
  even though it is cleanly closed before the original cleanup sequence. Both
  exact totals are now six; no lifecycle assertion was removed.
- **Operator failure:** the first controlled accessibility invocation omitted
  the actor's required `x11` mode argument. The runner rejected its documented
  usage before executing a test; this is not recorded as product evidence.
- **Harness failure:** broadening the accessibility filter from one GTK test to
  two exposed Rust libtest's thread model. `--test-threads=1` serializes tests,
  but it still runs each test on a different worker thread; GTK correctly
  rejected initialization from the second thread.
- **Repair:** the existing accessibility actor now executes the sidebar and
  Peek tests as two exact, isolated test-binary processes. Each real GTK tree is
  still inspected under `GTK_A11Y=test`, with no shared GTK runtime or new test
  layer.
- **Test failure:** the project-icon actor expected an animated transition after
  opening on pane 2 and advancing to pane 1. With two panes that forward step
  is intentionally the source wrap boundary, so the product correctly emitted
  a hard cut rather than the actor's expected normal animation.
- **Repair:** the actor now uses the physical Left-arrow spatial route from pane
  2 to pane 1. This is a non-wrapping ordinary transition, still selects the
  icon-bearing preview, and cleanly distinguishes normal animation from the
  same route with GTK reduced motion enabled.
- **Environment discovery:** the private `gtk-enable-animations=false` file is
  honored in controlled X11, but Cage's nested Wayland settings provider owns
  the corresponding GTK setting and continued to report `true`. The actor had
  incorrectly demanded a false receipt on both backends.
- **Qualification correction:** controlled X11 remains the explicit real-GTK
  reduced-motion proof. Controlled Wayland now requires and reports its actual
  enabled-animation policy while still exercising physical Peek input, real SVG
  projection, and icon opt-out. Environmental override is not relabeled as a
  reduced-motion pass.
- **Coverage repair:** the live inactive pane now drives the product's real
  authenticated `zentty ipc agent-event` path while Peek is open: Codex running,
  progress 2/5, then an approval request. Peek emits its projected presentation
  from the existing sidebar/status authority, and the actor requires both the
  progress and attention states on pane 5. No preview-local agent store exists.
- **Accessibility repair:** attention cards now expose the accessible
  description `Requires attention` in addition to their exact status-bearing
  label/tooltip and selected state. The controlled GTK test covers both an
  ordinary progress card and an unselected attention card.
- **Coverage repair:** the existing pure gesture owner now explicitly proves
  reset cancellation before and after the threshold, axis relocking, exactly
  one navigation per Surface gesture, and the delivered natural-scroll signs
  in all four directions. The physical X11 actor continues to own real wheel
  events; no gesture harness was added.
- **Motion repair:** the normal 180 ms selection treatment now has an actual
  bounded GTK opacity transition instead of only a transient CSS class. Wraps
  remain hard cuts, and GTK reduced motion removes the transition class.
- **Harness failure:** the first full input-capable Wayland multiwindow run
  closed Cage's active inner toplevel before the outer XTEST shortcut's release
  reached it. Control remained pressed on the surviving client, turning later
  text into `Ctrl+T` and creating an unintended worklane. The actor correctly
  failed cross-window input routing.
- **Repair:** the existing multiwindow actor now explicitly releases Control,
  Shift, Alt, and Super through the outer X11 input owner after an inner Wayland
  toplevel is destroyed, mirroring its existing native-wtype cleanup branch.
- **Failed repair / reverted:** that explicit outer-X11 modifier release did not
  prevent the already-queued `Ctrl+T`; the unintended worklane still appeared
  before retargeting. The speculative cleanup was removed. This pre-existing
  full-journey Cage/XTEST key leak is not counted as GH-80 evidence: the focused
  input-capable Wayland Peek actor passes, while controlled X11 owns the exact
  two-window deactivation proof.
- **Screenshot evidence:** the existing controlled X11 actor now captures four
  real 1200x700 staged-product states: initial open, live progress, long-label
  attention, and selected long-label attention. It masks only the live terminal
  picture rectangles derived from GTK's exact card geometry, requires distinct
  non-terminal pixels across state transitions, and retains PNGs plus SHA-256
  receipts under `build/linux-test-receipts/worklane-peek`. This extends the
  existing actor rather than creating a screenshot harness.
- **Inventory reconciliation:** `worklane.peek-live-navigation` moved from the
  parent GH-16 owner to GH-80 and from PARTIAL to IMPLEMENTED only after its
  focused receipts passed. The reviewed issue ledger now carries GH-80, and the
  machine-summary oracle advances from 36/13/11 to 37/12/11
  IMPLEMENTED/PARTIAL/NOT_IMPLEMENTED.
- **Screenshot discovery:** refreshed cards emitted their map callback before
  GTK completed allocation, yielding placeholder 16x16 bounds. The initial-open
  screenshot was masked correctly, but later masks used those premature bounds
  and therefore retained terminal pixels.
- **Repair:** the existing card-geometry receipt now retries for at most 500 ms
  and publishes only a real allocation larger than GTK's 16x16 placeholder (or
  an explicit allocation-timeout receipt). Pointer targeting and screenshot
  masking therefore share actual post-layout geometry on every refresh.
- **Screenshot discovery:** post-layout geometry is emitted during GTK's frame
  tick, before Xvfb necessarily presents that frame. The first attention capture
  could therefore contain the prior progress pixels despite correct new bounds.
- **Repair:** screenshot capture waits one bounded 100 ms software-renderer
  settle interval after the receipt before reading the X drawable. This delay is
  limited to visual evidence; input/lifecycle assertions remain receipt-driven.

## Focused receipts

- ReleaseSafe staged build: PASS; package notices remained 75 Cargo, 27
  Ghostty, 104 catalog entries.
- `rust-source-ux-x11`: PASS with six real Ghostty PTYs, five simultaneously
  mapped inactive surfaces, live terminal/agent refresh, physical Peek routes,
  resize cancellation, exact focus restoration, and four normalized screenshots.
- `rust-project-icons`: PASS on controlled X11 and input-capable Wayland. X11
  owns normal plus reduced motion; Wayland explicitly reports compositor-owned
  normal motion and does not claim reduced-motion coverage.
- `rust-multi-window`: PASS on controlled X11, including real second-window
  Peek open and deactivation cancellation. The broader Wayland actor remains
  affected by its documented pre-existing Cage/XTEST queued-key failure and is
  not represented as a pass.
- `rust-worklane-accessibility`: PASS for both exact GTK trees on controlled X11
  and Wayland under `GTK_A11Y=test`.
- Pure Peek traversal/spatial tests: 2 PASS; precision scroll tests: 4 PASS.
- Feature inventory: PASS at 60 entries, 37 IMPLEMENTED, 12 PARTIAL, 11
  NOT_IMPLEMENTED.
## 2026-08-22 — Final qualification exposed an unbounded installed-Gemini wait

- The first post-implementation `linux/tests/qualify-local` run did not complete: after roughly 33 minutes, its Wayland representative cell had spent roughly 27 minutes inside the real installed-Gemini controlled-endpoint journey in `linux/tests/rust-agent-ipc`.
- Process-tree inspection established that the product, Gemini CLI, and controlled endpoint were all still alive. The outer qualification runner had no per-cell command deadline, while the journey's log waits were individually bounded but the overall process lifecycle was not.
- This was not accepted as a slow pass or environmental absence. The qualification run was interrupted, so it produces no qualification claim. The installed-Gemini journey must first be reproduced in isolation, its stalled lifecycle diagnosed, and its real-system boundary given an honest bounded failure before another full qualification run.
- A focused follow-up established two concrete harness problems without changing
  the GH-80 product: the OpenCode live-theme journey replaces the config file
  and thereby removes the previously written confirmation opt-out, and bounded
  polling is followed by an unconditional process `wait`. A subsequent clean
  attempt also detected that the machine-global Gemini is 0.56.0 while the
  qualification contract and repository-owned node-tools installation pin
  0.53.0. None of those conditions was converted into a pass.
- Further repair and qualification rerun are deliberately deferred to
  [GH-83](https://github.com/TamedTornado/zentty/issues/83). The speculative
  harness edits used during diagnosis were reverted so this feature commit does
  not absorb unrelated qualification policy. GH-80 is being finalized from its
  complete focused real-system receipts, with the interrupted full run stated
  explicitly and no full-qualification claim.
- The final lint sweep was initially invoked without ShellCheck's `-x`, so it
  refused to follow the existing repository-relative test-library imports with
  SC1091. This was a lint-invocation error, not a source finding; the corrected
  `shellcheck -x` run is the recorded receipt below.
