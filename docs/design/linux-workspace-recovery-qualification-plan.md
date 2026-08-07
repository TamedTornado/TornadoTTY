# Linux workspace recovery qualification plan

- **Status:** completed
- **Date:** 2026-08-07
- **Owner:** #3, #6

## Problem

Two release-tier recovery cells remain prose-only even though the Rust store
already uses temporary-file replacement and reports decode failures. The code
does not yet make the rename durable by syncing its parent directory, and the
tests do not interrupt a real writer process or distinguish source-compatible
future data from malformed/type-incompatible data.

## Acceptance

1. Begin red by making the orchestration contract require both recovery cells
   to be executable, isolated, self-testing matrix entries.
2. An atomic snapshot write uses a same-directory mode-0600 temporary file,
   writes and syncs it, renames it over the destination, and syncs the parent
   directory after the rename. No `.bak`, journal, or version-history feature
   is introduced.
3. A real child process is killed after publishing a temporary file. The
   destination observed afterward is always the complete prior or complete new
   JSON document, never partial JSON; a subsequent save succeeds without
   treating an abandoned temporary file as restorable state.
4. A real rename failure leaves the prior destination byte-for-byte intact and
   removes the failed writer's temporary file.
5. Malformed and type-incompatible snapshots return a bounded decode failure
   without modifying the bad evidence. Future recipe versions and unknown
   fields remain accepted, matching Swift `Decodable` behavior rather than an
   invented "unsupported version" policy.
6. The focused runner and core suite reject missing interruption evidence,
   over-permissive temporary files, partial publication, a changed destination
   after failure, a subsequent-save failure caused by an abandoned temporary
   file, and accidental future-version rejection.
7. The existing `SessionRestoreStore` remains the single persistence system.
   Run every presently executable matrix cell, review the diff, update dogfood,
   then commit and push only if qualification is green.

## Non-goals

- Exhaustive kill-at-every-filesystem-phase campaigns.
- Backup generations, `.bak` files, journals, or PTY adoption.
- Claiming the still-incomplete real product restore cells are qualified.
- Treating filesystem durability as guaranteed beyond the sync contract of the
  filesystem and storage stack used by the operator.
