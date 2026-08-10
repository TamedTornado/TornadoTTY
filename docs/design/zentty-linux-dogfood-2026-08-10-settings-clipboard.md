# Zentty Linux dogfood — settings owner and automatic Clean Copy

Date: 2026-08-10
Issues: GH-20, GH-35, GH-17

## Slice contract

This slice introduces the first narrow part of Zentty's single process-level
settings authority: the source `[clipboard]` schema and automatic Clean Copy.
It does not create a second settings store for the clipboard feature and does
not pretend to deliver the complete settings window.

Acceptance order:

1. Port the source clipboard defaults, strict known-value validation, and
   unknown-key compatibility into a pure Rust configuration model with red
   source-derived tests first.
2. Load one XDG `zentty/config.toml` snapshot before application composition.
   Missing files use source defaults; malformed known values retain the last
   good/default snapshot and produce a bounded diagnostic without contents.
3. Pass that immutable snapshot from the application coordinator to every
   window. No window-local parser, watcher, or fallback preference is allowed.
4. Make ordinary Copy use the configured source cleanup options when
   `always_clean_copies` is true. Copy Raw remains byte-exact; explicit Clean
   Copy remains available. Context presentation substitutes Copy Raw for Clean
   Copy exactly as the source does.
5. Extend the existing real compositor/PTY clipboard journey to launch from a
   private XDG config, prove automatic clean versus raw bytes, and prove both
   windows receive the same process snapshot without recreating PTYs.
6. Mutation-test parsing, defaults, option mapping, and Copy-style policy; then
   run all affected controlled journeys and the authoritative matrix.

Explicit later work remains external file watching, atomic/symlink-preserving
writes, the complete settings window, and every non-clipboard settings section.
Those gaps stay visible in GH-20 and the qualification matrix.

## Discoveries and evidence

- The macOS source schema is authoritative in `AppConfig.Clipboard` and
  `AppConfigTOML.decodeClipboardAssignment`. It has nine keys. Unknown keys are
  ignored, invalid known booleans or aggressiveness reject the snapshot, and
  the default for `always_clean_copies` is false.
- The source menu policy is explicit: ordinary Copy is cleaned when automatic
  cleaning is enabled, and the adjacent contextual escape hatch changes from
  Clean Copy to Copy Raw. Linux currently exposes all commands unconditionally
  and always treats ordinary Copy as raw, so the feature is not yet present.
- The source-derived Rust tests were added before implementation. The required
  red run failed at compile time because `zentty_core::AppConfig` did not exist;
  no pre-existing parser or hidden settings authority satisfied the tests.
- The first store test caught that forwarding the TOML parser's display error
  could echo a hostile/secret invalid value into diagnostics. Startup warnings
  now contain only the config path and a generic parse/known-value failure;
  config contents never enter receipts.
- The first controlled-X11 product attempt used Cargo's development binary
  directly and failed before startup because it intentionally has no staged
  `libghostty-gtk-embed.so` runtime path. This is a packaging-boundary mistake,
  not product evidence. The affected journey is rerun against the normal
  ReleaseSafe staged product below.
- Strict Clippy passed after moving the process config into the existing
  application dependency bundle instead of adding an eighth shell-constructor
  argument. The first workspace test run then reproduced the known sandbox
  denial for eight real Unix-listener agent tests (`EPERM`); all tests reached
  before that boundary passed. The identical workspace suite is rerun with
  local-socket permission rather than faking those transports.
- The elevated full workspace rerun passed, including all eight real Unix
  listener tests; no transport was replaced for sandbox convenience.
- The initial bounded-reader patch passed its focused tests but strict Clippy
  rejected a potentially truncating `u64` allocation cast, a manual
  `match`/early-return, and two ambiguous default expressions. The allocation
  now uses checked conversion, the encoding branch is explicit, and the tests
  name `AppConfig::default()`; strict workspace Clippy then passed.
- Config reads are capped at 1 MiB, require a regular file, and classify
  oversized or non-UTF-8 content as invalid without echoing bytes. Tests cover
  the absent, valid, invalid-value, oversized, invalid-encoding, and directory
  boundaries. This is intentionally read-only: symlink-preserving atomic
  writes and live reload remain GH-20 work rather than a second partial store.
- The ownership validator first rejected the new `config` shell projection as
  an unassigned field. The repaired contract now names one process-level
  `config_store` authority, hashes and inventories its implementation, assigns
  the immutable window projection, and places config loading once at the head
  of construction. Both the positive validator and its negative self-tests
  pass.
- A freshly rebuilt staged ReleaseSafe product passed the complete existing
  multi-window journey on controlled X11 and input-capable Wayland. Each real
  process emitted exactly one config-load receipt and both real windows emitted
  the same projection receipt across clean restore, SIGKILL restore, live pane
  transfer, construction rollback, and non-final window closure.
- The freshly staged product also passed the existing real clipboard/search
  journey on controlled X11 and Wayland: ordinary Copy was automatically
  cleaned from the private XDG config while Copy Raw remained byte-exact in an
  independent compositor clipboard client. These runs exercise real Ghostty
  PTYs and the delivered binary, not a config-only scenario application.
- The feature inventory now promotes the complete settings feature only from
  `NOT_IMPLEMENTED` to `PARTIAL`; it does not overstate the narrow clipboard
  subset. The matrix adds one executable PASS cell for that parser/store
  boundary while the complete platform-settings cell remains explicitly
  `NOT_IMPLEMENTED` with its remaining UI, schema, reload, and persistence
  obligations.
- The first focused config-store mutation run found 12 survivors. They exposed
  real missing boundaries (permission errors, exact size, post-metadata growth,
  empty XDG values, and XDG/HOME precedence) plus redundant metadata/allocation
  branches. The store was simplified to one capped reader and the missing tests
  were added. The final scoped result is 19 mutants: 15 caught, 4 unviable, 0
  missed. Cargo Mutants ran with the repository-enforced `gitignore=true` and
  `copy_target=false`; no build tree was copied into scratch workers.
- A broad clipboard-module mutation probe initially reported six survivors.
  Four were pre-existing GUI-integration boundaries and two revealed that the
  markdown fixtures did not discriminate classifier branches. Transformation
  was extracted as a pure policy function, stable receipt names were asserted,
  and truly discriminating markdown/prose fixtures were added. The acceptance-
  scoped automatic/raw/markdown policy result is 9 mutants: 8 caught, 1
  unviable, 0 missed. The real compositor journey remains the evidence that
  deleting the GTK/Ghostty bridge itself cannot pass product qualification.
- Adding the new matrix cell first failed the runner because its capability was
  not in the closed required-capability vocabulary. The cell now correctly
  shares `platform_settings` with the explicit incomplete full-contract cell;
  matrix focused tests pass and no parallel settings capability was invented.
- The first authoritative full gate executed every product cell successfully
  except `architecture-contract-v1`. Its failure was legitimate: the separate
  architecture mirror still described the old full-settings defect and omitted
  the new subset cell, even though the narrower shell-ownership contract passed
  locally. The architecture requirement mapping, capability family, and
  proposed-cell mirror now include both the passing clipboard subset and the
  still-`NOT_IMPLEMENTED` complete settings contract. Positive and negative
  architecture validators pass. Because environmental absence is never a pass,
  the entire executable matrix is rerun rather than editing the failed receipt.
- The corrected authoritative rerun passed every presently executable support
  and matrix cell in 374,030 ms. Declared totals are `PASS=94`, `FAIL=0`,
  `BLOCKED=7`, `XFAIL=1`, and `NOT_IMPLEMENTED=21`. The implemented local suite
  passed; release and full Linux qualification remain correctly not passed.
  Debug Valgrind remains **PASS with reviewed suppressions**, suppression
  governance passed, and the ReleaseSafe Valgrind gap was not hidden or
  broadened. The machine receipt is `build/linux/qualification-summary.json`.
