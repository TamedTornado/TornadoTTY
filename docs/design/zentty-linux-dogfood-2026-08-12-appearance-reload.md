# Zentty Linux dogfood: Ghostty appearance and live reload

Date: 2026-08-12
Tracking: GitHub issue #20

## Initial audit

- **Discovery:** the safe Rust embedding adapter has surface construction and
  terminal actions but no runtime configuration update operation. Recreating
  surfaces would restart PTYs and violate the source behavior.
- **Ghostty boundary:** Ghostty already owns default config loading and propagation
  through `CoreApp.updateConfig`. The missing capability is only a narrow GTK
  embedding entry point; configuration parsing must not be duplicated in Zentty.
- **Upstreamability decision:** add one boolean runtime reload function to the
  experimental embedding header, version script, Zig implementation, sys binding,
  and safe adapter. Product-specific watching, persistence, settings policy, and
  theme UI stay in Zentty.
- **Source behavior:** source theme mode remembers independent dark/light choices,
  serializes automatic mode as `dark:<name>,light:<name>`, clamps opacity, preserves
  unrelated Ghostty configuration, resolves included files, and reloads live
  surfaces.
- **Qualification risk:** a log-only reload test is insufficient. The product
  journey must retain the same child PID and terminal scrollback while observing a
  real Ghostty runtime property change on multiple already-open surfaces.

Discoveries, failures, repairs, evidence, and remaining uncertainty will be
appended during implementation.

## Implementation record

- **Ghostty change:** branch `zentty/gtk-embed-reload-config`, commit
  `f4e85f032a0118eca32d2179b4f379a3769c7bb2`, adds exactly one exported
  operation. It loads Ghostty's normal configuration stack and invokes the
  existing core update path. The public boundary returns `false` for a null or
  stale runtime. No Zentty theme, shortcut, persistence, or UI policy entered the
  Ghostty fork.
- **Engine tests:** the focused `runtime reload` Zig test, the full GTK embedding
  library test target, and Zig formatting passed before the Ghostty commit was
  pushed.
- **Product boundary:** the raw sys declaration is wrapped by the existing
  main-thread-only `GhosttyRuntime`; no second runtime, surface registry, or
  configuration parser was introduced. `app.reload_config` routes through the
  existing shortcut registry and action router and has no invented default key.
- **ABI audit discovery:** the first old/new mismatch run failed with
  `fixture identity or hardening is invalid`. The new export had correctly made
  the authoritative API ledger stale. The ledger now records the 27-commit,
  16-export fork delta, the safe Rust owner, the real product caller, and the real
  journey. The normalized API audit and deliberate historical/current library
  mismatch then passed. `nm -D` reports the new operation under
  `GHOSTTY_GTK_EMBED_1.0`.
- **Qualification failure:** the first complete local qualification rerun stopped
  both ReleaseSafe and Debug build prerequisites after successful compilation.
  The lower-level ABI inventory was current, but `linux/tests/abi-surface` had a
  second explicit export allowlist and rejected the new symbol. That build gate is
  intentionally independent defense, not a parallel runtime system. Its list was
  reconciled to the reviewed 16-symbol ABI before rerunning qualification; the
  failed receipt remains in this record rather than being presented as a pass.
- **Real reload discovery:** the first controlled X11 journey changed native cell
  height from 17 to 42 logical pixels and retained the same foreground PID, but
  the following unbound `Ctrl+S` no longer reached the PTY. Reload had displaced
  keyboard focus. The product now restores focus immediately and once more after
  Ghostty's configuration propagation/layout turn; the input assertion then
  passed.
- **Multi-surface strengthening:** the consolidated shortcut/settings journey now
  creates a second real Ghostty pane before reload. It requires before/after
  native cell metrics for both surfaces, a larger cell height on both, and the
  same nonzero foreground PID for each. This is not a mock or log-only assertion:
  the metrics and PIDs are read through the native Ghostty ABI from live surfaces.
- **Controlled compositor receipts:** the strengthened journey passes in both the
  private Xvfb X11 environment and the nested Cage Wayland environment with
  virtual physical input:
  `rust-shortcuts-settings-x11: PASS real-gtk-settings physical-recorder
  real-ghostty-reload preserved-pty ...` and the equivalent Wayland receipt.

## Remaining uncertainty and scope

- The reload contract still lacks direct public-boundary tests for foreign and
  stale runtime pointers, malformed-config diagnostics, teardown races, and rapid
  successive reloads. These remain explicit API evidence gaps rather than passes.
- This slice proves process-global propagation across two existing surfaces in one
  product window. A cross-window product journey is still required before claiming
  every-window behavior exhaustively.
- Reload Configuration is only the enabling feature. Theme-mode memory, safe
  comment-preserving/symlink-preserving writes, opacity, the Appearance settings
  projection, theme resources/gallery, background images, file watching, and the
  platform blur alternative remain in issue #20.
- Full Linux qualification remains impossible while the authoritative matrix has
  BLOCKED, XFAIL, or NOT_IMPLEMENTED cells. Passing results here are scoped to the
  presently exercised feature journeys only.

## Qualification checkpoint

After the ABI allowlist repair, the complete presently executable local run
passed in 439.61 seconds. The machine summary reports `PASS=115`, `FAIL=0`,
`BLOCKED=7`, `XFAIL=1`, and `NOT_IMPLEMENTED=23`; the implemented local suite
passed, while release and full Linux qualification correctly remain not passed.
Suppression governance was accepted; the applicable result remains **PASS with
reviewed suppressions**, not an unsuppressed-clean claim.
