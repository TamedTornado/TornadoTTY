# Ghostty GTK embedding API review

Status: **GH-11 closeout; retain a qualified downstream fork for now**.

This is the human companion to
[`linux/ghostty-api-audit.json`](../linux/ghostty-api-audit.json). The JSON is
the machine-readable inventory; `linux/tests/ghostty-api-audit --self-test`
checks it against the exact locked Ghostty checkout and Zentty product callers.
The validator intentionally compares semantic ledgers and public symbols. It no
longer hashes every patch and hunk: exact Git revisions plus the reviewed file,
operation, caller, and test inventories are sufficient and avoid turning the
audit into a second build system.

## Decision

We will **retain the embedding work in `TamedTornado/ghostty`** rather than send
the entire patch series upstream now.

This is not a judgment that Ghostty maintainers would reject the work. The
current boundary creates an alternate GTK application-host model and therefore
asks maintainers to own a substantial new lifecycle and support contract. We
have one proven consumer—Zentty—and no maintainer invitation to add that public
surface. A broad unsolicited pull request would contradict our requirement to
keep upstream proposals small and respectful.

The decision can be revisited if:

1. another non-Zentty consumer demonstrates demand;
2. Ghostty maintainers express interest in an alternate GTK host API; or
3. the foundation can be reduced further without weakening Zentty.

Any upstream discussion or submission requires Jason's explicit review and
approval. The Ghostty candidate remains language-neutral C. Rust declarations,
safe ownership, product policy, and orchestration stay in Zentty.

## Exact reviewed revisions

| Item | Revision |
|---|---|
| Official repository | `ghostty-org/ghostty` |
| Current official base | `6dcf68fc0b12e8caebbfc43770d66edac124b4f8` |
| Downstream candidate branch | `TamedTornado/ghostty:linux/context-menu-model` |
| Qualified candidate tip | `eb1eb6281466ee7e85f629012ac77de7d9cac712` |
| Downstream commits | 34 |

The previous pin was rebased rather than merged. Two Ghostty-local commits that
only added Zentty policy to `AGENTS.md` were deliberately dropped. The candidate
diff contains no Zentty workspace, worklane, agent, qualification, or operator
policy.

## Rebase findings

The 33-commit downstream series replayed on current official main with two
textual conflicts and three compile-level adaptations:

- Ghostty added GTK native-blur snapshot code beside our explicit early surface
  deinitialization. Both behaviors were retained as separate functions.
- `gtk4-layer-shell` moved from a direct lazy dependency to `LocalDeps`; the
  reproducible prefix map now uses `deps.upstream`.
- Ghostty's clipboard request became a payload-carrying union and its result
  became `ClipboardReadResult`. The engine-owned spike was adapted to the new
  request shape.
- The public backend type was an implementation-defined C enum. Under
  `-fshort-enums` it became one byte while Zig accepts `c_int`. It is now an
  exact `int32_t` typedef with typed constants; C17 and C++17 remain four bytes
  with and without `-fshort-enums`.
- Three experimental exports had no Zentty caller and were removed:
  `ghostty_gtk_embed_runtime_new`, `ghostty_gtk_embed_surface_new`, and
  `ghostty_gtk_embed_surface_request_paste`.

The minimized allowlist now matches the product-owned Rust declarations exactly.
The latest independent operation lets an alternate GTK host replace only the
surface's `GMenuModel`; Ghostty continues to own right-click routing, anchoring,
mouse-reporting interaction, and popover lifecycle.

## Reviewed file inventory

| File | Owner and purpose |
|---|---|
| `build.zig` | Build, test, install the GTK embedding library and resources. |
| `include/ghostty/gtk.h` | Fixed-width language-neutral public ABI and lifecycle contract. |
| `src/gtk_embed_lib.zig` | Opaque runtime and fourteen product-used operations. |
| `src/gtk_embed_lib.version-script` | Exact ELF allowlist/version node. |
| `src/gtk_embed_options.zig` | Size-versioned copied surface-option validation. |
| `src/gtk_embed_spike.zig` | Ghostty-owned alternate-host lifecycle/PTY/input/signal exercise. |
| `src/gtk_embed_spike.valgrind.supp` | Reviewed external-library suppressions for that engine test only. |
| `src/apprt.zig` | Select GTK runtime for the library artifact. |
| `src/apprt/gtk/Surface.zig` | Resolve runtime through explicit surface ownership. |
| `src/apprt/gtk/class/application.zig` | Carry the alternate host's GTK application runtime. |
| `src/apprt/gtk/class/global_shortcuts.zig` | Avoid default-application assumptions. |
| `src/apprt/gtk/class/surface.zig` | Explicit owner, child environment, signals, and IME focus lifecycle. |
| `src/build/SharedDeps.zig` | GTK library dependencies and reproducible source mapping. |
| `src/termio/Exec.zig` | Preserve copied shell-command lifetime for the child process. |
| `pkg/dcimgui/build.zig` | Reproducible bundled compiler path. |
| `pkg/glslang/build.zig` | Reproducible bundled compiler path. |
| `pkg/gtk4-layer-shell/build.zig` | Current LocalDeps support and reproducible bundled compiler path. |
| `pkg/harfbuzz/build.zig` | Reproducible bundled compiler path. |
| `pkg/spirv-cross/build.zig` | Reproducible bundled compiler path. |

The JSON associates every file with concrete focused or product tests. Any added
or removed file fails the audit until reviewed.

## Public ABI allowlist

Exactly fourteen functions are exported under `GHOSTTY_GTK_EMBED_1.0`:

1. `ghostty_gtk_embed_runtime_new_with_async_backend`
2. `ghostty_gtk_embed_runtime_free`
3. `ghostty_gtk_embed_runtime_tick`
4. `ghostty_gtk_embed_runtime_reload_config`
5. `ghostty_gtk_embed_surface_new_with_options`
6. `ghostty_gtk_embed_surface_close`
7. `ghostty_gtk_embed_surface_grab_focus`
8. `ghostty_gtk_embed_surface_send_text`
9. `ghostty_gtk_embed_surface_binding_action`
10. `ghostty_gtk_embed_surface_cell_size`
11. `ghostty_gtk_embed_surface_read_text`
12. `ghostty_gtk_embed_surface_read_selection`
13. `ghostty_gtk_embed_surface_foreground_process_id`
14. `ghostty_gtk_embed_surface_set_context_menu_model`

Each has:

- an exact raw Rust declaration in `zentty-ghostty-sys`;
- one safe owner in `zentty-ghostty`;
- at least one real Zentty product caller;
- a negative C contract path; and
- at least one real staged-product journey in the qualification matrix.

No dependency implementation symbols are exported. The public Ghostty-owned
types are the opaque runtime, size-versioned surface options, synchronous text
callback, logical cell-size pair, fixed-width async backend, and fixed-width
text extent. `GtkWidget` is the one external public type dependency.

## Lifecycle and ownership contract

- All operations run on the GTK main thread.
- The runtime must be created before `gtk_init()` and before constructing any
  GTK object.
- Only one runtime may ever be created in a process, including after free,
  because Ghostty process-global state is not restartable.
- The host closes and finalizes every surface, drains GLib, and only then frees
  the runtime.
- Surface construction returns a GTK widget governed by normal GTK
  container/reference ownership. `surface_close` deinitializes Ghostty state;
  it does not release the caller's GTK reference.
- Calls using null, foreign, stale, uninitialized, or closed handles fail with
  null/false/zero and do not mutate output buffers.
- Text callbacks are synchronous; their byte slices are borrowed only for the
  callback.
- The Rust owner disconnects GTK signal handlers before final widget release
  and retains a runtime lease until GObject finalization completes.

The API does not promise off-main-thread use, multiple runtimes, runtime
restart, arbitrary dangling-pointer safety, or a GTK-neutral widget type.

## Independently reviewable upstream candidates

If Jason later approves upstream discussion, do not submit the 33-commit branch
as one pull request. The plausible small candidates are:

1. the independent `Exec` command-lifetime repair and unit test;
2. preedit cancellation on GTK surface focus loss, with real IBus/fcitx proof;
3. safe logging during rejected pre-runtime initialization;
4. explicit non-default application ownership, but only after discussing the
   alternate-host direction;
5. individual product-neutral queries/actions after the foundation is wanted.

Private downstream spike orchestration, Valgrind receipts, packaging policy,
and Zentty's Rust adapter do not belong in an upstream proposal.

## Qualification

Focused closeout commands are:

```sh
GHOSTTY_SOURCE_DIR=build/linux-deps/ghostty linux/tests/ghostty-api-audit --self-test
linux/tests/ghostty-async-backend-abi-test
GHOSTTY_INSTALL_DIR=build/linux-deps/ghostty/zig-out linux/tests/ghostty-async-backend-abi
linux/tests/abi-surface
linux/tests/abi-version-node
linux/tests/nested-x11 linux/tests/runtime-init-order
linux/scripts/build-abi-mismatch-fixture
linux/tests/ghostty-abi-mismatch --self-test
linux/tests/ghostty-abi-mismatch
linux/tests/ghostty-regression
```

The ABI-representation cell is now `PASS`, not XFAIL. Debug Valgrind remains
**PASS with reviewed suppressions**, never an unsuppressed-clean claim.
ReleaseSafe Valgrind remains XFAIL and is not part of this API decision.
