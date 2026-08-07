# Ghostty runtime-order and ELF ABI qualification plan

- **Status:** completed
- **Date:** 2026-08-07
- **Owner:** #11

## Problem

Two locally feasible Ghostty boundary cells are still declarations rather than
executable evidence. The public header requires runtime construction before GTK
initialization, but no fresh-process test proves the rejected order is safe.
The shared library has a linker version script, but the qualification matrix
does not prove that every exported embedding symbol carries its version node.

## Acceptance

1. A fresh-process C consumer exercises both initialization orders against the
   real staged Ghostty library under controlled Wayland and X11 sessions.
2. Runtime-before-GTK succeeds. GTK-before-runtime is rejected deterministically,
   leaves GTK usable, and exits without a crash, hang, or second runtime attempt.
3. The runtime-order harness is built with the same warnings, hardening, header,
   library, and runtime-path policy as the existing C API consumer.
4. A focused ELF audit proves the exact twelve exported functions are attached
   to `GHOSTTY_GTK_EMBED_1.0`, and rejects unversioned or wrongly versioned
   exported embedding symbols.
5. Runner self-tests cover missing artifacts and malformed version evidence;
   the authoritative rows become PASS only after the real staged artifacts pass.
6. Existing orchestration owns the cells. No second compositor/session runner
   or product harness is introduced, and no Ghostty change is made unless the
   real misuse journey exposes a contract defect.
7. Dogfood records the red tests, actual native behavior, repairs, exact
   receipts, and every remaining limitation before commit.

## Non-goals

- No claim that an incompatible old header/new library pair is qualified; that
  remains the separate `ghostty-abi-old-new-mismatch` cell.
- No attempt to make Ghostty process-global state restartable.
- No new public ABI or Rust-specific Ghostty API.
