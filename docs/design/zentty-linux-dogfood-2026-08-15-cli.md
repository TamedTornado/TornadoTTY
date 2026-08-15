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
