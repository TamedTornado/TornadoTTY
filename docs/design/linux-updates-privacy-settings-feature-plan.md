# Linux Updates & Privacy settings feature plan

Status: implemented and aggregate-qualified for GitHub issue #20
Date: 2026-08-13

## Source authority

The settings-page authority is
`Zentty/UI/Settings/UpdatesPrivacySettingsSectionViewController.swift`, with
configuration keys defined by `Zentty/Config/AppConfig.swift` and
`Zentty/Config/AppConfigTOML.swift`. The source page has exactly two controls:

1. **Update Channel** selects Stable or Beta and persists `[updates].channel`.
2. **Error Reporting** persists `[error_reporting].enabled` only when the build
   has an error-reporting backend, and otherwise renders the control disabled
   with the explicit text “Error reporting is unavailable in this build.”

The page is not itself an updater, release-notes view, crash uploader, or
telemetry implementation. Those behaviors remain owned by issue #23 and by the
authoritative inventory entries `lifecycle.updates-channels` and
`trust.about-licenses-privacy-errors`.

## Linux platform boundary

- Preserve the Stable/Beta setting now so the later Linux package update
  service has one source-compatible channel value. Do not imply that selecting
  a channel currently installs or discovers releases.
- Zentty Linux currently has no reviewed crash-reporting transport, redaction
  policy, consent receipt, or endpoint. Therefore Error Reporting is visibly
  unavailable and cannot be toggled. A disabled source-shaped row is honest;
  an inert enabled switch or a fake local transport is not.
- No network request is introduced by this slice. Ordinary use remains free of
  telemetry and crash transmission.

## Test-first acceptance

1. Core configuration tests pin source defaults, exact table/key names, both
   channel values, forward-compatible unknown keys, and rejection of invalid
   known values.
2. ConfigStore tests prove a channel update preserves comments, unknown keys,
   unrelated tables, symlinks, private mode, and the error-reporting value.
3. Focused settings-model tests pin the Stable/Beta ordering and the explicit
   unavailable error-reporting presentation.
4. Controlled X11 and nested Wayland journeys open the real nonmodal Settings
   window through the command palette, select Beta with physical GTK input,
   observe the persisted source key, restart the staged product, and select
   Stable again. The same journey must observe the unavailable crash-reporting
   row; absence cannot be reinterpreted as a pass.
5. Add separate executable matrix cells for this page. Keep the complete
   platform settings contract and issue #23 update/crash-delivery capabilities
   explicitly NOT_IMPLEMENTED.
6. Run format, Clippy, architecture, matrix validation, focused journeys, and
   every presently executable qualification cell before commit.

## Architecture constraints

- Extend the existing `zentty_core::AppConfig` and Linux `ConfigStore`; do not
  create a page-local configuration store.
- Put the focused GTK page in its own module and pass it into the existing
  settings shell.
- The page may emit intent only through `ApplicationShell`; GTK callbacks must
  not edit files or invoke package/network commands directly.
- Issue #23 must later consume these configuration values rather than adding a
  second updater or error-reporting settings model.
