# Zentty Linux pane-transfer dogfood record — 2026-08-10

Tracking: [GH-33](https://github.com/TamedTornado/zentty/issues/33)

This record continues the implementation checkpoint in
`zentty-linux-dogfood-2026-08-09.md`. It covers the one acceptance gap that
kept the feature open: real destination-failure rollback.

## Acceptance audit and test-first failure design

- Commit `299888f` already implements the successful source feature and proves
  live Ghostty/PTY transfer on controlled X11 and Wayland. The public issue was
  deliberately left open because successful transfer does not prove rollback
  after the source runtime has been detached.
- No test-only application mode, second actor, or injected alternate terminal
  owner is needed. Every destination shell constructs its real persistent tmux
  compatibility product. An externally written future-version tmux store is a
  deterministic failure at that existing production construction boundary.
  The source shell has already loaded its product, so this fault occurs after
  live-runtime extraction and before destination adoption.
- The existing `rust-multi-window` actor now specifies the missing behavior
  before any product repair: split a second real pane, record the real PTY child
  set and one terminal-ready construction, inject `{"version":2}` at the
  isolated session's real tmux store, invoke **Move Pane to New Window** through
  the real command palette, and require the exact construction error.
- Rollback acceptance is deliberately broader than a topology receipt. The
  actor requires no destination toplevel, exact source topology and focus,
  unchanged PTY PIDs, one pane-2 surface construction, physical input through
  the restored callback, authenticated agent and tmux routing under the source
  identity, and a one-window aggregate live snapshot containing the pane once.
  Environmental absence cannot pass this journey.

## Focused real-system result

- The new acceptance journey passed without a product-code repair. This is a
  useful result rather than a green test invented after the fact: the existing
  transaction already restored its extracted model and re-adopted the detached
  live runtime when real destination construction returned the future-schema
  error. The missing work was proof of that path, not another implementation.
- Initial controlled X11 session
  `cf529e4b28fcef994ccf988ebcb07a69b3c7f58cc0f39547e3d70ab8feefd287`
  and Wayland session
  `64e5e6ba2d420083aedba575c8ceaa81ea741c0168a368cc7e2ac163bd0ab7cc`
  passed the complete multi-window actor with the new rollback segment. The
  first actor draft removed the injected file after failure. Review tightened
  this before final qualification: the actor now snapshots whether the real
  store existed and, when present, restores and byte-compares its exact prior
  contents before post-failure routing checks. The injected schema cannot
  survive the isolated session or become a second persistence policy.
- Focused evidence is not the final issue receipt. The actor's final
  machine-readable line is being strengthened to name the real construction
  rollback, after which both compositor journeys and every presently executable
  matrix cell must be rerun before issue closure.
- After exact store-byte restoration and the explicit machine receipt were in
  place, controlled X11 session
  `e4884332fb88833e192883dee09102597c0e8b7f80252ae2a7997a55064f2a65`
  and controlled Wayland session
  `2e4e9a6587690b6cfb414445d9cea853eb58afcc61f6d0274ec7a00104c80c75`
  passed. ShellCheck also passes the actor with sourced-file analysis enabled;
  the deliberately child-evaluated command is documented rather than hidden by
  a broad lint exclusion.
- The complete locked Rust workspace passed, including 108 Linux-shell tests,
  every real socket/CLI/transport test, and all doc tests. Strict all-workspace,
  all-target Clippy passed with warnings denied. Architecture, inventory, and
  orchestration validators also pass; the actor count remains unchanged.

## Final qualification receipt

- The mandatory complete `linux/tests/qualify-local` run passed every presently
  executable support and matrix cell in 511.960 seconds. Declared totals are
  `PASS=91`, `FAIL=0`, `BLOCKED=7`, `XFAIL=1`, and `NOT_IMPLEMENTED=21`.
  Implemented-local and product-boundary qualification pass. Release and full
  Linux qualification correctly remain false while declared non-PASS cells
  remain visible. The machine-summary SHA-256 is
  `59e3cec69fb5de020bf61e815c5bc1db4bbe103867d9d6376e599b33a4767035`.
- Both authoritative multi-window cells executed the new real construction-
  failure rollback journey: controlled Wayland passed in 19.070 seconds and
  controlled X11 passed in 34.140 seconds. This evidence is part of the normal
  matrix, not a standalone manual receipt.
- Debug Valgrind is **PASS with reviewed suppressions**, not unsuppressed clean.
  Raw evidence contains 427 errors/contexts, 6,080 definite bytes, and 41,394
  indirect bytes. Post-suppression evidence contains zero errors/contexts and
  zero definite/indirect bytes. The report, raw receipt, and suppressed receipt
  SHA-256 values are respectively
  `612930197b9705dda7428c3c7a9b313dc7e1bd1d983c6efdf4f841035103f90d`,
  `6052a9e09047d4ee963e262aff99fa9cbe2b2bbc06b84464933db9a408ca7907`,
  and `7783c6994b8094b5f3757c2656e29aa352bdbf93dc0eaadbf579e8837c95227e`.
  Suppression governance passed, ReleaseSafe Valgrind remains XFAIL, and this
  acceptance closure added or broadened no suppression.
- Implementation commit `299888f` and rollback-qualification commit `053fc74`
  are public on `linux/port`. The exact receipts were posted at
  `https://github.com/TamedTornado/zentty/issues/33#issuecomment-5238079227`,
  and issue #33 was closed as completed. The umbrella inventory remains
  `PARTIAL` only for separately scoped existing-window transfer and direct
  cross-window drag/drop behavior.
