# Zentty Linux visual parity dogfood — 2026-08-23

Issue: GH-84

## Starting state

- The repository had one source hero screenshot and four transient Worklane
  Peek screenshots, but no authoritative element-by-element parity map, no
  checked-in Linux visual baselines, and no generic stale-baseline/mask
  governance.
- `rust-source-ux-x11` already masks only real terminal-picture rectangles and
  proves that progress, attention, and selection screenshots differ. Those
  images are transient receipts, not reviewed parity baselines.
- Scaling journeys prove geometry, SIGWINCH, physical pointer coordinates, and
  compositor surface commits at 1x/1.5x/2x, but did not preserve screenshots.
- External staged-product AT-SPI remains owned by GH-86. In-process GTK
  metadata cannot be used to claim external semantic parity.

## Decisions

- Add a strict evidence-map validator to the existing support-test phase rather
  than create a new qualification scheduler.
- Extend existing actors for screenshots. Do not add a screenshot-only fake
  shell or a second workspace actor.
- Permit masks only for terminal content whose rectangle is emitted by the
  running product journey. No static broad mask can cover product chrome.

## Correction: do not recreate hash-based evidence bookkeeping

- The first plan draft proposed source, image, baseline, binary, scenario, and
  mask hashes, and the initial map included a SHA-256 of the source screenshot.
  That repeats the discarded paranoid harness design: Git already versions the
  source and reviewed baselines, while pixel comparison already detects visual
  changes.
- The duplicate hashes were removed before the validator was implemented.
  Visual governance will check schema, required files, narrow masks, semantic
  receipts, and actual pixel comparisons—not maintain a parallel hash ledger.

## Existing actor audit and repair

- The first real X11 source-UX rerun stopped before the screenshot phase. Its
  sidebar lifecycle assertion expected exactly one `sidebar-card` render for
  the entire journey, even though the journey had intentionally reprojected
  configuration before the title-only check. This was a test-boundary bug, not
  a product failure.
- The assertion now snapshots the render count immediately before the
  metadata-only title transition and proves that transition does not rebuild
  the row. This preserves the lifecycle contract without coupling it to prior,
  legitimate renders.
- The actor also wrote `SHA256SUMS` for transient Peek screenshots. Those
  hashes were unused by any comparison or qualification decision, duplicated
  the images themselves, and contradicted the no-parallel-ledger decision.
  The unused hash receipt has been removed; the actor still performs direct
  pixel-difference assertions on the images.
- After that repair, the journey reproducibly exposed a product layering bug:
  the full-width pane drag strip was added after the hover controls and covered
  the upper portion of their real pointer hit targets. The old coordinate now
  reached `pane-drag-zone` instead of `New Pane Below`. The overlay order is
  corrected so the controls render and receive input above the drag strip;
  the existing real-pointer journey remains the regression proof.
- The next run reached all four Peek captures but failed while masking the last
  image because the actor selected the newest card log line even when that line
  explicitly reported `unavailable=allocation-timeout`. A preceding valid
  geometry receipt still described the displayed card. Capture now selects the
  latest valid `x/y/width` receipt, while absence of any valid geometry remains
  a hard failure. This fixes evidence collection without treating transient
  allocation absence as success.
- Making the visual CWD stable under the repository accidentally reintroduced
  real Git/project discovery into an actor whose lifecycle checks require a
  non-project shell. That changed sidebar content and invalidated pointer
  coordinates before evidence capture. The stable CWD is now the dedicated
  non-project `/tmp/zentty-source-ux-cwd`; it keeps visible labels deterministic
  without contaminating the scenario with the developer checkout.
- The selected long-label capture revealed a second determinism error: the
  recorded card allocation described one frame of Peek's transition while the
  X11 drawable had advanced to another, so otherwise narrow terminal masks no
  longer aligned. Motion itself already has real enabled/reduced coverage in
  `rust-project-icons`. The source-UX evidence actor now disables GTK animation
  so each semantic state has one settled geometry, and publication rejects an
  image whose real dimensions differ from its scenario contract.

## First governed receipts

- The existing X11 source-UX actor now publishes the already exercised
  multi-lane, progress, attention, and selected long-label states through one
  shared receipt boundary. It records scenario environment, semantic assertion
  IDs, and the exact dynamically derived terminal rectangles; it neither
  launches another product nor hashes evidence.
- Two consecutive controlled real-product runs produced zero differing pixels
  for all four normalized images. The reviewed images are now checked in as the
  first baselines, and a third run passed direct baseline comparison from the
  actor.
- Publication contract tests cover a wrong owning actor, wrong real image
  dimensions, and actual pixel mismatch in addition to the map runner's
  missing/stale baseline and false-claim cases.

## Main-shell evidence expansion

- The actor now clears its controlled PTY after each synthetic command and
  hides the cursor. That keeps real Ghostty/PTY pixels present and deterministic
  so main-shell states need no terminal mask at all; only the Peek thumbnails
  retain their geometry-derived masks.
- The first expanded run caught a 60-pixel difference at the focused pane edge
  in the progress receipt. Earlier and later state was identical; the X11
  drawable had been read only 100 ms after a GTK geometry receipt while more UI
  work now preceded Peek. Capture settling is increased to 300 ms rather than
  accepting a tolerance or broadening a mask. The direct baseline gate remains
  strict.
- Inspection then showed the changed pixels were actually previously unmasked
  strips of a Peek terminal preview after the new deterministic clear, not an
  unstable pane border. Existing PASS gates correctly rejected that intentional
  evidence-input change; candidate images are being regenerated and must prove
  repeatability before reviewed baselines move.
- Main-shell captures exposed two additional issues before any baseline was
  accepted. Ghostty's normal resize-size overlay appeared in responsive states,
  so the controlled actor now uses Ghostty's real `resize-overlay = never`
  configuration rather than waiting or masking it. GTK's pane popover is a
  separate X11 surface, so parent-window capture omitted the open menu; that
  state now captures the real root pixels cropped to the measured application
  window.
- The 1600x900 state also exposed a real Zentty layout gap: two columns retain
  their prior widths and leave a large blank region instead of distributing
  the wide viewport. This remains tracked by GH-87 and must not be converted
  into a passing visual baseline.
- Two bootstrap runs matched exactly, but the first authoritative run still
  caught a changing `peek-live:N` title in the progress image. A fixed sleep
  had merely been lucky twice. The real PTY child now waits on an actor-owned
  `progress-captured` barrier before starting title churn; the actor releases
  it only after the progress pixels and receipt exist. Attention/live-refresh
  behavior remains real, but the progress state no longer depends on scheduler
  timing.
- The next authoritative run found a real Ghostty cursor blinking in the eight
  pixel thumbnail padding just outside the reviewed terminal mask. Two earlier
  runs happened to capture the same blink phase. The controlled visual profile
  now sets Ghostty's supported `cursor-opacity = 0`; cursor behavior is not the
  subject of these shell-chrome scenarios, and hiding it is narrower than
  masking card borders or allowing pixel tolerance.
- With the PTY barrier, resize overlay disabled, and cursor hidden, two fresh
  bootstrap runs again differed by zero pixels across all eleven X11 states.
  After reviewing and updating the ten non-defective baselines, a separate run
  against the authoritative map passed every direct comparison. The wide state
  produced a valid receipt with `scenario_status=FAIL` and
  `capture_result=PASS`; evidence collection succeeded, but the product layout
  did not.
- The map validator previously printed a bare `visual-parity: PASS`, which was
  technically its schema result but became misleading as soon as a governed
  scenario was explicitly FAIL. Its report now says `visual-parity-map: VALID`
  and prints every scenario status total plus the limited qualification claim.
