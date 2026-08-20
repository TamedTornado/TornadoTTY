# Dogfood: live pane transfer into an existing window

Date: 2026-08-20  
Owner: GH-32

## Source contract and design

`AppDelegate.buildMoveToWorklaneCatalog`, `AppDelegate.movePaneToWorklane`,
`MoveToWorklaneMenuBuilder`, and
`WorklaneStoreCrossWindowTransferTests` establish the source behavior:

- the source window's worklanes come first, followed by other windows in
  application window order;
- the source worklane alone is excluded, and window groups are separated
  without invented window-header rows;
- selecting a foreign destination transfers the existing pane rather than
  reconstructing it;
- the destination focuses the moved pane, and an emptied source window closes.

Linux extends the already-qualified linear live-runtime handoff used by **Move
Pane to New Window**. `WorkspaceState` moves the pane plus agent projection;
`PaneRuntimeCoordinator` detaches/adopts the one existing Ghostty surface; and
`ApplicationCoordinator` validates both owners, commits the destination, rolls
back adoption failure, updates routing, closes an empty source, and republishes
the one aggregate application topology. No Ghostty change, second workspace
model, alternate persistence path, or new integration harness was introduced.

## Test-first findings and repairs

1. The first two focused core tests failed to compile with six missing-method
   errors. They specified final-source-pane transfer, invalid and duplicate
   destinations, destination viewport geometry, title/CWD/command retention,
   and agent-state ownership before production methods existed.
2. The first implementation used `HashMap::append`, which is unavailable, and
   then held an immutable focus-fallback borrow across mutation. Replacing it
   with `extend` and cloning the replacement identity before mutation repaired
   both compiler failures without weakening the tests.
3. The typed GTK action increased the action registry from 116 to 117. The
   focused registry and architecture contract initially rejected the stale
   census; both now record the new `(window ID, worklane ID)` string-pair action.
4. The first real X11 actor overlapped two unmanaged toplevels and hit the
   second window while seeking the first. Directly raising and focusing the
   owned X11 toplevel repaired physical target selection without coordinates
   pretending to identify a window.
5. The first destination assertion used the new-window-transfer worklane ID
   (`window-2-worklane-1`) rather than the actual fresh-window source identity
   (`worklane-window-2`). The rendered pointer receipt exposed the bad fixture
   assumption; the actor now consumes the product's source-derived identity.
6. The first successful transfer later aborted on an authenticated agent event
   with `RefCell already mutably borrowed` at the destination-catalog provider.
   The provider borrowed `ApplicationCoordinator` during a shell render that
   was itself reached from the coordinator's mutable tick. This was a real
   product concurrency defect, not a test artifact. The repair replaces the
   re-entrant callback with a coordinator-published immutable view projection:
   summaries are collected after releasing the coordinator borrow, installed
   into every shell, and then rendered. Subsequent agent events cannot re-enter
   coordinator ownership.
7. Cage supplies controlled Wayland keyboard injection but deliberately no
   outer pointer route. Terminal Tab input also does not constitute GTK focus
   traversal. The feature journey therefore uses the existing
   `nested-wayland-multiwindow-v1` Weston profile, whose controlled outer-X11
   input drives real Wayland GTK popovers. Environmental absence was not
   converted into a pass.
8. Weston placement made fixed menu coordinates invalid. Pointer-enter
   receipts now locate the actual rendered pane button, drill-down action, and
   foreign destination across the compositor window. After the source closes,
   the moved terminal itself is hover-resolved and physically clicked before
   post-transfer input.
9. The first mutation run caught eight of ten generated mutants, missed the
   inactive-worklane active-selection branch, and found one compiler-unviable
   mutant. A three-worklane test now proves removing a non-active middle lane
   cannot change the active lane. The rerun caught all nine viable mutants.
10. The mandatory all-target/all-feature Clippy pass exposed current-toolchain
    drift in pre-existing code: inferred `Default` constructions, intentionally
    long integration/GTK boundary functions, one lazy option fallback, one
    needless owned diagnostic, and a missing statement semicolon. The repair
    makes types and borrows explicit, applies narrow length annotations only at
    the existing orchestration boundaries, and changes no product behavior or
    test coverage. The complete mandatory command is rerun below rather than
    treating the first four diagnostics as the whole result.
11. The first full matrix rerun found two orchestration regressions and one
    expected dirty-tree packaging refusal. Replacing Cage with Weston for the
    entire historical multi-window journey broke its keyboard-focus assumptions;
    the repair first separated the pointer-dependent foreign-worklane
    transaction into its own authoritative Weston cell. The full journey also
    needs Weston now because its two mapped windows require deterministic
    post-close pointer focus; it explicitly locates and clicks the surviving
    real terminal rather than assuming compositor activation. The same-window
    sidebar journey also expected the old
    destination receipt before window identity became explicit; its physical
    target now asserts the complete `(window, worklane)` receipt. The first
    isolated sidebar rerun also showed GTK placing the now-taller destination
    popover outside the actor's legacy left-half scan. The actor now searches
    the complete owned toplevel and still accepts only the exact GTK pointer
    receipt, rather than broadening the semantic assertion. Packaging
    correctly refused to qualify an uncommitted checkout and must be rerun from
    the reviewed commit rather than weakened.
12. The widened scan proved placement was not the cause: the move affordance
    was genuinely absent. The immutable cross-window projection contained the
    source window's startup worklane snapshot, so later local worklane creation
    was invisible and the source catalog appeared empty after exclusion. The
    shell now replaces its own group with live local summaries on every sidebar
    projection while retaining coordinator-published foreign groups. This keeps
    one catalog path without re-entering coordinator ownership or requiring a
    second synchronization system.
13. Restoring the historical Cage profile exposed a pre-existing physical-
    input race after accepting window closure: the closing toplevel could be
    destroyed before virtual-keyboard modifier releases landed, and the next
    text became `Ctrl+Shift+N`. The journey now explicitly clears virtual
    keyboard Shift/Control state against the surviving compositor seat before
    asserting survivor input. This mirrors the existing X11 destroyed-window
    key-state repair instead of hiding the misroute with model assertions. A
    release-only `wtype` invocation did not clear GTK's stale state because the
    new virtual-keyboard client had no matching local press; a balanced
    press/release cycle with no character is required. The authoritative Weston
    route additionally uses physical pointer focus, which is the deterministic
    contract for choosing between surviving mapped Wayland toplevels.
14. Weston then proved that source `target_second_window` was an X11-only
    operation: it intentionally did nothing on Wayland and relied on new-window
    activation. The shared actor now identifies the requested real Wayland pane
    by its hover receipt and clicks it through the controlled compositor before
    any physical typing, so every later target selection has the same explicit
    meaning on both display stacks.
15. A cancelled application-quit dialog similarly left keyboard focus on the
    dismissed transient under Weston. The lifecycle journey now reselects the
    surviving pane with the same receipt-authenticated physical pointer path
    before asserting post-cancellation terminal input. Later clean/crash and
    ordinary second-window close journeys exposed the same compositor-owned
    focus boundary, so the actor consolidates all post-close selection through
    one `focus_outer_wayland_pane` helper rather than accumulating per-journey
    mechanisms.
16. To keep the authoritative matrix from paying twice for the same Weston
    transaction, the historical full multi-window cell skips Journey 0c while
    the dedicated `product-existing-worklane-transfer-wayland` cell owns it.
    X11 continues to execute Journey 0c inside its full cell. This is test
    partitioning within the one existing actor, not a second harness.
17. The orchestration contract still pinned the historical Cage profile and
    correctly rejected the final Weston matrix declaration. Its expectation now
    names the controlled multi-window profile; the cell count, real actor,
    ReleaseSafe binary, display pairing, and capability requirements remain
    unchanged.
18. The first clean post-commit full run exposed a real scheduler race. Every
    custom `build-local` output—including packaging work directories—was
    mirrored back into the shared profile bundle by deleting and replacing it.
    A package worker therefore removed the ReleaseSafe executable while live
    product cells were using it; sleep reacquisition named the executable as
    `(deleted)`, and other helper-backed journeys failed downstream. Profile
    mirroring now occurs only for the documented default `build/linux` output;
    explicit profile and packaging outputs remain isolated. The installed-X11
    cell separately reported a late process-group cleanup failure after its
    product assertions passed; an isolated rerun passed unchanged, so no portal
    behavior was hidden or relabeled.
    A real `build/custom-output-isolation` ReleaseSafe build then preserved the
    shared profile's inode, size, timestamp, and SHA-256
    `dc76b39aa45298914c22d90fa25e0481e89392ce31b279dcd7ac7b35a8904185`
    while producing its own executable.
19. The next full run executed every other implemented cell successfully but
    caught a nondeterministic Journey 0c input path: a broad pointer sweep
    sometimes crossed another Weston-managed toplevel instead of the open GTK
    popover and never reached `Move Pane to Worklane`. Environmental absence
    was not converted into a pass. Early keyboard-traversal repairs passed
    focused runs but were not retained: under full scheduler load, multiple
    navigation events could queue before the focus receipt was flushed and
    activate the wrong command. This was evidence against the repair, not a
    reason to increase its timeout.
20. The following clean full run passed Journey 0c but exposed an unrelated
    pre-existing receipt race in `agent-integration-x11`. The product emitted
    `ssh-identity ... state=remote`, then the actor immediately tailed the log
    once for the presentation line that `apply_observations` writes on the next
    GTK render. Under the loaded scheduler the reader won that interval even
    though the product contract orders the render directly after the identity
    update. The actor now bounds and polls for that post-identity presentation
    receipt rather than treating a partially flushed log as a product failure.
    An unchanged isolated real-X11 rerun passed before the synchronization
    repair (session
    `07cd1fcba015dea1c99e6991886d032368f1f9d45618144b9c8d76e5a7ea94b0`),
    confirming that no SSH/product behavior needed to be weakened or
    relabeled. Three consecutive post-repair real-X11 journeys also passed:
    `bdc927c5582a0e868af1fe474820a09e8846ac3c8534014a01889994a26de564`,
    `fd00942a71e2539dbda22d291efc6f892cdc837b0efe8ef23dbcfd6a35be33b7`,
    and `0be8f3a58f258b6d58b1445425ecac467874803e05405c69b4af30fde3318277`.
21. The third clean full run passed the SSH repair and exposed why Journey 0c
    remained load-sensitive. Every unrelated server/project/agent metadata
    refresh called `refresh_pane_menus`, which unconditionally replaced even a
    visible GTK popover. A user could therefore lose an open contextual menu
    while a background scanner updated the sidebar; the test was detecting a
    real product UX bug rather than merely struggling to click. Visible pane
    popovers are now preserved as coherent transaction snapshots while the
    action router continues to validate their target at commit time. Closed
    menus still receive the latest catalog on the next render.

    The final actor physically opens the pane menu, searches only the bounded
    neighborhood of that real anchor, and requires a fresh pointer-enter
    receipt on a slow replay before clicking `Move Pane to Worklane`. The
    replacement submenu gives its sole foreign destination GTK focus; the
    actor requires that exact `(window, worklane)` receipt and activates the
    real focused button with one modifier-cleared physical Return. No test-only
    product API or model mutation was added. One focused run followed by five
    consecutive stress runs passed. Preserved sessions include
    `1c64e8dc7c4d914bc94f267bffd346413dfb68afd520a2032578c0b75a57bee0`,
    `e25f6e10c51a4bb18223b19ace0eb816cd6a0d15063a00f1f78f49ce0ae30655`,
    `475f90a9aea1812b23d6ccf17340978e42844dbc06d4620932a1aaec9a5cb929`,
    `c8a73a0f0e714538cc6f6511b57880885970ea2128ba8882ec401b12bceb704f`,
    and `3685f41a84342c1a87e72501459686980447d61b0fcb06e1df0ee6befda77dc3`.
    After removing the discarded focus instrumentation and rebuilding from the
    final source, three more consecutive journeys passed:
    `3f269d01346e6564df181f66b46834761e0ad64373d57ea69ac3a79930036f6a`,
    `93273bbc8678a24c58721738b8cb6a4754f0400948b9a26b8037b1787391274f`,
    and `26e563fd4dadcb0ea72cbce96ac4679725be2e9f29653eaae6be67f6fef1e89d`.

22. **The four-worker matrix was scheduling latency-sensitive real-display
    actors beside compiler-heavy rebuilds.** The first full run from the final
    source failed three otherwise independent actors while `ghostty-regression`
    or the clean-package rebuild occupied the host: Debug X11 did not present
    its window before its bounded deadline, the Wayland transfer actor could
    not locate the physically opened menu row, and the X11 source-UX actor did
    not observe its physical divider drag. Running those exact three cells
    sequentially, without changing product or actor code, passed with sessions
    `87122178fbc9f1aa1a5714ba7ced99dfb87d18bb9138174a2bcffb3c5e805c72`,
    `cfebcb420773fd5a7dcf6c5b12ab83ef828ce1bd437bbe2e1b1afb09b5ed74f5`,
    and `0bc546ce6246ee5d33e963ea4a423b801431a1eaa4e4dd033c0331810d9009f6`.
    This was qualification-scheduler contention, not three product defects and
    not justification for wider actor timeouts. The authoritative matrix now
    marks its four compiler-heavy rebuild cells with `blocks_display`. The
    scheduler permits those cells to overlap non-display checks but proves in
    the machine-readable timing receipt that none overlapped a real-display
    cell. A negative runner test rejects malformed policy and a closed-set test
    prevents broadening it silently. The worker limit remains four; the repair
    does not return the suite to serial execution.

23. **Compiler/display separation exposed a narrower actor family conflict.**
    In the next full run, the earlier Debug smoke and source-UX failures stayed
    green, confirming the load barrier, but the existing-worklane Wayland actor
    and the full X11 multi-window actor both failed while physically searching
    the same dense pane menu in parallel. The full Wayland multi-window actor
    passed in that run, and the existing-worklane actor had already passed both
    alone and repeatedly; this is shared timing sensitivity inside one real
    input-journey family. The three full menu journeys now declare the focused
    `multi-window-menu-input` resource and therefore serialize only with one
    another. Short fleet modes skip this menu transaction and remain parallel.
    The same run's package lifecycle and two support-test failures were not
    product failures: all three correctly rejected the intentionally dirty
    checkout containing this uncommitted scheduler repair. The repair must be
    committed before the clean-checkout qualification can produce valid package
    evidence; it remains unpushed until that evidence passes.

24. **Family serialization was necessary but not sufficient for the densest
    pointer transaction.** The clean-checkout rerun made both broad X11 and
    Wayland multi-window journeys pass, but the focused existing-worklane move
    still missed its transaction while eleven other declared display actors
    overlapped it. Because this actor repeatedly passes alone and is the only
    journey that must discover a foreign window/worklane submenu through nested
    controlled Wayland/X11 input, it now declares `blocks_display` itself. It
    remains parallel with non-display checks, but its timing receipt must prove
    it did not overlap another compositor actor. The same run exposed false
    parallelism among Cargo tests: `clipboard-settings-subset` compiled while
    the Ghostty regression and several same-target Cargo tests ran, then three
    deliberately concurrent lock-contract tests exceeded their acquisition
    deadline. All matrix commands that invoke `cargo test`, plus the four real
    compiler-heavy producers, now share `compiler-load`. This serializes access
    to the shared Cargo target/compiler load—not unrelated model, platform, or
    display checks—and should reduce redundant lock waiting rather than hiding
    a product failure. No product timeout was widened.

25. **The remaining clean run failure was an actor receipt-order race, not a
    reload loop.** Every product cell from this feature and the compiler-lock
    repair passed. The Wayland config-reload actor observed its visible settings
    refresh, sampled the count of completed reload receipts, and began the next
    self-write assertion before the preceding reload's final `result=applied`
    line was emitted. That late, legitimate line was then counted as a second
    self-write reload. The actor now requires both the visible refresh and the
    corresponding config-authority completion receipt before establishing its
    next baseline. This uses existing product logs and the existing journey; it
    adds no sleep, timeout, fixture path, or product instrumentation.

## Real-system evidence

- Focused core transfer tests: 2 PASS.
- Typed action tests: 5 PASS; sidebar catalog tests: 8 PASS.
- Architecture and ownership contract, negative validator tests, and safe
  cargo-mutants copy/resource-isolation policy: PASS.
- Mutation: 10 generated, 9 caught, 1 compiler-unviable, 0 missed; two workers,
  `.cargo/mutants.toml` retained `gitignore = true` and `copy_target = false`.
- X11 real product: session
  `16b872b083211d762a9aaae6f7b75b822a87845130a5d7e8aadc63fa9898b57b`.
- Wayland/Weston real product: session
  `189082dba9c26b2823410ecbdac6269875fca5e32821c6aea4a6c8a3467e9299`.

Both controlled journeys create two real GTK windows and two real Ghostty PTY
children, physically open the source pane menu, select the foreign worklane,
move the final source pane, observe the empty source window close, preserve the
exact child PID set and single terminal-ready receipt, send physical input
through the moved pane, and publish an authenticated agent event through its
new `(window, worklane, pane)` route.

## Remaining scope and claim limits

- Source cross-window drag/drop animation is not implemented by this command
  transaction. It remains explicit GH-16 UX scope; the feature inventory stays
  `PARTIAL` rather than claiming full handoff parity.
- The broad deterministic screenshot and accessibility-tree program is also
  owned by GH-16. This slice preserves named accessible buttons, adds focus
  receipts, and drives the real visible controls, but does not claim that the
  broader visual/accessibility matrix is complete.
- The matrix's full workspace-restore cells remain `NOT_IMPLEMENTED` for their
  separately stated physical-divider/double-click/relaunch criteria. This
  transfer does not relabel those unrelated gaps.
- Release and full-Linux qualification must remain false while any required
  matrix cell is not PASS.
