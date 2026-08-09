# ApplicationShell decomposition plan

Tracking epic: GH-25. Characterization gate: GH-26. Ordered implementation:
GH-27 through GH-31.

## Why this work exists

`crates/zentty-linux/src/application_shell.rs` is a single coherent product
system, but it has accumulated GTK action routing, live pane ownership, agent
event projection, persistence handoff, navigation, and UI rendering. Splitting
text into files is not the goal. The goal is explicit ownership with one
authority for each stateful responsibility.

The authoritative field, method, action, owner, lifecycle, and evidence map is
[`application-shell-responsibilities-v1.json`](../architecture/application-shell-responsibilities-v1.json).
The architecture gate compares that inventory to the actual Rust source and
the PASS evidence in `linux/qualification-matrix.json`. Any added or removed
field, method, or action therefore requires an intentional ownership decision.

## Non-negotiable authorities

- `ApplicationShell` remains the composition root and owns the sole live
  `WorkspaceState` for its window.
- GH-27 creates one action router. It owns registration, typed decoding,
  availability, shortcuts, and dispatch—not product state.
- GH-28 creates one pane runtime coordinator. It becomes the sole map from
  stable pane IDs to transient Ghostty surfaces, frames, focus controllers,
  callbacks, and child lifetimes.
- GH-29 creates one agent event coordinator around the existing sole
  `AgentRuntime` and core reducer. It does not introduce another transport,
  queue, reducer, worker runtime, or persisted status store.
- GH-30 creates one lifecycle coordinator around the existing
  `SessionRestoreStore`. It does not create backups, journals, generations of
  files, or a second serialization path.
- Existing tmux compatibility stays in its already focused pure crate and
  product runtime bridge.
- `main.rs` owns the one default GLib context/main loop and process scheduling;
  window-local timers and sources target that authority rather than creating a
  second executor.

## Construction and shutdown

Construction stays runtime-before-GTK, then one validated workspace, one agent
runtime, one action registry, and transactional surface creation. Shutdown
first stops actions/producers, cancels agent enrichment, freezes one snapshot,
quiesces and releases surfaces exactly once, lets native finalizers settle
without blocking, saves through the one store, and only then releases the
Ghostty runtime.

Ordinary pane close remains distinct from shutdown. Closed-pane undo remains
transient. A failed terminal constructor cannot commit an orphan pane or leave
a live surface outside the registry.

## Test construction order

1. GH-26 freezes the actual source inventory and existing real-boundary
   evidence. No production ownership moves in this issue.
2. Each extraction begins with a failing structural contract plus focused
   negative/lifecycle characterization.
3. Production code moves once. Superseded fields and routes are deleted in the
   same slice; forwarding facades may be transitional only within that slice.
4. Pure decisions receive focused mutation testing through
   `linux/tests/mutate-rust`; native ownership remains covered by ABI,
   compile-fail, lifecycle, and real-product tests.
5. Relevant real product cells run for the slice. GH-31 reruns every presently
   executable authoritative cell and performs the final duplicate-authority
   audit.

No child issue may add an aggregate runner, fake terminal, alternate product
mode, ambient-desktop dependency, hidden retry, or environmental pass.

## Final GH-31 audit contract

GH-31 hashes and inventories the process root, action router, pane runtime,
agent-event coordinator, tmux bridge, persistence coordinator, and window
shell together. Its machine map names workspace state, GTK action registry,
surface registry, session store, agent IPC transport, agent projection, tmux
compatibility state, and GLib scheduling separately and assigns exactly one
owner to each. High-level operations may coordinate multiple owners, but they
must not retain selectable legacy routes or duplicate the owned state.

The final audit is complete only after locked Cargo metadata and dependency
direction, structural negative tests, focused mutation campaigns from the
extraction commits, real staged-product journeys, and every presently
executable authoritative matrix cell pass. Required `BLOCKED`, `XFAIL`, and
`NOT_IMPLEMENTED` cells continue to prevent release/full qualification claims.
