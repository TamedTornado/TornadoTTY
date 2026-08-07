# Linux Claude Code workflow parity plan

- **Status:** in progress
- **Date:** 2026-08-07
- **Owner:** [#7 — essential Zentty workflow parity](https://github.com/TamedTornado/zentty/issues/7)
- **Inventory ID:** `agent.claude-code`

## Product contract

The checked-in Swift adapter, reducer, session store, launch bootstrap, and
restore tests define parity. Linux must run the installed Claude Code binary in
a real Ghostty PTY and carry its real hooks through the staged helper,
authenticated socket, canonical reducer, visible sidebar, persisted restore
draft, and exact `claude --resume <session>` relaunch. Only the remote model
response may be controlled.

## Remaining slices

1. Complete source hook mapping, structured question formatting, meaningful
   notification filtering, and the Stop/late-Notification race in the existing
   canonical adapter and reducer.
2. Add the bounded per-session correlation state required for task progress,
   richer interaction preservation, background hook routing, and SessionEnd
   cleanup. It must be private, atomic, symlink-safe, and concurrency-tested;
   it must not become a second agent-status store.
3. Reconcile Claude terminal-title idle/running presentation without weakening
   explicit questions or approvals.
4. Extend the existing installed-Claude journey from team creation to an
   ordinary completed turn, question/approval, Stop race, session persistence,
   and real resume. Do not create a second installed-product journey.
5. Run focused mutation, workspace/static checks, and every presently
   executable qualification cell. Promote `agent.claude-code` only after every
   named source behavior has executable evidence.

## Matrix convergence

Qualification is a gate, not a substitute backlog. After this feature slice,
reconcile the stale `product_pane_terminal_lifecycle` matrix family against the
already delivered Rust product tests: retain distinct evidence only where an
axis exercises distinct behavior, remove no requirement silently, and do not
rerun identical product journeys merely to inflate cell counts. Then implement
the next genuinely absent, locally feasible cells under their owning issues.

## Stop conditions

- No new agent daemon, status store, fake Claude binary, or parallel product
  journey.
- No user Claude settings are modified persistently.
- No inventory or matrix promotion based only on focused tests.
- No Ghostty change unless a missing behavior is proven to belong to its
  embedding boundary.
