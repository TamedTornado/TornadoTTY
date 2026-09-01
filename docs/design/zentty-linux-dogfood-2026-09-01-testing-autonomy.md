# Linux testing-autonomy dogfood report — 2026-09-01

This report tracks the work initiated by the external critique of TornadoTTY's
Linux test architecture. It is an active report: a green focused check is not
an assertion that the full suite, release qualification, or autonomy gate has
passed.

## Baseline discoveries

- `zentty-linux` contains 365 inline Rust test functions, while
  `crates/zentty-linux/tests` contains one integration-test source file.
  `zentty-core/tests` contains 26 integration-test source files. The critique
  that the GTK-facing middle layer is thin is therefore valid.
- The pane-divider test checks formatter-produced names and labels directly.
  Those assertions can detect an accidental string change but do not establish
  divider topology or resize semantics. GH-147 owns its replacement with a
  semantic integration cohort.
- Focused mutation receipts exist and several repaired cohorts report zero
  missed mutants, but there is no current-source, scope-aware mutation policy
  whose result gates unattended maintenance. Mutation strength is therefore
  not established for the unit layer as a whole. GH-146 owns that gate.
- The repository contains 45 test/support files over 300 lines. GUI journeys
  use many literal product-log assertions. The receipt environment variable
  `ZENTTY_TEST_RECEIPTS` is limited to the sidebar-management journey, but the
  broader dependence on human-oriented `zentty-linux:` log text is real.
  GH-148 owns a typed receipt contract; GH-149 owns the single Rust journey
  driver and incremental migration.
- The two ReleaseSafe Valgrind XFAIL cells were not missing explanations in the
  current matrix. Both already named tracking record
  `DOGFOOD-2026-08-02-RELEASESAFE-VALGRIND` and described the unsuppressed
  findings. The real defect was weaker: the validator accepted whitespace-only
  metadata, and BLOCKED did not require a tracking identifier.

## Planning record

- GH-145 is the testing-autonomy epic.
- GH-150: deferred-status explanation governance.
- GH-147: semantic GTK-adjacent middle layer, beginning with pane dividers.
- GH-146: current-source mutation autonomy gate.
- GH-148: typed product receipt boundary.
- GH-149: Rust GUI journey driver and staged Bash migration.

The implementation order is deliberate: first make the matrix honest, then
construct one semantic cohort, use that cohort to prove the mutation gate, and
only then replace the journey protocol and driver. This avoids blessing weak
tests with a mutation badge or creating a second journey system without a
migration path.

## GH-150 repair

### Failure

Matrix validation used string length to validate `tracking`, `defect`, and
`prerequisite`. A value containing only spaces or tabs therefore passed.
XFAIL required tracking, but BLOCKED did not.

### Repair

- BLOCKED and XFAIL now require a non-whitespace tracking ID.
- Each also requires a non-whitespace defect or prerequisite.
- FAIL and NOT_IMPLEMENTED defects receive the same whitespace-safe check.
- The JSON schema now rejects whitespace-only policy strings.
- A focused `qualification-gap` helper is the single formatter for both cell
  result metadata and the concise human report. Deferred evidence is preserved
  under `qualification_gap` in the machine result and printed as a `Deferred:`
  line in the human report.

### Regression evidence

The focused runner tests now reject missing, empty, and whitespace-only
tracking and reasons for each real ReleaseSafe Valgrind XFAIL cell. A synthetic
BLOCKED cell proves both rejection and the valid prerequisite form. The helper
test checks exact JSON and human renderings without running the product matrix.

Commands run after the repair:

```text
linux/tests/lib/qualification-gap-test
  Qualification gap focused tests passed

linux/tests/qualification-matrix-test
  Qualification matrix focused tests passed

linux/tests/qualification-matrix --validate-only
  Qualification matrix schema and coverage passed
```

No qualification cell was executed and no full-qualification claim is made.
The ReleaseSafe Valgrind cells remain XFAIL; no suppression was added or
broadened.

## Remaining uncertainty

- No current mutation kill-rate yet gates autonomous work.
- The pane-divider assertions have not yet been replaced by semantic tests.
- Product receipts remain fragmented across log strings.
- Large Bash journeys remain in production until the Rust driver proves real
  equivalence and individual migrations remove their predecessors.

## GH-147 pane-divider semantic cohort

### Discovery

The original `pane_dividers.rs` mixed four responsibilities: topology identity,
axis selection, physical-event interpretation, and GTK widget construction.
Its only local test asserted the strings produced by `name()` and `label()`.
The actual bounded resize lived behind callbacks and was not represented at the
public `zentty-linux` crate boundary.

### Repair

- Added a display-independent `pane_divider_model` public module. It owns the
  typed column/pane boundary, axis, axis-specific pointer coordinate, physical
  key eligibility and sign, nonzero resize request, callback payload, and
  bounded visible-margin calculation.
- Kept GTK as an adapter: GDK keys are translated once, and both drag and key
  paths now submit the same typed request to the existing workspace callback.
  No second resize or topology state was added.
- Removed the formatter-only inline test and added
  `tests/pane_divider_semantics.rs`, which crosses the crate boundary without a
  display. Six tests cover horizontal/vertical exclusivity, both keyboard
  directions, delimiter-like and deliberately confusable IDs, signed callback
  payloads, callback suppression at zero, rounding, and both integer bounds.

### Failures and evidence

- The first mutation invocation stopped at its unmutated baseline because the
  safe `gitignore=true` policy correctly omitted the ignored multi-gigabyte
  `build/` tree, including Ghostty's native library. The rerun supplied the
  existing library through an absolute `GHOSTTY_LIB_DIR`; no ignored tree was
  copied and no isolation rule was weakened.
- Final scoped mutation result: **24 generated, 21 caught, 3 compiler-unviable,
  0 missed, 0 timed out**. The unviable replacements required `Default` for
  semantic enums/types that intentionally have no meaningless default. All
  viable mutations—including sign deletion, callback replacement, zero-bound
  inversion, coordinate replacement, and bound arithmetic replacement—were
  killed.
- `cargo test -p zentty-linux --test pane_divider_semantics`: 6 passed.
- `cargo check -p zentty-linux --bin zentty-linux`: passed, proving the GTK
  adapter consumes the new model.
- Focused `cargo clippy --no-deps ... -D warnings`: passed. The first clippy
  invocation without `--no-deps` exposed two pre-existing `zentty-core`
  pedantic failures (`assigning_clones` and `too_many_lines`); they were not
  changed or hidden by this slice.

### Mutation operators cargo-mutants could not synthesize

The generated axis replacement was compiler-unviable because `DividerAxis`
intentionally has no arbitrary `Default`; cargo-mutants also generated no
same-typed topology-field swap. Each operator was therefore applied manually
and reverted with a reviewable patch:

- Swapping horizontal and vertical axis results failed three semantic tests,
  including both axis-specific cohorts.
- Swapping the pane request's `column_id` and `after_pane_id` failed
  `callback_receives_typed_unswapped_target_and_signed_delta`.
- After reverting both mutants, all six tests passed again and the model had no
  source diff from the committed implementation.

This is evidence that the assertions kill those named operators; it does not
misreport compiler-unviable generated mutants as caught.

### Inline-test classification audit

All 364 remaining inline tests are now classified by primary intent in
`zentty-linux-inline-test-audit-2026-09-01.md`: 331 behavioral, 29
contract/snapshot, zero remaining confirmed formatter-mirroring tests, and four
GTK widget-smoke tests. The zero is not a kill-rate claim: exact presentation
contracts remain legitimate snapshots, and GH-146 must still mutate them.

A focused audit check extracts the real inline test identities from Rust
source, validates every explicitly named contract/smoke test, rejects duplicate
classification, and pins the category arithmetic. It passed directly and as
part of `qualification-matrix-test`. The audit prioritizes source-text parity
snapshots, the six largest inline cohorts, widget-smoke migration boundaries,
and presentation-contract mutation.
