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
