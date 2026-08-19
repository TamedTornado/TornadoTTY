# Zentty Linux Rust port recovery plan

- **Status:** Ratified execution plan
- **Date:** 2026-08-03
- **Owner:** Zentty Linux port
- **Product decision:** Rust + `gtk4-rs`, using Ghostty through its
  language-neutral C ABI
- **Supersedes as an execution plan:** continuing to expand the transitional C
  qualification host or its recursive qualification framework
- **Related architecture decision:**
  [`../architecture/0001-rust-gtk4-linux-product.md`](../architecture/0001-rust-gtk4-linux-product.md)
- **Field record:**
  [`zentty-linux-dogfood-2026-08-01.md`](zentty-linux-dogfood-2026-08-01.md)

## 1. Purpose

Deliver a real Linux Zentty application without allowing the C embedding spike
or its test framework to become the product.

The previous phase proved that the pinned Ghostty fork can be embedded through
a C ABI in real GTK, PTY, X11 and Wayland processes. It also over-invested in
mechanically qualifying the qualification machinery. This plan keeps the real
system coverage and removes recursive, slow, product-irrelevant governance.

The standard is:

> Product claims come from tests that launch the delivered Rust executable
> against real Ghostty, GTK, PTYs and controlled display services. Focused unit
> and contract tests support those integrations; they never substitute for
> them.

## 2. Starting truth

The current 121-cell inventory contains 65 executable cells and 56 declared
gaps. The executable cells substantially qualify the transitional C host:

| Current category | Cells | Present value |
| --- | ---: | --- |
| Terminal behavior | 24 | Real Ghostty/GTK/PTY/display coverage, but through the C host |
| API misuse | 6 | Real raw Ghostty C-ABI behavior |
| Lifecycle | 9 | Real child, PTY, focus, input/output and teardown behavior, but through the C host |
| Build and staged host | 5 | Real artifacts, but not a Zentty product package |
| Valgrind | 8 | Real processes and useful defects; two ReleaseSafe XFAILs |
| Suppression governance | 3 | Required raw/suppressed evidence discipline |
| X11 physical input and resize | 2 | Real Xvfb/X11 protocol execution |
| Governance and supporting contracts | 8 | Runner, mutation, schemas, source freeze and audits; not product behavior |

There is no Rust Linux product yet. No current cell qualifies a delivered
Zentty binary. The failed final run also found a real, unsuppressed 24-byte
definite leak on both Wayland and X11 through Ghostty's
`SurfaceChildExitedBanner`/GTK mnemonic-label lifecycle. That finding remains a
failure until repaired or independently classified; it must not be hidden by a
new suppression.

## 3. Non-negotiable decisions

1. **The shipped Linux product is Rust.** No product feature is added to the C
   host.
2. **The C application spike is disposable.** It is removed after a short,
   explicit Rust replacement overlap.
3. **Small C ABI probes remain.** C is the correct language for checking the
   language-neutral header, symbol, representation and raw misuse boundary that
   Rust consumes.
4. **No Rust-specific API is added to Ghostty.** Zentty owns
   `zentty-ghostty-sys` and the safe `zentty-ghostty` adapter.
5. **The real product is the integration-test subject.** Tests launch the same
   Rust artifact that will be packaged.
6. **Controlled services are real services.** Local Wayland uses real nested
   Weston; local X11 uses real Xvfb. GTK, Ghostty, PTYs, child processes,
   Valgrind, IBus and external input drivers are not replaced with application
   fakes in product integration tests.
7. **Environmental absence never passes.** Missing prerequisites remain a
   failure or an explicitly declared BLOCKED cell.
8. **ReleaseSafe Valgrind remains XFAIL** until its tracked real defect is
   repaired. Suppressions are not broadened to manufacture green results.
9. **No recursive qualification.** A qualification command, its self-test, or
   its mutation campaign may not invoke the aggregate qualification command.
10. **No more infrastructure without product demand.** A new harness feature,
    schema or evidence layer requires a failing real-product test that cannot
    be expressed using the existing controlled services and logs.

## 4. Final test architecture

### 4.1 Fast support tests

Purpose: prove pure state, FFI declarations, ownership adapters and small
qualification utilities.

- `cargo test --workspace` owns pure Rust state, serialization, safe-adapter
  ownership and failure injection.
- Small C/C++ probes own Ghostty header/link/symbol/representation contracts.
- Focused runner tests own status classification and unexpected-skip behavior.
- Deterministic fixtures are permitted here, but these tests are labeled
  `unit` or `contract`; they do not qualify the product.
- No fast test launches a full qualification matrix.

### 4.2 Real product integration tests

Purpose: prove behavior through the delivered `zentty-linux` Rust executable.

Every product integration test uses, as applicable:

- the real pinned Ghostty library;
- the real safe Rust adapter and Rust composition root;
- real GTK4/GDK/GLib;
- a real PTY and deterministic child program;
- real nested Weston/Wayland or Xvfb/X11;
- real external input/resize drivers;
- real filesystem/XDG layouts; and
- exact semantic acknowledgement from the application and PTY.

A controlled deterministic child is test data, not a terminal-engine fake. It
must expose exact input, output, resize, exit and teardown acknowledgements.

### 4.3 Real memory tests

- Run the same delivered Rust executable and safe adapter under Valgrind.
- Preserve raw and suppression-enabled receipts for every run.
- Retain reviewed suppression manifests, ceilings and independent reproducers.
- Describe accepted Debug results only as **PASS with reviewed suppressions**.
- Keep the minimal C or GTK reproducer only when it distinguishes an upstream
  library defect from Zentty/Ghostty ownership.

### 4.4 Qualification inventory

`linux/qualification-matrix.json` remains the single inventory of product and
dependency-boundary claims, including explicit BLOCKED, XFAIL and
NOT_IMPLEMENTED entries. It no longer treats test-framework self-tests as
product qualification cells.

The matrix may contain:

- real Rust product behavior;
- real installed-artifact behavior;
- real Ghostty dependency-boundary probes; and
- explicit future product gaps.

The matrix must not contain commands for:

- its own aggregate runner self-test;
- the aggregate mutation campaign;
- test-architecture self-validation;
- qualification-host freeze or retirement bureaucracy;
- archive self-tests; or
- any other test whose primary subject is the testing framework.

Support tests run once before product qualification. A small orchestrator may
stop on their failure, run the product matrix once, and write one concise JSON
summary. It must not recursively re-execute either stage.

### 4.5 Evidence proportionality

Retain:

- exact command and exit classification;
- source revision and dirty/clean state;
- per-cell logs and their hashes;
- controlled environment identity and cleanup outcome;
- raw/suppressed Valgrind totals and suppression review; and
- atomic publication of the final summary.

Remove unless a concrete product or ordinary CI maintenance need exists:

- recursive source-snapshot validation inside every test layer;
- a mutation campaign for the Bash evidence framework;
- multi-schema attestation and review-record machinery that does not protect a
  delivered-product claim;
- relocatable archive attack simulations as a local product gate; and
- hour-long outer evidence spans caused by nesting aggregate suites.

CI may retain short-lived logs for debugging. Artifact signing, release
approval, and hosted-evidence governance are separate future release-design
questions and must not be inferred from qualification work.

## 5. Mutation-testing policy

The current 32-mutant campaign primarily mutation-tests Bash qualification
governance. It is removed from the product matrix and retired during cleanup.

Mutation testing returns only where it protects implemented product logic:

- Rust workspace transition invariants;
- persistence and migration behavior;
- safe Ghostty adapter ownership/drop ordering;
- callback quiescence and late-callback rejection; and
- error/rollback paths that focused tests can execute deterministically.

Each mutant must invoke one focused owning test. An aggregate workspace or E2E
suite is not an owning test. Timeout, crash, compile failure and unrelated
failure remain invalid kills. Mutation testing supplements real integration;
it does not run inside the product qualification matrix.

No mutation tool is introduced before the first Rust vertical slice passes its
real Wayland and X11 integration tests.

## 6. C spike removal plan

### 6.1 Freeze immediately

Until deletion:

- `linux/src/main.c` and `linux/src/host_options.*` receive no product behavior;
- no new C-host matrix axes, packaging or evidence policy are added; and
- the host is used only to preserve a short comparison window while the first
  Rust terminal path is brought up.

### 6.2 Port valuable behavior to Rust

Replace the C-host assertions with Rust-product E2E coverage for:

1. runtime initialization before GTK;
2. one real terminal and PTY child;
3. keyboard input and exact output;
4. terminal/widget resize and PTY geometry;
5. child exit and callback delivery;
6. repeated create/close;
7. two simultaneous terminals;
8. shutdown ordering; and
9. Debug Valgrind on Wayland and X11.

### 6.3 Delete the C application path

After the Rust parity gate passes, remove or rewrite the following C-host
product path:

- `linux/src/main.c`;
- `linux/src/host_options.c` and `linux/src/host_options.h` once their useful
  option semantics are represented in Rust;
- the C-host build and staged-bundle portions of `linux/scripts/build-local`;
- `linux/tests/single-terminal`;
- `linux/tests/multi-terminal`;
- `linux/tests/interaction`;
- `linux/tests/host-contract`;
- `linux/tests/configured-command`;
- `linux/tests/repeated-lifecycle`;
- `linux/tests/staged-bundle`;
- `linux/tests/qualification-host-freeze*`;
- qualification-host retirement records and matrix cells; and
- C-host-specific traceability and packaging claims.

`linux/tests/memory-safety`, controlled input/resize drivers and their matrix
cells are rewritten to launch the Rust product rather than deleted when they
still prove required behavior.

### 6.4 Retain narrow dependency probes

Keep, simplify and clearly label as dependency contracts:

- C/C++ public-header compilation;
- exported-symbol and ABI-version inspection;
- async-backend enum representation probe until the ABI is fixed;
- minimal raw C-ABI misuse probes owned by Ghostty behavior;
- the independent non-Ghostty GTK/IBus reproducer; and
- a minimal reproducer for the current mnemonic-label leak until ownership and
  disposition are resolved.

No retained probe may open a product window or be packaged as Zentty.

## 7. Ordered execution milestones

### Milestone 0 — Stop and preserve facts

**Actions**

- Stop the failed recursive qualification run and confirm all children are
  gone.
- Record the 24-byte mnemonic-label leak and raw receipts in dogfood.
- Do not broaden suppressions.
- Do not commit a passing qualification claim.

**Exit criteria**

- No qualification/Valgrind/display process remains.
- The failure is reproducible with a focused command.
- This plan is present and treated as normative execution order.

### Milestone 1 — Remove recursive test architecture

**Tests first**

Add focused architecture assertions that fail if:

- any matrix command invokes `qualification-matrix`,
  `qualification-matrix-test`, or `qualification-mutations-test`;
- a mutation owning test names an aggregate suite;
- a product cell launches the C qualification host after Rust replacement; or
- a product claim is owned only by a unit/fixture test.

**Implementation**

- Remove governance/self-test cells from the product matrix.
- Remove the special 3600-second outer receipt allowance.
- Retire the Bash-governance mutation campaign from qualification.
- Reduce `qualify-local` to a non-recursive support gate followed by one product
  matrix execution and one summary.
- Delete/defer unused attestation/archive machinery identified in section 4.5.

**Exit criteria**

- No recursive execution path exists.
- Support tests complete within a measured short developer feedback cycle.
- The product matrix does not execute the product yet and therefore reports the
  Rust cells honestly as NOT_IMPLEMENTED, without substituting the C host.
- No new schema or generalized harness is introduced.

### Milestone 2 — Rust adapter and terminal vertical slice

**Tests first**

Create failing tests for:

- raw declaration/header parity;
- safe runtime initialization order;
- surface GObject transfer and drop order;
- one real product launch on Wayland;
- one real product launch on X11;
- exact PTY input/output;
- externally driven resize; and
- child exit and shutdown cleanup.

**Implementation**

- Create the ratified Cargo workspace and crates:
  `zentty-core`, `zentty-ghostty-sys`, `zentty-ghostty`, `zentty-linux`, and
  `zentty-test-support`.
- Implement only the Ghostty operations required by the failing slice.
- Keep all unsafe code private to named adapter modules with `// SAFETY:`
  justification.
- Launch the real Rust artifact in every E2E case.

**Exit criteria**

- The same Rust binary opens a Ghostty terminal, exchanges PTY bytes, resizes
  and shuts down under real controlled Wayland and X11.
- No test-only alternate product implementation exists.
- Focused tests and both real E2E paths pass.

### Milestone 3 — Resolve the real memory failure

**Tests first**

- Preserve a focused reproducer for the 24-byte mnemonic-label leak.
- Prove whether the leak occurs in minimal GTK/libadwaita, Ghostty lifecycle, or
  Zentty ownership.
- Add a Rust-product regression that fails on the same finding when applicable.

**Implementation**

- Make the smallest owner-appropriate repair.
- Keep Ghostty changes language-neutral and upstream-reviewable.
- Do not suppress a finding merely because it is small or external-looking.

**Exit criteria**

- Debug Valgrind passes with the already reviewed suppression set, or the cell
  remains an explicit tracked FAIL/XFAIL with a minimal upstream reproducer.
- Raw evidence remains preserved.

### Milestone 4 — Delete the C application spike

**Actions**

- Apply the deletion inventory in section 6.3.
- Rewrite retained drivers and memory tests to target the Rust product.
- Remove the C host from build, packaging, matrix and documentation.
- Retain only the narrow probes in section 6.4.

**Exit criteria**

- No shipped or staged artifact contains the C host.
- No product test launches it.
- The Rust vertical slice owns every behavior previously used as a parity gate.
- A repository search and build graph audit prove the C application path is
  unreachable.

### Milestone 5 — Expand real product coverage

Add behavior only test-first, in this order:

1. single/multi terminal across required async backends;
2. workspace/worklane/pane model and projection;
3. persistence and recovery;
4. IME and physical input on both display paths;
5. external resize and compositor scaling;
6. clipboard and Linux platform services;
7. packaging, install, upgrade and uninstall; and
8. representative desktop/hardware qualification where it tests supported
   product behavior.

Each new PASS cell must launch the delivered product or a clearly named narrow
dependency probe. Gaps remain explicit until that is true.

## 8. Time and scope controls

These controls prevent another multi-day harness detour:

1. Milestone 1 is a deletion/simplification task, not a framework rewrite.
2. No new evidence schema, archive format, mutation framework, compositor
   wrapper or generalized test DSL is allowed before Milestone 2 is green.
3. If infrastructure work exceeds one focused work session without producing a
   failing or passing Rust-product E2E result, stop and report the scope drift.
4. Two consecutive failures caused solely by the test framework trigger a stop
   and simplification review; they do not justify another policy layer.
5. Test runtime is reported by category. No aggregate test is nested inside
   another aggregate test.
6. A timeout can never count as a pass, expected failure or mutation kill.
7. Every milestone ends with diff review, focused tests, dogfood update, commit
   and push before the next milestone begins.
8. Public documentation describes exactly what was exercised. It never calls
   C-host evidence a Zentty Linux port or calls suppression-reviewed Valgrind
   evidence unsuppressed-clean.

## 9. Definition of the next substantial review artifact

The next human-reviewable artifact is not another qualification report. It is:

- a warning-clean Rust workspace;
- a real `zentty-linux` GTK window;
- one real embedded Ghostty terminal;
- exact PTY input/output and resize acknowledgements;
- controlled Wayland and X11 E2E receipts;
- a focused shutdown/lifecycle result; and
- a diff showing that no new C-host product behavior or generalized QA
  framework was added.

Until that artifact exists, the Linux port remains in implementation, not
release qualification.
