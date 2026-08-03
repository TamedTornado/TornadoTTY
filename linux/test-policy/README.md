# Linux test policy

This directory contains only machine-readable contracts that directly protect
a Linux product or dependency-boundary claim.

The normative execution plan is
[`../../docs/design/linux-rust-port-recovery-plan.md`](../../docs/design/linux-rust-port-recovery-plan.md).

## Retained contracts

- `qualification-matrix.schema.json` describes the authoritative cell
  inventory in `../qualification-matrix.json`.
- `environment-manifest.schema.json` describes receipts emitted by controlled
  Wayland, X11, and isolated-session wrappers.
- `qualification-host-freeze.json` freezes the disposable C qualification host
  during the short Rust replacement overlap. It is deleted with that host.
- `../tests/valgrind-suppressions.json` and its adjacent schema govern the
  effective Valgrind suppression set.

## Test layers

1. Focused support tests validate matrix classification, controlled display
   wrappers, ABI probes, and suppression governance. They run once before the
   matrix and are not matrix cells.
2. Matrix cells exercise real product or narrowly named dependency-boundary
   behavior. They never invoke aggregate qualification commands or support
   self-tests.
3. A real product cell must launch `build/linux/bin/zentty-linux`, the same Rust
   artifact that will be staged and packaged.

The matrix records environmental gaps as BLOCKED, XFAIL, or NOT_IMPLEMENTED.
Exit 77 is an unexpected skip and cannot pass qualification.

## Intentionally retired machinery

The pre-product phase created mutation manifests for Bash governance, nested
aggregate runner tests, source-snapshot attestations, archive attack fixtures,
review-record schemas, and retirement-record schemas. Those artifacts were
removed because they tested the evidence framework rather than the delivered
application and caused recursive multi-hour qualification.

Mutation testing returns after the Rust vertical slice is green and only for
focused Rust state, ownership, callback, persistence, and rollback logic.

## Local entry points

Fast validation:

```sh
linux/tests/qualification-matrix-test
linux/tests/qualification-matrix --validate-only
```

Complete local support gate followed by one matrix execution:

```sh
linux/tests/qualify-local
```

No aggregate command may invoke either of those aggregate entry points.
