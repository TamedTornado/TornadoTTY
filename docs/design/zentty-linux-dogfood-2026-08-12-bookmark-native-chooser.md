# Zentty Linux dogfood — native bookmark chooser closeout

Date: 2026-08-12
Issue: GH-18 (`workspace.bookmarks-presets` closeout)

## Contract

This record follows the closeout section in
[`linux-bookmarks-presets-feature-plan.md`](linux-bookmarks-presets-feature-plan.md).
The product keeps one bookmark store, one import/export envelope, and one GTK
action owner. Only the Linux native chooser coordination boundary may change.

## Starting evidence

- Controlled X11 maps the real portal chooser but synthetic keyboard activation
  cannot activate Save; the absence of an exported file exits 1.
- Controlled Wayland maps the portal but cannot associate it reliably with the
  Zentty parent surface; the GNOME portal backend requires a GNOME session that
  the isolated compositor deliberately does not assume.
- The final Git/review qualification rerun exposed a related cleanup fact:
  `xdg-document-portal` can retain a private FUSE mount after the D-Bus process
  group exits. The nested-X11 wrapper now deterministically unmounts and proves
  removal, but that repair does not make portal keyboard routing a product pass.
- Source uses the macOS native save/open panel for a Zentty-owned portable file.
  An in-process transient GTK chooser is the direct Linux platform analogue and
  avoids making desktop-portal implementation behavior part of the product's
  own file-format contract.

## Discoveries and repairs

- **Portal ownership was the wrong boundary.** `GtkFileDialog` delegated this
  application-owned format to a desktop portal whose availability and focus
  behavior varied across isolated compositors. Zentty now uses one modal,
  transient-for-parent `GtkFileChooserDialog`; the model, envelope, and store
  are unchanged. GTK deprecated this widget in 4.10 in favor of the portal API,
  so the two chooser functions carry narrow `deprecated` allowances and the
  reason is documented at the call site.
- **Nested popovers retained transient ownership.** Rename, edit, export, and
  import previously closed only their inner action popover. On Wayland that
  left the parent bookmark popover mapped while a modal was presented and the
  next transient could not reliably receive compositor input. Each route now
  closes both popovers before presenting a dialog; the formerly failing full
  management journey passes without a product test mode or timing retry.
- **The create/import control was not keyboard reachable.** `GtkMenuButton`
  delegates input to an internal toggle widget. Marking only the wrapper
  focusable did not place the real target in GTK traversal. The internal target
  is now explicitly focusable and emits the existing focus receipt. X11 proves
  Search → Tab → Create/Import → Import using physical keys.
- **X11 modifiers leaked between nested controls.** Save initially appeared to
  ignore its mnemonic because synthetic modifier state survived window
  activation. Releasing modifiers before opening the popover and using
  `xdotool --clearmodifiers` made the real `_Save` activation deterministic.
- **Open requires a selected file, not merely a filtered path.** GTK's location
  entry displayed the exact exported file while leaving Open insensitive. A
  diagnostic screenshot established that state. The journey now physically
  double-clicks the sole real filtered result, matching ordinary chooser use;
  it does not inject a path into the product or bypass chooser validation.

## Focused staged-product evidence

- Controlled X11 import/export: PASS. The real chooser wrote
  `Portable.zenttypreset`; the journey validated schema and portability,
  deleted the live bookmark, selected that real file in a second chooser,
  imported it, and inspected the actual persisted store.
- Controlled Cage/Wayland import/export: PASS with the same byte/store
  assertions and virtual-keyboard compositor input.
- Controlled Cage/Wayland management: PASS for standard context menu, rename,
  edit, duplicate, pin, convert, linked update, unlink, and delete.

The three GH-18 XFAIL cells are therefore promoted to PASS. Final whole-matrix
totals and receipt hashes are recorded after the complete qualification rerun;
until that succeeds, this record makes no release or full-qualification claim.

## First whole-matrix rerun

- The promoted Wayland management and both import/export cells passed, but the
  complete run correctly rejected two previously passing X11 journeys:
  management and save/restore.
- The regression was test orchestration, not a tolerated skip. The chooser
  scenario starts Openbox because GTK dialogs need normal activation semantics;
  its shared `open_bookmarks` helper had therefore been changed to
  `_NET_ACTIVE_WINDOW`. The ordinary isolated X11 cells deliberately run with
  no window manager, where that request fails before the product shortcut is
  sent.
- The one existing harness now selects the real input primitive from the
  environment it owns: `windowactivate` only when its Openbox process exists,
  otherwise direct `windowfocus`. Focused reruns of X11 management and the full
  save/quit/relaunch/restore journey both pass. No second harness or product
  route was added.

## Second whole-matrix rerun

- All six bookmark cells passed. The run later failed in the pre-existing
  `product-source-ux-x11` journey: its last hover Close action targeted pane 1
  rather than pane 2 after several earlier layout mutations. This cell does not
  traverse bookmark code, and a focused rerun passed unchanged, including its
  real-pointer pane-local right/below/close assertions.
- The failure is retained as evidence rather than silently relabeled. Because
  the authoritative complete receipt still failed, another whole-matrix rerun
  is required before commit; the focused pass alone is not qualification.

## Third whole-matrix rerun

- The source-UX cell passed, and five of six bookmark cells passed. Wayland
  management progressed through save, rename, and duplicate, then a subsequent
  bookmark shortcut did not open the popover. The real log showed that
  duplicate/pin/convert/delete handlers closed neither nested popover, unlike
  the already repaired rename/edit/export routes.
- This was a product lifecycle defect exposed by repeated real action use, not
  an environmental absence. Every template action now releases both the action
  popover and its parent before dispatch. Two consecutive controlled Wayland
  management journeys and the X11 counterpart pass after the repair.

## Final qualification receipt

- `linux/tests/qualify-local`: PASS for every presently executable cell in
  450.54 seconds. Declared totals are PASS=113, FAIL=0, BLOCKED=7, XFAIL=1,
  NOT_IMPLEMENTED=21. Implemented-local and product-boundary qualification
  passed; release and full Linux qualification correctly remain not passed.
- Summary SHA-256:
  `4508cc67678d13abd3ac85d5d3187bc246e80fb56c3119646afd5dbbdf3712ba`.
- Debug Valgrind is **PASS with reviewed suppressions**, not an unsuppressed
  clean result: raw errors/contexts 423/423, definite bytes 6240, indirect bytes
  41388; post-suppression errors/contexts and definite/indirect bytes are zero,
  with 427 suppressed errors/contexts. Both raw and suppressed receipts remain
  attached to the machine summary. The sole remaining XFAIL is the pre-existing
  Ghostty async-backend ABI representation contract; no suppression was
  broadened for this slice.
