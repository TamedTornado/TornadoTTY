# Linux Gemini agent integration port plan

- **Status:** first vertical slice implemented; terminal-notification and real
  model-turn enrichment remain
- **Date:** 2026-08-06
- **Owner:** [#7 — essential Zentty workflow parity](https://github.com/TamedTornado/zentty/issues/7)
- **Inventory ID:** `agent.gemini`
- **Field record:**
  [`zentty-linux-dogfood-2026-08-01.md`](zentty-linux-dogfood-2026-08-01.md)

## Product contract

The Swift product and its tests define this slice. A wrapped `gemini` launch
must use the installed Gemini executable and its real terminal/PTY while
Zentty supplies a per-pane system-settings overlay. The overlay preserves the
existing readable settings, forces Gemini terminal notifications on, and
appends Zentty's hook command without modifying the user's file.

The source hook mapping is:

- `SessionStart` -> PID attach plus `starting`;
- `BeforeAgent` and `BeforeTool` -> `running`;
- `AfterAgent` -> `idle`;
- `SessionEnd` -> session/PID clear; and
- `Notification` with case-insensitive `ToolPermission` -> `needs-input` with
  approval text derived from the message or structured tool/path details.

Other Gemini notifications and unknown hook events are intentional no-ops,
not protocol failures. Restore uses `gemini --resume` when a valid pane CWD is
available and does not require a session ID.

## Boundaries

- Reuse the existing agent wrapper, authenticated socket, reducer, sidebar,
  Ghostty surface, and PTY paths. Do not add another agent daemon, status
  store, restore harness, or product-only test route.
- The settings overlay is private runtime state owned by the pane launch. It
  may not replace, rewrite, chmod, or otherwise mutate the user's settings.
- Keep the external Gemini CLI real in the final product journey. A fixture
  binary is acceptable only for focused exec/argv/environment contract tests;
  it is not agent integration evidence.
- No Ghostty change is expected. If one becomes necessary, stop and document
  the Ghostty-owned defect before changing the fork.

## Tests-first order

1. Add source-pinned adapter tests for every mapping, alternate JSON key,
   approval-detail fallback, case handling, no-op, malformed input, and event
   ordering.
2. Add launch-plan tests for merge/preservation, exact hook groups/timeouts,
   command escaping, notification forcing, invalid/unreadable input, secure
   atomic overlay creation, opt-out, and no user-file mutation.
3. Add real CLI subprocess tests for wrapper discovery/exec, PID propagation,
   stdin hook delivery, authenticated canonical targeting, empty-success JSON,
   and explicit transport failures.
4. Stage the Gemini wrapper and prove product-relative relocation and absence
   when no real Gemini executable exists.
5. Add one focused real-product journey, reused on controlled X11 and
   input-capable Wayland: launch Gemini in a real Ghostty PTY, consume the
   generated overlay, emit its real hook subprocesses through the staged CLI
   and private socket, observe sidebar state, restart with `--resume`, and
   prove cleanup. Only Gemini's remote model response may be controlled if a
   model turn is required.
6. Run focused mutation testing for adapter and overlay decisions, then the
   full workspace, strict Clippy/format, inventory, architecture, compositor,
   and presently executable qualification gates.

## Exit criteria

`agent.gemini` may advance from `NOT_IMPLEMENTED` only when the staged product
crosses the real wrapper, Gemini process, Ghostty/PTY, generated overlay, hook
helper, authenticated socket, reducer, and sidebar boundaries on both
controlled compositor axes. Focused fixture-binary tests alone are not enough.
The entry remains `PARTIAL` if terminal-notification enrichment, restore, or
any named source lifecycle behavior is still missing.
