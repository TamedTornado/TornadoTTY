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

## AI disclosure

Initial repository analysis, implementation assistance, and this report were
prepared with OpenAI Codex under Jason Maskell's direction. Any proposed
upstream Ghostty communication or contribution must be reviewed, understood,
edited, and submitted by the human contributor in accordance with Ghostty's
contribution policy.
