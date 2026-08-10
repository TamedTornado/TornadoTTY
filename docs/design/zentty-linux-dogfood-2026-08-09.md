# Zentty Linux dogfood record — from 2026-08-09

This is the active dogfood record for Linux product work beginning with the
real multi-window composition slice. Earlier investigation and delivery history
remains in
[`zentty-linux-dogfood-2026-08-01.md`](zentty-linux-dogfood-2026-08-01.md).
That file is closed after its in-flight multi-window entry; new discoveries,
failures, repairs, evidence, and remaining uncertainty belong here.

The recording standard is unchanged:

- record the real failing receipt before the repair;
- distinguish product defects, harness defects, and environmental limits;
- name controlled-environment session IDs and qualification totals exactly;
- never turn environmental absence into PASS;
- describe Valgrind success as **PASS with reviewed suppressions**; and
- keep remaining gaps aligned with `linux/qualification-matrix.json`.

## 2026-08-09 — Multi-window qualification handoff (#32)

- The implementation and detailed failure/repair trail began in the closing
  section of the previous record. The final focused journeys pass under
  controlled X11 session
  `9c6ded7d4d94e79b2b414471cbec358b5487d9d39882bc9a58e7008f52b3309a`
  and controlled Wayland session
  `7d3b0ded144159293d01b2f4ee1ab4805de67842671e88eca1fb59ba0e1032d2`.
- The authoritative matrix now has explicit ReleaseSafe X11 and Wayland PASS
  cells for clean two-window creation, routing, persistence, restoration,
  non-final close, survivor input, and aggregate teardown. Broader workspace
  restoration remains NOT_IMPLEMENTED for multi-window crash recovery,
  window-frame/divider persistence, and complete CWD coverage.
- The first all-executable-cell rerun completed in 360.36 seconds. Both new
  multi-window cells passed, but the run correctly failed overall because two
  mirrors still described the former composition root: the Ghostty API ledger
  expected `ticking_runtime.tick()` in `main.rs`, and the architecture mirror
  had not yet incorporated the two new matrix cells. The dependent
  `rust-ghostty-api-product-usage` cell consequently failed at the same ledger
  prerequisite. These are documentation/contract drift exposed by
  qualification, not product-test failures, and must be repaired before the
  candidate can be committed.
- The Ghostty API ledger now assigns runtime ticking to
  `ApplicationCoordinator` in `application.rs`, and its real-journey evidence
  includes the multi-window runner. The architecture mirror was reconciled
  with the remaining workspace-restore gaps. The exhaustive ApplicationShell
  ownership contract now records the three application lifecycle handlers,
  new actions/methods, generalized persistence functions, and hashes both the
  new application coordinator and pure window-set implementation. Its negative
  test now proves an unreviewed second coordinator runtime is rejected.
- The first focused `WindowSet` mutation run tested 24 mutants: 21 were caught,
  one was compiler-unviable, and two arithmetic mutations survived because the
  tests covered closing a middle active window but not the last active window
  in a non-empty set. A new exact fallback test closes `window-c` from three
  windows and requires `window-b`. The rerun caught all 23 viable mutants; one
  remained compiler-unviable. Both runs used the governed
  `gitignore=true`/`copy_target=false` wrapper, so ignored build trees were not
  copied into mutation scratch space.
- The repaired candidate passed every presently executable support and matrix
  cell in 361.05 seconds. The two new matrix-owned multi-window receipts are
  controlled Wayland session
  `1adc795406b1ff3336ba63045331225704ab8b5575147ae06139af4df5ef358f`
  (8,260 ms) and controlled X11 session
  `7cd48084b1cbb6041dcf02765d6809de4659ed7253cefaf2b29073a60936926f`
  (12,310 ms). Implemented-local and product-boundary qualification pass;
  release and full Linux qualification correctly remain false. Declared totals
  are `PASS=90`, `FAIL=0`, `BLOCKED=7`, `XFAIL=1`, and
  `NOT_IMPLEMENTED=21`. The machine-summary SHA-256 is
  `c99154726d1036ee4445d251397cddba0a203ac4a76ca97948766eef4dc815d7`.
- Debug Valgrind remains **PASS with reviewed suppressions**, not an
  unsuppressed-clean result. Raw evidence contains 427 errors/contexts, 6,160
  definite bytes, and 41,428 indirect bytes. Post-suppression evidence contains
  zero errors/contexts and zero definite/indirect bytes, with all 427 contexts
  governed. The report, raw receipt, and suppressed receipt SHA-256 values are
  respectively
  `7150a65c68d041099d2a87c12f4352b4b39a1f5133659763d7c66b6ff9efcc0d`,
  `8fd99d92042e8b1363bf12ede5b8d5cc7d500ddc064fd5ccca9a4c2019107038`,
  and
  `12fbeac9c8ea1a277fc184389dc3f268bb881e8be64784ae8d8096ba7113e514`.
  ReleaseSafe Valgrind remains the expected XFAIL; no suppression was widened
  for this slice.

## 2026-08-09 — Planned multi-window crash-recovery slice (#32)

- The next bounded acceptance slice extends the existing
  `linux/tests/rust-multi-window` journey rather than creating another product
  runner. It will create two real GTK/Ghostty windows through physical input,
  wait for the aggregate debounced `liveSnapshot`, verify the lifecycle is
  marked unclean, SIGKILL the staged product, and prove its real PTY children
  terminate.
- Relaunch will explicitly disable normal restoration. The unclean lifecycle
  must nevertheless restore both ordered windows and route physical input to
  the recorded active window, proving crash-recovery classification rather
  than ordinary preference-driven restore. A subsequent clean quit must mark
  the lifecycle clean and consume the now-disabled snapshot.
- Only that narrow X11/Wayland contract may become PASS. Exact frame/divider
  restoration and complete CWD coverage remain visible in the broader
  NOT_IMPLEMENTED workspace cells.
- The first focused X11 attempt did not reach the product. Xvfb rejected
  `/tmp/.X11-unix` because the rebooted development environment had left the
  standard socket directory owned by `nobody:nogroup` rather than `root:root`.
  The directory already had the required `1777` mode; restoring only its
  standard host ownership repaired the controlled-environment prerequisite.
  A second sandboxed attempt still saw the sandbox namespace's deliberately
  remapped `nobody:nogroup` directory; running the controlled display harness
  through its approved elevated path used the repaired host socket directory.
  Neither environmental failure is counted as product PASS or FAIL.
- Controlled X11 session
  `0e1bbadb17d44d4be2a4e174449dd081a9e494f72b64736ebc1245b2add9ec72`
  and controlled Wayland session
  `d44b943db09adb1ed239264256513732b7ea6880c70c0a154de6fb97b464684a`
  passed the expanded journey. Each confirmed the ordered aggregate live
  snapshot and unclean lifecycle, SIGKILLed the real staged process, observed
  its PTY children terminate, relaunched with normal restoration disabled,
  restored both GTK/Ghostty windows because crash recovery overrides that
  preference, routed physical input to `pane-window-2`, then completed a clean
  quit that marked the lifecycle clean and consumed the disabled snapshot.
- The two existing narrow matrix cells are renamed to make both clean and
  SIGKILL restoration explicit without executing the same expensive real
  journey twice. The broader workspace cells remain NOT_IMPLEMENTED only for
  exact divider/resize persistence, window-frame restoration, and complete CWD
  coverage; multi-window crash recovery is no longer listed as a gap.
- The first focused contract command used the nonexistent spelling
  `qualification-matrix --validate`; the runner correctly rejected it and
  printed its `--validate-only` interface without executing any cells. This was
  an operator invocation error, not a product or runner failure; the corrected
  contract run follows below.
- The corrected matrix validator, matrix negative tests, orchestration
  contract, architecture contract, and ownership negative tests all passed.
  The subsequent all-presently-executable qualification run passed in 360.44
  seconds. The expanded multi-window cells passed in 10,740 ms on controlled
  Wayland and 19,120 ms on controlled X11. Declared totals remain `PASS=90`,
  `FAIL=0`, `BLOCKED=7`, `XFAIL=1`, and `NOT_IMPLEMENTED=21`; implemented-local
  and product-boundary qualification pass, while release and full Linux
  qualification correctly remain false. The machine-summary SHA-256 is
  `63aa37c82a95098abcfb58711b20b78afd36eefcfe05148cb701563f255a637b`.
- Debug Valgrind remains **PASS with reviewed suppressions**. The rerun again
  recorded 427 raw errors/contexts, 6,160 definite bytes, and 41,428 indirect
  bytes; post-suppression totals are zero, with all 427 contexts governed. The
  report, raw receipt, and suppressed receipt SHA-256 values are respectively
  `556df63269fbd7c03c28654063c17569b99b912d25c6cd55429b96685c5d7da3`,
  `73a5c8c4f1db7b9a67641d7ef3e6b1e15853b37c1827d33e915d5a40b52c3d8c`,
  and `8ad67b787896e02729385ec86336aed1adf739dd85484ddd31b38feee5ee81e5`.
  ReleaseSafe Valgrind remains XFAIL; no suppression changed.

## 2026-08-09 — Planned per-window frame-size slice (#32)

- The source exports every real `NSWindow.frame`, rejects non-finite frames or
  frames smaller than 320×240 during restore, and uses the validated frame as
  the initial layout seed. GTK4 intentionally exposes no portable application
  API for authoritative toplevel coordinates: Wayland compositors own
  placement, and GTK4 also removed the old cross-backend move API. Linux will
  therefore preserve any imported source coordinate/screen metadata but claim
  only truthful client-size persistence and restore requests.
- Focused pure tests will first pin validation boundaries and snapshot
  projection. Production wiring will then record each mapped GTK window's
  actual allocated width/height in its existing `WindowRecipe.frame`, and apply
  validated width/height before presenting a restored shell. No second state
  store, persistence path, or integration runner will be added.
- The existing real multi-window journey will inspect both persisted frames
  and exact per-window restore-request receipts on X11 and Wayland. X11 will
  additionally resize a real product toplevel externally and verify its
  restored actual geometry. Wayland will verify the client request and report
  compositor-owned placement/negotiation rather than pretending coordinates
  are controllable.
- The first focused Cargo invocation supplied two positional test filters;
  Cargo accepts only one and rejected the command before compiling or running
  tests. The corrected module-level filter is used below. This is an operator
  invocation error, not a test or product failure.
- The corrected focused tests passed, then strict Clippy rejected two review
  issues before product testing: adding frame setup pushed the existing shell
  constructor to 107 lines, and the validated floating-point-to-integer size
  conversion lacked an explicit truncation-safety annotation. Frame setup is
  extracted into a focused helper, and the conversion receives a local lint
  allowance documenting the preceding finite/range/rounding proof rather than
  weakening crate-wide lint policy.
- The first extraction left the constructor at 101 lines, so Clippy continued
  to fail rather than accepting a threshold dodge. Default/restored recipe
  selection is now extracted as its own existing concern as well; no
  `too_many_lines` allowance is added.
- The first staged build could not resolve GitHub from the restricted network
  namespace while verifying the pinned Ghostty dependency. It failed before
  compilation and is rerun through the approved network-capable build path;
  dependency verification is not bypassed and the failure is not a product
  result.
- The first governed focused mutation run tested 42 mutants: 37 were caught,
  one was compiler-unviable, and four survived. The missing boundaries were a
  NaN height paired with a valid width, exact `i32::MAX` width/height, and only
  one undersized allocation dimension. Tests are expanded for those precise
  cases before rerunning; no production predicate is retained merely to make a
  mutant distinguishable.
- The strengthened rerun caught all 41 viable focused mutants; one remained
  compiler-unviable. The repository-governed mutation wrapper preserved
  `gitignore=true` and `copy_target=false`, so the ignored Ghostty build tree
  was not copied into scratch workers.
- Controlled X11 session
  `8093d11f71e60b63e10f972d12ea6ba43c7ff363057ed53784413992073da526`
  passed after externally resizing the real second GTK toplevel to 1110×730.
  The aggregate live and clean recipes stored both actual window sizes; clean
  relaunch emitted exact per-ID restore requests and the second mapped X11
  window's actual geometry was again 1110×730. Controlled Wayland session
  `7581d5e4bc03083ee957050b59bf929fa979e947e736f2b3510274ac5d2434b3`
  passed the same persisted-size and exact per-ID client-request assertions in
  nested Cage. It does not claim application control of Wayland coordinates or
  final compositor placement.
- The existing two multi-window cells are renamed to state their expanded
  clean, SIGKILL, and size contract without duplicating the same real journey.
  Broader workspace restoration remains NOT_IMPLEMENTED for exact pane-divider
  persistence, cross-compositor coordinate policy, and complete CWD coverage.
- Every presently executable support and matrix cell passed in 361.16 seconds.
  The expanded multi-window cells passed in 10,730 ms on Wayland and 19,320 ms
  on X11. Declared totals remain `PASS=90`, `FAIL=0`, `BLOCKED=7`, `XFAIL=1`,
  and `NOT_IMPLEMENTED=21`; implemented-local and product-boundary
  qualification pass, while release and full Linux qualification correctly
  remain false. The machine-summary SHA-256 is
  `c01ab3c5a62a82c006162f4fda6e9e0723ecfcda9d14dded2cc73a40e787c95f`.
- Debug Valgrind remains **PASS with reviewed suppressions**, not an
  unsuppressed-clean result. Raw evidence contains 427 errors/contexts, 6,240
  definite bytes, and 41,461 indirect bytes; reviewed post-suppression evidence
  contains zero errors/contexts and zero definite/indirect bytes. The report,
  raw receipt, and suppressed receipt SHA-256 values are respectively
  `7e1900bec05bc083a2cdd691d448eaf275c42d28a4d080d5d198b1ee2f5c90d7`,
  `07d56666ff65da0c41d8eb5fc12c2fad8fe43efbdcc12aacf256e54afee3484e`,
  and `86997a58f410f967956122b0e1cd12e75e32404903b17e306e6b29e1e21052df`.
  ReleaseSafe Valgrind remains XFAIL and no suppression changed.

## 2026-08-09 — Remaining housekeeping consolidated into one delivery

- The prior cadence incorrectly committed crash recovery and window-size
  qualification separately. Those were legitimate acceptance work but not
  feature-sized user outcomes. The remaining non-feature tail is frozen as one
  delivery: exact stored pane widths/heights after real relaunch, every
  restored pane/draft CWD reaching its real child, invalid/partial multi-window
  restore policy, explicit X11/Wayland coordinate limitations, two-window
  accessibility/screenshot evidence, and remaining mutation/lifecycle cases.
- Source and issue audit corrected the meaning of “complete CWD coverage.”
  Issue #3 requires recipe round-trip and restored launch CWD fidelity; it does
  not promise live tracking of arbitrary interactive `cd` changes. Ghostty's
  current embedding ABI accepts an initial working directory but exposes no
  dynamic CWD callback. The batch will test every stored launch CWD through
  real PTYs and will not invent an unsupported live-CWD requirement.
- The existing Linux renderer already consumes persisted `ColumnRecipe.width`
  and `paneHeights`; the missing work is real externally observed
  persist/relaunch evidence, not a second divider model. Tests will extend the
  canonical restore/multi-window journeys rather than add another harness.
- The first real X11 CWD assertion exposed a fixture defect rather than a
  product pass: `pane-agent` requested nonexistent `/tmp/project`; Ghostty
  correctly logged `cannot access cwd, ignoring` and the real child inherited
  the repository directory. The journey now rewrites both that pane and its
  matching restore draft to existing `/tmp`, then requires the real controlled
  child itself—not merely the Rust configuration—to report `/tmp`. Invalid-CWD
  fallback remains separate negative-policy coverage.
- The repaired canonical restore journey passed under controlled X11 session
  `ea21cc6a7bea564e7ea1f60c16b023fafb640623db80ae27b3d23b3af37ff61c`
  and Wayland session
  `3b8e1e82410f090f320b1117408bb4086075db2ed3b8ea7b696b692b10dcb23a`.
  Both real controlled agent children reported `/tmp`, matching their stored
  pane/draft CWDs, while the product also logged the per-pane configuration
  passed across the safe Ghostty boundary.
- Partial construction coverage previously failed only inside one window. The
  existing construction-failure journey now builds a complete first window
  with real Ghostty surfaces, begins a second window, accepts its first real
  surface, then rejects an interior-NUL CWD on its second surface. Controlled
  X11 session
  `1d6453467570f1561e1e3600a1e9737e741ddc7aa08b0784ebd3ab004f4c0198`
  proved application-wide rollback: no shell was presented, all surfaces and
  the private agent runtime were released, the input snapshot remained byte
  identical, lifecycle remained unclean, and exit status was 1.
- The first placement-policy contract used full prose phrases that Markdown
  wrapped across source lines, so literal `grep` correctly failed even though
  the policy was present. The contract now pins shorter semantic phrases that
  cannot be invalidated by ordinary paragraph wrapping; no policy assertion was
  removed.
- Final scope review prevents another category error: draggable pane dividers
  are missing user interaction, and accessibility/visual parity changes what
  users can operate and perceive. They are feature work owned by the next
  source-UX delivery, not qualification housekeeping. This batch does not
  erase those gaps or claim #32 complete; it closes the non-feature tail and
  leaves the matrix NOT_IMPLEMENTED entry explicit for divider behavior.
- The consolidated candidate passed every presently executable support and
  matrix cell in 360.26 seconds. Declared totals remain `PASS=90`, `FAIL=0`,
  `BLOCKED=7`, `XFAIL=1`, and `NOT_IMPLEMENTED=21`; implemented-local and
  product-boundary qualification pass, while release and full Linux
  qualification correctly remain false. The machine-summary SHA-256 is
  `ba45e73dfceb95c67a8bc77138210485501e5718964e527a1adf65a41949d162`.
- Debug Valgrind remains **PASS with reviewed suppressions**. Raw evidence has
  427 errors/contexts, 6,160 definite bytes, and 41,428 indirect bytes;
  post-suppression evidence has zero errors/contexts and zero definite/indirect
  bytes. The report, raw receipt, and suppressed receipt SHA-256 values are
  `1c2146b712851db2e0dd503ce17cb7cd363ddd60d355492d8a75f80c7664cbba`,
  `2d9b2aa09ddccb5d35057c62f85d9cb5e7c3887c58ca4edcc355651e640d4cfe`,
  and `1544c2aabeedbe5948203828452f3ccbeafb10a247aca86cb62433ae91cd43cf`.
  ReleaseSafe Valgrind remains XFAIL; no suppression changed.

## 2026-08-09 — Source pane dividers become a real product feature

- The source audit confirmed that Zentty owns two direct divider classes:
  adjacent columns resize horizontally and adjacent panes in one column resize
  vertically. Dragging preserves the pair's combined extent, clamps both sides
  to terminal minimums, and double-click equalizes the adjacent pair. Linux's
  prior static `GtkBox` layout only consumed stored geometry; users could not
  create that geometry.
- Test-first core cases initially failed because `WorkspaceState` exposed no
  divider transition. The repaired model now changes only the adjacent pair,
  preserves total width/height weight, clamps at 160×80 fallback minimums, and
  round-trips fractional dragged geometry exactly through `WindowRecipe`.
- The GTK feature is one overlay handle implementation, not a second layout
  system. Stable, focusable accessibility separators overlay the existing
  column/pane boundaries; pointer drags and focused arrow keys reach the same
  canonical `WorkspaceState` operations. Existing Ghostty frames remain
  mounted while their width/height requests change.
- The first physical vertical-drag assertion used the old assumed 38-pixel
  chrome height and missed the actual handle. The harness now derives the
  content origin from the product's observed viewport height instead of
  encoding a theme-dependent chrome size.
- The next run proved both real drags, but later fixed-coordinate controls
  failed because the test had intentionally left the leading column 60 pixels
  wider. That was test contamination, not a product regression. The journey
  now returns both dividers to their original positions before continuing its
  established pane-management assertions.
- An attempted live-snapshot assertion also established that
  `--no-session-restore` deliberately does not publish a restore snapshot. The
  assertion was removed rather than treating absence as a pass; exact geometry
  serialization is covered at the canonical recipe boundary, while the
  existing restore journey remains responsible for process relaunch.
- Controlled X11 session
  `28457230981a59c195e33120c8567ea435212f845ff565565d6e06ff8f2ee2b7`
  passed the complete source-UX journey after dragging both real GTK handles,
  observing model receipts, double-click equalizing both adjacent pairs back
  to their baselines, and completing all later real Ghostty/PTY pane actions.
  The prior drag-only baseline pass was
  `19bc56a2c3582436292328c204d74dc719f646b53dc0ddd7f57e5a8b100903e2`.
  Earlier diagnostic sessions were
  `2f1e409b5d9702f67ae67f79fcbb937befd8edb8c10337f36857ded5df2d2c22`
  (vertical coordinate defect),
  `5bfc4a043fa54202463de1129ffdc0a20b86c1275ba9093c4a7f9981c31b9295`
  (downstream geometry contamination), and
  `fb57eba77324f670f00b07781d8fc2084f212b2fcca21ac7326c3adab3a48fdd`
  (invalid no-restore snapshot assumption).
- A later parallel qualification run disproved synthetic double-click as a
  reliable real-system receipt: the vertical equalize event was delivered in
  the standalone X11 journey but disappeared under qualification load. The
  same run also failed to discover one Debug X11 window, while the isolated
  cell passed immediately afterward in session
  `d35f99a762af05c052e87e58bfc7dd397bcb55de138ddbebebdd09c4292d122c`.
  Neither environmental failure was converted to a pass. Equalization remains
  covered at the deterministic model layer and the compiled GTK route; the
  missing deterministic physical multi-click receipt is explicit in the
  NOT_IMPLEMENTED matrix cells. The canonical physical-pointer journey now
  restores its coordinates by dragging
  both handles back instead of depending on synthetic multi-click timing.
  Two consecutive repaired end-to-end runs passed in sessions
  `85df5cead1b8d86f538d64e205586c2ae40f745002fe5be7361f2cf0bf76f31d`
  and
  `8dad2e49145b59e25831abc7c97f2550b63573181d3d025de22892ade42f07f5`.
- The first attempted Wayland regression put `GDK_BACKEND=wayland` on the
  outer nested compositor wrapper. Its X11 bootstrap correctly replaced that
  value, so session
  `87ffc24d4c1d686f02027c82e2643782a9e66b06dda79b45041776c92d383ee7`
  was an additional X11 pass, not Wayland evidence. Passing the backend inside
  the wrapper command produced controlled Wayland input session
  `b000d028577604960e8bb66382e977a69f8b09a917b023e72ecee27d994e21a0`,
  where the complete real restore/relaunch/crash journey passed with the new
  overlay layout.
- A workspace-wide Rust test run inside the filesystem sandbox failed eight
  helper CLI cases at private Unix-socket creation with `EPERM`; unrelated
  non-socket cases passed. The exact same locked workspace gate passed outside
  that sandbox (all unit, integration, CLI subprocess, transport, and doctest
  targets). This was recorded as an execution-environment restriction, not a
  product failure or a skipped pass.
- After those failures and repairs, the completed feature candidate passed
  every presently executable support and matrix cell in 359.40 seconds.
  Totals remain `PASS=90`, `FAIL=0`,
  `BLOCKED=7`, `XFAIL=1`, and `NOT_IMPLEMENTED=21`; implemented-local and
  product-boundary qualification pass, while release and full Linux
  qualification correctly remain false. The machine-summary SHA-256 is
  `2388de97a5f8b909d3bf4a9981224e650b3e0923aa25097155512007012200a1`.
- Debug Valgrind remains **PASS with reviewed suppressions**: raw evidence has
  427 errors/contexts, 6,240 definite bytes, and 41,429 indirect bytes;
  post-suppression evidence has zero errors/contexts and zero definite/indirect
  bytes. The report, raw receipt, and suppressed receipt SHA-256 values are
  `052f479c2ca3f0944b56fc1b5b33e7dd29a2653ae3a3ae6b7fd06092a40bf95f`,
  `6228d414875f00ee710fe4cfb53b1e30d7fceb37e68848cd8674458b658ed35f`,
  and `03d9733aab6304a97e2e6895b179195095857a41b24e3630899eaa81e4d1c944`.
  ReleaseSafe Valgrind remains XFAIL and no suppression changed.

## 2026-08-09 — Keyboard pane resizing uses Ghostty's real cell metrics

- A fresh source audit found four exact commands—`Resize Pane Left`, `Resize
  Pane Right`, `Resize Pane Up`, and `Resize Pane Down`—whose step is five
  terminal cells. The Linux substitution is `Ctrl+Alt+Shift+Arrow` for the
  source macOS `Command+Option+Shift+Arrow`; command semantics and palette
  titles remain source-exact while platform shortcut policy remains in
  Zentty.
- The preceding divider slice described 160×80 as the terminal minimum. That
  was a Linux fallback, not a source-derived invariant. The implementation now
  asks each live Ghostty surface for its rendered cell dimensions, derives the
  five-cell step and drag minimum from those metrics, and retains only a
  120-logical-pixel defensive floor when a metric is temporarily unavailable.
- Ghostty's internal cell dimensions are physical renderer pixels. Returning
  those values directly would make host geometry wrong under compositor
  scaling, so the new generic getter divides by the surface's GTK content
  scale and exposes logical pixels. It rejects null, foreign, uninitialized,
  invalid-scale, and invalid-metric calls without modifying the caller's
  output structure.
- The Ghostty change is deliberately one product-neutral C query plus its
  exported fixed-layout structure and version-script entry. Commit
  `b992c688a680067fae8b51112d1833611136fb57` on
  `zentty/gtk-embed-cell-size` passed the focused `gtk-embed-lib` and
  `gtk-embed-lib-test` build with Zig 0.16. Initial attempts exposed three
  environment mistakes rather than product defects: no default `zig` binary,
  an incompatible Zig 0.15 invocation, and a Zig 0.16 run missing the pinned
  Blueprint compiler plus `-fno-sys=gtk4-layer-shell`. The corrected pinned
  tool environment passed; none of the failed attempts was relabeled as a
  pass.
- Zentty's unsafe declaration is wrapped by a safe `CellSize` result. C API
  misuse coverage preserves sentinels for rejected calls. Core tests prove
  that horizontal commands alter only the focused source column and vertical
  commands alter only the selected adjacent pair, including the source's
  transient last-interacted-divider preference and minimum clamps.
- The first focused action-router run failed because its explicit registry
  cardinality still expected 55 actions after the four resize commands were
  registered. The assertion was repaired to 59; uniqueness, parameter schema,
  registration, and topology sensitivity remain checked rather than weakening
  the registry test.
- Strict workspace Clippy then rejected the new pure divider-selection query
  without `#[must_use]`. The API now makes accidental result disposal an
  explicit compiler warning; no lint was suppressed.
- Diff review found that the first real X11 keyboard journey proved physical
  dispatch and real cell-metric consumption but did not independently assert
  the resulting rendered allocations. It now observes each horizontal and
  vertical allocation change and its reverse step. Additional model coverage
  exercises first, interior, and last column edge behavior, minimum/maximum
  clamps, invalid directions, and both vertical directions. Strict Clippy
  rejected exact floating-point assertions in that new test; they were changed
  to epsilon comparisons rather than suppressing the lint.
- The first strengthened X11 run caught a harness race: the `Down` action
  receipt had arrived, but the test sampled the preceding rendered height line
  before `render()` emitted the new allocation. The journey now waits for both
  the action receipt and a new pane-height allocation receipt. Controlled X11
  session `506bbf3cc52b94e15ccb4a77be01cd33e87a115a0474aba6b3c3f520bf235f57`
  then passed with all four physical commands, both rendered-axis changes, both
  reverse steps, and the remainder of the established source-UX journey.
- Controlled X11 session
  `536ade4e10f38028691df37403d7d33456eca7d85bba890d95fc2528fa194285`
  built the real staged product at the new Ghostty pin, physically delivered
  all four keyboard chords through GTK/GDK, observed nonzero real cell
  dimensions in the product receipts, reversed the changes, and completed the
  established real Ghostty/PTY source-UX journey.
- Remaining evidence is explicit: controlled Wayland physical delivery of
  these four chords, font-change transitions, fractional-scale transitions,
  off-main-thread misuse, and deliberately different per-pane font metrics
  are not established by the X11 receipt. They are not converted into a full
  Linux qualification claim.
- The first authoritative matrix run stopped after its dependency floor:
  Ghostty regression passed, but both product builds rejected the newly
  exported symbol because the independent staged ELF allowlist still named 12
  functions, and the architecture contract still pinned the preceding
  Ghostty revision. These were closed-world ledgers doing their job, not
  compiler or runtime failures. The allowlist now explicitly names
  `ghostty_gtk_embed_surface_cell_size`, and the architecture contract pins
  `b992c688`; neither validator was weakened. Because those build dependencies
  failed, downstream cells were not executed and the run made no qualification
  or suppression-review claim.
- After repairing those ledgers and strengthening the rendered-allocation
  checks, the final rerun of every presently executable support and matrix cell
  passed in 355.96 seconds. Declared totals remain `PASS=90`, `FAIL=0`,
  `BLOCKED=7`, `XFAIL=1`, and `NOT_IMPLEMENTED=21`; implemented-local and
  product-boundary qualification pass, while release and full Linux
  qualification correctly remain false. The machine-summary SHA-256 is
  `3e46efc6713c61359e99b328f17a4733d115ebf688e3a582186d4af9b1f3a83c`.
- Debug Valgrind is **PASS with reviewed suppressions**, not unsuppressed clean:
  raw evidence contains 427 errors/contexts, 6,240 definite bytes, and 41,397
  indirect bytes; post-suppression evidence contains zero errors/contexts and
  zero definite/indirect bytes. The report, raw receipt, and suppressed receipt
  SHA-256 values are
  `fba131ba2a17c644265c2ea5d2ce668af32ec7d28207ffeece88cd4daaa64201`,
  `4e5147dea99c7b092b589e5a3402ba7f2e2019d8bd4c4a1de2884da511a73ad4`,
  and `c730ae84380dbbf5d5b4b9e9f8bc83a449c9e7c6a927c0b3d4921d43331eca0a`.
  Suppression governance was accepted; ReleaseSafe Valgrind remains XFAIL and
  no suppression was broadened.

## 2026-08-09 — Next feature is source live-pane window transfer

- Direct source review corrected the prior broad phrase “cross-window moves.”
  Zentty has a specific, named **Move Pane to New Window** transaction in
  `AppDelegate`, `WorklaneStore+DragDrop`, `MainWindowController`, pane/sidebar
  context menus, and the command registry. It preserves the live runtime rather
  than serializing and recreating a terminal.
- Source behavior distinguishes two cases: one pane extracted from a
  multi-pane worklane receives a new destination worklane that inherits title,
  color, next-pane numbering, and auxiliary state; the sole pane of one
  worklane in a multi-worklane window moves that entire worklane identity. The
  only pane of the only worklane is explicitly ineligible.
- The source detaches the runtime before model extraction, adopts it into the
  destination registry before coalesced observers run, and restores it if
  extraction fails. This makes single live-surface ownership and transactional
  rollback acceptance requirements, not optional implementation polish.
- A Linux-specific risk is now explicit: the running PTY's inherited source
  window/worklane variables cannot be mutated. Post-move authorization must use
  the host's capability registry and canonical pane identity; inherited child
  environment is historical metadata, not routing authority.
- Public issue #33 and
  `docs/design/linux-live-pane-window-transfer-plan.md` freeze source authority,
  ownership, red-test order, real X11/Wayland PID/scrollback/input evidence,
  failure rollback, persistence, and claim limits before production code.
  Existing application, persistence, agent, and integration actors must be
  extended; a parallel transfer harness or recreated terminal is forbidden.
- Model tests were added before implementation and failed to compile because
  `WorkspaceState::split_pane_to_new_window` and its result did not exist. The
  first red test also used the wrong remembered launch-context method name;
  source inspection corrected it to the existing `configure_pane_launch`
  rather than adding an alias.
- The first pure transaction now passes focused tests for multi-pane extraction,
  complete single-pane-worklane transfer, forbidden final-pane transfer, stale/
  empty/colliding identity rejection without mutation, normalized destination
  geometry, exact title/color/CWD/command/custom-title preservation, active
  source fallback, and moving canonical agent status to the destination. Strict
  Clippy initially rejected an internal `expect` as an undocumented panic; the
  lookup is now completed before mutation and propagates `None`, with no lint
  suppression. This is model evidence only and makes no live PTY-transfer claim.

## 2026-08-10 — Live pane-to-window transfer implementation and real-system evidence

- The initial window-local agent-transport design could not preserve a moved
  pane's inherited capability: a running child cannot change its socket path or
  token, and closing the source window would destroy that transport. The repair
  is one process-wide `AgentRuntime` owned by `ApplicationCoordinator`; each
  window retains only transcript enrichment and UI projection. This replaced
  the prior transport placement rather than adding another IPC path.
- Capability routing originally keyed only by worklane and pane. That is
  ambiguous when separate windows use the same worklane identity. The registry
  now records canonical `(window, worklane, pane)` ownership and retargets the
  unchanged token after transfer. Real stale source-window/worklane claims are
  ignored in favor of that authenticated canonical target.
- The live transaction now extracts the source model, detaches the exact
  Ghostty surface and its focus/callback/frame ownership, builds a destination
  shell with that pane launch-deferred, adopts the detached runtime exactly
  once, retargets IPC, presents/focuses the destination, and publishes through
  the existing aggregate persistence path. No replacement terminal, second
  shell implementation, or transfer-only snapshot path was introduced.
- Initial compilation found two implementation errors: the tmux failure reply
  is fallible, and the attempted map extraction API did not match Rust's actual
  signature. Strict Clippy then rejected an unbounded stale-reply iterator,
  excessive transaction arguments, and an oversized function. The repair used
  a bounded loop, the existing runtime bundle, and focused helpers; no lint was
  suppressed.
- The complete workspace test initially could not create eight private Unix
  sockets inside the filesystem sandbox (`EPERM`). The unchanged suite passed
  outside that restriction: all workspace targets, 105 Linux unit tests, and
  every real socket/CLI/transport test passed. This is recorded as a sandbox
  limitation, not a product pass from the failed invocation.
- The expanded controlled journey first raced the new split child's creation:
  Ghostty's terminal-ready receipt can precede the shell child becoming visible
  to `pgrep`. A bounded wait for exactly two real children replaced the
  instantaneous assumption; absence still fails. Wayland also accepted only a
  suffix of the first post-move physical input because the new toplevel had not
  settled focus. The journey now requires a fresh pane-focus receipt before
  typing rather than sleeping or accepting partial input.
- One manual Wayland invocation accidentally placed `GDK_BACKEND=wayland`
  outside the nested input wrapper. Its X11 bootstrap replaced the variable, so
  that receipt was rejected as Wayland evidence. Correct evidence always runs
  `nested-wayland-input env GDK_BACKEND=wayland ...`.
- The larger restore journey exposed a real compositor-specific product bug.
  X11 must present inactive restored windows first and the persisted active
  window last. Wayland focus-stealing policy requires the persisted active
  toplevel to map first because a later `present()` cannot take activation.
  Startup now selects ordering from the actual GDK display type. A delayed
  focus callback also ran after an immediately closed window and emitted GTK's
  “window shown after destroyed” warning; it now checks shell shutdown before
  presenting/focusing.
- Diff review found a rollback defect not exercised by the successful journey:
  tearing down a launch-deferred destination after failed adoption would revoke
  the moved pane's shared capability even though the destination never owned
  its runtime. Shell shutdown now unregisters only pane IDs backed by live
  runtimes in that shell. Deferred identities are not falsely treated as
  capability owners, so source rollback retains the running child's token.
- Pure tests cover multi-pane extraction, whole-worklane extraction, invalid
  and final-pane rejection without mutation, source focus fallback, agent state
  movement, destination geometry, CWD/command/title/color, next-pane numbering,
  and bookmark metadata for both extraction shapes. Action-router tests cover
  topology-sensitive availability; pane/sidebar tests pin the source command
  name and contextual control.
- After the rollback ownership repair, controlled X11 session
  `56b61a58475cec6cc75ac330abb369a2a35e4c92dbdffa88dca8a4356d5485e1`
  and controlled Wayland session
  `6fae17821119dd8cc426fa7f208885bce6daf81f0258ab4f409a27c87723ebae`
  passed the expanded real-product journey. It proves the PTY child set is
  unchanged across the move, pane 2 has exactly one terminal construction,
  pre-move scrollback survives, physical post-move input works, agent and tmux
  traffic route to the destination despite stale child claims, exact source and
  destination topology is projected, aggregate live/clean snapshots agree,
  clean and confirmed-live crash relaunch restore ownership, and closing the
  restored destination leaves the source PTY responsive.
- The feature inventory remains `PARTIAL`: **Move Pane to New Window** is now
  implemented, while move into an existing foreign worklane and cross-window
  drag/drop still require equivalent live-runtime evidence. A real forced
  destination-construction/adoption failure journey is also not yet present;
  successful-path evidence is not relabeled as rollback evidence.
- The first complete matrix run correctly refused qualification on three
  cells. Two API-audit cells found that the corrected direct Ghostty checkout
  now retains `upstream/main` although the provenance snapshot and prose still
  described that ref as absent. Both official and fork main resolve to the
  recorded `ac04fc276` base; the machine fact is now `true`, and the unchanged
  16-file/57-hunk/13-export audit plus normalization self-test pass.
- The source-UX journey exposed harness drift caused by the new fourth
  pane-local button: its former fixed Split Right coordinate now correctly hit
  **Move Pane to New Window**, changing real product state and failing the old
  expectation. No product behavior was weakened. The journey now clicks the
  leading control's reviewed coordinate and controlled X11 session
  `89256a5f227dee7f22adab2ee71bf01088a7b15c44f291a157921d0bd23a1749`
  passes the complete real pointer/PTY/source-UX path.
- After those repairs, every presently executable support and authoritative
  matrix cell passed in 357.44 seconds. Declared totals remain `PASS=90`,
  `FAIL=0`, `BLOCKED=7`, `XFAIL=1`, and `NOT_IMPLEMENTED=21`; implemented-local
  and product-boundary qualification pass, while release and full Linux
  qualification correctly remain false. The machine-summary SHA-256 is
  `8fd0dd5b5a901e75cb2cfbf3f5537fdc9830ffc2e20c9b0609420eb50849a0d7`.
- Debug Valgrind is **PASS with reviewed suppressions**, not unsuppressed clean:
  raw evidence contains 427 errors/contexts, 6,240 definite bytes, and 41,429
  indirect bytes; post-suppression evidence contains zero errors/contexts and
  zero definite/indirect bytes. The report, raw receipt, and suppressed receipt
  SHA-256 values are
  `61436226207caf00d23d865213c8548407471ba3bd9e16ffc9a0ee839bc53789`,
  `c3f4ea94a5923b7366def2216965d1854eaa44763d7e1f012d19d5502edcb78f`,
  and `b0eabbde73916a7739efca02d7d8b94b4e09f353b0e81d9a0f1d1685d832df68`.
  Suppression governance passed; ReleaseSafe Valgrind remains XFAIL and no
  suppression was added or broadened for this feature.

## AI disclosure

Initial repository analysis, implementation assistance, and this report were
prepared with OpenAI Codex under Jason Maskell's direction. Any proposed
upstream Ghostty communication or contribution must be reviewed, understood,
edited, and submitted by the human contributor in accordance with Ghostty's
contribution policy.
