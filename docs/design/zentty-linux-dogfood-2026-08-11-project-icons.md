# Zentty Linux dogfood: project icons

Date: 2026-08-11
Issue: GH-18

This record is chronological. Every discovery, failed assumption, repair, and
remaining limitation from the project-icon slice is recorded here.

## Starting constraints

- The Swift source uses a fixed ordered candidate list; it does not recursively
  crawl a repository. Linux must preserve that bounded shape.
- A discovered path is untrusted project content. Canonical containment and
  actual image decoding are both required before GTK projection.
- Icon discovery must not block terminal interaction or create another project
  context owner.
- No qualification claim is made until controlled compositor journeys and the
  complete presently executable matrix pass.

## Resolver construction and discoveries

1. The first source-order implementation used `?` inside candidate loops.
   A missing early candidate therefore ended the complete scan instead of
   advancing to the next candidate. The real ordered-candidate test failed.
   Every optional source is now an explicit `continue`, and the same test
   proves direct candidates, HTML fallback, and TSX fallback in sequence.
2. The first escaping-symlink test left a later valid candidate in the project.
   The resolver correctly selected that later candidate, exposing a faulty test
   assumption rather than a product defect. The fixture now removes the later
   candidate before asserting a miss.
3. A parser hardening test proved that `data-rel="icon"` was accepted as the
   real `rel` field. Field matching now rejects identifier/name prefixes while
   continuing past unrelated or malformed blocks. Direct parser tests cover
   HTML and object-literal syntax, field lookalikes, invalid dimensions, empty
   and network hrefs, traversal, and query/fragment removal.
4. Canonical containment accepts an in-project symlink target and rejects an
   escaping target. Reads are capped at 256 KiB for source manifests/markup and
   8 MiB for icon candidates. PNG, ICO, and SVG payload signatures are checked
   before the GTK decoder is reached.
5. Positive and negative cache behavior is explicit. The default negative TTL
   is proven at 299/300 seconds, custom TTL and per-root invalidation are proven,
   and `invalidate_all` is covered. Manual Git/review refresh invalidates the
   focused icon root through the existing project-context coordinator.
6. The source's `NSWorkspace.icon(forFile:)` fallback for macOS bundle suffixes
   (`.app`, `.xcodeproj`, and related bundles) has no cross-platform image
   contract and is not imitated with a guessed Linux icon. Linux preserves the
   portable favicon, AppIcon manifest, and markup sources; projects with only a
   macOS-generated file icon correctly have no project icon.

## Runtime and projection discoveries

1. Discovery was added to the existing single-flight GIO blocking probe rather
   than creating a watcher or second coordinator. Each returned observation is
   accepted only when both the observed and current pane directories still
   canonicalize to the same path. An early comparison of two failed
   canonicalizations could have treated `None == None` as current; it was
   repaired to require two successful canonicalizations.
2. The first controlled X11 journey resolved and decoded the real SVG in window
   chrome but timed out waiting for the sidebar. The project-context refresh
   path updated only the worklane Git row; pane metadata was refreshed only by
   unrelated workspace renders. A focused sidebar icon refresh was added to the
   same presentation method. The rerun then proved sidebar and chrome decoding.
3. The first Wayland journey incorrectly required the optional outer-X11 Weston
   input path even though the default controlled Cage environment provides a
   real virtual keyboard through `wtype`. The harness now drives the held
   Ctrl+Tab chord with XTest when available and `wtype -M/-P/-p/-m` under Cage.
   Environmental absence is not treated as a pass.
4. The resulting X11 and Wayland journeys start from a real escaping symlink,
   observe a miss, replace it with a real SVG, physically invoke the existing
   refresh command, decode the icon in sidebar and chrome, physically open
   Worklane Peek and decode it there, then restart with
   `show_project_icons=false` and prove discovery occurs without projection.
5. After the final concurrency/cache repair, both journeys were rerun against
   the rebuilt ReleaseSafe staged product and passed with real X11/Xvfb and
   Wayland/Cage compositor input. These are product-boundary results, not mock
   coordinator receipts.

## Mutation and quality evidence

- The first focused mutation run tested 82 mutants and reported 27 missed, two
  timeouts, four unviable, and 49 caught. The misses exposed untested default
  bounds/TTL, ICO and invalid payload handling, AppIcon size/scale ordering,
  `invalidate_all`, and parser field boundaries. Tests were expanded rather
  than waived.
- A second run reduced the result to four missed, one timeout, four unviable,
  and 72 caught. One remaining size-bound mutation required an accepted 2 MiB
  icon fixture. The remaining arithmetic mutations were either parser cursor
  infinite loops or an off-by-one that did not alter accepted fields; cursor
  math was rewritten with checked additions, and a redundant post-field
  boundary predicate was removed because the required `=`/`:` delimiter already
  establishes that boundary.
- The final governed run tested 65 mutants: 61 caught, four unviable, zero
  missed, and zero timeouts. No survivor or waiver remains.

## Remaining limitation

- Discovery is deliberately poll/refresh based and does not add another
  filesystem watcher. Positive entries remain stable until explicit
  invalidation, matching the bounded source cache contract. Agent-completion or
  general filesystem-trigger refinements remain part of the combined
  `project.git-review-icons` inventory item and keep that broader item PARTIAL.

## Final qualification receipt

- `linux/tests/qualify-local` passed every presently executable support and
  matrix cell in 420,510 ms.
- Declared totals: PASS 109, FAIL 0, BLOCKED 7, XFAIL 1,
  NOT_IMPLEMENTED 25.
- Implemented local suite: PASSED.
- Release qualification: NOT_PASSED.
- Full Linux qualification: NOT_PASSED.
- Valgrind result remains **PASS with reviewed suppressions**; suppression
  governance was accepted. No unsuppressed-clean claim is made.
- Machine receipt: `build/linux/qualification-summary.json` (ignored build
  evidence, reproducible through the authoritative matrix).
