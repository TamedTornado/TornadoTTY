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
aggregate runner tests, standalone source-snapshot attestation systems,
archive attack fixtures, review-record schemas, and retirement-record schemas.
Those artifacts were removed because they tested the evidence framework rather
than the delivered application and caused recursive multi-hour qualification.
The final machine summary instead embeds one compact source/input provenance
envelope and verifies it at publication; it is not another execution layer.

Mutation testing returns after the Rust vertical slice is green and only for
focused Rust state, ownership, callback, persistence, and rollback logic.

## Local entry points

Fast validation:

```sh
linux/tests/qualification-matrix-test
linux/tests/lib/qualification-provenance-test
linux/tests/qualification-matrix --validate-only
```

Complete local support gate followed by one matrix execution:

```sh
linux/tests/qualify-local
```

`qualify-local` is the only complete local aggregate. Its focused support tests
are unique, top-level entries that can also be run independently; the product
matrix is executed exactly once. No support test or matrix cell may invoke an
execution aggregate. A runner may call the matrix's `--validate-only` mode
because that mode executes no cell.

`linux/ci/run-pr-subset` is an advisory CI selection, not another local or
release qualification path. It runs the reviewed PR support list and selected
matrix cells once and cannot make local, release, or full-qualification claims.

Release qualification is a deliberate `qualify-local` run against the exact
release candidate revision before its tag is published. The release workflow
only accepts an existing `v<version>` tag and packages that revision; ordinary
pushes and pull requests do not run or claim release qualification.

## Polling and retries

Aggregate runners invoke each selected support test or matrix cell once. A
failed invocation remains failed; it is never rerun into PASS. The bounded
batch helper has a focused exactly-once failure test.

Real GUI journeys may repeatedly observe a receipt while waiting for an
asynchronous compositor, GTK, process, or filesystem transition. A small
number of input-readiness probes may repeat the same harmless physical action
only while the expected transition is explicitly absent. These loops establish
or observe readiness; they do not rerun a failed journey or discard an original
failure. Other uses of retry are product behavior under test, teardown of an
owned transient resource, or package-manager download recovery. New command or
test retries require a tracked defect and must preserve the original failure in
the result.
