# Zentty Linux dogfood — GH73 safe lifecycle

## Scope and authority

- **Issue:** GH73, child of GH23.
- **Source outcome:** destructive close is scoped to the exact pane, worklane,
  window, or application; idle shells do not nag; real work does; cancellation
  is safe; window state remains compositor-owned.
- **Architecture:** `zentty_core::close_decision` is the single pure decision
  authority. GTK presentation and `/proc`/Ghostty evidence collection stay in
  `application_shell/close_runtime.rs`. The existing application coordinator
  remains the sole owner of window registration, PTY teardown, persistence,
  and final application shutdown. No second lifecycle or integration harness
  was added.

## Discoveries and repairs

1. **The old close predicates disagreed.** Quit always prompted, window close
   treated every live interactive shell as destructive, pane close followed
   only its preference, and worklane close never prompted. WM close also
   entered a different callback path. All routes now build `CloseEvidence` and
   call one typed `decide_close` authority.
2. **A live terminal is not necessarily live work.** Ghostty owns the real
   foreground PID. Interactive `bash`, `dash`, `fish`, `nu`/`nushell`, `sh`,
   and `zsh` without `-c`/`--command` are idle; another foreground executable
   is destructive work. Agent phase and meaningful session history are
   independent evidence. Risk precedence is process, active agent, history.
3. **Shell integration looked like user history.** `PROMPT_COMMAND=` and
   `_zentty_` bootstrap titles caused false prompts. They are excluded without
   excluding ordinary user commands.
4. **Modal callbacks can become stale.** Confirmation records canonical target
   evidence. Acceptance re-reads the target and rejects any changed identity,
   membership, or risk evidence. A pending confirmation rejects repeated close
   requests, and the accepted action is consumed once on the next GTK loop
   turn.
5. **WM close bypassed the authority.** `close-request` now calls the same shell
   request as the named close action. Last-window close correctly becomes an
   application-scope decision rather than silently using a narrower predicate.
6. **Shutdown re-entry exposed a real RefCell panic.** Accepted last-window
   shutdown can synchronously cause GTK shutdown callbacks. Shutdown guards and
   non-panicking coordinator borrows make teardown idempotent instead of
   recursively borrowing the coordinator.
7. **Window state needs a real compositor.** Bare Xvfb has no WM and therefore
   cannot acknowledge fullscreen or minimize. The focused X11 state/lifecycle
   mode starts Openbox and verifies `_NET_WM_STATE_HIDDEN`; it does not turn WM
   absence into a pass. Wayland verifies GTK's real fullscreen notifications.
   `xdg-toplevel.set_minimized` has no minimized-state acknowledgement, so the
   Wayland result is explicitly a compositor request, not a fabricated native
   minimize PASS.
8. **State and destructive-close journeys contaminated each other.** Fullscreen
   changes geometry and focus timing, so fullscreen/minimize are isolated in a
   state-only mode while close/quit cancellation and acceptance remain in a
   lifecycle-only mode of the existing `rust-multi-window` actor.
9. **Cage virtual input loses a key-up when key-down creates a surface.** A
   synthetic Control+T could move focus while the chord was still down and
   later replay New Worklane into the survivor. Modifier-release guesses were
   removed. The Wayland destructive journey invokes the real New Worklane
   action through the real command palette, and lifecycle qualification uses
   controlled labwc outer-X11 input. Cage remains useful elsewhere, but this
   Cage virtual-keyboard artifact is not product evidence.
10. **Function keys were absent from the shortcut model.** F1–F12 now round-trip
    through source-compatible storage, GDK physical translation, settings
    preview, and conflict governance. F11 is the standard Linux fullscreen
    default; minimize is available as a named/palette action without inventing
    a default chord.
11. **An unscoped mutation baseline hit an unrelated sandbox assumption.** The
    first targeted-file invocation still ran every `zentty-core` test; the
    existing Open With FIFO test cannot create its special file in the mutants
    scratch sandbox (`EPERM`). No mutant ran and this was not recorded as
    lifecycle evidence. Restricting Cargo's test arguments to the close reducer
    isolated the owned authority without weakening its tests.

## Focused evidence

- Core close authority: **6 PASS**.
- Linux evidence/stale-callback tests: **3 PASS**.
- Window registration/boundary reducer: **9 PASS**.
- Core shortcut tests: **8 PASS**.
- Linux shortcut/settings tests: **10 PASS**.
- Close-decision mutation run (`--gitignore=true`, `--lib close_decision`):
  **6 mutants**, **3 caught**, **3 unviable**, no survivors.
- `cargo clippy -p zentty-core -p zentty-linux --all-targets -- -D warnings`:
  **PASS**.
- `shellcheck -x linux/tests/rust-multi-window`: **PASS**.
- Controlled X11 state journey: **PASS** — real two-window staged product,
  F11 round trip, native Openbox minimize and reactivation.
- Controlled X11 lifecycle journey: **PASS** — cancel and accept for worklane,
  window, and application close; survivor PTY routing; owned PTY reaping.
- Controlled labwc Wayland lifecycle journey: **PASS** — the same real-window,
  real-PTY cancel/accept contract.
- Controlled Wayland state journey: **PASS** — fullscreen round trip and an
  explicitly request-only minimize outcome.

Existing `rust-multi-window`, session-restore, product-lifecycle, agent-IPC, and
workspace reducer coverage continues to own pane close, active/waiting agent
state, crash/clean restart, transfer-empty boundaries, persistence, and exact
PTY cleanup. This feature extends those authorities; it does not duplicate
them. No app-wide/full qualification run was used as a development oracle, and
this report does not claim full Linux qualification.

## Remaining platform substitution

- Wayland deliberately does not restore absolute coordinates: GTK requests
  persisted size and the compositor owns placement. X11 externally driven
  placement remains WM-owned as well.
- Wayland minimize cannot be queried through xdg-shell. Zentty can request it,
  but only X11 currently yields an independently observable minimized state.
- macOS Hide and Mission Control are not imitated. Linux exposes ordinary
  minimize, fullscreen, activation, and compositor/window-manager behavior.
