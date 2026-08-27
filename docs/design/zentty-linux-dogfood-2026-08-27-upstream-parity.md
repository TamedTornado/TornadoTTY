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

## GH-116 source merge and ledger repair

Upstream `main` merged without conflicts and touched exactly 80 macOS,
GhosttyKit-build, release, and Swift-test files. It did not modify Linux product
code, Rust crates, Linux packaging, or Linux qualification orchestration. The
merge is therefore retained as a source-reference update rather than manually
copying selected Swift changes and losing provenance.

The first mechanical owner/status patch matched repeated JSON fields in three
adjacent inventory entries rather than their intended IDs. Inspection caught
the error before staging: `worklane.navigation-identity`,
`terminal.input-selection-scrolling`, and `configuration.toml-settings` were
restored to their existing implemented owners, and subsequent edits were
anchored by feature ID. No product code or committed history was affected.

The renewed ledger now records 63 features: 42 `IMPLEMENTED`, 9 `PARTIAL`, and
12 `NOT_IMPLEMENTED`. Five old broad green claims became explicit partial
claims under GH-117 through GH-120 and GH-122. Three narrow entries were added
for Settings-window lifecycle (GH-121), Codex activity animation (GH-123), and
active-preedit IME Backspace (GH-124). This is discovery of previously absent
scope, not a regression in the running Linux product.

The evidence ledger now names source `0d0a3eff`, release `v0.2.0`, all 40
audited releases, and exact child-issue ownership. The CLI source contract uses
the same source head. The inventory validator learned to validate a pending
merge through Git's actual `MERGE_HEAD` path, including worktrees, rather than
producing a false stale-head failure before the merge commit exists. Its
machine-summary expectations and negative tests were updated for the expanded
ledger; old tests that required the affected entries to remain green now
require their explicit partial status and new owner instead.

Focused receipts before commit:

- `linux/tests/feature-inventory`: PASS, 63 entries.
- `linux/tests/feature-inventory-test`: PASS, including negative cases.
- `linux/tests/cli-source-contract`: PASS, 40 commands, 40 source symbols,
  13 output contracts, and 6 schemas.

No product qualification or operator QA was run for this source-only merge.
The operator will continue using the installed product while the parity
children are implemented; visible changes will be deployed and reviewed in
meaningful batches rather than interrupting normal use after every issue.

## GH-117 tmux end-of-options and Claude teammate handshake

### Source cases and first failure

Upstream commits `f5fce0b6` and `b864e857` establish two separate contracts:
the generic parser consumes a bare `--` and preserves all following tokens
verbatim, while only the exact detached, printed, pane-ID-formatted, explicitly
targeted `-- cat` split is a deferred Claude bootstrap. Ordinary `cat` and
other commands still launch immediately.

Two source fixtures were added before implementation. The focused parser test
failed with actual positionals `["--", "cat"]` instead of `["cat"]`, proving
the old Linux defect. The parser repair is three control-flow lines: consume the
terminator, append the remaining iterator, and stop option processing. The
split planner separately recognizes the complete source signature; it does not
special-case arbitrary `cat` commands.

Inspection also found that Linux's existing `respawn-pane -k` runtime would
replace a live pane. The current source accepts respawn only for a pane awaiting
its deferred launch command. The runtime now rejects non-deferred panes before
surface removal, while the existing parser continues to require `-k`, an exact
target, a nonempty safe command, and to reject unsupported `-c`/`-e` options.

### Real journey repairs

The existing `rust-tmux-compat-product` journey was extended rather than
creating another harness. It now uses three distinct real teammate panes:

1. an ordinary immediate command;
2. the exact Claude `-- cat` bootstrap followed by a compound respawn;
3. the existing commandless deferred pane followed by send-keys launch.

The first unprivileged nested-X11 attempt failed before product launch because
the host `/tmp/.X11-unix` socket directory was unavailable to the sandbox. An
unprivileged headless-Wayland attempt was likewise denied while binding its
private socket. Neither was called PASS. The approved private Wayland run then
exposed two harness bookkeeping errors from the deliberate extra pane: teardown
still expected the two-pane list and the final command-count assertion still
expected seven `list-panes` calls. The first killed the child actor and left the
GUI to its 90-second safety timeout; the second failed after normal application
shutdown. Both were repaired by preserving exact four-pane teardown and eight
real observations. Moving the third pane until after the original layout checks
also retained the pre-existing equalization-mutation proof rather than weakening
it to accept a no-op.

### Receipts

- Source parser fixture first failed with `["--", "cat"]`, then passed.
- `cargo test -p zentty-tmux-compat`: PASS, 29 tests.
- `cargo test -p zentty-linux tmux_compat::tests`: PASS, 20 tests.
- Focused `rustfmt`, `shellcheck -x`, and package clippy with warnings denied:
  PASS. Clippy initially rejected the expanded 127-line planner test; the new
  bootstrap cases were split into their own focused test instead of suppressing
  the lint.
- `linux/tests/nested-wayland linux/tests/rust-tmux-compat-product`: PASS with
  staged ReleaseSafe Zentty, private headless Weston, real Ghostty surfaces,
  PTYs, authenticated CLI/socket, exact routing, and teardown.
- `linux/tests/nested-wayland-input linux/tests/installed-claude-agent-team`:
  PASS with real pinned Claude Code 2.1.201, staged shim, real CLI/socket,
  Ghostty, PTYs, teammate, physical input, and teardown; only the model endpoint
  was controlled.

No operator QA or installed-product deployment was requested for this
compatibility slice. The ordinary dogfood instance was not restarted.
