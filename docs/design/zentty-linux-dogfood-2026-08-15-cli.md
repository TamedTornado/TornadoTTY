# Zentty Linux dogfood — CLI, topology, and shell integration

This report owns discoveries, failures, repairs, receipts, and remaining
uncertainty for GH-22. The design and closure contract are in
`linux-cli-topology-shell-integration-plan.md`.

## Source and existing-system audit

- The retained macOS product ships a real `zentty` companion in `ZenttyCLI/`.
  Its public topology vocabulary is `version`, discovery/list aliases, pane
  selection, directional splits, fixed grids, layouts, pane operations,
  worklane title/color operations, theme commands, notifications, servers, and
  agent integration commands.
- The Linux staged `zentty` binary is not a blank slate. It already owns real
  agent-event adapters, Codex notification forwarding, development-server
  commands, agent launching, and the internal tmux compatibility client.
- Linux already has one private `AgentRuntime` Unix socket and pane capability
  registry. It currently accepts only agent-event, server, and tmux routes.
  Adding a second CLI socket or translating commands through GTK automation
  would be architectural drift and is prohibited by the closure plan.
- The source uses `discover` and `pane` wire kinds. The Linux transport's
  versioned envelope already matches the source-shaped fields but correctly
  rejects those two unimplemented routes today. That rejection is the red
  boundary for the next test-first slice.

## Red parser and protocol boundary

The first focused test encodes source discovery aliases, canonical mutation
vocabulary, grid command argv preservation, selector ambiguity, dimension and
layout bounds, unknown routes, and request/reply size ceilings. It failed to
compile only because the planned `ProductIpc*` protocol and parser exports did
not exist. This is the intended pre-production red state; no fake socket or
test-only product implementation was introduced to satisfy it.

The first implementation run caught one genuine parser defect: pane titles are
positional source arguments, but rename option validation initially re-read the
title as an unknown option. Four of five focused tests passed and the exact
rename case failed. Validation now starts after the positional title while
retaining the title/`--clear` exclusivity check.

## First real product journey

The existing tmux/product actor was extended rather than adding another GUI
runner. Its first X11 run proved real version/discovery JSON, pane/worklane
rename, worklane color, authenticated split, token-bearing discovery, and
cross-pane close. The first 2x2 grid also created four real Ghostty surfaces in
the correct two-column/two-row geometry. Immediately afterward the product
aborted on a `RefCell` reborrow: product command completion manually iterated
the GLib context while already executing inside the coordinator's GLib tick,
allowing the live-snapshot callback to re-enter the mutably borrowed
coordinator. This was a product lifecycle defect in the new route, not a test
failure. The manual iteration was unnecessary and has been removed; the sole
main loop resumes naturally after the reply is returned.

The corrected run passed the new CLI assertions but failed a pre-existing tmux
receipt that intentionally names the first generated panes `pane-2` and
`pane-3`. Running the CLI split/grid setup before that legacy sequence consumed
those monotonic IDs, so the tmux behavior passed with later IDs while its exact
receipt correctly rejected the drift. The CLI sequence has been moved after
the identity-sensitive tmux checks. The production ID allocator was not reset,
special-cased, or made test-aware.

That ordering exposed a second test-only assumption: the final tmux teardown
assertion hard-coded `pane-4` and `pane-5`, even though their identities are not
part of the behavior under test. The new public CLI legitimately consumes IDs
between the identity-sensitive teammate sequence and final teardown. The child
now records the two pane IDs returned by the real final tmux commands, and the
outer receipt requires teardown of exactly those IDs. The product allocator and
the earlier source-relevant `pane-2`/`pane-3` evidence remain unchanged.

The corrected actor now passes end-to-end under both controlled displays. The
X11 receipt runs in the private Xvfb harness; the Wayland receipt runs the same
staged CLI and application inside the private Cage compositor. Both invoke the
real CLI subprocess from a real Ghostty PTY and observe actual topology changes
and teardown. This establishes the first public topology slice, not completion
of GH-22: new-window grids, exact split sizing, zoom, percentage resize,
notifications, integration management, hostile-data goldens, and the remaining
negative/recovery journeys are still open work.

The first architecture check rejected the new pane-token lookup because the
agent-event coordinator's exact function inventory had not been reconciled.
Discovery obtains capabilities through that existing sole coordinator rather
than reaching into a second registry. Its ownership manifest now explicitly
includes the read-only lookup, keeping the contract and implementation in sync.

Reviewing selector semantics before committing the first slice found a serious
gap: authentication resolved the token's real pane, but the shell did not yet
reject every contradictory selector, and positional numeric pane selection was
treated as an opaque pane ID. The real-product actor now requires a caller-token
attempt against another pane to fail without mutation and exercises numeric
selection with that pane's actual capability before authorized close.

The split/resize audit found that Rust had incremental cell-based resizing but
no absolute vertical fraction primitive. The macOS source represents pane
heights as relative weights, clamps the requested share to 5–95%, and preserves
all other panes' relative proportions. A focused core test was red on the
missing method before the same bounded weight transform was added to the sole
`WorkspaceState` owner.

The first public split route created the correct real pane but ignored
`--equal`, `--golden`, and `--ratio`; percentage resize was still an explicit
placeholder. The handler now applies the source's post-split layout semantics
to the newly focused pane and absolute horizontal resize uses the materialized
viewport plus the existing runtime-derived minimum width. The staged actor
exercises equal horizontal split, 60% resize, a 65% vertical split, and the
theme command's source-compatible resulting-mode stdout.

Notification construction began with a focused red parser case: the existing
Linux binary recognized only `codex-notify`, so the public `notify` command
fell through to the generic IPC usage error. The parser now requires and trims
the source title, omits blank optional subtitle/body values, preserves embedded
newlines, and carries the independent inbox and sound flags over the bounded
authenticated pane route. Delivery is not yet claimed by this parser receipt.

The first inbox implementation exposed a lifecycle collision: the shared inbox
previously assumed every item was derived from live agent state, so the normal
next reconciliation with no agent status immediately resolved a manual pane
notification. A red core test reproduced it. Items now carry a private origin;
agent reconciliation resolves only agent-origin entries, while explicit
activation, stale-target cleanup, dismissal, and clear still apply uniformly.
This extends the one inbox rather than creating a parallel notification store.

Pane notification delivery now routes at the application owner: the existing
freedesktop service receives the exact title/body and an explicit
`suppress-sound` hint for `--silent`, while the existing shared inbox records
the source entry unless `--no-inbox` is present. Desktop-service absence is
logged as unavailable, not converted into proof of delivery; the staged
topology actor presently proves both inbox branches and the real private socket
route. A controlled real D-Bus receipt remains required before closure.

The expanded actor found a target-versus-focus defect before reaching its
negative selector assertion. The shell eagerly focused the authenticated pane
for every product command, so `theme dark` invoked from the source shell stole
focus back from the pane just created by `split`. The source theme handler does
not focus its routing context, and neither should rename/color/close. Target
existence and authorization are now validated without selection; only commands
whose operation is inherently focus-relative (split, focus, resize, layout,
grid) select their authenticated target. This also prevents an unauthorized
selector rejection from changing UI focus as a side effect.

New-window grid design exposed two pre-existing multi-window hazards before the
feature was exercised. Pane/worklane counters were window-local but emitted
global-looking `pane-N`/`worklane-N` IDs, allowing a split in window 2 to reuse
window 1's pane ID and retarget its capability. Non-primary windows now emit
window-qualified opaque IDs and skip any restored collision. Also, closing the
last pane in any window directly quit the process; it now delegates to the one
application window-close coordinator, so other windows and their PTYs survive.
The primary window retains its established IDs for compatibility receipts.

The completed new-window actor creates a real 2x2 grid in `window-2`, discovers
all four live PTYs through the public CLI, requires their window-qualified IDs
and capabilities to be unique, closes them using those exact capabilities, and
then proves `window-1` and its shell remain alive. The same staged ReleaseSafe
binary and CLI passed this journey under the controlled X11 and Wayland
compositors. This is direct evidence for multi-window grid lifecycle, including
the two identity/last-window repairs above; it is not a claim that all GH-22
commands are complete.

The source audit also caught a representation mismatch hidden by the first
theme receipt. Linux persists automatic mode as `automatic`, while the macOS
public CLI intentionally prints `auto`. A separate `cli_token` projection now
keeps the persisted schema private, and the core test locks the source-facing
spelling. New-window presentation failure now tears down the registered shell
and window rather than leaving a headless, discoverable destination behind.

Presently executable receipts after these repairs:

- `cargo test -p zentty-agent-ipc --locked`: 45 tests passed across parser,
  helper/launcher subprocesses, authenticated real Unix sockets, bounds,
  concurrency, timeouts, and product transport.
- focused workspace and inbox suites: 51 and 12 tests passed.
- architecture contract plus negative self-tests: passed.
- staged `rust-tmux-compat-product`: passed under controlled X11 and controlled
  Wayland, including exact split ratios, percentage resize, notification inbox
  policy, authenticated negative selection, current-window grids, and the new
  multi-window lifecycle.

A pedantic Clippy audit initially found that the new command dispatcher and
grid/window builders had grown beyond the repository's 100-line function
limit, along with avoidable formatting allocations. Those product paths were
decomposed into focused split, focus, metadata, resize, theme, grid parsing,
grid delivery, and window-snapshot helpers; the new findings are cleared. The
workspace-wide Clippy command is not recorded as passing: it still reports
older unrelated debt in the application tick/callback owner, attention view,
sleep inhibitor, window chrome, and one oversized workspace test. This slice
does not hide those findings or turn that existing absence into a clean lint
claim. The post-decomposition staged X11 and Wayland journeys both passed
again, so the structural repair did not replace the real-system receipt.
