# Zentty Linux dogfood — clipboard workflow closeout

Date: 2026-08-20
Tracking: GH-35, GH-17, GH-20

## Frozen closeout plan

GH-35 already owns the source-derived Clean Copy transformer, explicit Copy,
Copy Raw, Clean Copy, Copy as Markdown and Select All actions, the single
process configuration authority, contextual automatic-clean substitution, and
real X11/Wayland standard- plus primary-selection proof. This closeout does not
replace those systems or create another clipboard actor.

The remaining acceptance work is deliberately bounded:

1. Preserve an existing non-text desktop clipboard owner when a real Ghostty
   surface has no text selection.
2. Make an empty text selection follow the same no-overwrite contract.
3. Publish text through GTK's fallible content-provider operation so a refused
   ownership change is visible and cannot be logged as success.
4. Extend the existing `rust-pane-search` compositor actor to prove that two
   real windows retain distinct Ghostty selections and that actions route only
   through the focused surface.
5. Promote the authoritative inventory only after focused policy, real X11 and
   real Wayland journeys, mutation review, and ownership/orchestration checks
   pass.

Remote file/image upload remains owned and already exercised by GH-17's remote
paste paths. GH-35 does not invent a second non-text paste implementation.
Likewise, GDK clipboard publication is a synchronous ownership operation; the
product will report its real synchronous failure rather than inventing an
asynchronous completion protocol which neither GTK nor the source application
provides.

## Initial audit

- The older Clean Copy dogfood report is historically accurate but stale: it
  says automatic cleaning was waiting for GH-20. Commit `2ea8ff23` subsequently
  delivered the single settings owner, automatic default-Copy transformation,
  Copy Raw substitution, and identical configuration projection into both
  windows.
- The current inventory correctly describes that delivered configuration but
  still labels `clipboard.clean-raw-markdown` as `PARTIAL`. Its remaining
  qualification text names primary selection even though the current
  controlled platform-clipboard cells already read the real primary channel
  externally on both X11 and Wayland.
- The Linux copy path reads the focused Ghostty selection synchronously and
  never polls or reads a prior desktop clipboard owner. This is stronger than
  the macOS callback choreography for the no-selection boundary. Publication,
  however, uses GDK's infallible convenience `set_text` wrapper, so it cannot
  distinguish a refused ownership transfer from success in its receipt.
- The existing real actor already proves no-selection preservation for a text
  owner, automatic clean versus raw bytes, Markdown, primary selection, and a
  paste round trip. It is the correct place to add non-text preservation and
  cross-window routing; a new actor would be test-layer accretion.

## Red tests and repairs

- The focused Rust test for an empty selection was added before its policy
  helper. The expected red build failed with an unresolved
  `prepared_payload` import. The implementation now rejects only zero-byte
  selection text; whitespace remains valid raw clipboard content.
- Clipboard publication now constructs one GTK content provider and calls
  GDK's fallible `set_content`. Failure returns before the success digest is
  emitted and records a bounded `clipboard-write-failed` diagnostic without
  clipboard bytes. Successful ownership remains a single atomic provider
  replacement.
- The first extended X11 journey reached and closed the second real window,
  but XTest reported `BadWindow` while delivering the shortcut's key release
  to the already-destroyed XID. The product close was not accepted through that
  harness failure. The actor now follows the canonical multi-window actor:
  tolerate destruction only for that close delivery, clear modifier state on
  the live root, and require the exact product `window-closed` receipt before
  continuing.
- The first inventory promotion patch matched an unrelated earlier `PARTIAL`
  field and temporarily promoted worklane navigation instead of clipboard
  parity. The focused inventory evidence assertion rejected the result. The
  patch was narrowed by feature ID, worklane navigation was restored to
  `PARTIAL`, and only `clipboard.clean-raw-markdown` was promoted. The runner
  now passes with 22 implemented, 24 partial, and 14 not-implemented entries.
- The first focused mutation baseline reached the unrelated real development-
  server listener test and the command sandbox denied its loopback bind with
  `EPERM`. No mutant was credited. The unchanged four-mutant
  `prepared_payload` scope was rerun with local-socket permission: the baseline
  passed and all four mutants were caught. The repository's permanent
  `gitignore=true` and `copy_target=false` disk-safety policy remained active.
- The first combined format/workspace command stopped at `cargo fmt --check`:
  the new three-argument helper was expanded across five lines instead of the
  repository formatter's single line. The patch was formatted explicitly and
  the unchanged command then passed formatting plus the complete workspace
  suite, including 264 Zentty Linux binary tests and every real local-socket
  test with the required permission. Strict all-target workspace Clippy also
  passed with warnings denied.
- The platform clipboard cells previously declared `terminals=single` even
  after their canonical actor gained the required second-window journey. Both
  authoritative cells and the architecture mirror now declare `multi`; the
  orchestration contract pins that axis so the coverage cannot silently
  regress to one terminal.

## Real compositor closeout evidence

- Final controlled X11 session
  `400d9862b744b09f9fcba00ef51c44cfb6c9d0ba67d04e0397afee9cff28c671`
  passed with a real PNG clipboard owner preserved across a no-selection copy,
  external primary and standard clipboard reads, a real PTY paste round trip,
  and distinct first/second-window selections. The second window's exact raw
  bytes remained externally readable after that window closed, before the
  surviving first window deliberately replaced them.
- Final controlled Wayland/labwc session
  `3202a75709c2b3da7e74f126b87535ecd104a3284f4c5ae9fd82cde10be42b56`
  passed the identical product journey through Wayland clipboard and primary-
  selection protocols. The nested X11 transport session was
  `e234463c7ecd51d5e5b1e9e6b20d088a6f3aa05bc7e14b72b2db36601400868f`;
  it was input transport only and was not mislabeled as the product backend.

## Full local qualification and Valgrind closeout

- The post-implementation `linux/tests/qualify-local` run executed every
  presently executable matrix cell in 2,319,060 ms. Its authoritative declared
  totals remained 168 PASS, 0 FAIL, 3 BLOCKED, 3 XFAIL, and 6
  NOT_IMPLEMENTED. Both platform-clipboard cells passed. The run did not claim
  implemented-local, product-boundary, release, or full Linux qualification.
- One otherwise passing Debug/X11 single-terminal Valgrind cell failed to
  publish its report because its real Zentty window did not appear within the
  actor deadline under full-matrix contention. The consequent governance cell
  correctly failed because the report allowlist was incomplete. This was not
  converted into a pass and no suppression was added.
- The exact failed cell was rerun alone with the same Debug bundle, X11 backend,
  and single-terminal scenario. It passed, retained both independently captured
  receipts, and published
  `build/linux/memory-safety-Debug-single-x11.json`. Its raw totals were 689
  errors in 558 contexts with 67,056 definite and 152,016 indirect leaked
  bytes. Post-suppression totals were zero errors and zero definite/indirect
  bytes: **PASS with reviewed suppressions**, not an unsuppressed clean result.
- Governance then rejected several scenario-specific Fontconfig cache ranges
  which were narrower than the receipts just produced by this complete local
  run. Every observed value remained inside the manifest's already reviewed
  global rule ceiling, every child still co-occurred with its separately
  tracked Pango root, and the stack-constrained suppression file itself was
  unchanged. The scenario ranges were extended only to the observed extrema;
  the audit found no new rule, stack shape, library, environment, or untracked
  suppression. Governance subsequently passed over the complete allowlisted
  evidence set.
- Because the immutable full-run summary records the original isolated startup
  failure, it remains an honest non-passing qualification receipt. The exact
  isolated repair plus the passing governance rerun close the transient cell
  failure without misrepresenting that earlier receipt or spending another
  forty minutes rerunning unrelated green cells.
