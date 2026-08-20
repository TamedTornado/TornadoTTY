# Zentty Linux dogfood — tmux compatibility closeout

Date: 2026-08-20
Tracking: GH-14, GH-22, GH-32

## Frozen closeout scope

The tmux compatibility feature is not a multiplexer and this closeout will not
create one. Ghostty remains the only terminal and PTY owner. The existing path
is authoritative:

```text
installed/staged tmux shim
  -> zentty __tmux-compat
  -> authenticated Agent IPC socket
  -> canonical pane capability target
  -> existing application/window/worklane model
  -> existing Ghostty surface and PTY
```

The source contract, pure Rust parser/renderer/store, authenticated transport,
product coordinator, persistent XDG store, wait-for authority, staged shim,
shell integration, real product actor, and installed Claude journey already
exist. The port plan kept GH-14 open for one explicit reason: the product actor
could not prove multi-window isolation before GH-32 delivered real live
multi-window composition. GH-32 is now complete, and the existing actor already
creates a second real GTK window with four real Ghostty PTYs.

This closeout is therefore bounded to the missing evidence rather than another
implementation system:

1. Extend `linux/tests/rust-tmux-compat-product`; do not create a new actor.
2. Route real tmux CLI subprocesses through pane capabilities from both live
   windows while deliberately retaining stale forwarded window/worklane
   environment from the original child.
3. Prove each token lists only its canonical window/worklane/panes and rejects
   explicit pane targets owned by the other window.
4. Prove active-pane selection and team-anchor mutations remain worklane scoped.
5. Prove the source-defined instance scope of named buffers and `wait-for`
   signals across windows; sharing those values is intentional, not leakage.
6. Run focused Rust contracts and the existing staged product journey on X11
   and Wayland. Do not run app-wide qualification for this bounded closeout.
7. Promote the authoritative inventory and close GH-14 only after those focused
   journeys pass and the diff is reviewed.

## Initial discovery

- The dated port plan says Phase 3 remains open only for multi-window scope.
  That blocker is stale because GH-32 now provides the real second-window
  lifecycle and the tmux actor already exercises it through the public CLI.
- Multi-window creation alone was not tmux qualification: all tmux commands in
  the actor still used the first pane's inherited capability. The test could
  prove public CLI window targeting but not tmux server canonicalization or
  compatibility-store scoping.
- `list-windows` is source terminology for worklanes in the routed Zentty GTK
  window, not a process-global list of GTK windows. Each `ApplicationShell`
  supplies only its own workspace projection after the shared socket routes a
  pane capability to that shell.
- Source-compatible buffers and wait signals are instance scoped. Active pane
  IDs and team anchors are keyed by worklane. A correct isolation test must
  prove both deliberate cross-window sharing and prohibited cross-worklane
  selection/anchor reach rather than assuming every datum is window local.

## Test-first change

Before changing product code, the existing staged-product child gained a real
second-window tmux journey. It exercises separate CLI processes and capability
tokens for both windows, cross-window target rejection, routed list/display and
selection, shared buffer and wait-for semantics, and creation/removal of a team
pane in only the second window. The parent actor requires an exact bounded
receipt so a child assertion cannot be hidden by ordinary application shutdown.

## Discoveries, failures, and repairs

1. **Explicit pane targets escaped the canonical worklane.** The first real
   X11 run reached the second GTK window correctly, but a first-window token
   could issue `display-message -t` for a second-window pane. `PaneTarget`
   deliberately falls back when a selector is absent from its supplied pane
   list, and `display-message` plus `select-pane` had omitted the explicit
   target validator used by the other mutating commands. This was an
   authorization/scoping defect, not a compositor problem. Both commands now
   reject an explicit pane outside the server-canonical routed worklane with
   `target_not_found`; a focused Rust regression observes both rejection and
   ordinary in-worklane selection.
2. **`wait-for` was accidentally window scoped.** The next real X11 run proved
   that signalling through the second window could not be consumed through the
   first, contradicting the audited source contract's instance scope. Each
   `ApplicationShell` owned a separate `WaitForSignals`. A small
   `TmuxCompatSession` now owns that volatile signal state once per
   `ApplicationCoordinator` and is cloned into each real window product. It is
   intentionally `Rc<RefCell<_>>`: GTK window products and their command
   handlers execute on the existing main thread. Separate application sessions
   remain isolated, and buffers remain shared through the already shared
   file-backed instance store.
3. **Ordinary product close paths left stale compatibility routing.** After the
   routing and wait repairs, the journey closed the second real window but the
   durable store still retained its `activePaneIDs` entry. Tmux-owned kill
   commands cleaned their state, while ordinary public CLI/UI pane and worklane
   closure did not. `TeamStore::remove_worklane`, the product's external-close
   completion methods, and the existing close paths now remove only the closed
   pane/worklane state. Final teammate removal still restores the leader width.
   Closing a window while the application remains alive clears that window's
   compatibility state; whole-application shutdown deliberately preserves the
   durable restart contract. Cleanup occurs only after the workspace accepted
   a pane/worklane close, so refusing to close the final pane or final worklane
   cannot erase still-live compatibility state.
4. **The actor's old exact receipts were global in one assertion.** Extending
   the existing journey correctly added more `display-message` and
   `list-panes` calls. The original fixed counts failed, and its AWK ordering
   check counted commands from both windows even though its claim concerned
   pane 1. Counts now include the intentional first-window calls and the
   ordering check is explicitly scoped to `pane-1`/`worklane-1`. No skip or
   relaxed product assertion was introduced.
5. **A local build invocation used a relative Ghostty library path.** Cargo
   interpreted it from a package build directory and rejected the pinned
   library as missing. Rebuilding with the absolute staged
   `build/linux-deps/ghostty/zig-out/lib` path succeeded. This was invocation
   error only; no dependency fallback or build-policy change was made.
6. **The authenticated CLI tests cannot create their Unix socket in the
   restricted command sandbox.** One focused invocation produced five
   immediate `Operation not permitted` failures at socket setup. It was rerun
   with the repository's normal local-IPC permission boundary and all 5 CLI
   plus 6 transport tests passed. This environmental denial was not converted
   into a pass and did not prompt a fake transport.
7. **The architecture mirror rejected the new shell method.** The ownership
   validator correctly reported `forget_tmux_worklanes` as unassigned. The
   mirror now assigns it to the existing tmux coordinator, documents the
   application-session wait-signal authority separately from each window's
   compatibility product, and enforces exactly one `TmuxCompatSession` field in
   `ApplicationCoordinator`. A new negative self-test injects a shadow session
   and proves the validator rejects it. No second tmux coordinator or state
   store was introduced.

## Focused qualification

No app-wide `qualify-local` run was performed for this bounded closeout.

- `linux/tests/tmux-compat-source-contract`: PASS, 23 commands, 34 fixtures,
  13 audited source files.
- `cargo test -p zentty-tmux-compat`: PASS, including 14 source-fixture tests,
  6 protocol tests, 5 security tests, and the core unit tests.
- `cargo test -p zentty-agent-ipc --test tmux_cli --test tmux_transport`:
  PASS, 5 CLI and 6 authenticated-transport tests.
- Focused `zentty-linux` tmux tests: PASS, 19 tests.
- Existing real staged-product actor under private X11/Xvfb: PASS, session
  `9e7e8d843b7eb522bdfbdadf727f5c47ec57cc7b8446c2fabc37b0e397ff860b`.
  Its logged `Killed` setup child is the actor's deliberate SIGKILL crash-
  persistence probe, not an uncontrolled test failure.
- The same actor under private headless Weston/Wayland: PASS, session
  `51e7476b84ddf595cf10c2632d14133972a759b272b1dc9dad2cabc5393995f7`.
- Governed focused mutation for target validation, external cleanup, and
  shared wait state: 10 mutants, 9 caught and 1 compiler-unviable. The first
  separately focused `TeamStore::remove_worklane` mutation survived, revealing
  that only the Linux consumer observed it. A direct store contract was added
  to prove target-lane removal, unrelated-lane preservation, and preservation
  of instance buffers; the rerun caught the sole mutant. All mutation runs used
  `linux/tests/mutate-rust`, `gitignore = true`, `copy_target = false`, a
  dedicated systemd scope, and at most two workers.
- Feature-inventory runner and its negative tests: PASS. The authoritative
  inventory moves from 22 IMPLEMENTED / 24 PARTIAL / 14 NOT_IMPLEMENTED to
  **23 IMPLEMENTED / 23 PARTIAL / 14 NOT_IMPLEMENTED**. Qualification-matrix
  statuses do not change: this closeout strengthens existing executable
  product evidence rather than implementing a previously NOT_IMPLEMENTED
  environmental cell.
- ApplicationShell ownership contract, its negative self-tests, and the test
  orchestration consolidation contract: PASS. The latter still reports one
  actor and three protocol endpoints; this slice added no harness layer.

The first X11 attempts are intentionally recorded rather than overwritten:
they successively exposed cross-window target fallback, per-window wait state,
stale active-pane state, and then two stale actor receipt assumptions. The
final X11 and Wayland runs passed the complete existing actor after those
repairs. This is focused feature qualification, not a claim that the entire
Linux release matrix was rerun or that full Linux qualification passed.

The unchanged authoritative matrix currently declares **168 PASS, 0 FAIL, 3
BLOCKED, 3 XFAIL, and 6 NOT_IMPLEMENTED** cells. Because app-wide qualification
was intentionally not rerun here, this closeout makes no fresh
`implemented-local` claim. Release qualification and full Linux qualification
remain **NOT_PASSED** while the declared non-PASS cells remain.
