# Zentty Linux terminal UX closeout dogfood

Date: 2026-08-22
Tracking: GH-17, GH-8

## Discovery: GH-17's only PARTIAL entry was stale

- GH-17 owns terminal input/selection/scrolling, remote SSH transfer, global
  search, and clean/raw/Markdown clipboard behavior.
- Remote transfer, search, and clipboard were already `IMPLEMENTED` with real
  staged-product X11/Wayland journeys. The remaining terminal-input entry said
  it was waiting for GH-8 native smooth-input qualification.
- GH-8 closed on 2026-08-21. Its physical layout/repeat, IBus/fcitx IME,
  compositor resize, Wayland fractional-scale, X11 scale, and Xwayland scale
  cells are all authoritative `PASS`. The GH-17 inventory prose had not been
  reconciled afterward.

## Decision and repair

- No new terminal, clipboard, scroll, SSH, or search implementation was added.
  The maintained real-system evidence already satisfies the feature boundary;
  adding another actor would duplicate the existing pane-search,
  session-restore, remote-transfer, native-input, IME, and scaling systems.
- `terminal.input-selection-scrolling` is promoted to `IMPLEMENTED`, its stale
  GH-8 blocker is replaced with the exact passing evidence, and GH-17's issue
  state is reconciled as closed.
- This is tracker repair, not a claim that unrelated GH-16 cross-window drag or
  visual polish is complete. `pane.drag-drop` remains `PARTIAL` under its
  existing owner.

## Validation

- Feature-inventory schema and negative runner tests passed after updating the
  expected totals to **36 IMPLEMENTED, 13 PARTIAL, 11 NOT_IMPLEMENTED**.
- Architecture and qualification-matrix validators remain green.
- The authoritative qualification matrix remains **191 PASS, 3 XFAIL, 2
  NOT_IMPLEMENTED**; full Linux qualification is not claimed.
