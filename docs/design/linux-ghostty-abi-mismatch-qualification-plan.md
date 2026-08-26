# Ghostty old/new ABI mismatch qualification plan

- **Status:** completed
- **Date:** 2026-08-07
- **Owner:** #11

Current-state note (2026-08-26): GH-11 retained this real historical fixture
but strengthened it to prove both mismatch directions reject before `main`.
The separately tracked backend representation XFAIL mentioned below is now a
fixed-width PASS contract.

## Problem

The remaining Ghostty ABI compatibility row is prose-only. Zentty does not yet
prove what happens when a binary compiled against the current embedding header
is launched with an older, incompatible real Ghostty embedding library.

## Acceptance

1. The machine API inventory names one immutable historical checkpoint from
   the audited embedding series, its exact exported symbol set, the current
   checkpoint, and the exact new symbol used to force an incompatibility.
2. A hardened C consumer compiled and linked against the current staged header
   and ReleaseSafe library passes with the matching current library.
3. The same untouched binary launched with the real historical ReleaseSafe
   library fails in the dynamic loader before `main`, with a bounded receipt
   naming the missing versioned symbol. A marker proves no consumer code ran.
4. A historical-header consumer using only that header's common surface passes
   against the current library, proving the test distinguishes compatible
   forward loading from an incompatible new-consumer/old-library pairing.
5. Fixture construction is deterministic, local to ignored build output, and
   validates exact Git revisions, clean trees, artifact hashes, exported
   symbols, hardening, and runtime paths. It does not mutate the managed current
   checkout or invent a mock Ghostty library.
6. Runner self-tests reject missing artifacts, revision drift, an accepted
   incompatible pair, a consumer marker reached during failure, and a receipt
   for the wrong missing symbol.
7. Existing isolated-session and matrix orchestration own the cell. Dogfood,
   full executable qualification, diff review, commit, and push follow.

## Non-goals

- No claim of a general Ghostty ABI stability policy or versioned SONAME.
- No new public ABI solely to make mismatch detection easier.
- No repair of the separately tracked async-backend enum representation XFAIL.
