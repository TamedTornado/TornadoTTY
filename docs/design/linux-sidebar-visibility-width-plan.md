# Linux sidebar visibility and width plan (GH-78)

## Objective

Close the source-owned sidebar visibility/width workflow without expanding the
scope to all GH-16 visual parity. Linux will use the existing `AppConfig`,
`ConfigStore`, reload authority, `ApplicationShell`, sidebar state machine, and
source-UX product journey. No parallel preference store, watcher, layout model,
or integration harness may be introduced.

## Source contract

The authoritative behavior is `AppConfig.sidebar`,
`SidebarVisibilityController`, `SidebarMotionCoordinator`,
`SidebarWidthPreference`, and their call sites in `RootViewController`.
Persistent modes are `pinnedOpen` and `hidden`; `hoverPeek` is transient.
Global Find focus prevents transient dismissal. Width is clamped to the source
180–420 px and available-content policy.

## Construction order

1. Add failing focused tests for typed sidebar parsing, invalid input, partial
   reload retention, normalization, and visibility focus guards.
2. Add the typed core configuration and extend the existing atomic
   `ConfigStore` updater. Do not add storage.
3. Initialize and live-project every shell from the shared config. Persist only
   user toggle/resize decisions through `ConfigStore`; let the existing reload
   authority project the committed result.
4. Connect Global Find focus to the existing sidebar state machine and prove
   that transient hover state never persists.
5. Extend the existing source-UX product journey for real X11 and Wayland
   pointer/key/focus/PTY/restart evidence. Add no second orchestration layer.
6. Run governed focused mutation tests, affected architecture/inventory/matrix
   validators, then one affected feature-boundary qualification pass.
7. Review the diff, reconcile GH-78/GH-16/GH-4 and the feature inventory, record
   exact receipts, commit, and push.

## Acceptance boundary

This feature may close GH-78 only when the real product proves default pinned,
persisted hidden, floating hover peek, search-held peek, clamped persisted
resize, live config projection, restart restoration, accessibility metadata,
and continuing real PTY input on controlled X11 and Wayland. Immediate Linux
transitions are an explicit reduced-motion-safe platform implementation for
this slice; animated source parity remains in GH-16 unless implemented and
qualified here.
