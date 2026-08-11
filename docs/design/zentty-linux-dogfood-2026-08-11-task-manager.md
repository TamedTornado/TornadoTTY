# Zentty Linux dogfood — Task Manager

Date: 2026-08-11
Issues: GH-19, GH-22

## Slice contract

The authoritative acceptance criteria and test-first construction order are in
[`linux-task-manager-feature-plan.md`](linux-task-manager-feature-plan.md).
This record captures every discovery, failed journey, repair, and remaining
uncertainty from the slice.

## Source and design discoveries

- Source Zentty has one application-level Task Manager window, not one per
  terminal or one per application window. Linux therefore owns one controller
  in `Application`, aggregates the existing application shells, and routes
  focus/close back through the shell that owns the stable pane identity.
- Source models network activity but its Darwin sampler currently returns
  unavailable. Linux does the same honestly. Interface counters, network
  namespace totals, socket ownership, and queued socket bytes do not prove
  per-pane throughput and were rejected rather than presented as invented
  parity.
- The source uses shell-integration root identity. The Linux Ghostty embedding
  boundary now supplies the real foreground process ID of each PTY. Remote
  panes remain explicitly remote instead of sampling a misleading local SSH
  client tree.
- Linux `/proc/<pid>/stat` identifies a process by PID plus start time. PID-only
  history would transfer CPU deltas to a reused PID, so history is keyed by
  `(pid, start_time_ticks)`. Parent identity is revalidated, cycles are bounded,
  and every read and traversal has an explicit size/depth/process limit.
- A pane ID is only unique within an application window. The initial projection
  used pane ID alone; review caught the multiwindow collision before product
  promotion. Stable Task Manager identity is now `window_id|pane_id`.

## Test-first construction and repairs

- Focused model and sampler tests cover aggregation, hottest-process ordering,
  peak preservation, filtering, hysteretic sorting, remote/missing roots,
  process names containing spaces and parentheses, stale parents, cycles,
  sibling history, exits, PID reuse, counter rollback, malformed input, and
  oversized files. The implementation uses a single bounded union sample so
  sibling pane trees do not erase one another's history.
- The real fixture creates two actual Ghostty PTY process trees. Each tree has a
  root plus CPU-busy child and touches bounded memory. The product journey
  requires two-process trees, nonzero kernel CPU deltas, roughly 59 MiB RSS,
  and an explicit `network=unavailable` receipt; it does not mock `/proc`, GTK,
  Ghostty, the clipboard, or pane lifecycle.
- The first X11 journey proved sampling but failed process-tree expansion
  because its assumed Tab chain was not a GTK contract. The search field now
  gives physical Down a deterministic accessible route to the first result;
  Enter then drives the real row activation.
- Three attempted Alt mnemonic/shortcut paths did not activate consistently
  through X11 synthetic modifier state. They were removed rather than leaving
  a test-only or duplicate command path. Task Manager exposes standard explicit
  keyboard actions instead: Ctrl+F search, Ctrl+Enter focus, Ctrl+Shift+C copy
  PID, Delete end task, Escape/Ctrl+W close. Tooltips document them.
- The first controlled Wayland attempt raced input before compositor focus and
  never opened the palette. The journey was consolidated onto the existing
  `product-input` authority, which waits for presentation/focus and understands
  the repository's controlled X11, Cage, and Weston input transports.
- The next Wayland run exposed a harness defect: the Task Manager helper passed
  `ctrl+shift+c` to `wtype` as a key name. Compound modifiers now use explicit
  key-down/key/key-up injection, matching the existing input authority.
- The corrected physical chord reached GTK but Copy PID still had no selection.
  Live sampling removes and reinserts list rows; GTK emitted selection-cleared
  during that rebuild, while logical selection restoration relied on a later
  signal that is not guaranteed when the same row object is reinserted. The
  view now restores its logical selection explicitly before selecting the GTK
  row. This was a real product lifecycle bug exposed by the slower controlled
  Wayland timing, not a harness waiver.
- A temporary all-key receipt established the exact Wayland key value and
  modifier state, then was removed. Narrow action and no-selection diagnostics
  remain useful product receipts.

## Real product evidence

- Controlled X11: `rust-task-manager-x11: PASS real-proc real-ghostty-pty
  multi-pane search expand clipboard focus close
  network-explicitly-unavailable`.
- Controlled Wayland: `rust-task-manager-wayland: PASS real-proc
  real-ghostty-pty multi-pane search expand clipboard focus close
  network-explicitly-unavailable`.
- Both journeys type through the compositor, use the real command palette,
  create two real panes, sample live `/proc`, search by the fixture's real child
  PID, expand the process tree, copy through the compositor clipboard, focus the
  owning pane, and close it through the existing workspace/pane lifecycle.
- The first mutation attempt correctly refused to run because its copied tree
  excludes ignored `build/` content and therefore lacked the Ghostty library.
  Supplying the absolute staged Ghostty library preserved `gitignore=true` and
  `copy_target=false` instead of copying the multi-gigabyte build tree. Its
  baseline then exposed the action registry's stale closed-world count (75
  rather than 76) and the sandbox's real-listener restriction; the count was
  repaired and mutation runs were elevated for the real kernel test baseline.
- The first successful mutation receipt found 30 surviving mutants. Boundary
  assertions for hysteresis, byte formatting, exact bounded reads, child PID
  floors, depth, duplicate processes, root PID floors, `getconf`, stat parsing,
  elapsed CPU math and PID counter rollback reduced that to ten and then two.
  The final two survivors exposed operand-order coverage and an untested
  children-file constant. The final repository-safe gate tested 115 mutants:
  100 caught, 15 compiler-rejected unviable, 0 missed.
- Strict focused Clippy, ShellCheck, feature-inventory validation, qualification
  matrix schema/coverage, the matrix runner's focused tests, and targeted
  mutation testing pass at this stage. The full presently executable matrix is
  still to be rerun before this slice is committed.

## Remaining limitation

- Feature status is **PARTIAL**, not implemented parity. Trustworthy per-pane
  network accounting remains unavailable in both source behavior and this port.
  Controlled multiwindow presentation/routing and cgroup/container attribution
  remain required under GH-19 even though the application architecture already
  aggregates multiple shells.
- No full or exhaustive Linux qualification claim is made while the
  authoritative matrix contains any BLOCKED, XFAIL, or NOT_IMPLEMENTED cell.

## Full-matrix promotion discoveries

- The first full run passed both Task Manager cells but correctly failed the
  closed-world feature-inventory support test after the feature moved from
  NOT_IMPLEMENTED to PARTIAL. Expected totals were updated from PARTIAL=26 and
  NOT_IMPLEMENTED=28 to 27 and 27; the schema was not relaxed.
- That run also hit an existing source-UX isolation defect. Its PTY inherited
  the repository CWD, so two ambient development listeners were legitimately
  discovered and a server-card update rebuilt the sidebar during a test that
  asserts title/focus metadata alone does not rebuild it. The journey now runs
  its real PTY in a private CWD. The focused controlled X11 rerun passed without
  disabling server discovery or bypassing the real UI.
- The second full run passed support, source UX, and both Task Manager cells.
  Suppression governance then rejected a warm-cache Pango layout observation:
  the same exact two reviewed root contexts accounted for 20,865 bytes rather
  than the prior cold 25,723-26,208 range. The suppression rule and maximum
  context count were not broadened. The explicit manifest now records both
  cold and warm byte totals, the unchanged Pango consumer stack, and the raw
  receipt. Governance and the presently executable matrix must pass once more
  before commit. Debug remains **PASS with reviewed suppressions** only;
  ReleaseSafe Valgrind remains XFAIL.
- The third full run accepted that reviewed suppression receipt and again
  passed source UX and both Task Manager cells. Its only unexpected failure was
  the unrelated installed-Claude X11 journey: the journey's nested `xvfb-run
  -a` selected display 100 at the same time as the parallel controlled IBus
  worker, so Xvfb rejected the duplicate server. This was qualification
  orchestration contention, not an agent/product failure.
- The shared nested-X11 authority no longer relies on `xvfb-run -a`'s
  check-then-create allocator. Each invocation atomically reserves a display
  from the private high range 2000-11999, passes it with `xvfb-run -n`, and
  releases the reservation on success, failure, or signal cleanup. Its
  automated test launches six concurrent private sessions and requires six
  distinct reservations with no surviving claim. A separate real-Xvfb stress
  run likewise passed with six distinct displays.
- The first focused installed-agent rerun was invoked without `qualify-local`'s
  pinned PATH prefix and correctly rejected ambient Gemini 0.54.4 against the
  reviewed 0.53.0 contract. No expectation was changed. Repeating the exact
  cell with the existing pinned binary passed real agent IPC, tmux
  compatibility, installed Codex, installed Claude agent-team lifecycle, and
  consolidated session restore under one controlled X11 session.
- The final full run passed every presently executable support and matrix cell
  in 406,810 ms. Both Task Manager product journeys, both agent-integration
  journeys, the concurrent nested-X11 regression, packaging, install/uninstall,
  Ghostty regression, architecture contracts, and suppression governance all
  passed. Declared totals are PASS=101, FAIL=0, BLOCKED=7, XFAIL=1, and
  NOT_IMPLEMENTED=21. Therefore **Implemented local suite: PASSED** while
  **Release qualification: NOT_PASSED** and **Full Linux qualification:
  NOT_PASSED**, exactly as the matrix policy requires.
- Debug Valgrind is described only as **PASS with reviewed suppressions**. The
  preserved unsuppressed receipt contains 427 errors/contexts and 6,160 direct
  plus 41,428 indirect definitely-lost bytes. The post-suppression receipt has
  zero remaining errors, contexts, or definitely-lost bytes and reports all
  427 error contexts as suppressed. Suppression governance accepted the full
  inherited-plus-project effective set; this is not an unsuppressed-clean
  claim. ReleaseSafe Valgrind remains XFAIL.
