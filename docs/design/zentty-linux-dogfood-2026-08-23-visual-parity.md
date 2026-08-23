# Zentty Linux visual parity dogfood — 2026-08-23

Issue: GH-84

## Starting state

- The repository had one source hero screenshot and four transient Worklane
  Peek screenshots, but no authoritative element-by-element parity map, no
  checked-in Linux visual baselines, and no generic stale-baseline/mask
  governance.
- `rust-source-ux-x11` already masks only real terminal-picture rectangles and
  proves that progress, attention, and selection screenshots differ. Those
  images are transient receipts, not reviewed parity baselines.
- Scaling journeys prove geometry, SIGWINCH, physical pointer coordinates, and
  compositor surface commits at 1x/1.5x/2x, but did not preserve screenshots.
- External staged-product AT-SPI remains owned by GH-86. In-process GTK
  metadata cannot be used to claim external semantic parity.

## Decisions

- Add a strict evidence-map validator to the existing support-test phase rather
  than create a new qualification scheduler.
- Extend existing actors for screenshots. Do not add a screenshot-only fake
  shell or a second workspace actor.
- Permit masks only for terminal content whose rectangle is emitted by the
  running product journey. No static broad mask can cover product chrome.

## Correction: do not recreate hash-based evidence bookkeeping

- The first plan draft proposed source, image, baseline, binary, scenario, and
  mask hashes, and the initial map included a SHA-256 of the source screenshot.
  That repeats the discarded paranoid harness design: Git already versions the
  source and reviewed baselines, while pixel comparison already detects visual
  changes.
- The duplicate hashes were removed before the validator was implemented.
  Visual governance will check schema, required files, narrow masks, semantic
  receipts, and actual pixel comparisons—not maintain a parallel hash ledger.

## Existing actor audit and repair

- The first real X11 source-UX rerun stopped before the screenshot phase. Its
  sidebar lifecycle assertion expected exactly one `sidebar-card` render for
  the entire journey, even though the journey had intentionally reprojected
  configuration before the title-only check. This was a test-boundary bug, not
  a product failure.
- The assertion now snapshots the render count immediately before the
  metadata-only title transition and proves that transition does not rebuild
  the row. This preserves the lifecycle contract without coupling it to prior,
  legitimate renders.
- The actor also wrote `SHA256SUMS` for transient Peek screenshots. Those
  hashes were unused by any comparison or qualification decision, duplicated
  the images themselves, and contradicted the no-parallel-ledger decision.
  The unused hash receipt has been removed; the actor still performs direct
  pixel-difference assertions on the images.
- After that repair, the journey reproducibly exposed a product layering bug:
  the full-width pane drag strip was added after the hover controls and covered
  the upper portion of their real pointer hit targets. The old coordinate now
  reached `pane-drag-zone` instead of `New Pane Below`. The overlay order is
  corrected so the controls render and receive input above the drag strip;
  the existing real-pointer journey remains the regression proof.
- The next run reached all four Peek captures but failed while masking the last
  image because the actor selected the newest card log line even when that line
  explicitly reported `unavailable=allocation-timeout`. A preceding valid
  geometry receipt still described the displayed card. Capture now selects the
  latest valid `x/y/width` receipt, while absence of any valid geometry remains
  a hard failure. This fixes evidence collection without treating transient
  allocation absence as success.
