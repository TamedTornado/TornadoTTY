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

## Controlled Wayland scaling capture

- The first map draft assigned dark X11 review/remote scenarios to the broad
  source-UX actor even though the existing `rust-git-review-context` and
  `rust-session-restore` actors own those meanings. The map now names the real
  owners before either actor is extended.
- For native Wayland scaling, the evidence image is the physical 1024x768
  nested compositor output at 1x, 1.5x, and 2x. The earlier 682x512/512x384
  values were logical surface sizes, not screenshot pixel dimensions. Scenario
  contracts now distinguish scale metadata from the physical reviewed image.
- The existing scaling actor, not a new actor, captures ReleaseSafe output only
  after its real PTY geometry, SIGWINCH, pointer, and compositor-commit stage
  assertions pass. Debug still executes every semantic stage but does not
  duplicate identical visual evidence.
- The first 2x image was solid black even though the compositor receipt proved
  `preferred_scale(240)`: wlroots' X11 backend does not expose a stable mapping
  from Wayland output names to the wrapper's two outer X windows. Capture now
  reads both wrapper-owned 1024x768 windows and selects the one containing
  presented pixels; it hard-fails if both are black. This follows the same
  output-identity boundary already documented by the actor's pointer path.
- After output selection was corrected, two controlled native-Wayland runs
  produced zero pixel differences at physical 1x, fractional 1.5x, and integer
  2x. All three images retain real Ghostty surfaces without masks. Their
  reviewed baselines are now PASS candidates subject to a separate run against
  the authoritative map.
- The separate authoritative run passed all three direct comparisons while
  repeating the full ReleaseSafe and Debug semantic scaling journey. This is
  physical screenshot PASS at each scale, not merely a logical-size receipt.

## Native multi-window and fullscreen capture

- The visual map initially described the Weston fullscreen receipt as
  1024x768, copying the separate wlroots scaling lab's output size. The real
  multi-window actor runs in its existing 1280x1024 nested Weston profile; the
  map now records those physical pixels rather than a convenient logical size.
- The existing `rust-multi-window` journey now publishes both receipts at the
  state it already owns: two live native X11 toplevels with distinct Ghostty
  PTYs, and a compositor-acknowledged Wayland fullscreen toplevel. No visual-
  only product launcher, fake surface, scheduler, image hash, or terminal mask
  was added.
- The first X11 candidate exposed the actor's PID-bearing route-probe title in
  the real terminal. The PTY actor now clears that probe through a fixed
  `visual-reset` command and hides its cursor before capture; this preserves
  real Ghostty pixels without turning nondeterministic process IDs into an
  accepted baseline.
- Two visually identical-looking X11 candidates still differed by 1,121 exact
  pixels: the pointer had sometimes left an Open With segment hovered, and a
  horizontal scrollbar was captured at different animation frames. Visual
  capture now parks the pointer outside both windows and disables GTK
  animations through the same private settings profile already used by the
  source-UX actor. Motion remains covered by its dedicated real-product actor;
  no tolerance or extra mask was introduced.
- With animations disabled, the next two X11 candidates were visually
  indistinguishable but still differed by 66 antialiased pixels at the two
  lower corners of the first window's previously hovered split button. The
  capture settle after parking the pointer was only 200 ms; it is now 500 ms,
  matching a completed GTK hover-state transition rather than accepting those
  pixels as noise.
- A 500 ms settle still alternated between the same two 66-pixel rasters. The
  only changing pixels were antialiased lower corners of a compound header
  button in the first 590px-wide window, indicating fractional layout/raster
  placement rather than motion. The evidence now uses the actor's already-
  established 600px native-window width with a real 20px inter-window gap
  (1220x700 physical receipt), avoiding an artificial fractional boundary.
- Integral geometry produced the same 66-pixel alternation, disproving that
  hypothesis. The changing pixels are GTK antialiasing on one inactive header
  button, not product state or Ghostty content. This actor now requests GTK's
  real Cairo software renderer in its already software-only controlled visual
  environment. Ghostty remains the real OpenGL terminal surface; only GTK
  chrome rasterization is pinned so baseline evidence is reproducible across
  fresh processes.
- Two fresh Cairo runs matched at AE=0 and completed the full X11 lifecycle.
  The first subsequent authoritative run failed earlier, in the pre-existing
  confirmation journey, because an idle Close Window unexpectedly displayed a
  confirmation. It never reached visual capture, so it did not validate or
  replace the receipt. This is recorded as an integration failure, not hidden
  behind the already-repeatable image result; a repeat must complete the whole
  actor before PASS promotion is final.
- The first Wayland candidate proved fullscreen capture itself, then the later
  worklane-close journey failed to route input after returning to windowed
  mode. Fullscreen had changed the nested compositor's pointer/focus target.
  The actor now explicitly re-establishes its existing second-window physical
  input precondition after the round trip instead of treating capture success
  as a passing integration run. The failed run is not accepted as a baseline.
- Moving fullscreen into the actor's previously dormant, independent window-
  state mode exposed why that mode could not simply be promoted into the
  matrix: it inherited close confirmations and then waited for an unconfirmed
  close as though it had succeeded. The mode now explicitly disables only its
  destructive-close confirmations before launch. The main confirmation
  journey remains unchanged, and the new matrix cell must pass the complete
  fullscreen/exit/close sequence before its visual receipt can be accepted.
- The next isolated run closed window 2 correctly but revealed a second stale
  assumption in that dormant mode: it sent Quit without physically restoring
  focus to the surviving window, so Weston discarded the shortcut. The actor
  now reacquires pane 1 through its existing real pointer-target path before
  quitting. A close log alone is not accepted as whole-mode success.
- The repeat authoritative X11 run completed the entire existing lifecycle and
  enforced the reviewed baseline at AE=0. The isolated Wayland window-state
  cell likewise passed fullscreen entry, physical 1280x1024 capture, exit,
  survivor focus, and quit with AE=0. Both receipts now report
  `scenario_status=PASS` and `capture_result=PASS`; the obsolete multi-window
  terminal mask was removed because both real PTYs are deterministic and
  fully visible.

## Git and pull-request chrome capture

- Review visuals remain owned by the existing `rust-git-review-context`
  actor, which already proves a real Git repository, controlled `gh`/HTTP
  forge boundary, PR metadata, refresh paths, and safe URL opening. Capture is
  inserted only after those initial real-context assertions; no review-state
  fixture bypasses the product resolver.
- The actor now uses an interactive real PTY child that clears its terminal,
  fixes its title, and hides its cursor after commands. Later Git and agent IPC
  commands still execute inside that PTY. This permits unmasked review
  screenshots rather than extending the Peek-specific terminal mask to cover
  a full shell.
- Dark X11 is captured from a physically resized 1400x800 native window.
  Light Wayland uses the actor's controlled compositor output and a persisted
  `theme_mode = "light"`; it does not infer light mode from an ambient desktop.
- Two fresh runs on each backend matched at AE=0 while repeating the complete
  Git/gh/HTTP-forge, refresh, agent-completion, and safe-open journey. The
  reviewed light receipt intentionally shows a light native titlebar and
  terminal palette with Zentty's dark product chrome; that is the current
  persisted light-mode projection, not an ambient-theme accident.
- Promoting `review-dark-x11` exposed a stale negative-test fixture: the
  missing-baseline test tried to promote that specific scenario, which now had
  a real reviewed baseline and therefore correctly passed. The negative test
  now promotes the still-pending `remote-dark-x11` scenario, preserving the
  intended missing-baseline failure without deleting real evidence.

## Verified remote-session chrome capture

- Remote visuals remain inside the existing consolidated session-restore
  journey. The capture point follows a real key-authenticated loopback OpenSSH
  launch, foreground-process identity detection, and rendered sidebar/pane
  projection. A title string alone is explicitly insufficient to reach it.
- The disposable SSH actor now clears its real remote PTY and hides the cursor
  after publishing the fixed `ssh-remote-ready` title. This keeps the full
  Ghostty surface visible and deterministic; no terminal mask or fake remote
  model is introduced, and all later clipboard/drop/rollback/cancellation and
  restore assertions continue in the same actor.
- X11 uses a real externally resized 1200x700 window. The Wayland session-
  restore matrix cell is the existing nested Weston multi-window profile, so
  its physical evidence contract is 1280x1024, not the map's earlier copied
  1024x768 Cage size.
- The first Weston image correctly captured the physical output but included
  compositor wallpaper, panel clock, and native titlebar around the clamped
  restored window. The clock makes that whole-output image nondeterministic and
  is not Zentty product chrome. GTK reports the oversized requested allocation
  (1280x1024), while Weston visibly clips it behind fixed 60px left and 124px
  top compositor regions. The first attempted allocation-derived crop therefore
  remained the whole output and the size gate correctly failed. Evidence now
  validates the 1280x1024 physical output and crops its controlled, visibly
  presented 1220x900 product-client region at +60+124; it does not synthesize
  or recapture the application through a fake surface.
- Two fresh X11 runs matched at AE=0 while completing the entire consolidated
  restore journey. Two fresh Weston runs matched the cropped Wayland client at
  AE=0 while completing the real physical remote-file-drop cell. Both remote
  baselines are unmasked. The missing-baseline negative fixture advances to
  the still-pending sidebar-overlay scenario.

## Wide light-mode evidence

- The existing source-UX actor now switches appearance through the real
  command palette, captures the same live two-pane 1600x900 topology in light
  mode, and restores dark mode before continuing its established journey. It
  does not inject a theme model or add another actor.
- The wide-layout defect tracked by GH-87 is theme-independent: the two live
  pane columns retain their prior widths and leave a large unused region on
  the right. The light scenario is therefore an explicit `FAIL`, not a passing
  baseline or an evidence-pending omission. Its unmasked receipt must still
  prove that the expected product state was captured successfully.
- The first integrated attempt switched back to dark mode and continued the
  long source-UX journey. Although the mode persistence succeeded, the live
  Ghostty panes did not recover their prior two-column geometry/palette; the
  later `pane-controls-hidden-x11` baseline rejected 622,690 changed pixels.
  That failed run was not accepted. Light evidence now uses an explicit early-
  exit profile of the same real actor and closes through the native window
  protocol after capture, while the authoritative full journey retains its
  previously qualified dark-state sequence. This records, rather than hides,
  the appearance-switch/layout interaction under GH-87.
- The first focused profile published the expected `FAIL`/capture-`PASS`
  receipt, then failed because this host's xdotool `windowclose` command does
  not support the `--sync` option accepted by several other xdotool commands.
  The unsupported option was removed; bounded PID waiting remains the actual
  clean-shutdown assertion. The failed actor invocation is not reported as a
  passing journey.
- A native X11 `windowclose` retry destroyed Ghostty's drawable before its
  renderer lifecycle completed and correctly failed with `BadDrawable` rather
  than laundering the evidence-only run as clean. The profile now uses the
  product's ordinary Ctrl+Q route from the shared real-input helper, asserts
  the configured `Quit Zentty?` confirmation, accepts it physically, and then
  requires a zero-status process exit.
- The corrected focused profile passed and published an unmasked 1600x900
  receipt with `scenario_status=FAIL` and `capture_result=PASS`. Visual review
  confirms the real two-pane light surface stops at its retained column width
  and leaves the remaining right side unused. The unchanged complete dark X11
  source-UX journey then passed all existing baseline gates and lifecycle
  assertions, proving the focused profile did not weaken the authoritative
  actor path.
