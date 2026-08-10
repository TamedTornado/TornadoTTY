# Linux SSH pane identity slice

Tracking: GH-17 (`terminal.remote-ssh-transfer`)

## Outcome

Identify a live SSH foreground process owned by a real Ghostty pane, parse its
destination, and project the remote identity into the existing pane/sidebar
presentation. This slice does not reconnect sessions or transfer files.

## Source authority

- `Zentty/Terminal/PaneSSHProcessProbe.swift`
- `Zentty/AppState/WorklaneContextFormatter.swift`
- `Zentty/AppState/PaneDisplayIdentityResolver.swift`
- `Zentty/UI/RemotePaste/RemoteImagePasteController.swift`
- `ZenttyLogicTests/PaneSSHProcessProbeTests.swift`

## Design

1. Ghostty exposes only the PTY foreground process ID already owned by its
   `Termio.getProcessInfo(.foreground_pid)` API. It does not parse `/proc`, SSH,
   or Zentty state.
2. A pure Rust policy parser recognizes source-supported SSH argv forms,
   including `-l`, `-p`, attached values, options consuming values, `--`, user
   targets, and IPv6. It rejects missing targets and non-SSH processes.
3. A Linux process probe reads bounded `/proc/<pid>/comm`, `cmdline`, and child
   relationships without shell execution. It verifies process identity and
   prefers the deepest live SSH descendant when the foreground process is a
   wrapper.
4. One window-local coordinator periodically probes live surfaces on the GLib
   main context and updates existing presentation state. It owns cancellation
   and emits no argv, path, or credential data in receipts.
5. Restored panes retain topology only. Reconnection is never automatic in this
   slice; a later explicit action/policy may offer it.

## Test order and acceptance

1. Port source argv goldens and hostile/malformed boundary tests first.
2. Test `/proc` parsing against real short-lived process trees and bounded
   missing/reused PID behavior.
3. Add the minimal Ghostty ABI with null, foreign-widget, uninitialized, and
   exited-process contracts plus C/C++ header and audit coverage.
4. Extend an existing real product actor: connect through a disposable local
   SSH server from a real Ghostty PTY, observe the remote marker, exit SSH, and
   prove the marker clears without recreating the pane or PTY. Repeat in
   controlled X11 and Wayland.
5. Run mutation testing for the pure parser, strict Clippy, workspace tests,
   architecture/inventory validators, and every presently executable matrix
   cell before commit and push.

No title-only detection, mocked terminal, second process registry, automatic
network reconnection, or file-transfer behavior is accepted.
