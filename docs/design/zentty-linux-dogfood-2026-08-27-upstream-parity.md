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

## GH-118 parent-level JSON for list subcommands

### Source contract and first failure

Upstream commit `089b1f2f` made `--json` inherited by the Swift `list` parent,
so both `zentty list --json panes` and `zentty list panes --json` are public
forms. The same ordering applies to `windows` and `worklanes`, and parent
options must continue to compose with output versions and filters.

The new source fixture was written before the parser repair. It failed with
`unexpected argument "windows"` for `list --json windows`, proving that the
Linux parser incorrectly assumed the resource was always the first token.

### Repair and negative coverage

`parse_list` now performs one state-aware scan. It removes exactly one known
resource while preserving flags and their values in their original order for
the existing discovery validator. Critically, a value such as `windows` after
`--window-id` remains a value rather than being mistaken for the resource.
There is no second parser or compatibility shim.

The focused tests cover both orderings for all three resources, aliases,
filters, and output version 1. Negative cases reject duplicate JSON and output
version flags across positions, two resources, unknown resources, misplaced
values, missing option values, and unsupported output versions. Environmental
absence was not converted into a pass: the first package-wide sandbox run was
denied while creating the existing private Unix sockets, and the first nested
journey was rejected because the headless wrapper cannot provide its required
physical-input environment. Both were rerun in the required controlled
environment.

### Receipts and dogfood policy

- Focused parser test first failed, then passed.
- `cargo test -p zentty-agent-ipc`: PASS, including private discovery sockets,
  parser, transport, credential, schema, and product-command suites.
- Focused `rustfmt`, package clippy with warnings denied, and `shellcheck -x`:
  PASS.
- `linux/tests/nested-wayland-input linux/tests/rust-cli-contract`: PASS with
  staged ReleaseSafe Zentty, real Ghostty surfaces and PTYs, the real staged
  CLI, authenticated socket routing, aliases, schemas, reviewed text output,
  physical input, and fail-closed negative cases.
- Feature inventory after reconciliation: 63 entries, 44 IMPLEMENTED,
  7 PARTIAL, and 12 NOT_IMPLEMENTED.

The installed dogfood product was not replaced or restarted. Operator QA is
now deliberately periodic and batched: each issue still receives focused
automated and real-product integration evidence, while visible deployments are
held for an explicit QA stop so normal dogfooding work is not interrupted after
every feature.

## GH-119 Ghostty-compatible Linux shortcut preset

The source commit `984e3076` adds a macOS Ghostty-compatible preset. Linux was
derived instead from Ghostty's real Linux defaults at the revision pinned by
`linux/ghostty.lock`, `80054768edbffd5df8568782e528363033a49192`; the exact mapping and deliberate
deviations are recorded in `ghostty-linux-shortcut-preset-v1.md`. Logical
character keyvals follow non-US layouts, while arrows, Tab, and F11 remain
physical. F11 is retained rather than copying Ghostty's Ctrl+Enter fullscreen.

The Settings header now exposes an accessible Ghostty preset action with an
explicit destructive-replacement confirmation. Acceptance constructs the new
manager before calling the existing persistence callback, so a conflict cannot
partially replace the live or stored registry. Left- and right-hand mapping
functions were not changed. Diff review caught that placing a sixth action in
the already dense one-row header would reintroduce Settings clipping; search is
now full-width above an aligned action row, with physical traversal re-proved.

The first focused compile rejected the missing exhaustive `HeaderAction`
variant; it was added without a fallback match. The first real journey applied
the preset but incorrectly expected normalized TOML to repeat the default F11
binding. The assertion now proves there is no fullscreen override or unbind and
no Ctrl+Enter displacement. The next run observed GTK returning focus to the
search entry after AlertDialog teardown; traversal was anchored to that real
focus rather than adding a timer or product focus hack. The final delivery
receipt initially used a fixed count across the journey's intentional product
restart/log truncation; it now measures the pre-action count.

Focused Rust tests, rustfmt, clippy with warnings denied, and shellcheck pass.
`linux/tests/nested-wayland-input linux/tests/rust-shortcuts-settings` passes
with real GTK Settings, confirmation, normalized XDG persistence, restart,
physical Ctrl+Shift+O input, Zentty action routing, and a new real Ghostty
surface/PTY. The installed dogfood application was not replaced or restarted;
operator review remains part of a later explicit batch QA stop.

## GH-120 named Move Pane destinations

Upstream commit `4b70b32f` changed Move Pane destination rows to prefer the
worklane's stable custom title over a mutable pane title. The Linux destination
catalog now carries a trimmed optional worklane title and renders it for both
single- and multi-pane destinations. Blank or absent titles retain the existing
pane-label fallback and `+N more` count; ordering, color, identity, exclusion of
the source worklane, and activation routing are unchanged.

The focused test was written first and failed with `shell` where `Release` was
expected. New cases cover trimmed titles, single and multiple panes, blank and
untitled worklanes, duplicate pane titles, fallback counts, and color retention.
Clippy then rejected the enlarged GTK menu builder, so pointer/focus receipts
were extracted into one focused helper rather than suppressing the lint or
creating another menu implementation.

The real journey exposed two test-design defects. First, unauthenticated
instance discovery correctly refused a mutating rename; the setup now obtains
the product-issued pane capability from the staged CLI and performs the rename
inside that pane-scoped environment. Second, waiting for a guessed diagnostic
log line was brittle even though the topology had changed. The journey now
reads the public `list worklanes --json` result through the same capability and
requires `worklane-2` to report `Backend Services` before exercising the UI.

Focused Rust tests, changed-file rustfmt, package Clippy with warnings denied,
and ShellCheck with sourced helpers pass. The existing controlled nested-X11
sidebar-management journey passes with nine real Ghostty PTYs: it reads the
rendered `Backend Services` destination using a real pointer, activates that
exact row, and proves the live source PTY moved to the stable destination ID.
Feature inventory after reconciliation: 63 entries, 46 IMPLEMENTED, 5 PARTIAL,
and 12 NOT_IMPLEMENTED. The installed dogfood application was not replaced or
restarted; this feature remains queued for the next explicit batch QA stop.

## GH-121 Settings-local Ctrl+W lifecycle

The current source handles Close Window against the active Settings toplevel.
Linux already had one capture-phase controller owned only by Settings, but it
handled Escape alone. Exact Ctrl+W now enters that controller and calls the
window's existing `close()` path, so the established close-request handler
remains the single owner of hiding Settings, presenting the parent, and
restoring terminal focus. The global shortcut registry and main-window router
were not changed, and no timer was added.

The focused test was written first and failed because the Settings-local chord
predicate did not exist. It now accepts Ctrl+W with lock-state noise and rejects
bare W, Ctrl+Shift+W, Ctrl+Alt+W, Ctrl+Super+W, and Ctrl+Q. Focused rustfmt,
Clippy with warnings denied, and ShellCheck pass. The staged ReleaseSafe build
also passed the Cargo publish-age audit before the real journey.

The controlled Wayland journey exposed three orchestration assumptions after
the product behavior itself passed. Reopening through the command palette had
to use the live left-preset Ctrl+X binding rather than the default Ctrl+Shift+P;
the reopened window starts on General rather than retaining Shortcuts; and GTK
visibility precedes compositor activation, so sending Ctrl+2 too early reached
the main window. The repaired journey accepts the current palette chord,
selects Shortcuts from an explicit focus anchor, and waits for the real GTK
activation receipt instead of sleeping.

The final journey physically proves Ctrl+Shift+W leaves the focused Settings
search control alive, Ctrl+W closes only Settings, the main window and child PTY
survive and regain focus, main-window Ctrl+W still invokes the configured
`pane.focus.up` action, and the same Settings toplevel reopens and completes the
existing preset, chooser, theme, reload, persistence, and raw-byte journey.
Feature inventory after reconciliation: 63 entries, 47 IMPLEMENTED, 4 PARTIAL,
and 12 NOT_IMPLEMENTED. The installed dogfood application was not replaced or
restarted; operator QA remains queued for the next explicit batch stop.

## GH-122 width-aware Clean Copy and real newlines

Upstream commit `0d0a3eff` established that libghostty already rejoins terminal
soft wraps, so Clean Copy must not infer that every newline in a paragraph is a
wrap. The Rust pipeline now carries one width-evidence value from entry to
agent prompts, separated bullets, lists, and ordinary prose. The public
compatibility entry point uses longest-line fallback; the live product calls
the same pipeline with the selected Ghostty widget width divided into native
cell widths. Missing or invalid metrics fail to `None` rather than inventing a
column count. No Ghostty ABI expansion or second transformer was introduced.

The translated source tests failed first because the width-aware entry point
did not exist. They now cover systemd units, key/value and colon records,
section headers, comments, prose beginning with `DEBUG=1`, short real newlines,
unknown-width fallback, and lines reaching or exceeding the terminal width.
Three older positive prose fixtures initially failed because they had asserted
wrapping without a terminal width; they now supply their intended narrow-pane
context while retaining their original expected text. The complete 32-case
Clean Copy target remains green.

Clippy rejected the first bounded float-to-column cast. It was replaced rather
than suppressed: positive finite cell widths are counted across the integer
GTK allocation, preserving fractional metrics without a lossy cast. Focused
Linux tests pin exact, fractional-boundary, zero, sub-pixel, and NaN cases, and
both core and Linux Clippy pass with warnings denied.

The first controlled X11 clipboard run supplied a real `columns=71` receipt but
found a mixed-selection false positive: a prose paragraph triggered the shared
flatten pass, which then joined a long `ExecStart=` record to
`RemainAfterExit=` even though a record-only selection was already protected.
The single paragraph flattener now returns all-structured paragraphs verbatim,
and a focused mixed prose/systemd regression test pins that repair.

The final clipboard-only journey passes with a real Ghostty Select All, exact
Copy Raw program bytes, a numeric live column receipt, exact width-aware Clean
Copy bytes read by external `xclip`, primary selection, paste back through the
real PTY, and two-window clipboard isolation. `Summary:` keeps its real newline,
width-reaching prose folds, and every systemd record newline survives. Feature
inventory after reconciliation: 63 entries, 48 IMPLEMENTED, 3 PARTIAL, and 12
NOT_IMPLEMENTED. The installed dogfood application was not replaced or
restarted; operator QA remains queued for the next explicit batch stop.

## GH-123 local Codex activity-title animation

Upstream keeps Codex pane identity stable while animating the exact Braille
activity token in the sidebar and focused window chrome. Linux previously
normalized every recognized spinner glyph to one dot before presentation.
That prevented animation and, on review, also erased later literal Braille
characters from subjects such as `Working ⠋ preserve ⠸ literal`.

The repair separates the two concerns. `zentty-core` now identifies only the
standalone token immediately after Working, Thinking, or Starting and replaces
only that token in stable identity. A focused Linux coordinator retains raw
eligible titles solely in per-window ephemeral memory. One GTK compositor
frame-clock callback renders deterministic 100 ms frames directly into the
existing sidebar labels and focused chrome label. It never writes a frame into
`WorkspaceState`, navigation history, persistence, terminal metadata, project
context, or pane-drag identity. There is no timeout, per-label clock, or second
agent-status system. GTK reduced motion holds frame zero and stops repainting.

Eligibility requires an existing local pane, no custom title, a canonical
Codex status in Starting or Running, and the exact token grammar. Remote panes,
other tools, idle/needs-input state, custom titles, malformed delimiters, and
literal Braille remain static. Title ownership changes remove the entry; live
surface removal clears it directly; the frame callback prunes tool, remote,
custom-title, and lifecycle changes and returns `Break` when the final entry
ends. Window destruction drops the owning widget and weak shell reference.

The first sandboxed X11 attempt failed before product launch because the
sandbox exposed `/tmp/.X11-unix` as `nobody:nogroup`; no product result was
claimed. The same existing `nested-x11` actor was rerun with GUI permission and
created its private Xvfb/Xauthority session normally. Its controlled PTY now
emits exactly one `Working ⠋ Bro` OSC title. The real GTK frame clock must later
render `Working ⠏ Bro`, both the existing sidebar label and focused chrome
label must accept the generated frame, the open Agent Status popover must
survive, the sidebar-card count must not change, and an authenticated idle
event must stop the callback. That focused journey passed with two real GTK
windows, two real Ghostty PTYs, authenticated agent IPC, and deterministic
widget-render receipts.

Focused parser/frame tests, animation ownership/reduced-motion/teardown tests,
the chrome stable-summary projection test, Bash syntax, strict all-target Linux
Clippy, and the controlled X11 fleet journey pass. Feature inventory after
reconciliation: 63 entries, 49 IMPLEMENTED, 3 PARTIAL, and 11 NOT_IMPLEMENTED.
The installed GNOME dogfood application was not replaced or restarted. Human
judgment of the animation's speed, color, and visual polish remains
intentionally queued for the next operator batch; this is not a claim of
operator visual acceptance or broad Linux qualification.

### GH-132 supersession note

Subsequent installed GNOME dogfood supplied the missing human judgment: the
Linux choice to replace complete GTK label text every 100 ms visibly flickered
in window, worklane, and pane projections, and could duplicate stable and
animated spellings in the window context. Upstream's custom CoreText view
retains its layout and paints animation independently, so the earlier Linux
implementation was not behaviorally equivalent despite its state-safety.

GH-132 supersedes only that projection detail. Stable labels now retain layout
and accessibility, a fixed-width pane activity cell alone receives spinner
frames, worklane context is not rewritten per frame, and focused chrome remains
semantic and stable. The shared frame clock, title ownership, parser, and agent
state contracts remain unchanged. See
`zentty-linux-dogfood-2026-08-27-title-flicker.md` for the repair and evidence.

## GH-124 real IME Backspace composition parity

The upstream repair is AppKit key-routing code, so copying it into Rust without
a demonstrated GTK failure would create a second keyboard owner. The existing
Linux actor already used real Cangjie5 preedit, commit/cancel, cross-pane focus,
and active-preedit pane destruction through controlled IBus and Fcitx sessions.
Its missing assertion was physical Backspace during active preedit and raw PTY
evidence that the key was consumed.

Pane 2 now enters raw/no-echo mode for one bounded probe. Physical Cangjie
input commits one `日`, starts a second `a` preedit, sends Backspace, then
commits another `日` and Return. The child requires exactly seven bytes:
`e697a5e697a50d`. A leaked DEL (`7f`) or BS (`08`), loss/corruption of the first
composed character, a failed subsequent commit, or a non-CR Return all fail the
same receipt. The actor then retains its prior independent focus-transfer,
refocus composition, active-preedit destruction, surviving-pane composition,
and exact receipt-count assertions. No sleep or product-side interception was
added for Backspace.

The unchanged staged ReleaseSafe product passed all four presently executable
real cells:

- IBus 1.5.29-rc2 / Cangjie5 on private Xvfb/X11;
- IBus 1.5.29-rc2 / Cangjie5 on private Cage/Wayland;
- pinned Fcitx 5.1.7 GTK module / Cangjie5 on private Xvfb/X11; and
- pinned Fcitx 5.1.7 GTK module / Cangjie5 on private Cage/Wayland.

Each used GTK 4.14.5, Ghostty revision
`80054768edbffd5df8568782e528363033a49192`, a real Ghostty surface/PTY, and a
wrapper-owned private input-method service and compositor/display. Every cell
reported `preedit-backspace exact-bytes commit focus-transfer
active-preedit-destruction real-pty`. Current pinned Ghostty GTK therefore
already satisfies the source behavior; there is no evidenced Ghostty or Rust
defect to patch. Environmental provider absence remains governed by the
existing matrix/wrappers and is not converted to PASS.

Feature inventory after reconciliation: 63 entries, 50 IMPLEMENTED, 2 PARTIAL,
and 11 NOT_IMPLEMENTED. The installed GNOME dogfood application was not
replaced or restarted, and no broad qualification was run or claimed.
ShellCheck and the feature-inventory runner passed. The broader
`test-orchestration-contract` again reported its documented pre-existing
`staged-shell-integration` inline-agent finding; GH-124 did not touch that
actor or add an integration layer, so the unrelated failure was recorded but
not folded into this behavioral issue.

## Operator repair: Shortcuts Settings layout and palette

The first installed parity batch exposed a page-level visual defect rather
than the previously tested single keyboard-width condition. Shortcuts forced
`#17191d` and `#202329` surfaces inside the otherwise light Settings window,
centered naturally oversized command labels inside a fixed browser width,
kept all six header actions in one rigid row, and allowed the detail pane to
hide overflow behind a horizontal scrollbar. The operator observed clipped
left labels, a clipped keyboard, contradictory light/dark surfaces, and
padding unlike every adjacent Settings page.

The page now inherits the Settings/libadwaita palette. Browser and keyboard
cards use named palette colors, selected rows use the named accent foreground
and background, and error text uses the named error color. Command labels are
left-aligned and ellipsized. Keycaps can shrink inside a horizontally
non-scrolling detail viewport; supporting labels wrap. The six header actions
use an explicit three-column/two-row grid so all actions remain visible and
physically traversable at the default width. Page margins now match the other
Settings content rather than the prior isolated dark panel.

The first real nested-Wayland journey proved the corrected keyboard geometry
(`viewport=470 content=470 keyboard=414 fits=true`) but failed when the rigid
header made Reset unreachable after Export. The responsive grid repaired that
failure. A later run reached an existing asynchronous AlertDialog mapping race
where an immediate synthetic Return preceded the response; no product timer or
focus hack was added. The final unchanged journey passed end to end with real
GTK Settings, physical traversal, native import/export choosers, persistence,
restart, a real Ghostty surface/PTY, and a final geometry receipt of
`viewport=410 content=410 keyboard=354 fits=true`. Six focused shortcut tests,
strict Linux Clippy, direct changed-file rustfmt, and the staged ReleaseSafe
build passed. This is focused evidence, not broad qualification.

## Operator repair: true Move Pane cascading submenu

The operator observed that **Move Pane to Worklane** destroyed the pane menu
and replaced it with a separate one-item destination page. Source inspection
confirmed macOS uses a real `NSMenuItem.submenu`; Linux explicitly called
`parent_popover.set_child(...)`. The replacement page was functional but was
not source-parity desktop behavior.

Linux now keeps the parent context menu intact and opens a distinct
right-positioned child `GtkPopover` parented to the Move Pane row. A normal GTK
button opens that nested surface explicitly; an attempted `GtkMenuButton`
inside the already-open custom popover did not activate in the controlled
journey and was removed rather than patched with a gesture workaround. The
submenu receives focus after the current GTK activation queue drains, without
a timer. Destination activation first dismisses the nested and parent surfaces
and dispatches selection/movement only from the parent's real `closed` signal.
This prevents menu refresh from preserving a visible but stale one-pane
catalog during a transfer.

The existing X11 sidebar actor was extended rather than adding a harness. Its
first revision correctly rejected the unactivated `GtkMenuButton`. The next
proved parent and submenu visibility plus a named live-PTY transfer, then
exposed stale repeated-open state. Event-driven close ownership repaired that.
A controlled screenshot proved the final New Worklane row was visibly
rendered; the remaining failures were harness mistakes: an activation claim
was asserted before the click, native popovers were scanned in client-window
coordinates, and eight Tabs wrapped an eight-item submenu instead of selecting
its last item. Those requirements were corrected without weakening product
assertions; the temporary screenshot hook was removed.

The final actor passed with nine real Ghostty PTYs, real pointer activation of
a named destination, simultaneous parent/submenu visibility, stable rows,
repeated reopening, physical keyboard traversal to **New Worklane in This
Window**, exact pane movement, and live PTY continuity. ShellCheck, strict
Linux Clippy, direct changed-file rustfmt, and the staged ReleaseSafe build
passed. The installed dogfood application was not replaced or restarted, and
no full qualification was run or claimed.
