# Ghostty GTK embedding preparatory API audit

Status: **preparatory evidence for Zentty issues #11 and #13; no upstream
decision and no product API ratification**.

This audit records the exact locally available history, the complete
downstream file/hunk delta, and the experimental C ABI at the qualified
checkpoint. It deliberately separates a **proven current need** of the C
qualification host from a **predicted product need** of the ratified Rust
application. Rust product callers now exist for lifecycle, focus, search, and
working-directory-aware surface construction; unproven operations remain
explicitly distinguished.

The machine-readable companion is
[`linux/ghostty-api-audit.json`](../linux/ghostty-api-audit.json). Its validator
is `GHOSTTY_SOURCE_DIR=/path/to/ghostty linux/tests/ghostty-api-audit`. Each
changed file records its aggregate add/delete count, exact hunk count, and the
SHA-256 of its binary Git patch. The validator recomputes those values for all
16 files and two ranges, so a changed, added, removed, or unaccounted hunk is
detectable without committing generated patches.

Every patch identity is normalized independently of operator Git config:
SHA-1 full-index lines, Myers diff, three context lines/no inter-hunk fusion,
indent heuristic, rename detection, relative paths, global attributes, and
custom ordering off, fixed `a/` and `b/` prefixes, no color/external diff/text
conversion, C locale, and SHA-256 over the resulting bytes.
`linux/tests/ghostty-api-audit
--self-test` reruns the complete validator with a conflicting
`core.abbrev=12` and requires the same result.

## Revision facts and scope

The read-only Ghostty checkout inspected for this report was clean at:

- head: `b992c688a680067fae8b51112d1833611136fb57`
- branch/ref: the locked direct-fork GTK embedding branch
- configured origin: `TamedTornado/ghostty`, whose GitHub parent/source is
  `ghostty-org/ghostty`

The direct fork was rebuilt after the earlier fork-provenance error. The
audited `origin/main`, recorded official base, and embedding-series base are
all `ac04fc276169c70d31aa6fcfc5b43fc160d6fe6e`. The branch contains 24
downstream commits after that base. The unrelated inherited smooth-scroll
series is not in this branch; it remains only on the explicitly archived
pre-refork branch and is outside this audit.

The reproducible managed checkout intentionally has only the direct fork's
`origin`; it does not retain an `upstream/*` remote-tracking namespace. The
exact official base commit is present and ancestry-checked locally, so the
absence of that convenience ref is recorded as `false` in the machine audit
rather than being mistaken for missing base evidence.

| Range | Meaning | Commits | Files | Hunks | Lines | Patch SHA-256 |
|---|---|---:|---:|---:|---:|---|
| `ac04fc276..b992c688` | GTK embedding series | 24 | 16 | 57 | +1401/-53 | `e6086309762e9589cd1ab10b7c37e8b3ff4e1ba341bc18cfe855c7e4a10669ab` |
| `ac04fc276..b992c688` | complete direct-fork downstream delta | 24 | 16 | 57 | +1401/-53 | `e6086309762e9589cd1ab10b7c37e8b3ff4e1ba341bc18cfe855c7e4a10669ab` |

## Complete downstream file/hunk ledger

The hunk count and per-file patch identity are normative in the JSON. The
table below provides the human classification of every changed file; no file
in `ac04fc276..b992c688` is omitted.

### GTK embedding partition

| File | Change | Hunks | Purpose and preparatory recommendation |
|---|---:|---:|---|
| `AGENTS.md` | +24/-0 | 1 | Zentty-only dogfood policy; downstream only. |
| `build.zig` | +141/-0 | 2 | Shared library/header install, private spike steps, and an opt-in focused embedding-options test are combined; split generic build/testing from downstream orchestration. |
| `include/ghostty/gtk.h` | +149/-0 | 1 | Experimental language-neutral ABI plus copied construction options, synchronous borrowed terminal-text reads, and logical-pixel cell metrics; retain only product-proven generic operations. |
| `pkg/gtk4-layer-shell/build.zig` | +5/-0 | 1 | Current-upstream shared-library dependency plumbing. |
| `src/apprt.zig` | +4/-1 | 1 | Select GTK runtime for a GTK library; retain only with that product. |
| `src/apprt/gtk/Surface.zig` | +1/-3 | 2 | Route through explicit surface owner; plausible generic foundation. |
| `src/apprt/gtk/class/application.zig` | +1/-1 | 1 | Current-upstream explicit application runtime plumbing. |
| `src/apprt/gtk/class/global_shortcuts.zig` | +13/-3 | 5 | Non-default application owner support for global shortcuts. |
| `src/apprt/gtk/class/surface.zig` | +152/-42 | 35 | Store/ref/unref explicit `Application`, apply per-surface child environment overrides, emit product-neutral progress and decoded desktop-notification signals, and replace default-app lookups in construction, config, input, notifications, resize, clipboard, and finalization; plausible generic foundation needing focused tests. |
| `src/build/SharedDeps.zig` | +4/-2 | 1 | GTK library native dependencies; keep with library build plumbing. |
| `src/gtk_embed_lib.version-script` | +18/-0 | 1 | Thirteen-symbol ELF allowlist/version node; the staged artifact now has an exact executable node audit. |
| `src/gtk_embed_lib.zig` | +296/-0 | 1 | Runtime and thirteen exports, including product-proven copied construction, mutex-safe plain terminal-text reads, deterministic GTK-first rejection, and scale-correct logical cell metrics; typed argv remains open. |
| `src/gtk_embed_options.zig` | +68/-0 | 1 | Pure size-versioned surface-option validation and focused tests, deliberately independent of the full GTK build graph. |
| `src/gtk_embed_spike.valgrind.supp` | +71/-0 | 1 | Private external-library suppressions; downstream test evidence only. |
| `src/gtk_embed_spike.zig` | +423/-0 | 1 | Private Zig alternate host reaching internal APIs; not proof of a public boundary. |
| `src/termio/Exec.zig` | +31/-1 | 2 | Independent generic command-lifetime fix plus unit test; review separately. |

## Current patch partition quality

The history is good chronological dogfood evidence, but it is not yet an
independently reviewable generic patch series:

- `4de99904` and `47e47ac0` are explicitly downstream policy.
- `116aa7ad`, `a67e1e7a`, `683c72ed`, and `cdf03636` grow a private Zig spike
  that reaches Ghostty internals rather than the installed C boundary.
- `6c230758` contains the plausibly generic explicit-application ownership
  change, but also edits that private spike.
- `910114ab` is a self-contained POSIX shell-command lifetime repair with a
  focused regression test and is plausibly independent of embedding.
- `d733efbf` combines library build plumbing, runtime selection, header, and
  implementation.
- `ea2b445c` is separable optional focus/text/paste surface area.
- `3ff0b36e`, `dfd8db8`, and `4c07b759` harden the ABI and add explicit surface
  closure. `958d97ec` ports that reviewed series to the current official base
  and necessarily touches three additional upstream GTK build/class files.
- `5c261e53` adds the generic binding-action bridge used by real terminal
  search. `07cfc9f3` adds a size-versioned surface-options constructor and
  copied working-directory override, driven by real closed-pane restoration.
- `9a4001c4` and `ff5703bf` expose and exercise the product-neutral progress
  signal; `a8ccbd8c` does the same for decoded desktop notifications.
  `977de1e9` is a focused five-line precondition guard that turns the
  documented GTK-first misuse from a native assertion abort into null/error.
  `b7ae0ced5` appends bounded per-surface child environment overrides plus an
  opt-in implementation test target, driven by authenticated concurrent-pane
  agent integration rather than process-global mutation.
- `b992c688` adds one generic, read-only logical-cell-metric query. It converts
  Ghostty's physical renderer cell dimensions through GTK's content scale;
  pane geometry, shortcuts, step counts, and minimum policy remain in Zentty.

A plausible future review partition—without predicting acceptance—is:

1. generic shell-command ownership fix and its unit test;
2. explicit non-default GTK application ownership plus a focused engine-owned
   regression test that does not depend on Zentty;
3. minimal shared-library build/runtime/header foundation;
4. each product-proven optional operation separately;
5. export/version policy and ABI contract tests;
6. downstream-only Zentty policy, private spike, suppressions, and product
   matrix outside any generic implementation patch.

No patch or upstream communication was prepared.

## Exported ABI inventory

The ELF version script has exactly **13 exported function symbols**, all under
`GHOSTTY_GTK_EMBED_1.0`; it hides every other implementation symbol. The
header additionally defines six **Ghostty-owned** ABI types and five enum
values:

- opaque `ghostty_gtk_embed_runtime_t`
- C enum `ghostty_gtk_embed_async_backend_t`
- `GHOSTTY_GTK_EMBED_ASYNC_DEFAULT=0`, `EPOLL=1`, `IO_URING=2`
- fixed-width `uint32_t` typedef `ghostty_gtk_embed_text_extent_t`
- `GHOSTTY_GTK_EMBED_TEXT_VIEWPORT=0`, `SCREEN=1`
- synchronous borrowed callback type `ghostty_gtk_embed_text_callback_t`
- fixed-layout logical metric pair `ghostty_gtk_embed_cell_size_t`

The enum is a **high-severity open ABI defect**, not a proven public type.
C chooses an implementation-defined enum representation, and a valid consumer
compiled with `-fshort-enums` can make this three-value type one byte while the
Zig export accepts `c_int`. The current C++ test proves only “is enum” and the
three numeric values; it does not prove size, alignment, compatible integer
type, calling convention, or generated Rust representation. The backend
constructor must not be ratified or safely bound until a fixed representation
and cross-language default/`-fshort-enums` contract test exist.

The deterministic tracked-XFAIL probe made the defect concrete on this
machine: both C17 and C++17 reported
`sizeof(ghostty_gtk_embed_async_backend_t) == 4` with default flags and `== 1`
with `-fshort-enums`. This is the proposed cell's header-representation
evidence, but it does not call the library or validate Rust and creates no
qualification PASS. Full repaired cross-language acceptance remains
NOT_IMPLEMENTED.

It also has one explicit **external public type dependency**: GTK-owned
`GtkWidget`, forward-declared as `typedef struct _GtkWidget GtkWidget;`.
`surface_new` returns `GtkWidget *`; `surface_close`, `surface_grab_focus`,
`surface_binding_action`, `surface_send_text`, `surface_read_text`, and
`surface_request_paste` and `surface_cell_size` each accept it as their surface
handle. Thus the
boundary is language-neutral C but not GTK-neutral: consumers must bind GTK's
GObject/widget type and lifetime rules. This dependency is distinct from the
six Ghostty-owned types. Constructor transfer and callback borrowing remain
explicit safe-adapter contracts rather than implicit Rust assumptions.

A non-authoritative observation of the existing built artifact found all thirteen
names as default-versioned `@@GHOSTTY_GTK_EMBED_1.0` symbols. Its SONAME was
the unversioned `libghostty-gtk-embed.so`; the static audit does not claim this
as fresh build or runtime qualification.

All operations say they must run on the GTK main thread, but this is prose,
not runtime enforcement. Runtime handles are opaque non-GObjects. A returned
surface is a `GhosttySurface` `GtkWidget`, but the public header says only
“normal GTK container ownership rules” and never declares transfer-none,
container, full, or floating ownership. The C host attaches it directly while
the API contract calls `g_object_ref_sink`; those are caller assumptions, not
proof of a stable public transfer contract. Internally the surface refs its
Ghostty application until finalization. This ambiguity blocks a sound safe
Rust constructor until it is specified and tested. The host must
destroy/finalize all surface widgets, drain GLib, remove tick/callback sources,
and only then free the runtime. A live foreign GObject is rejected by type
check, but an arbitrary dangling pointer cannot be made safe by that check.

| Export | Proven current need/caller | Predicted Rust product need | Recommendation |
|---|---|---|---|
| `ghostty_gtk_embed_runtime_new` | API misuse/signature tests only; the C host never uses it | Probably redundant because DEFAULT can be passed to the typed constructor | Remove/minimize unless another caller proves the convenience symbol. |
| `ghostty_gtk_embed_runtime_new_with_async_backend` | Safe Rust runtime plus all real product backend cells | Required capability; current enum/`c_int` representation is defective under `-fshort-enums` | Retain downstream while the separately tracked representation defect remains XFAIL. |
| `ghostty_gtk_embed_runtime_free` | Safe Rust `Drop`, repeated product teardown, and API misuse | Required by the current safe owner | Retain; the adapter holds runtime leases through final GObject disposal. |
| `ghostty_gtk_embed_runtime_tick` | Safe Rust GLib tick source and every real product journey | Required under the current implementation, but could become an internal GLib source | Retain downstream; revisit only with an engine-owned scheduling replacement. |
| `ghostty_gtk_embed_surface_new` | Retired C host and API test only | None in the Rust product | Exclude from the Rust binding; leave ABI removal to a separately reviewed Ghostty decision. |
| `ghostty_gtk_embed_surface_new_with_options` | Rust close/restore, source native-command launch, CWD, authenticated agent environment, and C ABI contract | Product-proven command/CWD/environment today; typed argv is not source-required | Retain the size-versioned generic constructor; do not add unproven fields. |
| `ghostty_gtk_embed_surface_close` | Rust product pane and repeated lifecycle teardown | Required by the current safe wrapper unless closure becomes a documented GObject disposal contract | Retain current capability; prove callback/drop ordering before upstream proposal. |
| `ghostty_gtk_embed_surface_grab_focus` | Real product pane focus plus physical-key paths and misuse test | Required active-pane behavior under #5 | Retain with the safe runtime lease. |
| `ghostty_gtk_embed_surface_binding_action` | Rust product pane-search actions plus real X11/Wayland scrollback scenarios | Likely generic terminal-owned action bridge | Retain as one generic parser/dispatcher; keep shortcuts and product policy in Zentty. |
| `ghostty_gtk_embed_surface_cell_size` | Rust product cell-based drag minimums and physical X11 pane-resize shortcuts | Required while Ghostty owns rendered cell metrics and Zentty owns pane geometry | Retain the scale-correct generic getter; keep all resize policy in Zentty. |
| `ghostty_gtk_embed_surface_send_text` | Real restore prefill and tmux send-keys product paths plus misuse test | Required programmatic text action | Retain through the safe wrapper. |
| `ghostty_gtk_embed_surface_read_text` | Real staged `capture-pane` reads the displayed Ghostty surface and scrollback under X11 and Wayland | Generic synchronous plain-text boundary | Retain the borrowed callback; keep line limits, tmux output, and buffers in Zentty. |
| `ghostty_gtk_embed_surface_request_paste` | C qualification clipboard relay and misuse test only | None; product paste uses ordinary input/binding behavior | Exclude from the Rust binding; leave ABI removal to separate Ghostty review. |

### Per-operation contract consequences

The JSON records each operation separately. Cross-cutting findings are:

- **Construction/configuration:** the runtime constructor selects only the
  async backend. `surface_new_with_options` copies nullable command/title/CWD
  plus at most 128 `KEY=VALUE` child environment overrides, so real restored
  PTYs no longer require shell interpolation for directory selection and each
  pane can inherit its own capability-scoped agent endpoint. Typed argv,
  approved broader Ghostty configuration,
  structured error, and resource location remain open.
- **Initialization order:** successful runtime construction must happen on the
  GTK main thread before `gtk_init()` and before constructing **any** GTK
  object because Ghostty owns signal and GTK setup order. Current successful
  callers respect this prerequisite, but no fresh-process misuse test reverses
  the order and proves a safe actionable failure.
- **Async enum representation:** the public C enum and Zig `c_int` boundary are
  not representation-compatible under all allowed consumer flags. Numeric
  values 0/1/2 and current successful calls are not ABI proof.
- **One-runtime lifecycle:** creation is process-global and irreversible after
  a successful constructor, even after free. Null/foreign/double/stale runtime
  operations are rejected, but no thread or reentrancy checks exist.
- **Ticking:** the public poll call exposes mailbox scheduling and undocumented
  cadence/reentrancy. Making the GTK implementation own a GLib source might
  reduce API and teardown burden, but this is a question, not a conclusion.
- **Surface transfer:** the returned widget's transfer is ambiguous. “Normal
  GTK container ownership” does not say whether this constructor returns a
  floating, full, container, or borrowed reference. Current `gtk_grid_attach`
  and `g_object_ref_sink` usage cannot substitute for a declared #13 binding
  invariant and positive/drop-order tests.
- **Callbacks:** the C ABI exports no callback-registration symbols. The C host
  directly discovers `GhosttySurface::init`, `clipboard-write`,
  `clipboard-read`, `notify::title`, and `notify::child-exited` by string, and
  observes GTK allocation/focus. These names and signatures are an implicit
  ABI not described by `gtk.h`. The C host owns userdata lifetime and signal
  disconnection; callback-after-drop is untested.
- **Focus/text/paste:** each borrows a live surface. Focus is void and silently
  ignores invalid live GObjects. Text is NUL-terminated UTF-8 and reports only
  boolean success. Paste is standard-clipboard-only and completes
  asynchronously by signal, with no cancellation contract.
- **Symbol/version policy:** `linux/tests/abi-surface` validates the nine
  names after stripping `@...`, so it does not assert the version node. There
  is no ABI macro/query symbol, versioned SONAME, tested evolution policy, or
  deliberate old/new mismatch failure. Non-ELF GTK targets are unqualified.

## Existing evidence and what it proves

At the pinned checkpoint, the authoritative matrix records:

- positive construction/tick/surface/teardown across Wayland and X11,
  ReleaseSafe and Debug, default/epoll/io_uring, and single/four surfaces;
- real PTY command/output/title/child-exit behavior;
- focus, text injection, standard clipboard paste, allocation/resize, staged
  relocation, binary hardening, and repeated lifecycle;
- C17 compilation and partial C++17 signature/enum compilation;
- an exact nine-name exported-symbol allowlist;
- API misuse rejection for null/foreign runtime and surface handles, invalid
  backend, second runtime, stale runtime, double free, null text, and
  uninitialized surface text/paste;
- Debug raw plus suppression-enabled candidate Valgrind evidence and unchanged
  full Ghostty Debug tests; suppression review remains a separate matrix gate.

This historical evidence proved the retired C qualification host at the pinned
revision. The current matrix additionally owns the safe Rust/product evidence.
The private Zig spike supplies useful implementation history but is not an
independent public-ABI consumer. The staged-product ReleaseSafe Valgrind rows
remain unimplemented, and the matrix keeps its compositor/IME/scaling/public-CI
gaps explicit.

## Missing proof before a final #11 decision

1. **Ghostty ABI review:** three exports excluded from the product-owned Rust
   binding remain in Ghostty's downstream C ABI pending separate compatibility
   and maintainer review.
2. **Construction:** the source-compatible native command, CWD, and environment
   are proven; typed argv remains intentionally absent until product behavior
   requires it. Fresh-process runtime-after-GTK rejection is now proven under
   controlled X11 and Wayland; structured native constructor diagnostics
   beyond the current error log and null return remain open.
3. **Thread/ownership:** Rust types are main-thread-only and real teardown now
   rejects callbacks after disposal. Off-main-thread native misuse, reentrant
   tick, stale foreign pointers, and Rust-unwind behavior still need any final
   public-ABI decision to define them.
4. **Callbacks:** product routing and after-dispose exclusion are proven, but
   the GObject signal/property contract still lacks a stable public declaration
   or userdata destroy-notification API.
5. **Operations:** all ten retained Rust declarations now have safe owners,
   product callers, and real journeys. The default constructor, legacy surface
   constructor, and `request_paste` intentionally remain excluded from the
   Rust binding because the product does not call them; deciding whether to
   remove those exports from Ghostty's C ABI remains open.
6. **Errors/misuse:** no fault injection for allocation, Ghostty/GTK init,
   unavailable backends, core tick failure, clipboard denial, or invalid UTF-8;
   arbitrary dangling C pointers remain outside the possible guarantee.
7. **ABI compatibility/evolution:** the async C enum versus Zig `c_int` is a
   high-severity open representation defect, with no repaired C/C++/Rust
   real-library proof across default and `-fshort-enums` modes. There is also
   an exact version-node assertion but no compile-time/runtime ABI identity,
   versioned SONAME, deprecation policy, non-ELF visibility policy, or general
   compatibility policy. The exact historical mismatch fixture now proves
   pre-main loader failure for one incompatible pair. C++ signature assertions
   cover only two of ten functions.
8. **Future rebase:** this audit is exact for official base `ac04fc276` and
   locked downstream head `b992c688`; later official movement requires a new
   normalized audit rather than silently reusing these identities.
9. **Qualification gaps:** ReleaseSafe Valgrind, controlled native Wayland
   input/IME/resize/scaling, X11 IME/scaling, packaging, and public CI remain
   non-PASS exactly as the unchanged matrix declares.

## Matrix integration reconciliation

This preparatory audit originally proposed the following IDs without editing
the matrix. They have since been reconciled into the authoritative
`linux/qualification-matrix.json` and `linux/test-policy/traceability.json`:

| Authoritative ID | Requirement / test ID | Current declared state | Purpose |
|---|---|---|---|
| `ghostty-api-audit-inventory` | `ZL-11-GHOSTTY-API-AUDIT` / `TEST-GHOSTTY-API-AUDIT` | PASS after validator/self-test | Static normalized commit/file/hunk/source allowlist identity only. |
| `ghostty-abi-version-node` | `ZL-11-GHOSTTY-ABI-COMPAT` / `TEST-GHOSTTY-ABI-VERSION` | PASS | All thirteen ELF exports carry exactly `GHOSTTY_GTK_EMBED_1.0`; malformed-evidence self-tests reject missing, wrong, and untracked symbols. |
| `ghostty-async-backend-abi-representation` | `ZL-11-GHOSTTY-ABI-COMPAT` / `TEST-GHOSTTY-ASYNC-BACKEND-ABI` | XFAIL, `DOGFOOD-2026-08-02-GHOSTTY-ASYNC-ENUM-ABI` | The deterministic C17/C++17 header probe exits 99 after reproducing 4-byte default versus 1-byte `-fshort-enums` representation. Full repaired C/C++/Rust size, alignment, and real-library call acceptance remains NOT_IMPLEMENTED. |
| `ghostty-runtime-initialization-order` | `ZL-11-GHOSTTY-API-AUDIT` / `TEST-GHOSTTY-RUNTIME-INIT-ORDER` | PASS | Fresh processes under controlled X11 and Wayland prove runtime-first succeeds and GTK-first rejects without aborting or corrupting subsequent GTK object construction. |
| `rust-ghostty-api-product-usage` | `ZL-13-RUST-GHOSTTY-ADAPTER` / `TEST-RUST-GHOSTTY-PRODUCT-USAGE` | PASS | Closed-world nine-operation Rust ledger plus real close/restore, binding-action, and text-read journeys. |
| `rust-ghostty-callback-drop-order` | `ZL-13-RUST-GHOSTTY-ADAPTER` / `TEST-RUST-GHOSTTY-CALLBACK-DROP` | PASS | Physical pane disposal rejects init/title/progress/child-exit callbacks after the close boundary. |
| `rust-ghostty-config-construction` | `ZL-13-RUST-GHOSTTY-ADAPTER` / `TEST-RUST-GHOSTTY-CONFIG` | PASS | Prove exact source-native command, title, CWD, environment, and invalid-boundary encoding. |
| `ghostty-abi-old-new-mismatch` | `ZL-11-GHOSTTY-ABI-COMPAT` / `TEST-GHOSTTY-ABI-MISMATCH` | PASS | A current-header consumer passes with the current library, a historical-header consumer passes forward against current, and the untouched current consumer fails in the loader before `main` with the real historical library. |

The machine audit retains the field name `proposed_matrix_cells` because it is
a byte-identified historical proposal snapshot. Its forward-looking
`integration_owner` text is not current authority. The matrix and traceability
registry now own IDs, statuses, commands, test ownership, and later changes.

## Audit commands

The following commands were used without modifying the Ghostty checkout. This
public rendering normalizes operator-specific locations; set
`GHOSTTY_SOURCE_DIR` to the exact pinned checkout:

```sh
git -C "$GHOSTTY_SOURCE_DIR" status --short --branch
git -C "$GHOSTTY_SOURCE_DIR" show-ref
git -C "$GHOSTTY_SOURCE_DIR" merge-base HEAD origin/main
git -C "$GHOSTTY_SOURCE_DIR" log --reverse --oneline ac04fc276..HEAD
# The validator owns the full normalized diff command. In abbreviated form:
git -C "$GHOSTTY_SOURCE_DIR" \
  -c core.abbrev=40 -c core.attributesFile=/dev/null -c core.quotePath=true \
  -c color.ui=false -c diff.algorithm=myers -c diff.indentHeuristic=false \
  -c diff.renames=false -c diff.relative=false -c diff.orderFile=/dev/null \
  diff --no-ext-diff --no-textconv --full-index --binary --no-color \
  --no-renames --no-indent-heuristic --diff-algorithm=myers \
  --unified=3 --inter-hunk-context=0 --src-prefix=a/ --dst-prefix=b/ \
  <range> | LC_ALL=C sha256sum
rg -n 'ghostty_gtk_embed_' linux/src linux/tests
nm -D --defined-only --with-symbol-versions \
  "$GHOSTTY_SOURCE_DIR/zig-out/lib/libghostty-gtk-embed.so"
readelf -d "$GHOSTTY_SOURCE_DIR/zig-out/lib/libghostty-gtk-embed.so"
GHOSTTY_SOURCE_DIR="$GHOSTTY_SOURCE_DIR" \
  linux/tests/ghostty-api-audit --self-test
GHOSTTY_SOURCE_DIR="$GHOSTTY_SOURCE_DIR" \
  linux/tests/ghostty-async-backend-abi # tracked XFAIL: exit 99; prerequisite: 77
```

The final command returned:

```text
Ghostty API audit inventory passed: 16 files, 52 hunks, 12 allowlisted function exports, 5 Ghostty-owned public types, 1 external GtkWidget dependency
Ghostty API audit normalization self-test passed: conflicting core.abbrev=12 did not change diff identities
```

The final representation probe returned the tracked XFAIL result:

```text
async-backend-abi: language=C17 enum_size=4 c_int_size=4 values=0,1,2
async-backend-abi: language=C17 enum_size=1 c_int_size=4 values=0,1,2
async-backend-abi: language=C++17 enum_size=4 c_int_size=4 values=0,1,2
async-backend-abi: language=C++17 enum_size=1 c_int_size=4 values=0,1,2
ghostty-async-backend-abi: XFAIL public enum representation differs from c_int (expected tracked defect exit 99)
```

Its exit status was 99. The probe compiles the installed header only; it does
not call the library or prove raw Rust representation. The repaired
C17/C++17/Rust acceptance proof therefore remains NOT_IMPLEMENTED.

Additional contract/regression results are recorded contemporaneously in the
canonical dogfood report rather than being inferred from this static audit.

## Static inventory result and non-authoritative runtime observations

The new audit **PASS claims static inventory identity only**. Runtime and build
commands below were useful local observations, but this audit did not retain a
compiler/linker environment manifest, artifact checksum, or fresh public
receipt bundle for them. They therefore create no independent qualification
claim:

- build/header/link observations remain governed by canonical `build-release`;
- the six API combinations remain governed by
  `release-api-{wayland,x11}-{default,epoll,io-uring}`;
- the Debug suite remains governed by `ghostty-regression`;
- the matrix validation is governed by `matrix-runner-self-test`.

The versioned-symbol observation motivates the proposed
`ghostty-abi-version-node`; the executable ELF audit now supplies that separate
evidence.

All commands below used the read-only source tree at the audited head. The
Ghostty rebuild/test cache and prefix were redirected to `/tmp`; the existing
dependency cache was copied there first so the run did not download anything
or write into the checkout.

```sh
bash -n linux/tests/ghostty-api-audit
bash -n linux/tests/ghostty-async-backend-abi
jq empty linux/ghostty-api-audit.json
GHOSTTY_SOURCE_DIR="$GHOSTTY_SOURCE_DIR" \
  linux/tests/ghostty-api-audit --self-test
GHOSTTY_SOURCE_DIR="$GHOSTTY_SOURCE_DIR" \
  linux/tests/ghostty-async-backend-abi # tracked exit 99; prerequisite exit 77
linux/tests/qualification-matrix --validate-only
linux/tests/qualification-matrix-test

cc -std=c17 -Wall -Wextra -Werror -Wconversion -Wshadow -Wformat=2 \
  -Wstrict-prototypes -Wmissing-prototypes -fanalyzer -fsyntax-only \
  -I"$GHOSTTY_SOURCE_DIR/include" linux/tests/api-contract.c \
  $(pkg-config --cflags gtk4)
c++ -std=c++17 -Wall -Wextra -Werror -Wpedantic -fsyntax-only \
  -I"$GHOSTTY_SOURCE_DIR/include" linux/tests/api-header-cpp.cc

# The same API contract was linked into /tmp against the existing pinned
# ReleaseSafe library, then run as a separate process for each backend value:
GHOSTTY_RESOURCES_DIR="$GHOSTTY_SOURCE_DIR/zig-out/share/ghostty" \
  GDK_BACKEND=wayland ZENTTY_API_CONTRACT_ASYNC_BACKEND=<default|epoll|io_uring> \
  /tmp/zentty-ghostty-api-audit-contract
GHOSTTY_RESOURCES_DIR="$GHOSTTY_SOURCE_DIR/zig-out/share/ghostty" \
  GDK_BACKEND=x11 ZENTTY_API_CONTRACT_ASYNC_BACKEND=<default|epoll|io_uring> \
  /tmp/zentty-ghostty-api-audit-contract

nm -D --defined-only --with-symbol-versions \
  "$GHOSTTY_SOURCE_DIR/zig-out/lib/libghostty-gtk-embed.so"
readelf -d \
  "$GHOSTTY_SOURCE_DIR/zig-out/lib/libghostty-gtk-embed.so"

mkdir -p /tmp/zentty-ghostty-api-audit-zig-global-cache
cp -a "${ZIG_GLOBAL_CACHE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/zig}/." \
  /tmp/zentty-ghostty-api-audit-zig-global-cache/
PATH="$(dirname "$BLUEPRINT_COMPILER"):$(dirname "$ZIG"):$PATH" \
  "$ZIG" build test \
  -Doptimize=Debug -Dcpu=baseline -Demit-macos-app=false \
  -fno-sys=gtk4-layer-shell \
  --cache-dir /tmp/zentty-ghostty-api-audit-zig-cache \
  --global-cache-dir /tmp/zentty-ghostty-api-audit-zig-global-cache \
  --prefix /tmp/zentty-ghostty-api-audit-zig-prefix --summary all
```

Non-authoritative local observations, except for the static validator itself:

- audit validator: PASS, 16 files / 52 hunks / 12 allowlisted exports / 5
  Ghostty-owned public types / 1 external `GtkWidget` dependency;
- qualification schema and negative runner self-tests: PASS;
- C17 and C++17 warning-as-error syntax checks: PASS;
- public C API contract: all six Wayland/X11 by default/epoll/io_uring
  combinations were observed to pass against the existing pinned ReleaseSafe
  artifact, but no new authoritative receipt was retained;
- built ABI observation: exactly 12 `T`/`W` exports, each
  `@@GHOSTTY_GTK_EMBED_1.0`; SONAME `libghostty-gtk-embed.so`;
- full Ghostty Debug observation: 94/94 build steps succeeded, 2707/2738 tests
  passed, 31 skipped, zero failures; the canonical claim remains the matrix's
  `ghostty-regression` cell;
- final Ghostty status remained clean at the audited head.

The first sandboxed Wayland API-contract attempt could not open the display.
It was an execution-permission/environment failure, not a product result. The
same already-built temporary binary passed all six display/backend contract
combinations when permitted to access the local display sockets. The full
Zentty display/Valgrind matrix was not rerun for this read-only preparatory
audit. A later canonical full rerun, including the integrated audit cells and
its remaining non-PASS suppression-governance result, is recorded in the
[dogfood report](design/zentty-linux-dogfood-2026-08-01.md#2026-08-02-canonical-cross-stream-integration-and-adversarial-qa-checkpoint).
