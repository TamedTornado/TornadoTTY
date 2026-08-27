# Zentty upstream parity refresh — 2026-08-27

Parent tracker: [GH-115](https://github.com/TamedTornado/zentty/issues/115)

## Authority and range

The Linux port forked from macOS source commit
`6e021b0d3d80025900de1a7ab2f37cc6510da3fe` on 2026-07-26. Current
`dedene/zentty/main` is `0d0a3effb6decb1ea395e1758e6f042226255ac1`
on 2026-08-26. The audited range contains 35 commits, changes 80 files, and
reports 5,227 insertions and 567 deletions. Release `v0.2.0` is inside the
range. The fetched topic refs `chore/update-libghostty-tip`,
`feature/69-ghostty-shortcut-compatibility`, `fix/62-list-json`, and
`fix/71-ime-backspace` contain no commit absent from `main`.

This document classifies behavior, not implementation language. Swift/AppKit
tests define the source result; Linux must implement applicable behavior through
the existing Rust/GTK/Ghostty owners. A fetched remote ref is not a durable
checked-in authority, so [GH-116](https://github.com/TamedTornado/zentty/issues/116)
must merge the source/test range before the evidence ledger changes its
`source_head`.

## Required Linux work

| Source change | Linux finding | Status and owner |
| --- | --- | --- |
| `f5fce0b6`, `b864e857`: tmux `--` parsing and Claude deferred teammate handshake | The Linux respawn path recognizes a delimiter, but generic split parsing retains bare `--`; the exact `-- cat` bootstrap is not pinned and can become an immediate invalid command. | **MISSING** — [GH-117](https://github.com/TamedTornado/zentty/issues/117) |
| `089b1f2f`: parent `zentty list --json` | Linux accepts `list panes --json`, but `list --json panes` is parsed as overview options rather than a resource subcommand. | **MISSING** — [GH-118](https://github.com/TamedTornado/zentty/issues/118) |
| `984e3076`: Ghostty-compatible shortcut preset | Linux Settings exposes only left- and right-hand presets. Fullscreen already has the idiomatic and physically tested F11 binding, but the new preset does not exist. | **MISSING** — [GH-119](https://github.com/TamedTornado/zentty/issues/119) |
| `4b70b32f`: stable Move Pane destination names | Linux still derives every destination from its first pane title plus `+N more`, even when `SidebarWorklaneSummary.top_label` contains a custom worklane title. | **MISSING** — [GH-120](https://github.com/TamedTornado/zentty/issues/120) |
| `984e3076`: Settings window close equivalent | Linux Settings owns Ctrl+1, Ctrl+2, Ctrl+[ / Ctrl+], and Ctrl+F but not exact Ctrl+W. | **MISSING** — [GH-121](https://github.com/TamedTornado/zentty/issues/121) |
| `0d0a3eff`: Clean Copy real-newline preservation | Rust Clean Copy has broad structure heuristics but no surface column input and can flatten after a real short line merely because another line is long. | **MISSING** — [GH-122](https://github.com/TamedTornado/zentty/issues/122) |
| `5180a977`, spinner portion of `4cc16928`: local Codex activity animation | Linux intentionally replaces recognized Braille frames with a stable dot. Current source instead keeps stable identity while animating an eligible live local title in the sidebar and window chrome, with reduced-motion support. | **MISSING** — [GH-123](https://github.com/TamedTornado/zentty/issues/123) |
| `19efc39a`: IME Backspace composition | The source repair is AppKit-specific. Linux delegates GTK IME key routing to Ghostty and has real IBus/Fcitx composition coverage, but the existing journey covers Escape cancellation rather than active-preedit Backspace byte behavior. | **UNPROVEN PLATFORM BEHAVIOR** — [GH-124](https://github.com/TamedTornado/zentty/issues/124) |

## Platform reconciliations

| Source change | Classification | Rationale / Linux owner |
| --- | --- | --- |
| `4b4bb8a7`, `9596d8eb`: force `minimum-contrast = 1` on a transparent terminal | **NOT_APPLICABLE** | macOS forces every Ghostty surface fully transparent over an app-owned backdrop, making Ghostty's invisible-background contrast math incorrect. Linux exposes the user's actual Ghostty background opacity and does not install this forced-transparent override. Copying it would discard a valid Linux user preference. |
| `bfc47a33`, `9813b5dd`, cursor portions of `0d73a60d` and `4cc16928` | **PLATFORM_ALTERNATIVE** | These patches restructure AppKit `NSCursor` tracking areas and delayed cursor-rect invalidation. GTK/Ghostty owns the terminal cursor on Linux; Zentty overlays are required not to intercept terminal rows and divider/drag controls own only their own GTK cursors. Existing real-pointer tests and recent first-row dogfood cover the Linux boundary. The Codex spinner portion is separately required in GH-123. |
| `0d4fe462`, `613db386`: AppKit custom title-bar drag and first mouse | **NOT_APPLICABLE** | Linux keeps compositor/window-manager-owned native decoration as its window drag target. Zentty's internal context chrome is application content, not a replacement title bar. Making status text initiate compositor moves would invent a Linux interaction and conflict with clickable branch/review controls. |
| `8be5dcd5`: GhosttyKit update to official base `9f0e1719` | **ALREADY AHEAD** | Linux pins `TamedTornado/ghostty` revision `80054768`, rebased onto official base `ac04fc27` on 2026-08-26, and carries only the reviewed GTK embed patch stack above that base. Do not regress to the macOS lock. |
| `82cc3b02`, `4f8d39d0`: macOS virtual-display harness and display-service UUID initialization | **NOT_APPLICABLE** | These own XCTest/CoreGraphics virtual displays. Linux uses the existing private Xvfb and nested controlled Wayland/X11 harnesses. Behavioral children extend those owners rather than porting Swift test infrastructure. |
| `acbe00d1`: bundled dependency license refresh | **PLATFORM ALTERNATIVE** | The macOS bundle catalog changed with GhosttyKit. Linux package notice collection and Debian package audit derive notices from its independently pinned Cargo and Ghostty closure. GH-116 must retain the upstream files while Linux retains its own package evidence. |
| `fc214352`, `19d16d5e`, `bf3ca7f2`: Fastlane/notarization | **NOT_APPLICABLE** | Apple signing, DMG notarization, and Fastlane are macOS release mechanics. Linux Debian packaging remains independently audited. |
| `f2fcc819`: macOS 0.2.0/build 836 metadata | **SOURCE SYNC ONLY** | Retain upstream product metadata in the macOS project. It must not replace the Linux package's independent Git-derived development version. |

## Release-note reconciliation

The `v0.2.0` notes compare against `v0.1.45` (`aa494988`), not against our
later fork baseline. The following advertised changes were already present in
the source we forked and are not newly discovered scope: default/shared Ghostty
theme healing, vivid worklane-color selection, macOS local-network entitlement,
menu-bar theme colors, Cursor status continuity, IME candidate positioning and
pixel/point normalization, and runtime-directory IPC recovery. They remain
subject to the existing Linux inventory and tests; their appearance in the
release announcement alone is not evidence of a new gap.

## Order and claim boundary

1. GH-116 lands the source/test baseline and refreshes machine-readable claims.
2. GH-117 repairs the agent-team compatibility regression because it affects a
   substantive workflow and current Linux code is demonstrably behind.
3. GH-118, GH-120, and GH-121 close deterministic, bounded interaction defects.
4. GH-122 and GH-124 close terminal-byte correctness contracts.
5. GH-119 and GH-123 deliver the larger Settings and visual features with real
   GTK acceptance.

Each child extends its existing test owner. Focused tests and the directly
affected real product journey are required; full qualification is not. Source
parity must not be claimed until every required child is closed and the
refreshed inventory contains no affected PARTIAL or NOT_IMPLEMENTED entry.
