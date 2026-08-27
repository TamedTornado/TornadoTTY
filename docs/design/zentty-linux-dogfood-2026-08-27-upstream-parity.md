# Zentty Linux dogfood: upstream 0.2 parity discovery

Date: 2026-08-27  
Trackers: GH-115 through GH-124

## Discovery

The operator noted that the macOS product had changed since the Linux fork and
required renewed parity. The locally cached `upstream/main` still pointed at
our old source baseline, so relying on the checkout without fetching would have
reported no change. After an explicit fetch, upstream moved from `6e021b0d` to
`0d0a3eff`, with 35 commits and release `v0.2.0`. Four newly visible topic
branches initially looked like possible extra scope; left/right revision counts
proved all four were already merged into `main`.

The first release query requested a JSON field unsupported by the installed
GitHub CLI. It failed without changing repository state and was rerun with the
CLI's advertised supported fields. The release body was then reconciled against
its `v0.1.45` comparison base rather than treated as a literal list of changes
after our later fork.

## False-green findings

The authoritative feature inventory still names source head `6e021b0d` and
marks tmux compatibility, Clean Copy, shortcut presets, CLI JSON, sidebar
chrome, and worklane titles IMPLEMENTED. Those claims were valid only against
the old source contract. Updating the hash alone would create false parity.
GH-116 therefore owns both the source merge and status demotion until the new
children pass.

Direct code inspection found concrete gaps before any implementation began:

- Linux's tmux split parser retains a bare `--`, despite its respawn path having
  a separate delimiter implementation.
- `zentty list --json panes` is not equivalent to `list panes --json`.
- Move Pane destination labels ignore an available custom worklane title.
- Linux has no Ghostty-compatible shortcut preset and Settings lacks Ctrl+W.
- Clean Copy is not given the real terminal width used by the new source
  algorithm.
- Linux deliberately stabilizes Codex's Braille spinner rather than rendering
  the source's new local animation.
- Real Linux IME coverage does not yet exercise Backspace during preedit.

## Decisions

No product code was changed during this audit. GH-115 is the parent and GH-116
through GH-124 are test-first behavioral children. AppKit mechanisms were not
blindly promoted to Linux requirements. Forced transparent contrast, AppKit
cursor tracking, custom title-bar dragging, Apple display services,
notarization, and Fastlane have explicit platform classifications in the audit.
The Linux Ghostty fork was verified to be newer than the macOS GhosttyKit base,
so parity work must not downgrade it.

The checked-in Swift tree remains at the old baseline until GH-116. Fetched
remote objects are sufficient for this local discovery but not durable CI
evidence. No qualification was run and no current-source parity is claimed.
