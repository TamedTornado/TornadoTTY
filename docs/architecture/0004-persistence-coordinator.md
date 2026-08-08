# ADR 0004: One synchronous persistence lifecycle coordinator

- Status: Accepted
- Date: 2026-08-09
- Tracking: GH-30, parent GH-25

## Context

Core already provides the only session schema and storage implementations:
`SessionRestoreStore` owns safe decoding, atomic replace, file and parent sync,
lifecycle markers, and clean-exit draft merging; `SnapshotPersistence` owns
stale-generation refusal. Linux `main` nevertheless sequenced startup and
clean-exit persistence directly.

## Decision

The process owns one `PersistenceCoordinator` around one
`SnapshotPersistence`:

- startup asks the existing store for its launch decision, validates the
  current one-window product limit, selects matching restore drafts, and marks
  the launch unclean before GTK/Ghostty product work;
- clean exit validates one frozen `WindowRecipe`, creates the existing envelope
  schema, submits one monotonic generation to `SnapshotPersistence`, and marks
  the lifecycle clean only after snapshot publication succeeds;
- clean-exit requests are single-flight and terminal after success or failure;
  reentrant and duplicate calls fail instead of creating another save path;
- all I/O remains synchronous and bounded by the existing store call. No queue,
  timer, worker pool, async runtime, dirty flag, journal, `.bak` file, or live
  snapshot cadence is introduced.

Closed-pane undo remains transient workspace state. Credentials, clipboard
contents, transcripts, pane capabilities, and ambient environment are not
members of the persisted recipe/envelope projection.

## Consequences

A snapshot write or directory-sync failure leaves the lifecycle unclean and is
reported to the composition root. A later launch therefore follows existing
crash-recovery semantics rather than falsely treating a failed save as clean.
The core store and its real-filesystem/interrupted-child tests remain the
authority for serialization and atomic-write behavior.
