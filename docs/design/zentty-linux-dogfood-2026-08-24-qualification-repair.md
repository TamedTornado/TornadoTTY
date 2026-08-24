# Zentty Linux qualification failure retirement dogfood

Date: 2026-08-24
Tracking: GH-88 and child issues GH-89 through GH-94

This append-only record starts the retirement of the non-pass outcomes from the
2026-08-23 local qualification receipt. The tracker issues own acceptance
criteria and test order. No full matrix rerun is permitted until isolated and
cluster evidence is green.

## GH-89: staged journey reproduction

- The preserved matrix logs initially suggested both staged journeys lost
  `build-metadata`. An independent X11 rerun proved that this was not an
  artifact deletion race: the staged bundle never copied that optional file.
- The actual cause of the metadata read was environment leakage introduced in
  GH-83. `qualify-local` exported the real Gemini binary/version around the
  entire matrix. Every invocation of `rust-agent-ipc`, including staged smoke,
  interpreted the mere binary variable as a request to run installed Gemini.
  That journey then read profile metadata which is not part of this deliberately
  assembled test bundle.
- Repair decision: only the two authoritative agent-integration cell commands
  select the required Gemini journey. `qualify-local` may put the reviewed pnpm
  bin directory on PATH, but it must not inject journey-selection variables
  into unrelated cells. The actor keys execution from
  `ZENTTY_REQUIRE_REAL_GEMINI=true`, never from an ambient binary variable.
- After the leaked Gemini journey no longer obscured the run, the independent
  X11 staged journey reached a second, real failure. Exact child-line mapping
  proved it was not `kill-window`: the public `zentty version` returned a full
  40-character commit while the ratified CLI contract requires 12 characters.
- This drift began when About/package provenance correctly changed
  `zentty_revision` to the full hash and the same value was accidentally reused
  as `ZENTTY_BUILD_COMMIT`. The repair keeps the full hash in build metadata and
  derives a separately validated 12-character display revision for the public
  CLI and embedded About surface.
- A current ReleaseSafe build completed in about 65 seconds. Its public receipt
  was `zentty 0.1.0 (5884df60005c)`, while `build-metadata` retained the full
  `5884df60005cec81f59bf022d203b32dc5b85035` provenance identity.
- The complete staged X11 journey then passed in its private Xvfb session:
  product smoke, the full compatibility-agent actor, tmux compatibility, and
  final staged-bundle assertion. The corresponding private Weston Wayland
  journey also passed. Neither run selected installed Gemini, and both receipts
  explicitly reported `real-gemini=prerequisite-not-requested`.
- The explicit installed-Gemini X11 scenario was then rerun against the rebuilt
  product and passed with `real-gemini=true`, proving the isolation repair did
  not disable the required journey.
- No concurrent artifact contender was involved in either reproduced failure,
  so an artifact mutex or immutable-bundle redesign would be a false repair.
  The shared environment-leak and full-vs-display provenance contracts are now
  checked by the focused orchestration contract. The authoritative full matrix
  has not been rerun.

## GH-90: controlled X11 focus reproduction

- `platform-clipboard-x11` reproduced independently and immediately with X11
  `BadMatch`, opcode 42 (`X_SetInputFocus`). A shell trace located the exact
  operation: `rust-pane-search` used a name-only `xdotool search`, accepted
  window `2097154`, and focused it without checking mapping or PID ownership.
- This actor had drifted from the shared input contract, which already binds to
  `--onlyvisible --pid "$product_pid"` and verifies the returned owner. The
  compositor warnings after the X error were unrelated cleanup noise.
- `rust-pane-search` owns three of the baseline failures, and
  `rust-task-runners` used the same stale/unmapped-window pattern. Both now
  require a mapped window belonging to their exact launched product before
  physical focus. No X error is suppressed and no alternate window is used.
- The mapped PID-owned lookup now lives once in `product-input`. Its focused
  runner rejects a missing mapped window and a visible window owned by another
  process, while retaining the exact successful XTEST sequence.
- Independent reruns passed for platform clipboard, terminal input, task
  runners, and the complete composite Ghostty API product-usage cell. The
  latter covered API audit, closed-pane restoration, full pane search, and tmux
  compatibility in one private X11 session.
- Platform clipboard and task runners were then deliberately run concurrently
  in two private Xvfb sessions. Both passed with distinct 64-hex session IDs,
  proving the repair scopes discovery by process/session rather than serializing
  unrelated X11 cells.

## GH-95: seven-day Rust crate quarantine

- An operator alert about an active Rust supply-chain attack interrupted visual
  qualification. The existing product boundary was useful but incomplete:
  `Cargo.lock` is committed and every authoritative Cargo build/test invocation
  uses `--locked`, so ordinary qualification cannot silently select a new
  publication. A deliberate `cargo update`, however, had no publication-age
  policy.
- Cargo 1.97.1 provides crates.io `pubtime` index data but stable Cargo rejects
  `-Z min-publish-age`. Adding only repository configuration would therefore
  advertise protection that the product toolchain did not enforce.
- A real disposable resolver probe selected exact `cc = "=1.4.4"`, published
  `2026-08-21T08:20:10Z`. Pinned nightly Cargo 1.100.0 rejected it as three days
  old under the seven-day policy and suggested the eligible 1.4.3 release. No
  crate was compiled and the product lockfile was not modified.
- The first attempted repair added a named update wrapper while leaving normal
  Cargo unconstrained. The operator correctly rejected that as procedural
  guidance rather than hardening, and the wrapper was removed before commit.
  The project now pins `nightly-2026-08-17` itself and enables native
  `min-publish-age` in `.cargo/config.toml`, so ordinary unwrapped Cargo commands
  inherit denial. GH-96 requires migration to the first stable Cargo release
  supporting this feature; the nightly pin is explicitly temporary.
- The independent audit intentionally does not trust resolver behavior because
  native Cargo permits young versions that are already locked. It enumerates
  every crates.io package record in `Cargo.lock`, reads the corresponding
  sparse-index records, requires canonical UTC publication times, and emits a
  machine-readable PASS/EXEMPT/FAIL receipt. Missing index records, missing or
  malformed times, and environmental absence are failures rather than passes.
- The first current-lock audit covered 77 crates.io packages: 77 PASS, 0 FAIL,
  0 exceptions. The youngest selected crate was
  `toml 1.1.4+spec-1.1.0`, published `2026-07-28T19:03:26Z` and 2,286,256
  seconds old at the receipt time. The oldest was `block-buffer 0.10.4`,
  published `2023-03-09T02:08:25Z`. No exception was needed.
- Deterministic negative tests now reject a too-new package, missing and
  malformed publication times, untracked authorization, expired and stale
  exceptions, a false PASS receipt, a resolver-policy override, and an
  unauthorized update. The valid paths prove both an old package and an exact
  Jason-authorized exception. CI consumes this as an advisory check and does
  not turn its receipt into release authority.
- **Audit correction before commit:** the initial independent audit consumed
  Cargo's active metadata graph, not every record retained in `Cargo.lock`.
  That silently omitted 14 inactive optional/target packages and made the
  preceding 77-package result incomplete. The complete lockfile contained
  `cc 1.4.4`, published only three days earlier. Native Cargo proved the gap by
  rejecting an ordinary `cargo update -p cc --precise 1.4.4`; the audit was
  repaired to parse all lockfile package records directly and then rejected the
  same crate. Cargo downgraded it to eligible `cc 1.4.3` without an override.
- The corrected complete-lock audit covers 91 crates.io packages: 91 PASS,
  0 FAIL, 0 exceptions. Its youngest package is now `wayland-backend 0.3.17`,
  published `2026-08-14T21:50:33Z` and more than nine days old. The incomplete
  77-package receipt is retained here as a discovery, not represented as final
  evidence.
- The initially selected `nightly-2026-08-24` resolved dependencies and passed
  tests, but strict all-target Clippy reproducibly hit a Rust compiler ICE while
  laying out GTK/GIO async opaque types. That toolchain is not an acceptable
  project pin. `nightly-2026-08-17` supports the same native Cargo policy and
  completes strict all-target Clippy, so the pin was moved back one week. GH-96
  still requires stable migration as soon as stable Cargo supports the policy.
- Final validation on the selected toolchain passed the deterministic policy
  suite, the 91-package complete-lock audit with zero exceptions, strict
  all-target Clippy, the complete workspace/all-target Rust suite, CI manifest
  and workflow contract tests, and a ReleaseSafe product build. The first
  sandboxed workspace run could not bind its real Unix test sockets and failed
  explicitly with `EPERM`; the required elevated rerun exercised those sockets
  and passed. This was environmental absence, not converted into a pass.
- A final ordinary, unwrapped `cargo update -p cc --precise 1.4.4` on
  `nightly-2026-08-17` was rejected because the crate was three days old versus
  the seven-day minimum. The rejected probe left `Cargo.lock` on eligible
  `cc 1.4.3`; the complete-lock audit remained 91 PASS and zero exceptions.
- Final diff review caught an orchestration defect before commit: the first
  `build-local` edit placed the audit after the build-metadata environment
  assignments, causing those assignments to apply to the audit instead of the
  following Cargo build. The audit now runs as a separate command immediately
  before the intact Cargo environment block. A structural regression test fixes
  that ordering and adjacency, and the repaired ReleaseSafe build passed.

## GH-91: fractional-scaling visual and geometry repair

- The exact `fractional-scale-wayland` actor correctly rejected an ambient run
  without its owned mixed-scale compositor. In the required private labwc/Xvfb
  environment, its first mismatch exposed ambient product state in the reviewed
  images: the actor launched from the mutable source checkout, so branch,
  dirty-tree, and review polling changed screenshot content asynchronously.
  Scaling evidence now launches from its private run root. Two independent
  `wide-dark-wayland` captures were byte-identical with SHA-256
  `843d8918f831f0981ac88c1c1df1013fada3dd2ceb87ac7337461bac0a825cfe`.
- The first private 1.5x captures still alternated by 2,655 pixels. Exact image
  analysis localized every difference to the fractional pane scrollbars: the
  screenshots were sampling different frames of their fade. The actor now
  disables GTK animations and refuses to publish until two consecutive exact
  compositor captures agree. It does not turn an unsettled frame into a pass.
- The former capture selected the brighter of both wrapper-owned output
  windows. After `MoveToOutput`, wlroots can retain the previous frame on the
  old output, so that heuristic published the 1.5x image again as purported 2x
  evidence. Each stage now records the exact outer X11 window that delivered
  real pane-local pointer events and captures only that pointer-proven output.
- A second false-positive was older `preferred_scale(240)` text satisfying the
  post-move wait. Scale waits now require the observed event count to increase
  after the operation. This exposed that the combined `wlr-randr` request had
  silently reordered outputs: X11-1 was at 1196 rather than the assumed origin,
  so “move right” never moved the product. Output scale and placement are now
  applied and asserted in ordered steps; the current pointer-proven output is
  moved left into the controlled 2x output. ReleaseSafe and Debug both traverse
  different outer window IDs at stage 3.
- Output reconfiguration publishes terminal rows and split columns in separate
  layout steps. The actor initially recorded old or hybrid PTY geometry, even
  though the eventual UI was correct. Each real PTY now requires changed
  geometry to remain identical for a sustained 400 ms before recording. The
  final Wayland receipts agree across both panes and profiles: 1x split
  `32x36` at `10x21` cells, 1.5x `20x23` at `19x42`, and 2x `14x16` at
  `19x42`, with increasing SIGWINCH counts at both logical-size transitions.
- The reviewed 1.5x and 2x images were replaced only after independent exact
  repeats. Final SHA-256 values are
  `7203ccf9380a43c771000e153f99e81f783178025616b838e265778142f49a3e`
  and `ed685f2c36252e0ff8e5fa4e6aa6297ef179fe3efe222c74eede0bf78d7002b8`
  respectively. The earlier matching-but-unsettled receipts remain documented
  here as discoveries, not final evidence.
- The shared repair was also exercised through the real owned Xwayland cell.
  Its 512-logical-pixel 2x viewport revealed that the old 100-pixel pointer scan
  could not produce two distinct clicks inside a narrow split pane, and that an
  arbitrary 180-pixel terminal-width assertion rejected the reviewed compact
  layout. A 50-pixel physical scan and an explicit minimum of 12 terminal cells
  now test usability without assuming sidebar geometry. The complete Xwayland
  ReleaseSafe/Debug journey passed with exact windows `1024x743`, `682x487`,
  and `512x359`, stable equal pane geometry, real PTY SIGWINCH, and real pointer
  translation at every stage.
- Final focused results: the scaling harness contract, ShellCheck, the complete
  controlled Wayland scaling cell, and the affected complete controlled
  Xwayland scaling cell all pass. No ambient absence or visual mismatch was
  converted into a pass. At that checkpoint, the remaining GH-91 cells and the
  authoritative full matrix had not yet been rerun.

### Fullscreen Wayland window state

- The isolated `product-window-state-wayland` mismatch contained only 943
  pixels in the centered window context. The current product showed the
  intentional generic project-folder icon beside `Visual window`; the reviewed
  baseline omitted it. Project icons predate the baseline, and the product log
  proved `owner=window-chrome fallback=folder-symbolic decoded=true`, so the old
  image had captured before asynchronous chrome projection rather than
  documenting an intentional icon-free state.
- Fullscreen evidence now waits for that exact project-icon projection before
  capture. Consecutive-frame settling moved into the shared visual-evidence
  owner, where deterministic tests prove an eventually stable capture is
  published and an alternating capture fails explicitly. The scaling actor now
  consumes the same shared contract instead of maintaining a second settling
  implementation.
- Two independent private Weston runs produced the byte-identical icon-bearing
  image `aa16ae3fd62f2186c606920b91fa36e8baf229210b172371f080a033877222ab`.
  After visual review, that image replaced the stale baseline. A third exact
  cell run passed the real F11 fullscreen entry and exit round trip; Wayland
  minimize remains honestly described as a compositor request because the
  protocol does not acknowledge minimized state.
- After consolidating capture settling, both complete scaling journeys were
  rerun through their real private environments. Wayland and owned Xwayland
  again passed ReleaseSafe and Debug, proving the shared helper preserved the
  pointer-proven output and exact scaling contracts.

### Remote-file and sidebar-resize visual evidence

- The exact controlled `remote-file-drop-wayland` cell first rejected 1,684
  changed pixels, all localized to the centered window context: the current
  product had projected the intentional generic folder icon beside
  `Active shell · jason@127.0.0.1`, while the stale baseline had sampled the
  asynchronous chrome before that projection. The actor now requires the owned
  `project-icon-projected owner=window-chrome fallback=folder-symbolic
  decoded=true` receipt and two identical compositor frames before publication.
- The first two pixel-identical Wayland captures nevertheless had different
  file hashes (`01396...` and `ee9b...`). Image inspection found an ancillary
  PNG capture-time chunk rather than a pixel difference. The shared settling
  owner now strips ancillary metadata when publishing the agreed frame, and a
  negative runner test rejects retained `png:tIME` metadata. Two subsequent
  captures were byte-identical and pixel-identical at SHA-256
  `cf51dc767c97e05ed5524daef6d5456893931db03f3aaa756b440583c2d93a6a`
  with AE 0. The reviewed baseline was updated only then.
- The same hidden timing defect existed in the X11 remote baseline even though
  GH-91 had named the Wayland mismatch: the first corrected capture differed by
  1,759 pixels in the same icon-bearing title context. Two controlled Xvfb
  captures agreed exactly at
  `656cf51dbf6adf8107c798c43a8b2a590fd6d8a6a44558ba766468d2a4afcd59`
  with AE 0, so the X11 baseline was repaired at the same owner rather than
  preserving backend-dependent stale evidence.
- Both complete remote journeys then passed, not merely their screenshots.
  Wayland session `cdc74b00...` and X11 session `735ba50a...` exercised the real
  loopback SSH server, file and PNG paste, physical drop, batch rollback,
  identity cancellation, two background agents, exact scrollback, clean
  relaunch, SIGKILL recovery, and corrupt-state recovery. Their controlled
  environment reports both record command exit 0.
- The sidebar-resize mismatches had the same missing project icon: 1,132 changed
  pixels on X11 and 1,627 on the initial Wayland comparison. Wayland also
  exposed a second unsettled input: its old baseline retained Ghostty's
  transient `71x30` resize overlay, while a later frame did not. The actor now
  disables that overlay and GTK animation on both backends, waits for the
  project-icon projection, and requires consecutive identical frames. It does
  not bless whichever transient frame happens to arrive first.
- Reviewed repeat captures were exact at
  `61c8937aaa4be16aefc09e50884c678bb294fe92bbe8d2116f9390351fa864be`
  for X11 and
  `896f1701ff978a31a41910b1902ae38af89da40d3f004e1d69ef81d938668991`
  for Wayland, each with AE 0 between independent runs. The final exact cells
  passed real divider drag and persisted width on X11, and real outer-pointer
  drag, width 330 persistence, hide/show preservation, same-PTY input, and
  clean close on Wayland. Environment sessions `209d24f...` and `9ce181e2...`
  record exit 0.
- An initial sandboxed X11 invocation could not create the private X socket
  because `/tmp/.X11-unix` is root-owned. That absence failed explicitly. The
  required elevated reruns used the harness-owned Xvfb/Xauthority environment
  and passed; the failed ambient attempt was not converted into evidence.
- Because PNG canonicalization is shared by the scaling actor, both full scaling
  clusters were rerun after this repair. Controlled Wayland session
  `5e8c3314...` and owned-Xwayland session `26dfe58e...` again passed their
  complete ReleaseSafe and Debug 1x/1.5x/2x geometry, pointer, PTY, and SIGWINCH
  journeys. Remaining uncertainty is limited to the other matrix cells outside
  this focused GH-91 cluster; no full-qualification claim is made here.

## GH-92: graphical and persistence interference isolation

### Recovered baseline and first independent replays

- The original GH-88 console receipt was recovered from the durable Codex
  rollout rather than reconstructed from memory. It contains 36 direct
  unexpected outcomes and six dependency-blocked outcomes: 34 ordinary FAILs,
  two invalid Debug Valgrind reports, and six cells blocked by failed
  prerequisites. This recovered set is the input to the machine-readable
  failure ledger; no baseline failure may disappear merely because a later
  isolated run passes.
- `ime-fcitx-wayland` passed independently in controlled Weston/Fcitx with real
  preedit, cancel, commit, and focus transfer. A noisy GNOME portal backend
  crashed during the run, but the owned Fcitx journey and receipt still
  completed. The baseline input failure is therefore not independently
  reproducible; portal noise is retained as uncertainty rather than called a
  product pass for the whole cluster.
- `product-existing-worklane-transfer-wayland` also passed independently. Its
  previously missing physical menu target was present and the exact controlled
  transfer journey completed.
- `product-multi-window-clean-crash-size-restore-wayland` reproduced alone at
  the first cancelled close dialog: after Escape, Weston nested under X11 left
  the parent toplevel inactive and the next input did not reach
  `pane-window-2-1`. Instrumentation established the state precisely instead
  of adding a delay: confirmation cancellation completed, the selected pane
  remained correct, but the parent was inactive and could not own keyboard
  focus.

### Confirmation focus repair and private-display overlap

- The application now restores the selected Ghostty surface after a cancelled
  confirmation. If the compositor has already reactivated the parent it does
  so immediately; otherwise it waits for the parent's real `is-active`
  transition and disconnects that one-shot observer before restoring focus.
  Confirmed pane/worklane removal uses the same path for its surviving surface;
  window and application shutdown deliberately retain their existing teardown
  path.
- Weston-in-X11 does not return activation to a transient's parent on dismissal.
  The actor therefore supplies a real pointer click to the exact pane before
  requiring the focus-restored receipt. This is not a sleep-only repair and it
  does not pretend an inactive window accepted input. The same explicit
  surviving-pane target was missing from the actor's final idle-window close;
  that omission caused a second apparent shutdown timeout after the product had
  correctly closed the other window.
- The focused lifecycle mode now excludes unrelated X11 screenshot publication.
  It exercises two real GTK toplevels, real Ghostty PTYs, cancel and accept
  paths, survivor input, and PTY reaping without allowing a visual baseline to
  hide lifecycle results.
- Two repeated paired runs launched the X11 and Wayland lifecycle actors at the
  same time with private displays and private state. All four runs passed. The
  Wayland sessions were `381103d8...` and `1b3f99a2...`; X11 sessions were
  `efc2039d...` and `8f7b915f...`. This proves these unrelated private-display
  actors can overlap and need no generic graphical mutex.
- The exact full Wayland cell advanced through the repaired confirmation and
  idle-close stages, then failed later when its physical pointer scan could not
  identify pane 2's toplevel. That is a distinct remaining replay finding. The
  full cell and GH-92 remain non-pass; focused lifecycle evidence is not being
  represented as full Linux qualification.
- `linux/qualification-failure-ledger.json` now records all 42 recovered
  outcomes individually: 36 direct unexpected results and six dependency
  blocks. Its current replay totals are 16 PASS, two FAIL, 18 NOT_RUN, and six
  BLOCKED, with 28 unresolved entries once passing-but-unclassified replays are
  included. A pass on one isolated replay therefore cannot silently erase the
  baseline or claim that its interference source is known.
- The ledger validator requires every ID to exist in the authoritative matrix,
  enforces unique IDs, declared baseline totals, outcome/status vocabulary,
  attempt counts, evidence text, and direct-versus-dependency invariants. Six
  negative fixtures cover missing and duplicate cells, unknown status, false
  zero-attempt PASS, unknown matrix identity, and malformed dependency state.
  The validator is part of both local qualification support and the public PR
  subset's advisory checks, so tracker drift is caught without making GitHub CI
  release authority.

### Remaining independent replays and narrow isolation repairs

- Every remaining graphical or persistence baseline failure was replayed alone.
  The only ledger entries still `NOT_RUN` are the two Debug Valgrind executions,
  which belong to the memory-safety child issue. Current totals are 22 PASS,
  12 FAIL, two NOT_RUN, and six dependency BLOCKED; 22 entries remain unresolved
  after including passing replays whose interference source is still unknown.
- Three more X11 actors had independently accreted the name-only window lookup
  already removed from pane search and task runners in GH-90. Development
  servers, git review, and sidebar management could resize or click a withdrawn
  window from another process. They now consume the shared mapped,
  PID-verified product-input owner. Development servers and git review pass
  independently with that repair. Sidebar management advanced past its missing
  pane-3 symptom and exposed a distinct later card-rebuild defect, which remains
  FAIL rather than being hidden by the input repair.
- A deliberate development-server/git-review contender pair found the next
  exact shared resource: private X displays do not isolate `/proc`. The
  development-server scanner observed the contender's real forge listener and
  changed governed chrome evidence. The actor now re-execs itself as PID 1 in a
  user-owned PID namespace with a private `/proc`; absence of `unshare` or the
  namespace is an explicit skip/failure, never a pass. The ordinary and pinned
  Docker journeys pass in that namespace. Two repeated concurrent contender
  pairs then passed with distinct private Xvfb sessions, proving no generic X11
  or graphical mutex is needed.
- Both project-icon cells reproduced alone because their test still prohibited
  every projected icon after an escaping symlink miss. GH-91 had ratified the
  generic folder fallback as intentional window chrome. The security assertion
  now rejects the exact outside canonical path while requiring the safe generic
  fallback; opt-out likewise rejects the resolved project path rather than the
  generic slot. X11 and Wayland pass alone and in two repeated concurrent
  private-display pairs. This was stale test policy, not a weakened containment
  boundary.
- The X11 shortcut cell passed independently. The Wayland shortcut, About,
  bookmarks import, workspace restore on both backends, global find, and
  Valgrind governance failures all reproduced alone and remain explicit FAILs.
  Workspace restore is notably identical across backends: stored divider width
  about 429 is relaunched as about 376.619 while vertical weights survive.
- Two matrix cells selected a compositor incapable of the behavior their actor
  asserted. Agent integration ends in a remote-session visual contract written
  for Weston's 1280x1024 output, while default Cage owns 1024x768. Config reload
  requires real pointer exit from a transient sidebar, which Cage's keyboard-only
  input profile cannot provide. Global find also needs independent physical
  targeting of multiple toplevels. These cells now select the private Weston
  input profile. Raw settings keystrokes in config reload were consolidated
  onto the environment-aware input owner; the Weston replay then advanced to a
  separate live quit-confirmation defect. The later failures remain FAILs, so
  correcting the environment profile does not manufacture a pass.
- The first full agent replay under Weston exposed an additional within-cell
  contaminant: the preceding tmux actor deliberately resized the wrapper-owned
  compositor window and never restored it, so session restore observed
  1180x974 instead of the owned 1280x1024 output. The tmux actor now snapshots
  and restores only that exact wrapper window in its cleanup. The next full
  composite replay passed real Gemini, tmux, and installed Claude integration,
  and passed the formerly blocked remote visual geometry before reaching a
  separate remote file-drop completion failure. Agent integration therefore
  remains FAIL; the actor-state repair is not presented as completion.
- Fcitx composition and existing-worklane transfer each passed alone, then
  passed in two repeated concurrent contender pairs. Their baseline failures
  are now classified as non-reproducible baseline outcomes, not invented
  interference: both private Wayland sessions remained distinct, and no shared
  resource or generic graphical lock is justified by the evidence.
- The first three-cell cluster combined Wayland confirmation lifecycle,
  private-PID development-server discovery, and X11 project icons. Lifecycle
  and icons passed, but development-server visual evidence changed under load
  despite process isolation. Exact inspection found its screenshot still used
  a fixed 300 ms delay. It now waits for the asynchronous generic project icon
  and requires consecutive identical compositor frames through the shared
  visual-evidence owner. The repeated three-cell cluster then passed all cells:
  Wayland session `5687afd3...`, development-server X11 session `048f1c8e...`,
  and project-icons X11 session `8fb47aa6...`. This is bounded settling, not a
  scheduler mutex or enlarged sleep.

### Pixel-exact workspace divider restoration

- The identical X11 and Wayland restore failures were a product defect, not a
  compositor discrepancy. A restored 1000 px window initially reported an
  819 px pane viewport while GTK still allocated only 180 px to the configured
  280 px pinned sidebar. The next allocation reported the final 719 px pane
  viewport. Zentty treated that startup allocation correction as a user-visible
  resize and multiplied every restored multi-column width by `719 / 819`,
  changing the stored 429 px divider to 376.619 px. The existing diagnostic
  receipt made the exact factor observable; vertical weights were never
  affected.
- The macOS source scales multi-column widths when its resolved readable layout
  context changes, but it does not publish an unresolved initial layout bound.
  The Linux shell now follows that ownership boundary: while the sidebar is
  pinned, pane-width reconciliation waits until GTK's actual sidebar allocation
  is within one physical pixel of the configured, clamped width. Hidden-sidebar
  layouts remain immediately eligible, and later settled viewport changes keep
  the existing proportional-resize behavior.
- A focused unit contract rejects the observed 180/280 startup state, accepts
  GTK's one-pixel rounding tolerance, and proves a hidden sidebar cannot block
  reconciliation. The five generated mutants for this predicate were all
  caught by the project-owned resource-isolated mutation runner. An initial
  direct `cargo mutants` invocation correctly failed because its gitignored
  scratch tree omitted the pinned Ghostty build; no mutant ran. Re-running via
  `linux/tests/mutate-rust`, as project policy requires, supplied the external
  library and caught all five mutants in two minutes.
- Fresh ReleaseSafe real-product journeys now preserve the exact stored
  horizontal divider and vertical weights through clean exit and relaunch on
  both backends. The private Xvfb session was `8737df3e...`; the controlled
  Weston session was `d7f8b26f...`. Both used physical divider drag and
  double-click input, three real Ghostty PTYs, the persisted clean-exit recipe,
  and post-restore terminal input. The failure ledger entries are now PASS with
  two attempts each; this repairs two cells only and is not a full Linux
  qualification claim.

### About navigation and catalog provenance

- The reported Wayland Back-action failure initially looked like a product
  navigation defect because the actor accepted an old `licenses-back` focus
  receipt. GTK briefly focused Back while changing stack pages, then the
  product deliberately focused the license search. The later Shift+Tab wait
  matched that stale line and sent Return to the search field. The actor now
  records the focus count before physical navigation and requires a new receipt.
  This exposed the real environment boundary: Cage's wtype virtual-keyboard
  path did not deliver Shift+Tab as backward GTK navigation, even with explicit
  modifier-frame separation or a direct `ISO_Left_Tab` keysym. Absence was not
  converted into a pass. The cell now uses the controlled Weston profile and
  compositor-visible outer-X11 input, which delivers the physical chord to the
  real Wayland client and preserves the exact keyboard-order assertion.
- Advancing past Back uncovered a separate staged-product security defect. The
  package catalog records a full 40-character Zentty revision, but the build
  exported only the 12-character display commit. `AboutMetadata` rejected that
  short value as malformed, reduced it to `unknown`, and catalog loading then
  intentionally skipped revision matching. A tampered catalog was therefore
  accepted even though the actor expected an exact mismatch diagnostic.
- Build metadata now keeps the short public commit while separately embedding
  `ZENTTY_BUILD_REVISION` with the full provenance identity. About accepts the
  12-character display commit for presentation but validates package notices
  only against the full 40-character revision; catalog source identities use
  that same strict revision validator rather than the display validator.
  Focused tests distinguish both formats, reject wrong lengths and alphabets,
  and all 12 generated mutants for the two validators, their parser use, and
  the compiled-revision selector were caught by the
  resource-isolated project runner.
- The build-orchestration mirror had also gone stale after GH-92 moved agent
  integration and global find to controlled Weston. It now mirrors those
  authoritative profiles and accounts for the additional provenance variable;
  the orchestration and matrix validators pass again.
- Fresh ReleaseSafe About/Licenses journeys passed concurrently and completely
  on private Xvfb session `26fc6dc7...` and controlled Weston session
  `17faa4c1...`. They use a
  real Ghostty PTY, physical palette and keyboard navigation, an exact installed
  catalog, a controlled external-link handler, and a second copied product
  bundle whose deliberately stale catalog is rejected but remains recoverable
  in the UI. Only the Wayland failure-ledger entry changed state; this remains
  focused evidence, not full qualification.

### Shortcut settings across real Wayland toplevels

- The initial Wayland replay did not reach the recorded import defect. The
  actor invoked `wtype` directly for settings-window and native-chooser input,
  even though the controlled Weston profile deliberately exposes input through
  its compositor-owned outer X11 window and does not advertise the virtual
  keyboard protocol. That was a second, scenario-local input system. Shift and
  Ctrl+Alt injection are now owned by the existing environment-aware product
  input driver, and every settings and chooser chord/type operation uses that
  driver rather than selecting a backend independently.
- Once real input reached the secondary settings toplevel, the journey advanced
  through recording, conflict replacement, export, and import. Its next failure
  showed that the old reactivation click targeted `(180, 34)`, outside the
  centered settings client in the 1280x1024 Weston desktop. The chooser closed
  correctly, but no Wayland client received that gesture. The actor now clicks
  the settings content center before continuing. After Settings closes, it
  likewise performs a real compositor click on the exposed terminal before
  requiring the existing `focus-pane` receipt; `present()` alone is not treated
  as compositor activation.
- Controlled Weston session `85dea5f4...` passed the complete real journey:
  two GTK toplevels, native export and import choosers, physical shortcut
  recording and conflict replacement, persisted bindings, two live Ghostty
  surfaces, runtime configuration reload without PTY replacement, theme
  projection, and process restart. Matching private-X11 session `f049f9ab...`
  also passed. The matrix now assigns the Wayland cell to the authoritative
  multiwindow Weston profile. The failure ledger records six bounded attempts
  and marks this one cell PASS; no broader qualification claim is made.

### Bookmark dialogs, keyboard popovers, and native chooser acceptance

- The bookmark replay first exposed a real product lifecycle gap after the Save,
  Rename, and Edit dialogs closed. Calling `close()` removed the transient, but
  Weston did not thereby activate its parent. Those exits now present the
  parent and publish both the request and eventual `is-active` transition. The
  actor then supplies the compositor-owned physical click that Wayland requires
  and waits for the real terminal focus receipt before continuing; environment
  absence or `present()` alone is not accepted as success.
- Advancing into the nested template action popover exposed a second product
  bug. Zentty opened both the bookmark popover and its keyboard context menu on
  key-down, while Weston was still delivering the originating chord releases.
  The releases returned routing to the underlying terminal and could dismiss
  the just-opened context menu. Zentty now defers search focus and keyboard-menu
  presentation by a bounded 50 ms key-release settlement interval. Search
  changes publish their
  actual query, so the actor retries only after proving that physical text did
  not reach the real search entry; a real outside click resets the mapped
  popover between at most three attempts. This is not a blind pass-producing
  sleep: focus, query, visibility, and action receipts remain mandatory.
- The original import failure was then reproduced after export and deletion.
  A captured real GTK chooser showed the full exact path in its location entry,
  the matching `Portable.zenttypreset` row selected, and the enabled Open
  button. The first Return resolves the location and selects that row; it does
  not accept it. The actor now performs the second ordinary keyboard activation
  and requires the application's persisted import receipt and portable JSON
  invariants. The native chooser uses the same 750 ms asynchronous mapping
  bound already qualified by the shortcut import/export journey.
- Controlled Weston session `457aa19c...` passed save, real export chooser,
  physical delete, real import chooser, and persisted portable import. Weston
  management session `ed75d68d...` separately passed Rename, Edit, Duplicate,
  Pin, Convert, Export-menu traversal, linked Update/Unlink, and Delete through
  the same context and dialog paths. Matching private-X11 sessions
  `cd4e5f1d...` and `079a3330...` passed. The Wayland matrix cell now owns the
  multiwindow Weston profile, and the ledger records 18 diagnostic/repair
  attempts before PASS. This changes one failure-ledger cell only and is not a
  full Linux qualification claim.

### Stable sidebar cards with current pane-drag identity

- The repaired PID-owned X11 actor advanced beyond its earlier window lookup
  failure and exposed a product lifecycle defect: creating nine real worklanes
  rebuilt an unaffected worklane card nine times. Card compatibility included
  the global topology generation, so every render invalidated every card even
  when its pane membership was unchanged. The journey correctly rejected that
  churn rather than treating the final visual state as sufficient.
- The generation coupling had also hidden a correctness dependency. Sidebar
  drag controllers captured their pane column, generation, and presentation
  when the card was constructed. Simply dropping the compatibility check would
  have left retained cards able to start stale drags. Zentty now has one shared
  `PaneDragSourceState`: topology renders advance its generation and refresh
  the complete payload catalog; metadata-only refreshes update that catalog
  without inventing a topology change. A real GTK drag snapshots the current
  payload during `prepare`. That immutable snapshot remains stale if topology
  changes while the drag is in flight, so the existing source/destination
  generation validation still fails closed.
- Focused tests prove that a retained controller reads the latest column,
  generation, and presentation, that a prepared payload does not silently
  become current after a topology change, and that a removed pane cannot start
  a drag. All six generated `PaneDragSourceState` mutants were exercised by the
  resource-isolated project runner: five were caught and one was unviable.
  Strict all-target Clippy passed. The complete package suite passed only in
  the required elevated environment because its real `/proc` listener test
  cannot bind inside the filesystem/network sandbox; the initial sandboxed
  `EPERM` was retained as environmental evidence, not called a pass.
- A fresh ReleaseSafe bundle passed the complete sidebar journey in private
  Xvfb session `da8bc03b27687c364c2f969282ef3da44271322783c14ff46d6249c1235ca5cd`:
  nine real Ghostty PTYs, overflow reveal, pointer reorder, keyboard reorder,
  contextual transfer, stable unaffected rows, and post-transfer input. The
  related real GTK pane-drag journey passed in private Xvfb session
  `3e5eac89e41bcac571b865a151a194a61a2993426eb7702f34d58bc7634a9f81`,
  preserving exact topology and the live PTY. The ledger now marks only
  `product-sidebar-management-x11` PASS after four attempts; this is not a
  claim of full Linux qualification.
