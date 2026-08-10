# Linux remote paste port plan

Tracking: GH-17

## Source contract

This slice ports the existing macOS feature; it does not invent a file-transfer
product. The authoritative source is:

- `Zentty/UI/RemotePaste/RemoteImagePasteController.swift`
- `Zentty/Terminal/RemoteImageUploader.swift`
- `Zentty/Terminal/RemoteFileUploadRequest.swift`
- `Zentty/Terminal/RemoteImageUploadPath.swift`
- `Zentty/Terminal/RemoteImagePasteDecision.swift`
- `Zentty/Terminal/TerminalClipboard.swift`
- `Zentty/Terminal/LibghosttyView.swift`
- their `ZenttyLogicTests` coverage and commits `afa145b` and `d89b531`

The source behavior is:

1. Ordinary text paste remains Ghostty-owned.
2. Pasted or dropped image data and local file URLs are intercepted only for a
   pane classified as remote.
3. The current foreground SSH process is the preferred destination authority;
   source fallbacks exist for title, stored label, and remote shell context.
4. Images are capped at 10 MiB, files at 500 MiB, and folders are rejected.
5. A sanitized, randomized `/tmp/zentty-paste-*` remote name is generated.
6. Bytes stream through the standard input of a separate noninteractive
   `/usr/bin/ssh` process into remote `sh -c 'umask 077; cat > ...'`.
7. Multiple files upload sequentially in drop order. Only successful remote
   paths are inserted, shell-escaped and space-separated.
8. Progress and classified failures are presented, concurrent uploads in the
   same pane are rejected, and pane/controller teardown cancels work.

The source uploads automatically after the paste/drop gesture. It does **not**
show a separate approval dialog. Linux will preserve that UX unless a distinct
product decision changes both platforms.

## Source gaps versus Linux trust requirements

The source uses randomized names to make collisions unlikely, but does not
atomically reject an existing destination. It does not checksum the remote
result, remove a partial remote file after cancellation/failure, establish an
explicit symlink policy, or revalidate the pane's SSH process identity before
inserting the completed path. Its process tests use fakes rather than a real
SSH daemon.

These are documented hardening gaps, not invented source features. Linux may
strengthen them without changing the visible source workflow:

- create into a private temporary name without clobbering an existing path;
- verify byte count and SHA-256 before publishing the final remote path;
- trap failure/cancellation and remove the partial temporary object;
- require a regular local file opened without following a substituted path;
- bind completion to the pane, foreground-process generation, and destination
  that authorized the upload;
- never insert any local or remote path after a failed or stale transfer.

## Construction order

1. Port pure path, request-resolution, batch-result, error-classification, and
   transfer-plan policy into `zentty-core`, with source fixtures first.
2. Add a single Linux remote-paste coordinator beside the existing SSH identity
   coordinator. It owns GTK clipboard/drop interception, upload task lifecycle,
   progress projection, and completion validation; it does not own terminal
   emulation, the PTY, or workspace persistence.
3. Use a real `ssh` child with exact argv and piped bytes. Do not introduce an
   SSH library, second terminal layer, shell interpolation of local values, or
   another async runtime.
4. Extend the consolidated real SSH actor to paste and drop actual files and
   image bytes through controlled X11 and input-capable Wayland, compare remote
   SHA-256, observe the inserted escaped path in the real PTY, and prove
   cleanup/cancellation/failure paths.
5. Run focused mutation, strict Clippy, workspace tests, both compositor
   journeys, the Ghostty regression floor, and the authoritative matrix before
   commit and push.

## Acceptance boundary

The slice is not complete when only pure transfer tests pass. It is complete
when the real staged product receives desktop clipboard/drop content, streams
the bytes through a real OpenSSH client and disposable server, inserts only a
verified remote path into the same real Ghostty PTY, and proves negative paths
without a second product actor or test-only product mode.
