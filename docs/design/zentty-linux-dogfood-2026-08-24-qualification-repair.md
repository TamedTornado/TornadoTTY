# Zentty Linux qualification failure retirement dogfood

Date: 2026-08-24
Tracking: GH-88 and child issues GH-89 through GH-94

This append-only record starts the retirement of the non-pass outcomes from the
2026-08-23 local qualification receipt. The tracker issues own acceptance
criteria and test order. No full matrix rerun is permitted until isolated and
cluster evidence is green.

## GH-89: staged journey reproduction

- The preserved matrix logs initially suggested both staged journeys lost
  `build-metadata`. An independent X11 rerun proved that this was not an
  artifact deletion race: the staged bundle never copied that optional file.
- The actual cause of the metadata read was environment leakage introduced in
  GH-83. `qualify-local` exported the real Gemini binary/version around the
  entire matrix. Every invocation of `rust-agent-ipc`, including staged smoke,
  interpreted the mere binary variable as a request to run installed Gemini.
  That journey then read profile metadata which is not part of this deliberately
  assembled test bundle.
- Repair decision: only the two authoritative agent-integration cell commands
  select the required Gemini journey. `qualify-local` may put the reviewed pnpm
  bin directory on PATH, but it must not inject journey-selection variables
  into unrelated cells. The actor keys execution from
  `ZENTTY_REQUIRE_REAL_GEMINI=true`, never from an ambient binary variable.
- After the leaked Gemini journey no longer obscured the run, the independent
  X11 staged journey reached a second, real failure. Exact child-line mapping
  proved it was not `kill-window`: the public `zentty version` returned a full
  40-character commit while the ratified CLI contract requires 12 characters.
- This drift began when About/package provenance correctly changed
  `zentty_revision` to the full hash and the same value was accidentally reused
  as `ZENTTY_BUILD_COMMIT`. The repair keeps the full hash in build metadata and
  derives a separately validated 12-character display revision for the public
  CLI and embedded About surface.
- A current ReleaseSafe build completed in about 65 seconds. Its public receipt
  was `zentty 0.1.0 (5884df60005c)`, while `build-metadata` retained the full
  `5884df60005cec81f59bf022d203b32dc5b85035` provenance identity.
- The complete staged X11 journey then passed in its private Xvfb session:
  product smoke, the full compatibility-agent actor, tmux compatibility, and
  final staged-bundle assertion. The corresponding private Weston Wayland
  journey also passed. Neither run selected installed Gemini, and both receipts
  explicitly reported `real-gemini=prerequisite-not-requested`.
- The explicit installed-Gemini X11 scenario was then rerun against the rebuilt
  product and passed with `real-gemini=true`, proving the isolation repair did
  not disable the required journey.
- No concurrent artifact contender was involved in either reproduced failure,
  so an artifact mutex or immutable-bundle redesign would be a false repair.
  The shared environment-leak and full-vs-display provenance contracts are now
  checked by the focused orchestration contract. The authoritative full matrix
  has not been rerun.

## GH-90: controlled X11 focus reproduction

- `platform-clipboard-x11` reproduced independently and immediately with X11
  `BadMatch`, opcode 42 (`X_SetInputFocus`). A shell trace located the exact
  operation: `rust-pane-search` used a name-only `xdotool search`, accepted
  window `2097154`, and focused it without checking mapping or PID ownership.
- This actor had drifted from the shared input contract, which already binds to
  `--onlyvisible --pid "$product_pid"` and verifies the returned owner. The
  compositor warnings after the X error were unrelated cleanup noise.
- `rust-pane-search` owns three of the baseline failures, and
  `rust-task-runners` used the same stale/unmapped-window pattern. Both now
  require a mapped window belonging to their exact launched product before
  physical focus. No X error is suppressed and no alternate window is used.
- The mapped PID-owned lookup now lives once in `product-input`. Its focused
  runner rejects a missing mapped window and a visible window owned by another
  process, while retaining the exact successful XTEST sequence.
- Independent reruns passed for platform clipboard, terminal input, task
  runners, and the complete composite Ghostty API product-usage cell. The
  latter covered API audit, closed-pane restoration, full pane search, and tmux
  compatibility in one private X11 session.
- Platform clipboard and task runners were then deliberately run concurrently
  in two private Xvfb sessions. Both passed with distinct 64-hex session IDs,
  proving the repair scopes discovery by process/session rather than serializing
  unrelated X11 cells.
