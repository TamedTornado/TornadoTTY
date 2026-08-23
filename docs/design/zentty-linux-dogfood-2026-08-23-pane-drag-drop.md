# Zentty Linux dogfood — pane drag/drop (GH-81)

Date: 2026-08-23

## Starting state

- Worklane-card reordering already uses real GTK `DragSource`/`DropTarget`, a
  full rendered worklane drag icon, and a live insertion card.
- Pane transfer already has named/menu actions for same-window worklanes, new
  windows, and existing foreign-window worklanes.
- Cross-window transfer already detaches and adopts the exact live pane runtime
  with rollback on rejection; GH-81 must reuse this authority.
- No pane row or terminal pane currently exposes a GTK pane drag source, and no
  pane/column/worklane target accepts a typed pane payload.
- The existing foreign-window named action first focuses the source pane and
  then calls a handler bound to the source window. A destination-owned drop
  therefore cannot safely reuse the UI action directly; it needs a coordinator
  request carrying explicit source window, pane, destination, and generation.

## Source contract discovered

- `PaneDropResolver` gives sidebar/worklane targets priority, followed by stack
  gaps, pane split zones, column insertion, then cancellation.
- The source renders and reflows the actual list/strip during drag rather than
  showing only an outline target. Linux worklane reorder already follows that
  standard; pane drag feedback must meet it too.
- Option-drag duplication exists in the source resolver. GH-81 is a move-parity
  issue and the existing Linux transfer transaction owns live surface identity;
  duplication will remain explicit until a source-compatible runtime-cloning
  contract exists rather than pretending a moved Ghostty surface was copied.

## Resolver test-first failure

- The first compile of the pure Rust resolver tests failed because a local
  `input` binding shadowed the test fixture function before later cases called
  it. This was test construction, not product behavior.
- The fixture is now explicitly named `base_input`; the production resolver was
  unchanged. The next receipt must compile and run all resolver cases before GTK
  wiring begins.

## Exact-pane core mutation compile failure

- The first test compile of the transactional exact-pane mover found two Rust
  borrow overlaps while copying replacement/focused column IDs after mutation.
  The intended state transitions had not executed.
- The repair snapshots only the small stable column IDs before the subsequent
  mutable assignment. No duplicate topology store or alternate move path was
  introduced.

## GTK content-format test compile failure

- The first GTK content-format isolation test used the C API spelling
  `contain_gtype`; gtk-rs exposes this as `contains_type`, so the focused Linux
  test target did not compile.
- The repair changes only the assertion to the gtk-rs method name. The same
  focused target is rerun before application wiring continues.

## Application wiring compile failure

- Adding the typed pane-drop application action made the coordinator match
  intentionally non-exhaustive until the commit path was present. `cargo check`
  stopped at that missing arm; no executable with a silently ignored drop was
  produced.
- The repair adds a fail-closed coordinator: both windows, both topology
  generations, and the exact source pane/worklane identity are checked before
  dispatching to the existing same-window mutation or live cross-window
  transaction.

## Sidebar constructor test compile failure

- Extending the focused worklane-card constructor with the optional pane-drag
  context exposed one direct unit-test call that deliberately bypasses the
  public renderer. The compiler rejected the missing argument.
- The fixture now passes `None`; production construction continues to pass the
  real window/generation/drop context.

## Cross-window core fixture compile failure

- The first exact-target transfer fixture called the extraction API by its
  abbreviated conceptual name. The real core method is intentionally explicit:
  `extract_pane_for_cross_window_transfer`.
- The fixture now calls the production API; no compatibility alias or second
  transfer path was added.

## Focused X11 journey lint failure

- The first standalone `shellcheck` invocation treated the repository-relative
  dynamic library sources and the intentionally deferred PTY-child command as
  unacknowledged informational findings.
- The script now documents those three deliberate cases at their exact lines;
  no broad shellcheck exclusion was added.

## First real X11 run: terminal was detached during rerender

- **Failure:** the first controlled X11 journey never reached
  `terminal-ready`. A normal startup rerender logged
  `gtk_overlay_remove_overlay` and `gtk_overlay_set_child` criticals, then left
  the Ghostty frame parented to the discarded pane-drop wrapper.
- **Cause:** the pre-existing generic reparent helper assumed every child of a
  `GtkOverlay` was an overlay child. Pane drop intentionally makes the durable
  pane frame the overlay's primary child.
- **Repair:** the helper now checks whether the widget is the primary child and
  clears it with `set_child(None)`; only actual overlays use `remove_overlay`.
  This preserves the single durable Ghostty widget rather than recreating it.

## Second real X11 run: fixed drag-origin coordinates missed the strip

- The GTK lifecycle repair worked: both real terminals initialized with no
  critical. The journey's hard-coded pane-strip coordinate did not begin a DnD
  session, however; the log showed only ordinary terminal focus.
- The focused journey now locates the actual rendered 15px strip through a
  pane-specific pointer-entry receipt before pressing the real mouse button.
  The product receipt is also useful accessibility/diagnostic evidence and
  avoids making qualification depend on window-decoration geometry.

## Third real X11 run: activation motion left the 15px source

- The pointer-entry receipt proved the rendered pane strip was reached, but the
  journey's diagonal threshold motion moved 10px vertically from an unknown
  point inside a 15px strip. It could leave the source before GTK claimed the
  drag, and no `drag-begin` receipt appeared.
- Threshold motion is now horizontal within the strip before traveling to the
  destination. This keeps the physical journey faithful to a user's drag while
  removing an invalid coordinate assumption.

## Fourth real X11 run: pane source lost the gesture arbitration

- Horizontal in-strip motion still produced pointer-entry but no `drag-begin`.
  Comparing the working real worklane reorder source exposed the product defect:
  that source participates in GTK's capture phase, while the new pane source
  had retained the default bubbling phase and lost arbitration to the embedded
  terminal gesture path.
- Both terminal-strip and sidebar-pane drag sources now use GTK capture phase,
  matching the established worklane DnD implementation. This is product code,
  not a test-only bypass.

## Fifth real X11 run: one synthetic threshold jump remained inconclusive

- Capture phase alone still produced no begin receipt when the journey emitted
  one 20px relative XTEST jump and immediately crossed to the destination.
- The journey now mirrors the already-qualified worklane reorder gesture: ten
  small within-source motions, followed by an explicit wait for GTK's
  `drag-begin`, then destination travel. This separates source activation from
  target resolution and makes any remaining failure attributable.

## Sixth real X11 run: empty overlay strip could not own GtkDragSource

- Incremental motion conclusively reached the strip but GTK never invoked the
  drag source. The visual `GtkBox` overlay receives pointer crossing, yet it is
  not a reliable native drag owner above the embedded GL surface.
- The source is now installed on the durable pane frame, which already owns the
  pane's pointer-controller boundary. Its `prepare` callback only provides
  content for the top 15px, preserving the source drag-zone contract and
  leaving terminal selection outside that strip untouched. The visual strip
  remains the hover affordance and diagnostic target.

## Seventh real X11 run: frame-level source still lost to the terminal

- The pane frame also received no `prepare` callback, confirming that moving a
  native drag controller higher in the embedded terminal hierarchy did not win
  the sequence.
- The source drag zone is now a real frameless GTK button spanning the same
  15px strip, with the source's centered ellipsis affordance. It is an explicit
  interactive owner rather than an empty overlay box, so GTK can arbitrate DnD
  without stealing terminal selection below the strip.

## Eighth real X11 run: GtkButton's click gesture won arbitration

- The explicit button received pointer entry but its built-in click gesture
  still prevented `GtkDragSource` from beginning.
- The strip now uses a non-empty `GtkLabel` with the same ellipsis affordance.
  Unlike the empty box it has real rendered content, and unlike the button it
  has no competing click gesture; the drag source remains its only press owner.

## Ninth real X11 run: gesture crossed 50px but dynamic prepare did not fire

- Temporary motion diagnostics proved the label received a 50px held-button
  motion and `GtkGesture` began, yet `GtkDragSource::prepare` was never emitted.
- Pane payloads are immutable for the life of each topology render, so the
  source now receives its typed content provider eagerly with `set_content`.
  The temporary per-motion logging was removed; the durable begin/drop/end
  receipts remain.

## Pointer-target discovery raced its own asynchronous receipt

- Pane-row and terminal-strip variants both showed entry receipts but no
  production drag. Comparing the focused scanner with the established divider
  and worklane journeys found that it moved again immediately, before GTK's
  stderr receipt could be observed; by the time the shell saw the prior entry,
  the physical pointer could be outside the source.
- Each bounded discovery move now allows 20ms for GTK dispatch before deciding
  to advance. This is the same synchronization pattern used by the existing
  physical-pointer actors, not a product delay.

## Corrected diagnosis: XTEST motion coalescing, not an unusable drag source

- The ten rapid 5px relative motions used while diagnosing source activation
  were coalesced into roughly two GTK updates and did not reliably cross GTK's
  drag threshold. This invalidated the earlier conclusions about empty boxes,
  frame ownership, and eager versus prepared content as independent causes.
- One held 40px motion after a 250ms press deterministically begins the real
  drag. The final source remains the explicit 15px label because it provides a
  clear rendered and accessible affordance, but no claim is made that labels
  are a GTK requirement.
- The first terminal-strip scan also started at x=285 and occasionally grabbed
  the real sidebar divider, widening the sidebar instead of dragging a pane.
  The actor now begins at x=400 and uses a pane-specific pointer receipt.

## Real target lifecycle failures and repairs

- A single large source-to-destination pointer jump did not establish GTK
  target crossing. Bounded intermediate points now represent a real pointer
  trajectory before the actor searches for the exact preview zone.
- Installing drop targets on the durable pane frame accumulated controllers
  across renders and eventually segfaulted. Targets now live on ephemeral
  wrappers; every closure that references a wrapper uses `glib::WeakRef`, so
  controller ownership cannot create a wrapper cycle.
- Selecting a sidebar pane synchronously from `drag_begin` rerendered the
  destination wrappers while GTK still owned the drag. Drag begin no longer
  changes product selection. Successful mutation focuses the moved pane;
  cancellation preserves the prior focus.
- Ghostty's terminal drop target and the new pane target both saw the same drag
  crossing. GTK 4.14 then reported `gtk_drop_target_async_handle_crossing` and
  `gtk_drop_target_async_handle_event` assertions when preload or capture phase
  was used on the pane target. A transparent, drag-only shield now becomes
  targetable only while a typed pane payload is active. The target uses normal
  propagation and no preload, while normal terminal input is never shielded.
- Mutating and rerendering synchronously inside `GtkDropTarget::drop` produced
  the same GTK lifecycle assertions. The drop callback now returns first and
  schedules the authoritative commit 16ms later on the GTK main loop.

## Integrated X11 actor exposed an asynchronous resize assumption

- The standalone focused journey passed, but its first integration into
  `rust-source-ux-x11` failed to reach the bottom stack gap. The integrated
  actor resized the toplevel to 1200px after fixed split widths had been
  resolved for the earlier 1000px viewport; x=1000 could therefore land in
  newly exposed blank viewport rather than pane-2.
- A captured real screenshot proved both panes were present and the rightmost
  region was blank. The actor now keeps its destination probe at x=800, which
  remains inside pane-2 under either valid allocation. Environmental absence
  was not converted into a pass.
- The integrated actor now proves terminal-strip to stack-gap and sidebar to
  leading split, exact geometry, one `terminal-ready` per pane, two and only
  two terminal spawn receipts, Escape cancellation, and invalid background
  release without topology mutation.

## First cross-window X11 drag: focus actor and product focus defects

- The first real two-window X11 drop succeeded: the live pane appended to the
  destination, the empty source window closed, pane identity stayed exact, and
  both original PTY PIDs remained. Post-drop input initially failed.
- The actor had temporarily narrowed both native windows to 600px, while the
  shared input helper clicked x=700. That click landed outside the surviving
  product and could not prove focus. Restoring width then exposed a separate
  product issue: the released pointer/keyboard focus could remain on a pane
  control, and the test's Return activated Add Pane Right.
- Cross-window and same-window pane commits now schedule one post-presentation
  selected-surface focus restoration and emit an exact completion receipt.
  The X11 actor gives the surviving native toplevel X focus without clicking
  blank viewport. Post-drop physical typing and the authenticated agent route
  then pass without constructing another terminal.

## Controlled Wayland required compositor-owned window arrangement

- The first nested-Weston cross-window drag saw only the source window: Weston
  had placed the two native Wayland toplevels on top of one another. Scanning
  harder merely exercised the source worklane repeatedly and correctly ended
  the drag without a destination.
- The actor now uses Weston's real Super+primary-drag to move the active source
  toplevel right and reveal the destination. This is compositor input, not a
  model shortcut or test window API. The subsequent GTK drag crosses two real
  Wayland surfaces and appends into the foreign worklane.
- The same controlled journey then drags the adopted pane from its sidebar row
  onto the sibling's leading split zone. Exact two-column geometry, both
  original PTY shell PIDs, two total Ghostty spawn receipts, a physical focus
  click, post-drop input, and the adopted agent route all pass.

## PTY-set assertion initially counted the intentional inhibitor service

- After sending the controlled `agent.running` event, the Wayland journey saw
  a third direct child and initially reported a PTY duplication. Process
  evidence identified it as the expected `systemd-inhibit` lease, not a shell
  and not a Ghostty spawn.
- The repaired assertion compares the exact direct `sh` PID set and also
  requires exactly two `info(io_exec): started subcommand` receipts. It does
  not weaken lifecycle coverage by accepting arbitrary children.

## Mutation testing found real boundary gaps

- The first scoped run used the `pane_drag` test filter while resolver tests
  lacked that prefix, so 22 of 48 mutants survived without executing the
  intended tests. All focused tests now share the prefix.
- The second run caught 43 mutants and exposed three unpinned inclusive bounds:
  x=0, y=0, and y=height. Explicit edge cases were added.
- Final scoped receipt: 48 mutants, 46 caught, 2 compiler-unviable, 0 missed.
  The run used `.cargo/mutants.toml` with `gitignore = true` and
  `copy_target = false` inside the repository's bounded systemd scope.

## Destroying a destination during a live drag exposed cancellation details

- A controlled X11 journey began a real sidebar-to-foreign-worklane drag and
  closed the destination native window while the pointer remained held. The
  source pane and PTY remained intact and no pane-drop commit was emitted.
- Releasing the pointer after the destination disappeared did not by itself
  end GTK's drag session. The actor therefore sends Escape and waits for the
  real drag-cancel, drag-end, and terminal-focus-restored receipts rather than
  treating a timeout or missing destination as success.
- Escape legitimately reached the raw test PTY input line during this race.
  The actor clears that unfinished line with physical Ctrl+U before typing an
  independent liveness marker; it does not sanitize product output or infer
  liveness from the prior input.
- Restoring focus through the original sidebar widget was unreliable because
  a render can replace that card during the drag. Cancellation now routes
  through a durable application-shell callback and restores the selected live
  terminal after the GTK drag has ended.

## Final route and snapshot assertions were initially incomplete

- The first physical cross-window receipts proved the authenticated agent
  route but did not independently exercise the tmux compatibility route after
  the drag. The canonical multi-window actor now sends a real tmux probe
  through the moved PTY, requires the destination worklane route, and waits for
  its terminal response.
- The same journey now waits for the debounced aggregate restore snapshot and
  requires the sole surviving window and exact two-pane ownership before exit.
  Clean/crash restoration remains owned by the existing multi-window
  qualification cells, which are explicit dependencies of the pane-drag cells;
  no second persistence actor was added.

## Pedantic lint review rejected an oversized core transition

- The first final Clippy pass rejected the 253-line pane-move transition along
  with needless owned parameters and clone assignments. Although the tests
  passed, that shape made validation, detachment, and insertion difficult to
  review independently.
- The core path is now decomposed into source lookup, target validation,
  mutation orchestration, and target-specific column/stack/split insertion
  helpers. The focused core tests remained green after the refactor.
- The final two-crate all-target lint pass is warning-free apart from the
  explicitly excluded pre-existing `worklane_peek::render` line-count lint;
  all other warnings remain denied.

## Final rerun initially used the wrong sandbox boundary

- The first final rerun could not start its private Xvfb and reported the
  user-namespace view of `/tmp/.X11-unix` as `nobody:nogroup`. Treating that
  diagnostic as host ownership damage was incorrect; an attempted ownership
  repair was denied and changed nothing.
- The controlled compositor journeys require the repository's approved GUI
  escalation so Xvfb can create its private sockets. Rerunning through that
  boundary started the same wrapper successfully. This was an invocation
  error, not a compositor/product failure and not evidence that host ownership
  should be changed.

## Full local qualification remains blocked by the deferred runner defect

- The final `qualify-local` invocation passed every support contract, then the
  product matrix refused an unexpected stale
  `build/linux/matrix-logs/.cell-results.U5Ypw3` directory dated 2026-08-22.
- This is the previously deferred GH-83 qualification-runner problem. Per the
  operator decision, it was recorded rather than deleted, normalized into a
  pass, or investigated inside this feature. All four presently executable
  pane-drag product cells and the actual GTK accessibility journey were rerun
  independently and passed against the final ReleaseSafe binary.

## Full crate test initially crossed the network sandbox incorrectly

- An un-elevated full `zentty-linux` test run passed 306 tests but the existing
  real `/proc` listener-correlation test could not bind its loopback socket and
  failed with `EPERM`. This was not accepted as a product failure or a skip.
- The same complete package run under the required real-socket boundary passed
  307 tests with the two controlled-display accessibility tests explicitly
  ignored there; those accessibility tests passed separately under actual
  `GTK_A11Y=test` and private X11.

## Current real-system receipts

- X11 same-window: `Rust source UX pane drag passed: x11,
  real-gtk-dnd=1, exact-topology=1, live-pty-preserved=1`.
- X11 cross-window: `Rust cross-window pane drag passed: x11,
  pointer=real-gtk-dnd, source=closed, destination=existing, pty=preserved,
  route=adopted`.
- Wayland cross/same-window: `Rust cross-window pane drag passed: wayland,
  pointer=real-gtk-dnd, source=closed, destination=existing, pty=preserved,
  route=adopted`.
- X11 destination destruction: `Rust pane drag destruction passed: x11,
  destination=closed-mid-drag, source=unchanged, pty=live`.
- Actual GTK accessibility test passes for the draggable sidebar pane,
  terminal strip, and worklane destination descriptions under `GTK_A11Y=test`.
- The existing menu, palette, and named-action transfer route remains present
  and is still the equivalence control for clean/crash restore. No alternate
  workspace actor or transfer protocol was added.
