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

## AI disclosure

Initial repository analysis, implementation assistance, and this report were
prepared with OpenAI Codex under Jason Maskell's direction. Any proposed
upstream Ghostty communication or contribution must be reviewed, understood,
edited, and submitted by the human contributor in accordance with Ghostty's
contribution policy.
