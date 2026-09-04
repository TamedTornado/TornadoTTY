# TornadoTTY product observability contract

Issue [GH-148](https://github.com/TamedTornado/TornadoTTY/issues/148) owns this
contract. Its purpose is narrow: a real TornadoTTY process can report bounded,
typed facts to integration journeys without turning wording intended for people
into an accidental test API.

## Single authority

`crates/tornadotty-test-receipts` is the only schema, encoder, parser, and safe
file writer. The `ReceiptEvent` Rust enum is the versioned definition. The
product adapter in `crates/zentty-linux/src/test_receipts.rs` constructs those
types, and `tornadotty-journey-driver` consumes them with the same parser.
There is deliberately no separately maintained JSON Schema that could drift
from the Rust definition.

The authoritative machine inventory is
`linux/test-policy/product-observability-v1.json`. It records:

- the authority paths and explicit activation variable;
- hard size/cardinality bounds and the six event families;
- representative journeys migrated in GH-148;
- every remaining machine consumer of human `zentty-linux:` prose, all tracked
  by GH-149; and
- every receipt-named environment channel, classified by its actual owner.

`linux/tests/product-observability-contract-test` fails on an undeclared
consumer or channel, a duplicate or unknown classification, a false migration
claim, an untracked partial migration, a restored literal assertion, a missing
authority, or activation outside the declared boundary.

## Format and lifecycle

The file is newline-delimited JSON. Every record contains exactly:

```json
{"schema_version":1,"sequence":1,"event":{"category":"lifecycle","state":"process_started"}}
```

Unknown versions, fields, event variants, widget/action/failure names, malformed
records, partial records presented for final validation, sequence gaps,
duplicate terminal lifecycle events, and invalid lifecycle order fail closed.
The first event must be `process_started`; clean product shutdown ends with
`process_stopped`. A running stream may validate as incomplete so a journey can
wait for intermediate evidence.

Event families are finite and typed:

- lifecycle: process start/stop and per-pane terminal ready/child exit;
- topology: bounded worklane, pane, selection, and focus identifiers;
- focus: pane or an enumerated product widget;
- geometry: window dimensions or exact pane columns;
- action completion: enumerated action, outcome, and optional target; and
- failure: enumerated code and optional target.

Identifiers accept only 1–96 ASCII alphanumeric, dash, underscore, dot, or
colon bytes. Titles, terminal contents, commands, working directories, tokens,
arbitrary errors, environment values, and other user text have no schema slot.
The stream is limited to 8 MiB, 8,192 records, and 8 KiB per record.

## Activation and filesystem safety

Journeys opt in with the explicit `TORNADOTTY_TEST_RECEIPT_FILE` environment
variable. Ordinary launches emit nothing. The target must be an absent,
absolute, normalized path inside an existing canonical directory owned by the
current user and inaccessible to group/other users. Symlink parents and targets
are rejected. The writer atomically creates a new mode-0600 file and flushes
each complete record boundary.

The Bash helper only prepares this boundary and calls the staged Rust driver;
it does not parse JSON or define a second schema. Example:

```sh
source linux/tests/lib/product-receipts
mkdir -m 0700 /tmp/tornadotty-receipts.$$
tornadotty_product_receipt_prepare \
  build/linux/bin/zentty-linux \
  /tmp/tornadotty-receipts.$$/events.ndjson
# Launch the real staged product with the exported variable.
tornadotty_product_receipt_wait \
  /tmp/tornadotty-receipts.$$/events.ndjson 10000 1 terminal-ready pane-1
tornadotty_product_receipt_validate \
  /tmp/tornadotty-receipts.$$/events.ndjson --complete
```

## What is not a product receipt

Many journeys ask a controlled shell, agent, CLI, input probe, package tool, or
display probe to write a small file proving what that external boundary saw.
Those actor/probe files remain valid integration evidence, but they do not
describe TornadoTTY internals and must not be folded into the product event
schema. The machine inventory classifies these separately.

Likewise, native protocol evidence remains authoritative for native behavior:
Wayland app IDs, X11 window properties, accessibility state, pixels, PTY output,
and filesystem/package results should be observed at their real boundary rather
than restated by a product receipt.

Human diagnostics remain useful and may change wording without a schema
version. Migrated journeys must never assert those strings. GH-149 owns the
explicit remaining legacy-consumer list and the move from large Bash journeys
to one Rust GUI journey driver; no completion claim is permitted while an
undeclared consumer silently remains.
