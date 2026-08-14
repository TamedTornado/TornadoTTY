# Linux agent sleep-inhibition implementation plan

Issue: GH-21
Source authority: `Zentty/AppState/NotificationStore.swift`

## Source behavior to preserve

- One process-wide authority aggregates all windows; it is not one inhibitor
  process per pane or per window.
- A source qualifies only when its integration is enabled and a recognized
  agent pane is in the source `running` phase. Starting, needs-input, idle,
  unresolved-stop, unrecognized, and stale auxiliary state do not qualify.
- The first qualifying source acquires once. Further qualifying windows reuse
  the same lease.
- Losing the last running source releases after a ten-second debounce. Running
  work returning within that interval cancels release without reacquiring.
- Disabling the setting releases immediately. Process shutdown/deinitialization
  releases immediately regardless of the debounce.
- The source prevents idle **system** sleep while allowing the display to sleep.

## Linux boundary

- `zentty-core` will own the deterministic aggregate/debounce state machine.
  It has no GTK, process, D-Bus, or timer ownership.
- `zentty-linux` will own one `systemd-inhibit` child under the application
  coordinator. It requests `what=sleep`, `mode=block`, `who=Zentty`, and a fixed
  local reason. This preserves display sleep, though logind cannot distinguish
  automatic from explicit system sleep as narrowly as macOS can.
- The inhibitor command runs a hidden shell-free mode of the same staged Zentty
  executable as its lease body. Its stdin is a private pipe held only by the
  main Zentty process. Graceful drop closes the pipe;
  process crash closes it in the kernel, so the helper and logind lock cannot
  silently outlive Zentty.
- Acquisition is not claimed merely because `spawn` succeeded. The command
  writes a readiness marker only after logind granted the lock; Zentty observes
  that marker asynchronously, separately detects early child exit, and rejects
  acquisition that never becomes ready within a bounded five-second deadline.
- Runtime absence or denial is an explicit unavailable/error state. It is never
  converted to a pass. No portal or fake fallback will run concurrently.
- The existing Agents setting becomes editable only when the backend executable
  is present. Persisted requested state remains visible if it is unavailable.

## Construction order

1. Add mutation-friendly core transitions and exhaustive focused tests first.
2. Add the single Linux lease owner and its early-exit/drop tests.
3. Wire coordinator aggregation, config reload/settings changes, window removal,
   agent phase changes, and process shutdown to that owner.
4. Enable the existing Agents setting without adding a second settings path.
5. Add a real product journey using authenticated Agent IPC and the host logind
   inhibitor list. Prove acquire-on-running, no acquire for non-running phases,
   debounce cancellation/release, immediate config disable, child/window exit,
   SIGKILL cleanup, and restart without an orphan.
6. Add explicit matrix cells and mutation targets. Environment absence is
   BLOCKED, not PASS.
7. Run every presently executable cell, preserve receipts, review the complete
   diff, and only then commit and push.

## Acceptance boundary for this slice

This slice completes `desktop.agent-sleep-inhibition`; it does not implement a
StatusNotifierItem, change notification privacy, or invent remote approval.
Release/full qualification cannot be claimed while other matrix gaps remain.
