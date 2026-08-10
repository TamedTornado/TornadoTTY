# Zentty Linux Clean Copy dogfood record — 2026-08-10

Tracking: [GH-35](https://github.com/TamedTornado/zentty/issues/35)

## Discovery and frozen plan

- The next actual feature slice is the source Clean Copy family, not another
  qualification-only cleanup. Public issue #35 and
  `linux-clean-copy-plan.md` freeze source authority, component ownership,
  test-first order, mutation expectations, and real compositor clipboard proof.
- Source inspection found a deliberately conservative ordered pipeline of
  roughly 1,600 implementation lines plus roughly 1,750 focused test lines.
  Replacing it with whitespace joining would corrupt code, columns, URLs,
  paths, and terminal transcripts. The Linux implementation therefore begins
  from source-derived goldens and bailout boundaries rather than UI wiring.
- Ghostty owns the real selection, but its GTK embedding API initially exposed
  only viewport/screen reads and a fire-and-forget `copy_to_clipboard` binding.
  The latter cannot distinguish “copied this selection” from “no selection, so
  the prior clipboard owner remains.” Treating the existing clipboard as the
  selection would therefore transform stale unrelated data. The minimal
  missing engine-owned primitive is a synchronous, borrowed-callback selection
  read; clipboard ownership and every formatting decision remain in Zentty.
- Automatic Clean Copy depends on the future single settings owner in #20. The
  explicit source actions can be implemented without inventing a second config
  path; that dependency will remain visible until the setting itself exists.

## Pure-policy construction

- The first source-derived Rust fixtures exposed three real defects before UI
  wiring: the draft treated time-of-day prefixes as grep gutters, omitted
  YouTube's host-specific share parameters, and the fixture itself accidentally
  contained a literal `+` in a continued command. The fixture typo was repaired
  without weakening its expected result; gutter recognition was split by
  delimiter with the source short-selection, time, IPv6, threshold, and
  monotonicity rules; and the full source tracking-parameter catalog plus
  subdomain-aware YouTube rules were ported.
- The focused corpus now covers ANSI/OSC removal, terminal padding, gutters,
  IPv6 and time bailouts, box structure, prompt-majority policy, explicit
  command continuation, prose/status preservation, URL repair and tracking,
  conservative path quoting, Markdown classification/reflow, and idempotence.
  source agent markers/rules/status blocks, plain prose and blockquotes,
  wrapped identifiers/paths/tokens, slash-command decoration, configurable
  command-flatten aggressiveness, focused tests, and strict Clippy. This is
  policy evidence, not a claim about desktop clipboard ownership.
- A direct SHA-256 dependency was added rather than relying on a transitive
  crate. Product receipts contain only action, pane identity, byte count,
  modification state, and digest; clipboard contents never enter logs or
  persistence.

## Product wiring and controlled compositor proof

- The initial asynchronous design called Ghostty's `copy_to_clipboard`, then
  read GTK's clipboard on an idle callback. Audit found that a no-selection
  action leaves the old owner intact, so this design could silently clean stale
  unrelated bytes. It was replaced rather than suppressed or papered over.
- Ghostty commit `472ba70436a7f13006d24612eab7d04cce820cc9`
  adds one product-neutral `ghostty_gtk_embed_surface_read_selection` function:
  10 header lines, one version-script symbol, and 14 Zig lines. It delegates to
  Ghostty's existing locked `Surface.selectionString`; it adds no Zentty,
  clipboard, formatting, or Rust policy. The isolated Debug GTK embedding
  library build passed before the product lock was advanced.
- The Rust adapter copies the borrowed bytes synchronously. Zentty then owns
  the four exact source actions (`Copy`, `Copy Raw`, `Clean Copy`, and `Copy as
  Markdown`), standard GTK clipboard publication, formatting policy, palette
  and context presentation, and focus return. Removing the idle choreography
  also removes stale-generation and callback-after-window-drop failure modes
  rather than adding another lifecycle layer.
- The first real X11 run proved the copy action itself but then typed subsequent
  fixture commands into the hidden palette entry. The clipboard path had failed
  to restore terminal focus. `copy_focused_selection` now returns focus to the
  selected live surface for raw and transformed actions; the next run delivered
  the command to the real PTY.
- Extending the existing `rust-pane-search` actor initially invalidated its
  global-search corpus: newly created panes displayed the clipboard URL rather
  than the original search text, so only one of three targets matched. Each
  focused new PTY is now explicitly loaded with the search corpus. The frozen
  fourth target instead starts with a functional parameter containing the
  search needle, preserving the original exclusion proof without requiring
  focus that Global Find intentionally retains elsewhere.
- The X11 actor now passes end to end in private Xvfb session
  `2087e5547510fd1f33a1da75a6e5d004edfccf47ac58659123fa4b425b5f918d`:
  real Ghostty Select All, all three command-palette copy actions, exact
  independent `xclip` bytes, and the existing pane/global search journey.
- The first attempted Wayland command inherited `GDK_BACKEND=x11` from the
  nested X11 transport and therefore proved X11 again, not Wayland. This was
  not counted. Setting `GDK_BACKEND=wayland` inside the controlled command
  produced genuine private Cage session
  `a59f938986c435d2592715e7386b996d88e2850849e56c3e00942ae8faa84f5b`,
  where the same actor passed with exact independent `wl-paste` reads.
- `wl-clipboard` 2.2.1 was installed from Ubuntu Noble so environmental absence
  could not be converted into a pass. The actor explicitly fails if `xclip` or
  `wl-paste` is absent.
- A final actor extension recopies the selected URL cleanly, externally checks
  it, and physically pastes it through Ghostty into the real PTY; the child OSC
  title is the exact round-trip acknowledgement. The final X11 and Wayland
  sessions above both include and pass this round trip.
- A later direct-selection run found that the palette search `Copy` correctly
  returns four related commands, while the actor incorrectly required every
  query to have one result. The shared palette helper now accepts an explicit
  expected result count; it still executes the exact-match first result and
  waits for the named action receipt.
- The actor now establishes an independent compositor clipboard sentinel,
  invokes Clean Copy with no terminal selection, requires the explicit
  `selection-unavailable` receipt, and proves the sentinel bytes remain exact.
  It then proves all four actions against a real Ghostty selection and an
  independent compositor reader. Current passing sessions are X11
  `db4b41a686a058a7f7e20cb3297e8e08a3921f81094387e4a9e5fa71f86c4191`
  and genuine Wayland
  `b2d279d6d2371f247b9a56eefc10a8797b87f1f12de2e3e2b81a41d9e6589de7`
  (nested transport X11
  `8f347be7a217503c290ffbd4f625dce8a4e063fcad49219b77f0f38fb7ea4d56`).
- The first sandboxed nested-X11 rerun could not create `/tmp/.X11-unix` and
  was recorded as an environmental failure, not a product pass. Re-running the
  same controlled harness with the required host permission succeeded.
- Advancing the pinned Ghostty revision exposed stale API-audit inventory and
  its old `upstream_remote_tracking_ref_available=false` fact. The fork now has
  the official upstream tracking ref after the provenance repair. Exact
  normalized range/file hashes, the 14-symbol ABI, 11 Rust declarations, C++
  signature, null/foreign/uninitialized misuse probes, and version allowlist
  were reconciled. `ghostty-api-audit --self-test` and `abi-surface` pass.

## Remaining boundary

- Explicit Copy, Clean Copy, Copy Raw, Copy as Markdown, and Select All are present.
  Automatic cleaning remains unimplemented until GH-20 supplies the single
  settings owner. The feature inventory therefore advances from
  `NOT_IMPLEMENTED` to `PARTIAL`, not `IMPLEMENTED`.
- The platform clipboard matrix still covers broader standard and primary
  selection semantics. This slice proves the standard clipboard only and does
  not turn those broader cells green.

## Mutation and harness audit

- The first Clean Copy mutation attempt was invalid as evidence: the new policy
  and golden-test files had not yet been added to Git, so cargo-mutants did not
  copy them into its scratch trees. The files were staged before rerunning. This
  is now an explicit review point whenever mutation scope includes a new file;
  a green ordinary test run does not prove the mutation workspace contained it.
- cargo-mutants shard indices are zero-based (`0/4` through `3/4`). An attempted
  `4/4` shard was rejected rather than reinterpreted. Parallel shards also
  contend on the default `mutants.out`; each concurrent invocation must have a
  distinct `--output` directory. The project wrapper still supplies the
  permanent disk-safety settings `gitignore = true` and `copy_target = false`,
  so ignored `build/linux-deps` trees are not replicated per worker.
- The first complete, correctly staged run exercised 493 mutants: 474 caught,
  17 missed, one unviable, and one timeout. The timeout reverses the terminal
  blank-line loop predicate and is a detected non-terminating mutant, not an
  unexplained product timeout. The missed set exposed independently invisible
  boundaries in blockquote majority, command aggressiveness, operator scoring,
  blank-line preservation, and path-token punctuation. Focused classifier tests
  now call the pure private policy helpers directly; they do not introduce a
  second integration harness or fake the clipboard/terminal components.
- Mutation review also found two genuinely redundant branches. Environment-only
  blocks were already protected by the later all-command veto at Low and Normal
  aggressiveness, while High intentionally permits them; the duplicate branch
  was removed. An all-lines-command score increment was unreachable as a useful
  discriminator after the earlier mode-aware guard and was also removed rather
  than retained solely to satisfy a coverage number.
- The next complete run reduced the policy surface to 487 mutants and reported
  484 caught, one missed, one unviable, and the same one detected timeout. The
  final miss showed that the High-aggressiveness direct-helper corpus did not
  prove two independent known shell commands remain separate. That source
  contract now has a direct regression assertion. Final mutation totals are
  487 tested, 485 caught, one compiler-unviable, one deliberately detected
  non-terminating timeout, and zero missed viable mutants. No surviving viable
  mutant is accepted as passing evidence.
- `qualify-local` has no help-only mode: invoking it with `--help` began real
  prerequisite qualification. That attempt correctly failed when its managed
  Ghostty checkout could not resolve the new, still-local pinned SHA. It was not
  counted as a product result. The reviewed 25-line Ghostty commit was pushed to
  the Zentty fork branch before the real qualification rerun so a fresh managed
  checkout—not an ambient worktree—can fetch and prove the exact lock revision.
- The first workspace-wide test rerun was sandboxed and eight real Unix-socket
  agent-helper tests failed at bind with `EPERM`; the three cases not requiring
  a listening socket passed. This is an execution-environment denial, not a
  product pass or failure. The unchanged workspace command is rerun with the
  permission required to create its private local sockets below.
- The first full matrix run passed every build, backend, compositor, lifecycle,
  packaging, Ghostty regression, agent, and reviewed-suppression cell that
  reached the product, but correctly refused qualification for four integration
  defects plus one unexpected XFAIL skip. Two were stale ledgers: the feature
  inventory's PARTIAL count/schema and the architecture/ownership contracts had
  not been advanced with the new Ghostty pin, actions, or clipboard coordinator.
  The API audit had also captured this developer worktree's `upstream` remote,
  although the controlled managed checkout intentionally has only `origin`;
  its environment-sensitive base statement was restored to the controlled
  checkout fact.
- The source-UX actor clicked the established first contextual action by a
  physical pointer coordinate. Prepending four clipboard actions silently made
  that coordinate invoke Copy. Rather than teaching an actor to accept shifted
  legacy controls, the new clipboard commands were moved after the established
  pane-management group, preserving the existing contextual positions; the
  real nested-X11 actor then passed in session
  `e66099fb1182c85ffe8a96976216acef80252e00a3159196ea61b7b9b7225dbe`.
- The XFAIL async-backend ABI cell found an ignored `zig-out` header generated
  under the previous Ghostty pin. Git checkout cannot update ignored outputs,
  so it was correctly an unexpected prerequisite skip, not the tracked XFAIL.
  An initial repair deleted the managed checkout's `zig-out` during source
  preparation. The next full run proved that wrong: several established native
  contract actors intentionally inspect that exact source-build library, so the
  deletion converted one stale-header skip into missing-library failures. The
  cleanup was reverted. The correct repair is to build the pinned managed
  Ghostty source output itself before qualification, making its header and
  library one coherent receipt rather than deleting a shared prerequisite.
- After rebuilding that source output with the exact pinned Zig 0.16.0,
  project-owned cache directories, baseline CPU, Debug optimization, and
  vendored layer-shell flags, the complete local qualification rerun passed
  every presently executable support and product/dependency cell. The matrix
  recorded 91 declared PASS cells, seven BLOCKED, one expected XFAIL, and 21
  NOT_IMPLEMENTED; it therefore reports **Implemented local suite: PASSED**,
  **Release qualification: NOT_PASSED**, and **Full Linux qualification:
  NOT_PASSED**. The Valgrind result is PASS with reviewed suppressions; this is
  not an unsuppressed-clean claim. The run's wall time was 373,960 ms and its
  machine receipt is `build/linux/qualification-summary.json`.
