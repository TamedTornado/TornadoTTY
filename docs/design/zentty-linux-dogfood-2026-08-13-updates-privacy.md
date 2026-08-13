# Zentty Linux Updates & Privacy dogfood — 2026-08-13

This append-only record covers the Updates & Privacy settings-page slice of
GitHub issue #20. The implementation plan is
`docs/design/linux-updates-privacy-settings-feature-plan.md`.

## Source audit

- The source settings page contains only Update Channel and Error Reporting.
  Update discovery, release notes, installation, rollback, About metadata, and
  license disclosure are separate source behaviors and remain issue #23 scope.
- The exact source update channels are Stable and Beta, persisted as `stable`
  and `beta` under `[updates]`.
- Source Error Reporting is disabled when its build configuration is absent and
  says “Error reporting is unavailable in this build.” Linux has no reviewed
  transport or consent/redaction contract, so that is the truthful current
  state. This slice must not manufacture a telemetry backend to make the row
  interactive.

## Implementation discoveries and decisions

- Source-compatible `UpdatesConfig`, `UpdateChannel`, and
  `ErrorReportingConfig` now live in the one core `AppConfig`. The settings page
  does not own a parallel model.
- Only Update Channel is writable. The error-reporting switch reflects the
  source default but is disabled because the Linux build has no reviewed
  runtime transport. The UI includes both “Unavailable” and the exact source
  unavailable explanation; no click can create an inert or misleading enabled
  state.
- The page includes an explicit Linux boundary note: choosing Stable or Beta
  does not install an update. This prevents the completed settings page from
  being confused with the still-NOT_IMPLEMENTED issue #23 updater.
- Growing the settings-shell constructor triggered Clippy's argument-count
  guard. Rather than suppress it, the shell now receives a small `SettingsPages`
  bundle. This is the one page registry and gives later settings pages a clear
  extension point without another constructor argument per page.
- The initial GTK control was a `GtkDropDown`, matching the macOS popup shape.
  Real X11 physical-input qualification proved that its mnemonic focused the
  control but normal Down, Space/Down/Return, and Alt+Down interactions did not
  commit a selection in this GTK version. A linked Stable/Beta segmented
  selector replaced it. This keeps the source values and single-choice
  semantics, makes both choices visible, and provides direct keyboard mnemonics
  instead of retaining a control that failed its accessibility journey.

## Harness discoveries, failures, and repairs

- **Environment failure:** the first nested-X11 attempt ran inside the managed
  filesystem sandbox. Xvfb could not bind its private `/tmp/.X11-unix` socket
  and emitted a misleading ownership diagnostic even after the host directory
  had correct root/sticky permissions. The controlled GUI journeys were rerun
  with the required elevated execution boundary; no product or wrapper bypass
  was added.
- **Physical-input failure:** four X11 attempts showed the settings window and
  focused the original dropdown, but no channel change was emitted. This was
  treated as a failed journey, not environmental success. The segmented-control
  repair then passed the same X11 journey using Alt+B and Alt+S.
- The journey starts the staged product twice against the same isolated XDG
  config. It changes Stable to Beta, verifies exact persistence plus preservation
  of comments, unknown update keys, and privacy state, terminates the product,
  proves Beta is loaded after restart, and changes back to Stable.
- The identical real-product journey passed under nested Cage Wayland with
  virtual keyboard input. Neither desktop cell assumes the developer's ambient
  compositor or configuration.
- **Aggregate failure outside the slice:** the first complete matrix run passed
  both new settings cells but failed the combined X11 agent cell late in its
  consolidated session-restore journey because the expected multi-file
  rollback receipt missed its deadline. The exact combined cell, including the
  pinned real Gemini, installed Codex, installed Claude, tmux compatibility,
  and consolidated restore, passed immediately afterward without a timeout or
  product change. The failure remains recorded rather than being relabeled.
- **Aggregate cleanup failure and repair:** the X11 notification product journey
  itself passed, but its environment report was rejected when
  `xdg-document-portal` published/withdrew its private FUSE mount during wrapper
  teardown. The nested-X11 wrapper already inspected and unmounted exactly that
  owned path, but one second race remained between inspection and recursive
  removal. Cleanup now performs a bounded inspect/unmount/remove loop, still
  failing if either the mount or private run root survives. Wrapper negative
  tests and the exact real notification cell pass after the repair; no portal
  process is ignored and no environmental absence becomes success.
- **First cleanup repair was incomplete:** a full rerun again rejected the same
  notification environment. Raw host inspection showed the disconnected
  `fuse.portal` endpoint remained mounted twice at the exact owned path.
  `mountpoint` returns false for a disconnected endpoint, so the wrapper never
  invoked `fusermount3`; a time retry could not help. The corrected repair uses
  `findmnt -M` to count every mount stacked at that exact path and unmounts until
  the count is zero before removal. An exact notification rerun now both passes
  and produces a validated environment report with `run_root_removed=true`.
  This evidence supersedes the earlier incomplete repair claim but preserves it
  as part of the failure history.
- **Third aggregate exposed a separate Global Find focus race:** every new
  settings cell and the repaired notification cell passed, but the established
  Wayland pane-search journey typed its short query after the row was visible
  and before GTK had focused its real search entry. The product now emits a
  focus receipt from the entry's actual `EventControllerFocus`, and the journey
  waits for that receipt before physical typing. This replaces timing luck with
  a real GTK boundary; it does not add a test-only action or alternate search
  path. The exact controlled Wayland journey passed before the repair and is
  rerun against the deterministic receipt below.

## Evidence

- `cargo test -p zentty-core --locked --test app_config` passes the exact source
  defaults, both tokens, forward-compatible unknown keys, and invalid known
  values.
- Focused Linux tests pass the page model/channel order and the ConfigStore
  symlink, comment, unknown-key, unrelated privacy value, and `0600` mode
  preservation contract.
- Clippy with `-D warnings`, Rust formatting, ShellCheck, architecture contract,
  qualification matrix tests, and qualification-boundary self-tests pass.
- `updates-privacy-settings-x11` and
  `updates-privacy-settings-wayland` pass with physical input, persistence, and
  restart evidence. The authoritative matrix now totals 119 PASS, 7 BLOCKED,
  1 XFAIL, and 23 NOT_IMPLEMENTED before the aggregate rerun.

## Remaining uncertainty

- Stable/Beta selection does not yet discover or install Linux packages.
- Error reporting remains unavailable until issue #23 defines and qualifies an
  explicit-consent transport, payload redaction, endpoint, and offline/error
  behavior.
- No full or exhaustive Linux qualification claim is made while the matrix
  contains BLOCKED, XFAIL, or NOT_IMPLEMENTED cells.

## Final aggregate receipt

- The final complete `linux/tests/qualify-local` run passed every presently
  executable support and product/dependency cell. Both Updates & Privacy cells,
  the stacked-portal X11 notification teardown, the combined X11 agent cell,
  and the Wayland Global Find focus journey passed in the same aggregate.
- Machine summary:
  `build/linux/qualification-summary.json`, SHA-256
  `c8c9477b0f6ab889a3104b7241f6e2f6f2f3c8bda6b4c20a75e3c0bd2afd14fb`.
- Declared matrix totals are 119 PASS, 7 BLOCKED, 1 XFAIL, and 23
  NOT_IMPLEMENTED. The implemented local suite and product-boundary
  qualification passed. Release and full Linux qualification correctly remain
  not passed.
- Debug Valgrind is **PASS with reviewed suppressions**; suppression governance
  was accepted. ReleaseSafe Valgrind remains XFAIL as required. This is not an
  unsuppressed-clean claim.
