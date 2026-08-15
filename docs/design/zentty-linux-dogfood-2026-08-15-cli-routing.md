# Zentty Linux dogfood: authenticated CLI routing

Date: 2026-08-15
Owner: GH-43

## Trigger

GH-42 made the source CLI syntax and output contract explicit. GH-43 now owns
the deeper question: whether the staged CLI can safely discover and mutate the
real multi-window product through one authenticated authority, including
hostile clients and restart recovery.

## Starting state

The authoritative matrix starts at `PASS 140`, `FAIL 0`, `BLOCKED 7`, `XFAIL
1`, and `NOT_IMPLEMENTED 16` (164 total). Implemented local and product-boundary
qualification pass; release and full Linux qualification do not.

Existing real journeys already cover substantial behavior: source CLI
discovery under controlled X11/Wayland, pane mutations through real Ghostty
surfaces, two GTK windows, stale endpoint rejection after restart, and tmux,
notification, and server routes on the shared socket. Existing transport tests
cover private endpoint permissions, canonical token routing, wrong tokens,
malformed input, oversized frames, and stalled clients.

## Initial discoveries

- There is one runtime, registry, socket, application router, shell mutation
  path, workspace store, and focus authority. GH-43 does not need a second
  control subsystem.
- The wire envelope permits client-supplied window/worklane/pane claims for
  compatibility, but the server discards them and derives the canonical target
  solely from the pane token before dispatch.
- Public mutating selectors are additionally checked against the authenticated
  target. A token for pane A cannot mutate pane B merely by naming pane B.
- Every process gets a random runtime directory and socket; every pane gets a
  distinct random capability. Restart behavior is already exercised in the
  tmux/product journey and proves both values rotate.
- The source CLI has no separate `window select` or `worklane select` leaf.
  Source selection is expressed by authenticating as a pane and focusing it;
  that operation selects its containing worklane and presents/focuses its
  window through the existing GUI authority. The issue wording describes the
  resulting behavior, not additional invented commands.
- Coverage gaps remain for partial transport writes, concurrent mixed-auth
  clients, explicit wrong-instance isolation, and fail-safe focus invariants
  after stale commands. These will be added to the existing transport and
  staged-product journeys rather than creating a new harness layer.

Failures, repairs, mutation results, display receipts, and final qualification
will be appended as the issue is executed.

## Failures and repairs

1. **The transport tests did not cover fragmented writers or mixed concurrent
   clients.** The existing server implementation already reads until the
   client's write half closes, but that behavior had no direct receipt. The
   existing `product_transport` suite now writes one valid request in
   seven-byte chunks, verifies the canonical token target despite forged
   topology claims, drives eight authorized and eight unauthorized clients
   concurrently, and proves two live sockets reject each other's capabilities.
   No second transport harness was added.
2. **The first partial-write fixture used Rust field names instead of the wire
   protocol's camel-case names.** The server correctly rejected it and returned
   an empty response ID. The fixture now sends `standardInput` and
   `expectsResponse`, reaches authenticated dispatch, and receives the original
   request ID and exact product output.
3. **The controlled CLI environment assertion compared an absolute staged CLI
   path with a relative test argument.** The product correctly publishes the
   absolute `current_exe` sibling. The verifier now canonicalizes the expected
   staged path rather than weakening the product contract.
4. **The source event-route guard had a surviving mutation.** Replacing
   `subcommand == "agent-event"` with `true` allowed an arbitrary `ipc`
   subcommand carrying a valid event to dispatch. A hostile raw-frame case now
   proves the wrong subcommand is rejected before event delivery. The repeated
   focused campaign caught that mutant.
5. **Cross-window CLI focus changed the workspace model but did not present the
   target GTK window.** `ApplicationShell::focus_selected_surface` deliberately
   ignores inactive windows, and the application router never performed the
   source command's containing-window selection. Successful public `pane
   focus` now presents the existing coordinator-owned window and marks the one
   `WindowSet` authority active. Failed focus, discovery, and other mutations
   do not present a window. Controlled X11 and Wayland tests move focus from a
   newly created real window back to the original and forward again, checking
   both discovery state and coordinator receipts.
6. **The first product mutation attempt could not build its untouched
   baseline.** This was not a product failure: safe `cargo-mutants` copying
   correctly omitted ignored `build/linux-deps`, while the Ghostty sys build
   script needs its pinned library. The rerun supplied an absolute
   `GHOSTTY_LIB_DIR`; `.cargo/mutants.toml` remained `gitignore = true` and
   `copy_target = false`, so no large build tree was copied.
7. **The first full matrix run failed `staged-wayland`.** The headless Weston
   profile has no activation protocol or input seat, so opening window 2 did
   not make it compositor-active and the coordinator still correctly recorded
   window 1 as active. Focusing window 1 therefore produced no change-only log,
   although the child independently verified window 1 remained selected. The
   product now emits a focus-routing receipt for both idempotent and changing
   window selection (`changed=false|true`); the test continues to require the
   independently discovered active window. The failed matrix receipt remains
   recorded and a clean complete rerun is required.

## Authority and real-system receipts

- The architecture contract now asserts exactly one
  `mpsc::Receiver<AuthenticatedProductRequest>` in `AgentRuntime`. Its negative
  suite injects a shadow product receiver and requires deterministic failure.
- The staged CLI journey proves capabilities are absent from ordinary
  discovery, explicit authenticated disclosure matches the real child
  environment, forged topology claims cannot redirect a token, wrong and stale
  targets fail without moving focus, and split-created Ghostty/PTY children
  receive the live private socket, instance/window/worklane/pane IDs, exact
  pane capability, staged CLI/hook command, and shell-integration resources.
- The same journey runs two staged Zentty processes concurrently. It proves
  distinct sockets, instance IDs, and capabilities; rejects both cross-instance
  capability combinations; mutates the second worklane; and verifies the first
  topology remains unchanged.
- Controlled X11 CLI receipt: `PASS staged-cli=true real-product=true
  authenticated-socket=true concurrent-instances=true`.
- Controlled Wayland CLI receipt: `PASS staged-cli=true real-product=true
  authenticated-socket=true concurrent-instances=true`.
- Controlled X11 and Wayland tmux/product journeys both pass with two real GTK
  windows and bidirectional CLI window selection through the coordinator.
- The complete `zentty-agent-ipc` package passes: 65 unit/integration tests,
  including real CLI subprocesses, actual Unix sockets, server and tmux routes,
  framing ceilings, handler shutdown, and hostile input.
- Focused transport mutation result: 20 mutants, 17 caught, three unviable,
  zero missed. Focused window-selection predicate result: six mutants, all six
  caught. Both campaigns used the safe repository copy policy.

## Matrix effect

GH-43 strengthens the two existing `cli-source-compatibility` release cells
and the existing X11/Wayland `agent-integration` cells rather than adding
duplicate commands that rerun the same staged journeys. The declared status
totals therefore remain 140 PASS, zero FAIL, seven BLOCKED, one XFAIL, and 16
NOT_IMPLEMENTED. That unchanged count is intentional: none of the 16
not-implemented cells describes authenticated CLI routing. Final matrix
execution is still required before commit.

## Final qualification

After the failed `staged-wayland` attempt and its receipt repair, a clean
`ZENTTY_QUALIFICATION_JOBS=4 linux/tests/qualify-local` run executed every
support test and presently executable matrix cell without an unexpected skip
or retry inside a cell:

- declared matrix totals: `PASS 140`, `FAIL 0`, `BLOCKED 7`, `XFAIL 1`,
  `NOT_IMPLEMENTED 16` (164 total);
- implemented local suite: passed;
- product-boundary qualification: passed;
- release qualification: not passed;
- full Linux qualification: not passed;
- wall time: 666,170 ms; upstream `ghostty-regression` remained the 368,830 ms
  floor and longest cell;
- both strengthened CLI compatibility cells and both agent-integration cells
  passed under their controlled X11/Wayland profiles;
- Debug Valgrind is **PASS with reviewed suppressions**, not an unsuppressed
  clean result. Raw totals were 427 errors/contexts, 6,080 definite bytes and
  41,331 indirect bytes. Post-suppression totals were zero, with all 427
  errors/contexts reviewed and accounted for. The unsuppressed and suppressed
  receipts are retained alongside the machine summary;
- ReleaseSafe Valgrind remains XFAIL/NOT_IMPLEMENTED as declared and no
  suppressions were broadened for this issue.

Full Linux qualification is not claimed while the seven BLOCKED, one XFAIL,
and 16 NOT_IMPLEMENTED cells remain.
