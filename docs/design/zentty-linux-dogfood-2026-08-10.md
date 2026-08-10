# Zentty Linux dogfood record — 2026-08-10

This file continues the field record from
`zentty-linux-dogfood-2026-08-09.md`. It records discoveries, failures, repairs,
raw evidence identities, and remaining uncertainty. Passing results are not
exhaustive QA claims while the authoritative matrix contains required non-PASS
cells.

## Global Find feature discovery and plan

- The next slice is the user-visible source **Global Find** feature, tracked by
  public issue #34. Qualification, documentation, and any incidental
  housekeeping are completion work inside that feature rather than roadmap
  slices of their own.
- Direct source review found that Global Find is not a text index or a result
  list. `GlobalSearchCoordinator` freezes the current window's ordered panes,
  starts each pane's existing terminal search, aggregates totals, and uses
  exact-pane navigation before asking that pane to advance its own Ghostty
  selection. A Linux port that copies scrollback into a second engine would be
  source-inaccurate and would create a second terminal-data owner.
- Source behavior distinguishes a cleared query from a closed search. It also
  dispatches queries of at least three characters immediately, debounces one-
  and two-character queries for 150 ms, and flushes a pending short query on
  explicit navigation while waiting for all initial totals. These are testable
  contracts, not optional responsiveness tuning.
- The audited Ghostty GTK widget already exposes the product-neutral search
  overlay, search entry, match-total/selection properties, and binding actions.
  The plan therefore begins with existing GObject/GTK behavior and does not
  assume another Ghostty ABI addition.
- `docs/design/linux-global-find-plan.md` freezes ownership, test-first order,
  real X11/Wayland evidence, privacy limits, and explicit later scope before
  production wiring. Existing source-UX and multi-window actors must be
  extended; another application actor or search harness is forbidden.

## Global Find model and first product wiring

- Tests were written before the coordinator. The first run failed at compile
  time because `GlobalSearchCoordinator`, its target/effect types, and its
  state did not exist. That red receipt established that the tests were not
  accidentally exercising an older search path.
- The new pure coordinator tests now cover ordered fan-out, aggregate ordinals,
  the 150 ms short-query contract, navigation while totals are pending,
  forward/reverse wrapping, zero-result panes, ambient-focus divergence,
  frozen membership, stale-pane removal, and the distinct clear/end lifecycle.
  The focused test run passed 6/6 and strict Clippy passed after replacing an
  unnecessary clone assignment and removing a misleading internal parameter.
- Linux wiring reuses each live `GhosttySearchOverlay` and Ghostty's native
  `start_search`, `navigate_search`, and `end_search` binding actions. The
  window owns one coordinator and one focused sidebar view; no scrollback
  scraper, text index, second search engine, or second application actor was
  added.
- The first Linux unit run exposed an intentionally pinned action-registry
  cardinality failure (64 actual versus 60 expected) after the four new Global
  Find actions were registered. The expected count was advanced to 64; this is
  evidence that the central action inventory observed the feature rather than
  silently omitting it.
- Remaining uncertainty before this feature is reviewable: the real GTK search
  overlay must prove that hiding its pane-local HUD preserves its active core
  search, and both controlled X11 and Wayland journeys must prove aggregate
  totals, cross-pane navigation, clearing, closing, and per-window isolation.

## Global Find real-product failures and repairs

- The first controlled X11 attempt did not reach Zentty: the rebooted
  development environment exposed `/tmp/.X11-unix` as `nobody:nogroup`, which
  Xvfb correctly rejected. The scenario was rerun outside the filesystem
  sandbox where the standard root-owned X11 socket directory exists. This was
  recorded as an environmental failure, not converted into a pass.
- The initial one-pane X11 journey found that `Return` in the sidebar search
  field did not activate navigation. A window capture route now maps physical
  Return and Shift+Return to next and previous while Global Find is visible.
- The first input-capable Wayland run exposed a real translation difference:
  wlroots delivered lowercase `f` with Shift retained in the modifier mask,
  while X11 delivered uppercase `F`. The shortcut resolver had accepted only
  uppercase `F`/`E`; it now accepts both physical keysyms under the same exact
  modifier contracts. A focused regression test pins the Wayland form.
- The most important product discovery was that GTK
  `GhosttySearchOverlay.active` is lifecycle-bearing. Setting it false to hide
  the pane-local HUD can terminate the search before navigation, especially on
  Wayland. Global Find now leaves Ghostty active and hides only that overlay's
  presentation/targeting with opacity and `can-target`; `end_search` remains
  the sole lifecycle endpoint. The real Wayland journey changed from failing
  navigation to a selected Ghostty result after this repair.
- A remembered pane-local query is replayed synchronously by Ghostty's
  `start_search`. Starting Global Find without clearing the inactive local
  entry produced a stale total and then restarted the search for the global
  needle. The product now clears the inactive entry before start, then installs
  exactly the coordinator's needle. This prevents stale totals from satisfying
  aggregate readiness.
- Expanding the actor through the real command palette found two pre-existing
  keyboard defects. First, Enter was not captured by the palette. Second, GTK
  could retain a detached selected-row handle after filtering, causing the
  typed `Split Right` query to execute stale `New Window`. Enter is now handled
  in capture phase and always resolves the first currently rendered row. The
  actor also waits for the filtered-result receipt before activation, avoiding
  a genuine async input race rather than sleeping optimistically.
- Ghostty Debug output from renderer/search threads can interleave at byte
  boundaries with product stderr. Exact action lines were therefore unsafe
  evidence for F3/Shift+F3. The actor now proves the action prefix plus the
  subsequent exact Ghostty selected-result transition; it does not weaken the
  semantic assertion or treat corrupted Debug text as a product failure.
- The final controlled journeys pass on X11 and input-capable Wayland. Each
  creates three independently running Ghostty PTYs through real palette
  actions, spans two worklanes, aggregates nine real matches, navigates through
  pane 2 into the inactive worklane's pane 3, keeps a distinct one-result
  Global Find active in a second real window, closes that window with its
  search still active, resumes window 1 at the next aggregate ordinal, clears
  without closing the HUD, closes the HUD, and proves input reaches pane 3's
  real PTY. Weston headless was explicitly rejected for this journey because
  it lacks the virtual-keyboard protocol; the existing private Cage/wlroots
  input environment was used instead.
- These passes establish the implemented feature boundary; they are not an
  exhaustive-Linux-QA claim. Required non-PASS matrix cells still prevent
  release and full qualification.

## Global Find mutation audit

- The first mutation command repeated `gitignore` and `copy_target` on the
  command line even though `.cargo/mutants.toml` already makes those safeguards
  permanent. `cargo-mutants` rejected the duplicate configuration rather than
  running. The corrected command relied on the checked-in configuration, so no
  ignored `build/linux-deps` tree was copied into mutation scratch space.
- The first valid focused run found 99 mutants: 64 caught, 22 missed, and 13
  unviable. The misses exposed under-specified boundaries around fresh-session
  capture, query reset, shrinking totals, reverse traversal, stale-pane
  reconciliation, and pending navigation after an initial zero total.
- Five initial boundary tests reduced that result to 79 caught, 7 missed, and
  13 unviable. Two equivalent compound predicates were then simplified rather
  than pinned with implementation-shaped tests. Three behavioral tests proved
  reverse traversal across three panes, preservation of a live selection when
  another pane closes, and retention of pending navigation when a target first
  reports zero and later reports a match.
- The final focused receipt was **95 mutants: 82 caught, 13 unviable, 0
  missed** over `zentty-core/src/global_search.rs`, with all 14 focused model
  tests passing. “Unviable” here means the mutation did not compile; it is not
  counted as behavioral coverage. Real compositor journeys remain the evidence
  for Ghostty, GTK, focus, and physical input behavior.

## Complete qualification provenance failure

- The first complete qualification run executed every scheduled cell, but two
  API-audit cells failed before their product journeys. The checked-in audit
  claimed an `upstream/*` remote-tracking ref was available, while the exact
  locked checkout produced by `linux/scripts/prepare-ghostty-source` has only
  the direct fork's `origin`. This was reproducibility drift from a prior run
  against an operator checkout, not a Global Find failure and not a pass.
- The repair records the machine fact for the authoritative managed checkout:
  no convenience `upstream/*` ref, while the exact official base object,
  fork-main ref, merge base, embedding ancestry, patch identity, and locked head
  remain independently verified. The prose audit was reconciled with the JSON;
  no product assertion or ancestry check was removed.
- All other scheduled product and support cells passed in that run, including
  the new controlled Wayland Global Find cell. A complete rerun is still
  mandatory before commit; partial success is not qualification.

## Complete qualification reentrancy failure

- The mandatory rerun passed the repaired API audit but exposed a real Global
  Find crash under controlled Wayland. A Ghostty total notification rendered
  the sidebar while `ApplicationShell` was mutably borrowed; render corrected
  the GTK entry text, synchronously emitted `search-changed`, and the callback
  attempted a second mutable `RefCell` borrow. The process aborted at the GTK
  callback boundary. Earlier focused journeys had not produced that event
  ordering, so their passes did not establish reentrancy safety.
- The view now retains the one installed `search-changed` handler identity and
  blocks exactly that handler around model-to-widget text projection. User
  edits remain connected; programmatic reconciliation cannot recursively enter
  the coordinator. This replaces the unsafe projection behavior without adding
  another state owner or callback path.
- The same complete run also failed the older source-UX divider assertion after
  `Resize Pane Down`; that cell is being investigated separately and is not
  being relabeled as an environmental pass. All remaining scheduled cells
  passed, including the repaired API audit and X11 Global Find journey.
- Three fresh controlled Wayland ReleaseSafe repetitions passed the full
  three-PTY/two-worklane/two-window Global Find journey after the callback
  repair. The previously failed source-UX journey also passed unchanged when
  run alone. Its matrix log showed that the real key action fired but GTK had
  not emitted the allocation receipt before the actor's five-second bound while
  other heavyweight cells were running. The Down/Up allocation waits now allow
  fifteen seconds under qualification contention; they still require the exact
  new rendered-layout receipt and never retry the input gesture or infer a pass
  from action dispatch alone.

## Pre-acceptance Global Find qualification receipt (superseded)

- The final complete run passed every presently executable support test and
  authoritative matrix cell in 506.970 seconds. Declared totals are
  `PASS=91`, `FAIL=0`, `BLOCKED=7`, `XFAIL=1`, and `NOT_IMPLEMENTED=21`.
  Implemented-local and product-boundary qualification pass; release and full
  Linux qualification correctly remain false because all declared non-PASS
  cells remain visible. The machine-summary SHA-256 is
  `373c96d1dfa834154dab090a8f4a8a39c779df552f7e5d9bb8c8588b26da3005`.
- Debug Valgrind is **PASS with reviewed suppressions**, not unsuppressed clean.
  Raw evidence contains 427 errors/contexts, 6,240 definite bytes, and 41,461
  indirect bytes. Post-suppression evidence contains zero errors/contexts and
  zero definite/indirect bytes. The report, raw receipt, and suppressed receipt
  SHA-256 values are respectively
  `1da3d8ddba377b352f9de4cc4a6f2b3f95f4a247054288a8a16451b00950e3ef`,
  `4b3b1b10085ba7eadbbf18839c3774e340623f021107d73420ee04dee740149f`,
  and `ecd60551f8b038aa643a179ca2d5ac33311bd4768ad7e561636dbe51809e0c1a`.
  Suppression governance passed; ReleaseSafe Valgrind remains XFAIL and no
  suppression was added or broadened for Global Find.
- Final review also passed the complete locked Rust workspace (including 107
  Linux-shell tests and 14 Global Find policy tests), strict all-workspace/
  all-target Clippy with warnings denied, ShellCheck over every changed shell
  runner, JSON parsing, the feature-inventory runner, the application-shell
  ownership contract, whitespace review, and a repository secret-pattern scan.

## Acceptance-criteria audit after qualification

- Reviewing public issue #34 before close found three criteria that the first
  actor expansion had not directly exercised: GTK sensitivity/action identity,
  real short-query navigation flush, and live frozen-membership changes. The
  issue was not closed and the feature was not committed on the strength of
  prose or model-only evidence.
- Focused presentation policy now pins empty/query visibility, zero/nonzero
  navigation sensitivity, count formatting, accessibility vocabulary, and all
  four detailed GTK action identities. The existing real-product actor—not a
  new harness—now schedules a two-character query, flushes it with Enter,
  creates a matching fourth PTY after target freeze, closes a searched pane,
  and requires totals to move from three to two without admitting the new pane.
- The first two expanded X11 attempts produced no query state: the shortcut
  helper returned before XTest had released every chord modifier, so immediate
  text could still arrive as modified keys. The helper now performs the same
  explicit physical key-release cleanup used by the other X11 lifecycle
  journeys. A third attempt showed that a 100 ms external deadline was itself
  too short for X11 focus settlement. The actor now waits for the semantic
  pending state, not elapsed time, and sends Enter only while that state is
  observed; if the timer dispatches first, the pending-state requirement fails
  rather than accepting timer dispatch as navigation-flush evidence.
- The first successful short-query dispatch disproved the actor's assumed total
  of nine: `se` also matches `noise` in every PTY and the earlier pane-1 focus
  marker, yielding exact real totals of 5/4/4 and aggregate 13. Rather than
  clear and restart immediately after the flush, the final journey uses `ga`,
  which has one real `gamma` match in each PTY. That keeps the same short-query
  flush evidence and makes source-order cross-pane membership totals exact and
  comprehensible: 3 before closure, 2 afterward.
- The next attempt flushed and selected the short query correctly, then exposed
  the same XTest release hole in the actor's Ctrl+A/Backspace clear gesture:
  immediate replacement text never reached the field. One shared `clear_text`
  helper now performs explicit X11 release cleanup and retains the existing
  Wayland chord. This consolidates the gesture instead of adding a retry.
- That repair still failed because `wait_for_fixed` accepted the cleared-state
  receipt emitted when the empty HUD first opened, before the new clear gesture
  completed. The actor now snapshots the receipt count and requires a fresh
  cleared transition after each gesture. This applies the existing counted
  synchronization standard instead of adding a delay.
- The simplified short-query journey then reached the frozen-membership step
  and exposed a product focus race: a deferred Global Find refocus scheduled by
  navigation could run after Command Palette opened, steal focus back to the
  search field, and leave the palette with only a prefix of `Split Right`.
  Deferred refocus now resolves the live shell and refuses to override a visible
  palette. Global Find remains the focus owner only when no higher-priority
  modal surface is active.
- Tightening the expanded actor exposed several stale-receipt hazards rather
  than product failures. Aggregate ordinals recur during wraparound, the same
  palette query is used before and after target freeze, and an empty/hidden HUD
  state exists earlier in the pane-local search journey. Those assertions now
  snapshot exact receipt counts before their gesture and require a later
  occurrence. The actor can no longer pass a repeated state using old log
  evidence. The four-pane palette also truthfully reports two `Close Pane`
  matches (the action and a pane result), replacing the obsolete assumed count
  of one while still requiring execution of the exact close action.
- The topology-change journey then exposed a second product focus race. Closing
  searched pane 3 scheduled a real terminal focus-enter callback; Global Find
  navigation selected pane 1, but that stale callback subsequently changed the
  active pane to the newly created, deliberately excluded pane 4. Surface focus
  callbacks now decline to mutate workspace selection while the window-local
  Global Find overlay owns focus. The repaired X11 and Wayland journeys both
  close the searched pane, reconcile 3 to 2 results without admitting pane 4,
  dismiss the HUD, and prove text reaches the selected real PTY.
- The public criterion requiring both physical-input and sidebar invocation is
  not represented by action-name inspection alone. A controlled X11 screenshot
  established the actual 1000x700 sidebar icon location; the X11 actor now
  clicks that real button while the controlled Wayland actor invokes the same
  action with the real compositor virtual-keyboard chord. The latest focused
  passes are X11 session
  `6a1e1c145dcba16e443ce9072ce4617755768a3a5d3e903617d1fa5e21b87a10`
  and Wayland session
  `5a226d6b2641e23a4f5d1780673573a4bf36d02825197c61a4ef86b99706b063`.
  A fresh complete matrix run remains mandatory because the earlier receipt in
  this record predates these acceptance repairs.

## Final post-acceptance Global Find qualification receipt

- A direct locked-workspace test attempt inside the restricted execution
  sandbox failed eight real Unix-socket tests with `EPERM`. This was recorded
  as an environmental failure, not treated as a product pass. The identical
  command was rerun with the required socket permissions and passed every
  workspace test, including 108 Linux-shell tests and 14 focused Global Find
  policy tests; strict all-workspace/all-target Clippy also passed with warnings
  denied.
- The mandatory post-repair `linux/tests/qualify-local` run passed all presently
  executable support and matrix cells in 510.810 seconds. Declared totals are
  `PASS=91`, `FAIL=0`, `BLOCKED=7`, `XFAIL=1`, and `NOT_IMPLEMENTED=21`.
  Implemented-local and product-boundary qualification pass. Release and full
  Linux qualification correctly remain false because the declared non-PASS
  cells remain visible. The final machine-summary SHA-256 is
  `8e793aec46da9c2cfd13c9bd6de890ae9456ff57c873da989a5adf47139e4ced`.
- Debug Valgrind is **PASS with reviewed suppressions**, never described as
  unsuppressed clean. Raw evidence contains 427 errors/contexts, 6,240 definite
  bytes, and 41,397 indirect bytes. Post-suppression evidence contains zero
  errors/contexts and zero definite/indirect bytes. The report, raw receipt, and
  suppressed receipt SHA-256 values are respectively
  `e08b811fa8d54a8a9c500543dab919aef8fbfda4e5e4eb34b9df398dd6ff4fde`,
  `19a2cd4140bf787d6816a0a266445b29f88d59d9b9d738cd789ef5adfda7acbf`,
  and `8776e57ce9f3ada84524f7a4aa014df5ca09c1477821bef6f1a0e054229a278a`.
  Suppression governance passed, ReleaseSafe Valgrind remains XFAIL, and this
  feature neither added nor broadened a suppression.
- The first public close command used a nonexistent GitHub CLI
  `issue close --comment-file` option and failed before mutating the issue. The
  receipt was then posted with the supported `issue comment --body-file`
  command and issue #34 was closed separately as completed. This kept the long
  public qualification receipt file-backed without shell interpolation.

## AI disclosure

Implementation assistance and this field record were prepared with OpenAI
Codex under Jason Maskell's direction. Any proposed upstream Ghostty material
must be reviewed and submitted by the human contributor under that project's
contribution policy.
