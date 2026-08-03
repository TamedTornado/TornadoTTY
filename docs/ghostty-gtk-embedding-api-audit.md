# Ghostty GTK embedding preparatory API audit

Status: **preparatory evidence for Zentty issues #11 and #13; no upstream
decision and no product API ratification**.

This audit records the exact locally available history, the complete
downstream file/hunk delta, and the experimental C ABI at the qualified
checkpoint. It deliberately separates a **proven current need** of the C
qualification host from a **predicted product need** of the ratified Rust
application. No Rust product caller exists yet, so no operation can presently
satisfy #11's final real-product-usage criterion.

The machine-readable companion is
[`linux/ghostty-api-audit.json`](../linux/ghostty-api-audit.json). Its validator
is `GHOSTTY_SOURCE_DIR=/path/to/ghostty linux/tests/ghostty-api-audit`. Each
changed file records its aggregate add/delete count, exact hunk count, and the
SHA-256 of its binary Git patch. The validator recomputes those values for all
32 files and three ranges, so a changed, added, removed, or unaccounted hunk is
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

- head: `5fc8fa2cf4b27bfe27072d561de98f33b2c16636`
- branch/ref: `zentty/gtk-embed`, also `origin/zentty/gtk-embed`
- configured remotes: `origin=TamedTornado/ghostty`,
  `dedene=dedene/ghostty`, and `upstream=ghostty-org/ghostty`

The locally available bases require careful wording:

1. `19e20f7664dc7a755d2d7a16ab545b2503f26caf` is the official Ghostty
   commit recorded by the existing Zentty dogfood history and is present
   locally. It is the parent/base of the eight inherited smooth-scroll
   commits.
2. `4e9fe4bb5adbd0140b0a94133bd39672076cb6de` is the immediate base of the
   11-commit GTK embedding series. It already contains the unrelated
   downstream smooth-scroll stack.
3. There is **no local `upstream/*` remote-tracking ref**, so this audit cannot
   truthfully claim a merge-base with current official `upstream/main`.
4. Local fork ref `origin/main` is
   `3706abab0c962d9c93c4c4af853149f9d55f4deb`, and
   `git merge-base HEAD origin/main` is that same commit. That ref is older
   than recorded official base `19e20f766`; using `3706abab0..HEAD` as an
   "embedding diff" would incorrectly include intervening official commits
   and dependency changes.

No fetch or rebase was performed, so the facts above are a local snapshot, not
a statement about the current upstream branch.

| Range | Meaning | Commits | Files | Hunks | Lines | Patch SHA-256 |
|---|---|---:|---:|---:|---:|---|
| `19e20f766..4e9fe4bb5` | inherited smooth/elastic-scroll stack | 8 | 20 | 105 | +747/-83 | `9d028d9d6436080952d6679385784559a5583e473149d9b862141f06a05dff0a` |
| `4e9fe4bb5..5fc8fa2cf` | GTK embedding series | 11 | 12 | 40 | +930/-35 | `6fc5aa33b3a85d76bf6cbf89d72c84a1658093c3c96a8d5aaeb0359f616744a1` |
| `19e20f766..5fc8fa2cf` | complete downstream delta from recorded official base | 19 | 32 | 145 | +1677/-118 | `94f9d7b72a8e6011dda8feba2b6d6dc402a1d6c9227e5f726e3663843c863bb1` |

## Complete downstream file/hunk ledger

The hunk count and per-file patch identity are normative in the JSON. The
table below provides the human classification of every changed file; no file
in `19e20f766..5fc8fa2cf` is omitted.

### Inherited smooth-scroll partition (not embedding)

| File | Change | Hunks | Classification |
|---|---:|---:|---|
| `include/ghostty.h` | +4/-1 | 2 | fractional/elastic scroll C API |
| `include/ghostty/vt/terminal.h` | +1/-1 | 1 | fractional scrollbar offset |
| `macos/Sources/Ghostty/Ghostty.Action.swift` | +1/-1 | 1 | Swift fractional scroll action |
| `macos/Sources/Ghostty/Ghostty.Surface.swift` | +12/-0 | 1 | Swift bridge |
| `macos/Sources/Ghostty/Surface View/SurfaceScrollView.swift` | +12/-5 | 8 | macOS scroll state/scrollbar |
| `macos/Sources/Ghostty/Surface View/SurfaceView_AppKit.swift` | +6/-14 | 6 | AppKit event forwarding |
| `src/Surface.zig` | +314/-24 | 24 | core scroll state, behavior, tests |
| `src/apprt/embedded.zig` | +22/-0 | 1 | existing embedded C API bridge |
| `src/renderer/State.zig` | +5/-0 | 1 | renderer scroll state |
| `src/renderer/generic.zig` | +90/-9 | 12 | renderer calculations |
| `src/renderer/metal/shaders.zig` | +6/-0 | 1 | Metal uniform layout |
| `src/renderer/opengl/shaders.zig` | +6/-0 | 1 | OpenGL uniform layout |
| `src/renderer/shaders/glsl/cell_bg.f.glsl` | +1/-1 | 1 | fractional cell background |
| `src/renderer/shaders/glsl/cell_text.f.glsl` | +11/-0 | 2 | fractional text clipping |
| `src/renderer/shaders/glsl/cell_text.v.glsl` | +1/-1 | 1 | fractional text offset |
| `src/renderer/shaders/glsl/common.glsl` | +2/-0 | 1 | shared uniform |
| `src/renderer/shaders/glsl/image.v.glsl` | +1/-1 | 1 | fractional image offset |
| `src/renderer/shaders/shaders.metal` | +13/-4 | 6 | Metal shader behavior |
| `src/terminal/PageList.zig` | +7/-2 | 4 | scrollbar model |
| `src/terminal/render.zig` | +232/-19 | 30 | terminal render state/tests |

Recommendation for all 105 hunks: keep them outside the #11 embedding review.
They may have their own value and history, but this audit makes no judgment on
that separate feature.

### GTK embedding partition

| File | Change | Hunks | Purpose and preparatory recommendation |
|---|---:|---:|---|
| `AGENTS.md` | +24/-0 | 1 | Zentty-only dogfood policy; downstream only. |
| `build.zig` | +126/-0 | 2 | Shared library/header install and private spike steps are combined; split generic build plumbing from test orchestration. |
| `include/ghostty/gtk.h` | +69/-0 | 1 | Experimental language-neutral ABI; retain as evidence, then minimize after Rust callers exist. |
| `src/apprt.zig` | +4/-1 | 1 | Select GTK runtime for a GTK library; retain only with that product. |
| `src/apprt/gtk/Surface.zig` | +1/-3 | 2 | Route through explicit surface owner; plausible generic foundation. |
| `src/apprt/gtk/class/surface.zig` | +54/-28 | 26 | Store/ref/unref explicit `Application` and replace default-app lookups in construction, config, input, notifications, resize, clipboard, and finalization; plausible generic foundation needing focused tests. |
| `src/build/SharedDeps.zig` | +4/-2 | 1 | GTK library native dependencies; keep with library build plumbing. |
| `src/gtk_embed_lib.version-script` | +13/-0 | 1 | Eight-symbol ELF allowlist/version node; retain if library remains and expand version-policy tests. |
| `src/gtk_embed_lib.zig` | +156/-0 | 1 | Runtime and all eight exports; defer final surface until Rust product evidence. |
| `src/gtk_embed_spike.valgrind.supp` | +71/-0 | 1 | Private external-library suppressions; downstream test evidence only. |
| `src/gtk_embed_spike.zig` | +377/-0 | 1 | Private Zig alternate host reaching internal APIs; not proof of a public boundary. |
| `src/termio/Exec.zig` | +31/-1 | 2 | Independent generic command-lifetime fix plus unit test; review separately. |

## Current patch partition quality

The history is good chronological dogfood evidence, but it is not yet an
independently reviewable generic patch series:

- `321afcd7` and `df682209` are explicitly downstream policy.
- `b9d9b9d5`, `e753619f`, `94fe18e7`, and `8f5141d7` grow a private Zig spike
  that reaches Ghostty internals rather than the installed C boundary.
- `32f631d5` contains the plausibly generic explicit-application ownership
  change, but also edits that private spike.
- `44743e83` is a self-contained POSIX shell-command lifetime repair with a
  focused regression test and is plausibly independent of embedding.
- `b8d92049` combines library build plumbing, runtime selection, header, and
  implementation.
- `7fa70f31` is separable optional focus/text/paste surface area.
- `5fc8fa2c` combines API documentation, handle checks, version visibility,
  and build changes.

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

The ELF version script has exactly **8 exported function symbols**, all under
`GHOSTTY_GTK_EMBED_1.0`; it hides every other implementation symbol. The
header additionally defines two **Ghostty-owned** ABI types and three enum
values:

- opaque `ghostty_gtk_embed_runtime_t`
- C enum `ghostty_gtk_embed_async_backend_t`
- `GHOSTTY_GTK_EMBED_ASYNC_DEFAULT=0`, `EPOLL=1`, `IO_URING=2`

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
`surface_new` returns `GtkWidget *`; `surface_grab_focus`, `surface_send_text`,
and `surface_request_paste` each accept it as their surface handle. Thus the
boundary is language-neutral C but not GTK-neutral: consumers must bind GTK's
GObject/widget type and lifetime rules. This dependency is distinct from the
two Ghostty-owned types, and its currently ambiguous constructor transfer is a
safe-Rust blocker.

A non-authoritative observation of the existing built artifact found all eight
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
| `ghostty_gtk_embed_runtime_new_with_async_backend` | C host plus API contract; drives all three backend cells | Likely capability; current enum/`c_int` representation is defective under `-fshort-enums` | Retain capability, but do not retain or bind this signature until repaired and cross-language-tested. |
| `ghostty_gtk_embed_runtime_free` | C host/API teardown | Required as safe `Drop` or equivalent | Retain; prove drop races and error policy. |
| `ghostty_gtk_embed_runtime_tick` | One-millisecond C-host GLib timer/API test | Likely under current implementation, but could become an internal GLib source | Defer public retention; investigate self-scheduling. |
| `ghostty_gtk_embed_surface_new` | One/four C-host real terminals and API test | Construction is required, current shell-string signature is not acceptable for #13 | Retain capability; redesign for typed argv/CWD/environment/config. |
| `ghostty_gtk_embed_surface_grab_focus` | C host focus/physical-key paths and misuse test | Likely active-pane requirement under #5 | Likely retain after real product focus proof. |
| `ghostty_gtk_embed_surface_send_text` | C qualification control and misuse test only | Possible programmatic text action, not yet proven | Defer/remove if real product has no caller. |
| `ghostty_gtk_embed_surface_request_paste` | C qualification clipboard relay and misuse test only | Possible product paste action; normal widget shortcuts may suffice | Defer/remove if real product has no caller. |

### Per-operation contract consequences

The JSON records each operation separately. Cross-cutting findings are:

- **Construction/configuration:** the runtime constructor selects only the
  async backend. `surface_new` copies nullable command/title, but maps command
  to a POSIX shell string and has no argv, CWD, environment, approved Ghostty
  configuration, structured error, or resource locator. This is enough for
  the qualification host, not #13's product construction contract.
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
- **Symbol/version policy:** `linux/tests/abi-surface` validates the eight
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
- an exact eight-name exported-symbol allowlist;
- API misuse rejection for null/foreign runtime and surface handles, invalid
  backend, second runtime, stale runtime, double free, null text, and
  uninitialized surface text/paste;
- Debug raw plus suppression-enabled candidate Valgrind evidence and unchanged
  full Ghostty Debug tests; suppression review remains a separate matrix gate.

This evidence proves the C qualification host at the pinned revision. The
private Zig spike supplies useful implementation history but is not an
independent public-ABI consumer. ReleaseSafe Valgrind remains XFAIL and the
matrix's compositor/IME/scaling/public-CI gaps remain unchanged.

## Missing proof before a final #11 decision

1. **Real usage:** no Rust workspace, `zentty-ghostty-sys`, safe adapter, or
   `zentty-linux` product caller exists. Every product-need statement above is
   prediction, not proof.
2. **Construction:** no typed argv/CWD/environment/config path and no proof
   against shell interpolation; no actionable structured constructor error;
   no negative fresh-process test for runtime construction after `gtk_init()`
   or after constructing any GTK object.
3. **Thread/ownership:** no off-main-thread, reentrant tick, live-surface
   runtime-free, stale widget, callback-during-teardown, child-exit-during-drop,
   or Rust unwind/drop-order test.
4. **Callbacks:** no stable public declaration of the GObject signal/property
   contract, disconnect token, userdata destroy notification, routing proof,
   or after-drop exclusion.
5. **Operations:** no product caller for any symbol; especially no non-test
   caller for default constructor, `send_text`, or `request_paste`.
6. **Errors/misuse:** no fault injection for allocation, Ghostty/GTK init,
   unavailable backends, core tick failure, clipboard denial, or invalid UTF-8;
   arbitrary dangling C pointers remain outside the possible guarantee.
7. **ABI compatibility/evolution:** the async C enum versus Zig `c_int` is a
   high-severity open representation defect, with no C/C++/Rust default and
   `-fshort-enums` proof. There is also no version-node assertion,
   compile-time/runtime ABI identity, versioned SONAME, mismatch failure,
   deprecation policy, or non-ELF visibility policy. C++ signature assertions
   cover only two of eight functions.
8. **Rebase/current upstream:** no current upstream ref was available, and the
   task intentionally performed no fetch/rebase. Conflict and regression
   measurements against a current official revision remain for later #11
   work.
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
| `ghostty-abi-version-node` | `ZL-11-GHOSTTY-ABI-COMPAT` / `TEST-GHOSTTY-ABI-VERSION` | NOT_IMPLEMENTED | Assert the eight ELF exports carry the intended node. |
| `ghostty-async-backend-abi-representation` | `ZL-11-GHOSTTY-ABI-COMPAT` / `TEST-GHOSTTY-ASYNC-BACKEND-ABI` | XFAIL, `DOGFOOD-2026-08-02-GHOSTTY-ASYNC-ENUM-ABI` | The deterministic C17/C++17 header probe exits 99 after reproducing 4-byte default versus 1-byte `-fshort-enums` representation. Full repaired C/C++/Rust size, alignment, and real-library call acceptance remains NOT_IMPLEMENTED. |
| `ghostty-runtime-initialization-order` | `ZL-11-GHOSTTY-API-AUDIT` / `TEST-GHOSTTY-RUNTIME-INIT-ORDER` | NOT_IMPLEMENTED | Prove runtime-before-`gtk_init`/any GTK object and safe reversed-order failure. |
| `rust-ghostty-api-product-usage` | `ZL-13-RUST-GHOSTTY-ADAPTER` / `TEST-RUST-GHOSTTY-PRODUCT-USAGE` | NOT_IMPLEMENTED | Trace every retained function to real safe-wrapper/product callers. |
| `rust-ghostty-callback-drop-order` | `ZL-13-RUST-GHOSTTY-ADAPTER` / `TEST-RUST-GHOSTTY-CALLBACK-DROP` | NOT_IMPLEMENTED | Fault callback/child exit during teardown and exclude after-drop calls. |
| `rust-ghostty-config-construction` | `ZL-13-RUST-GHOSTTY-ADAPTER` / `TEST-RUST-GHOSTTY-CONFIG` | NOT_IMPLEMENTED | Prove typed argv, CWD, environment, and approved config. |
| `ghostty-abi-old-new-mismatch` | `ZL-11-GHOSTTY-ABI-COMPAT` / `TEST-GHOSTTY-ABI-MISMATCH` | NOT_IMPLEMENTED | Require incompatible staged header/library pairs to fail before use. |

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
git -C "$GHOSTTY_SOURCE_DIR" log --reverse --oneline 19e20f766..HEAD
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
Ghostty API audit inventory passed: 32 files, 145 hunks, 8 allowlisted function exports, 2 Ghostty-owned public types, 1 external GtkWidget dependency
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
`ghostty-abi-version-node`; it is not a substitute for that missing cell.

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

- audit validator: PASS, 32 files / 145 hunks / 8 allowlisted exports / 2
  Ghostty-owned public types / 1 external `GtkWidget` dependency;
- qualification schema and negative runner self-tests: PASS;
- C17 and C++17 warning-as-error syntax checks: PASS;
- public C API contract: all six Wayland/X11 by default/epoll/io_uring
  combinations were observed to pass against the existing pinned ReleaseSafe
  artifact, but no new authoritative receipt was retained;
- built ABI observation: exactly 8 `T`/`W` exports, each
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
