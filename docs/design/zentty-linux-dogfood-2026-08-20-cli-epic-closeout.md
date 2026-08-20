# Zentty Linux dogfood — CLI epic closeout

Date: 2026-08-20
Tracking: GH-22; children GH-42 through GH-49; dependencies GH-9, GH-10,
GH-14, and GH-32

## Frozen scope

This is one reconciliation and closeout sweep, not another CLI implementation
layer and not an app-wide qualification campaign. The eight GH-22 child issues
are closed with their own committed contracts, unit/mutation evidence, and
real-product journeys. Packaging, advisory CI, tmux compatibility, and live
multi-window authority are also complete. This sweep must:

1. compare the two GH-22-owned feature-inventory entries with the current
   machine-readable CLI and agent-event contracts;
2. distinguish a stale ledger status from a genuine product gap;
3. run the existing focused contract and real shell/CLI actors without adding
   another harness;
4. update the plan, inventory, issue ledger, and GH-22 only if every epic gate
   is already backed by executable evidence; and
5. retain all unrelated matrix BLOCKED, XFAIL, and NOT_IMPLEMENTED cells.

## Initial discovery

- `shell.environment-integration` is still `NOT_IMPLEMENTED`, with no focused
  tests or product scenarios, even though GH-45 closed in commit
  `a9c54c969a5a182a1844250a7030aa1e8d8bf80a`. The authoritative matrix has
  twelve PASS cells: four staged real-shell resource/startup contracts and
  Bash, Zsh, Fish, and Nushell real Ghostty-pane journeys under both X11 and
  Wayland. This is ledger drift, not missing product code.
- `cli.topology-control-grid-notify` remains `PARTIAL` and its test plan still
  names GH-46/GH-47 as remaining work, although GH-42 through GH-49 are closed
  and the matrix has PASS cells for the complete real CLI journey on X11 and
  Wayland.
- The CLI source contract contains 38 `IMPLEMENTED`, one `PARTIAL`, and one
  `PLATFORM_ALTERNATIVE` command. The sole partial command is hidden `ipc`.
  Its own record says every source IPC form and adapter is accepted; the named
  remainder is downstream persistence of arbitrary event-supplied launch
  arguments and environment.
- The separate agent-event contract continues to record those two downstream
  context fields as `PARTIAL`. Linux parses and retains them in volatile
  canonical status but deliberately excludes untrusted arbitrary environment
  from durable restore state. Supported resume commands are reconstructed from
  validated tool/session identity. Therefore this limitation must remain
  visible in the agent feature ledger, but it does not make the CLI parser,
  authenticated transport, or hidden IPC command partial. Treating it as a CLI
  command gap duplicated ownership across GH-22 and GH-46.

No production implementation gap was found in the two GH-22-owned features at
this checkpoint. The next step is to correct the contracts first, let their
negative validators fail if that classification is unsound, then run only the
existing focused CLI and shell evidence.

## Reconciliation

- Hidden `ipc` is now `IMPLEMENTED` in the CLI source contract. Its record
  explicitly links the downstream security/persistence boundary instead of
  deleting it. The separate agent-event contract still marks
  `context.launch.arguments` and `context.launch.environment` `PARTIAL`, so the
  unresolved broader agent behavior remains visible under GH-7/GH-46 evidence
  and cannot be mistaken for completed durable arbitrary-environment restore.
- `cli.topology-control-grid-notify` is now `IMPLEMENTED`. Its acceptance text
  points to all 40 source command records and no longer names already-closed
  GH-46/GH-47 as future CLI work.
- `shell.environment-integration` is now `IMPLEMENTED` and names the existing
  shell prerequisite, staged real-shell, pane environment, and real-product
  actors. No new shell implementation or harness was added.
- Inventory totals change from 23 IMPLEMENTED / 23 PARTIAL / 14
  NOT_IMPLEMENTED to **25 IMPLEMENTED / 22 PARTIAL / 13 NOT_IMPLEMENTED**.
  The matrix does not change status: these actors were already declared PASS;
  the stale feature ledger had failed to consume their completion evidence.

## Focused closeout evidence

No app-wide `qualify-local` run was performed.

- CLI source contract: PASS, 40 commands, 40 source symbols, 13 output
  contracts, and 6 schemas. Contract negative tests and the separate
  agent-event inventory negative tests passed.
- Feature inventory plus negative runner tests: PASS with the two GH-22-owned
  entries required to be `IMPLEMENTED` and to retain their named evidence.
- Staged real-shell contracts: Bash, Zsh, Fish, and Nushell all PASS. The shell
  prerequisite resolver and its negative cases also pass.
- Focused Rust CLI/transport suites: 15 helper CLI, 8 integration-management,
  11 product CLI, and 14 authenticated product-transport tests PASS. They use
  real local processes, filesystem state, and Unix sockets rather than a fake
  transport.
- Real staged CLI through the existing GTK/Ghostty product and authenticated
  socket on private X11: PASS, session
  `3fdf3a44da87516db652b0f276b6ba34ad0695d3496b92173edfeba361dc4af2`.
- The first Wayland invocation used the headless compositor wrapper and failed
  explicitly because this physical-input journey requires
  `nested-wayland-input`. Environmental absence was not converted into a pass.
  The corrected input-capable private Wayland journey passed, session
  `fb608841cae2356e1e6e404fc530ae363b22ae3d8bb2472275472c5a93a2639d`
  (nested X11 transport session
  `7f18c60321dd1394db151dde8d7c071cae118e2be460bc1212242ea1680c453a`).
- Application API inventory and negative tests: PASS, 19 operations, 3
  specialized endpoints, 4 CLI-local responsibilities, and all 40 CLI records.
  API schema, architecture contract/negative tests, ApplicationShell ownership
  contract/negative tests, mutation-resource policy, and the consolidated test
  orchestration contract all pass. The orchestration receipt still reports one
  actor and three protocol endpoints; no parallel product system accreted.
- No new mutation run was warranted: this closeout changes only ownership and
  status ledgers plus their negative validators. The parser, authorization,
  routing, shell policy, and output decisions were mutation-tested in the
  completed child issues and no production decision was changed here.

The authoritative matrix remains **168 PASS, 0 FAIL, 3 BLOCKED, 3 XFAIL, and 6
NOT_IMPLEMENTED** declarations. The most recent immutable full-run receipt
contains the already-documented transient Debug/X11 Valgrind startup/report
failure from the clipboard closeout; that exact cell and suppression governance
subsequently passed in isolation. This GH-22 ledger-only reconciliation does
not rewrite that receipt or claim a fresh complete run. Release qualification
and full Linux qualification remain **NOT_PASSED** because declared non-PASS
cells remain.
