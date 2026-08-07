# Linux Rust/Ghostty adapter qualification plan

- **Status:** completed
- **Date:** 2026-08-07
- **Owner:** #13

## Problem

The matrix still says that the delivered Rust product and safe Ghostty adapter
do not exist. They do, but that makes the declarations stale rather than
automatically passing: the raw Rust binding still declares three exports with
no safe/product caller, surface configuration validation is only partly
focused, and the real close/restore journey does not explicitly reject a
callback arriving after its safe owner is disposed.

## Acceptance

1. The existing Ghostty API audit gains a closed-world Rust-binding ledger.
   Every raw declaration retained by Zentty maps to one safe owner, real Linux
   product callers, and named real qualification evidence.
2. Raw declarations that exist only for the retired C probe or speculative
   paste control are removed from the Rust binding without changing Ghostty's
   language-neutral ABI in this slice.
3. Focused Rust tests prove exact command, title, working-directory, and
   environment encoding plus every invalid/NUL/count boundary before native
   construction.
4. Main-thread-only compile contracts remain, and a real physical-input pane
   close/restore journey rejects any init/title/progress/child-exit callback
   from the disposed pane after the close boundary.
5. Product-usage qualification runs the one canonical audit and existing real
   close/restore, search-binding, and tmux text-read journeys in controlled
   X11. It does not introduce another compositor, actor, or session harness.
6. The three #13 matrix rows become PASS only after those exact commands pass.
   Architecture and prose mirrors are reconciled with the authoritative
   matrix, and all remaining gaps stay explicit.
7. Diff review, complete presently executable qualification, strict Clippy,
   workspace tests, static contracts, and dogfood evidence precede commit.

## Non-goals

- No Rust-specific Ghostty ABI.
- No typed argv API unless source behavior proves that a string native command
  is insufficient; original Zentty itself uses a native command string.
- No broad Ghostty patch or API deletion solely to make a ledger green.
- No duplicate integration runner.
