# Linux development-server discovery and control plan

Date: 2026-08-10
Tracking: GH-19, GH-20, GH-22

## Product outcome

When a real process launched from a Zentty pane begins listening on a local TCP
port, Zentty attributes it to that pane and worklane, surfaces the source-ranked
server in the existing sidebar/chrome and command system, opens it through the
configured Linux browser boundary, and can stop only a listener it proves is
owned by that pane. Zentty does not host, proxy, or emulate the server.

This is a port of the source `Zentty/Servers` feature. Linux-specific process and
desktop mechanisms may differ, but source verbs, ranking, ownership rules, and
user-visible outcomes remain authoritative.

## Non-negotiable architecture

1. Pure server identity, URL normalization, registry merge, relevance ranking,
   ignored-port policy, and termination decisions live in focused Rust core
   modules and are mutation tested.
2. One Linux `ServerCoordinator` owns bounded `/proc` listener discovery,
   polling cadence, GTK projection, browser launching, and stop execution per
   window. No second workspace, PTY, process registry, or scenario product is
   permitted.
3. Pane identity and terminal ownership come from the existing
   `WorkspaceState`, `PaneRuntimeCoordinator`, and real Ghostty foreground PID.
4. Listener ownership is revalidated immediately before signaling. Zentty never
   signals an unattributed, cwd-only, Docker, manual, stale, PID-reused, pane
   shell, or unrelated-user process.
5. Browser opening is an argv-safe Linux process boundary, never a shell string.
   The controlled integration journey substitutes only the external browser
   application, not Zentty, Ghostty, GTK, PTYs, sockets, or server processes.
6. Ignored ports extend the single process config authority introduced by GH-20;
   there is no feature-local preferences file. Durable write support must be
   atomic and symlink preserving before an Ignore action claims persistence.
7. Docker-published discovery remains a required explicit cell. If the local
   environment cannot supply a real disposable container, it is BLOCKED rather
   than inferred from host listeners or converted to PASS.

## Test-first construction order

### A. Source-derived pure contracts

- Red tests for supported URL/host/port normalization, wildcard-to-localhost,
  IPv4/IPv6/private-host policy, hostile schemes, missing ports, paths/query/
  fragments, output URL extraction, punctuation, and deterministic deduping.
- Red tests for source/confidence/focus/running/freshness relevance weights,
  ignored ports, one primary, deterministic ties, registry source precedence,
  first-seen preservation, stale removal, and pane/worklane isolation.
- Red tests for exact and ranged ignored-port parsing/normalization.
- Red tests for listener attribution by process ancestry, bounded cycle/PID
  reuse handling, safe cwd fallback, broad-root rejection, and ambiguous panes.
- Red tests for termination authorization and graceful/escalation state.

### B. Real Linux process boundary

- Parse bounded `/proc/net/tcp{,6}` LISTEN rows and correlate socket inodes with
  real `/proc/<pid>/fd` links without scanning unrelated file contents.
- Read bounded process stat, start time, parent PID, and cwd; retain start time
  in observations so PID reuse invalidates ownership.
- Run real IPv4 and IPv6 HTTP listener fixtures beneath distinct process trees;
  prove exact listener PID, port, ancestry, attribution, disappearance, and
  unrelated-process rejection against kernel data.

### C. Product coordinator and UX

- Poll only while qualifying local panes exist, with one cancellable GLib source
  and no overlapping scan; apply results on the GTK thread.
- Project the primary server on the owning worklane/pane and add source verbs:
  Open, Open With, Refresh, Ignore Port, Stop Ignoring Port, and Stop Server.
- Add command-palette entries and availability driven from the same registry.
- Revalidate stop ownership, send graceful SIGINT, observe exit, and only then
  expose or execute the documented bounded force escalation.
- Open the normalized URL through the Linux browser launcher with exact argv.

### D. Authenticated CLI and output-reported servers

- Extend the existing pane-token-authenticated IPC socket with one bounded
  development-server request route. Do not create a feature socket, alternate
  transport, or encode server commands as agent events.
- Port `zentty server set/clear/list/open/watch/watch-set/watch-clear`, including
  source argument names, strict parsing, pane routing derived from the token,
  bounded replies, and deterministic stale-target failures.
- `zentty server watch -- <argv...>` launches the real child without a shell,
  preserves stdin, tees stdout and stderr unchanged, incrementally detects
  normalized URLs across chunk boundaries, registers the newest output URL as
  `.watch`, returns the child's exit status, and clears only that pane's watch
  record on exit.
- Construct these tests red-first at the protocol/parser boundary, then extend
  the one staged-product development-server journey. The journey may replace
  only the external browser; the CLI, authenticated socket, child process,
  Ghostty PTY, GTK product, and HTTP listener remain real.

### E. Real product journeys

- In staged ReleaseSafe Zentty, launch separate real dynamic-port HTTP servers
  from real Ghostty PTYs in at least two panes/worklanes.
- Verify sidebar attribution and ranking, focus-dependent primary selection,
  exact browser argv, refresh/removal, ignore/unignore, graceful stop, hostile
  URL rejection, unrelated listener isolation, and terminal focus afterward.
- Repeat representative GUI-sensitive behavior in controlled X11 and Wayland.
- Exercise one real container-published server when the controlled host provides
  a supported runtime; preserve an explicit blocked prerequisite otherwise.

### F. Qualification and completion

- Extend the authoritative feature inventory, architecture ownership contract,
  and qualification matrix; do not silently remove Docker or platform-open
  gaps.
- Mutation-test normalization, ranking, attribution, port rules, and termination
  authorization with the repository copy-safety configuration.
- Review the complete diff, run strict Clippy, the full workspace, every affected
  controlled journey, and all presently executable matrix cells before commit.
- Promote `utilities.dev-servers` from `NOT_IMPLEMENTED` only when every required
  non-blocked behavior above is delivered and the remaining blocked environment
  cells are explicit.

## Explicit exclusions

- No embedded browser, HTTP proxy, web inspector, file tree, editor, or server
  implementation.
- No control of listeners that cannot be proven descendants of the owning pane.
- Task runners and Task Manager remain separate GH-19 feature slices.

## Browser-target parity closeout — 2026-08-12

The passive, Docker, authenticated CLI/watch, ignore/unignore, refresh, and
owned-stop paths are implemented and qualified. Source audit leaves one named
gap: Linux still rejects every `zentty server open --browser` value except the
system default and offers no preferred-browser selection in the product UI.
GH-18 has now supplied a reviewed Linux application-discovery and direct-launch
pattern, but server browsers remain a separate source-owned catalog rather than
being folded into project Open With.

Construction order:

1. Add red core tests for a server-browser catalog containing the always-
   available system default, enabled discovered desktop handlers, executable
   custom browsers, deterministic IDs/order/deduplication, unavailable saved
   preferences, and fallback to system default. A browser launch plan accepts
   only a previously normalized HTTP(S) URL and produces GIO application launch
   or exact executable argv without a shell.
2. Parse the source-compatible `server_detection.custom_browsers` records and
   normalize preferred/enabled IDs. Reject missing/duplicate/reserved IDs,
   missing names/paths, and unavailable executables without weakening the
   bounded configuration authority.
3. Discover Linux HTTP(S) desktop handlers once into the existing
   `ServerRuntime`. Add explicit per-browser command-palette actions for the
   focused primary server, persist an explicit selection as the preference,
   and route primary, palette, and authenticated CLI opens through the same
   catalog and launcher. Unknown or unavailable explicit IDs must fail rather
   than silently open in another browser; implicit preferred-browser absence
   falls back to the system default as source Zentty does.
4. Extend the one existing development-server product journey. Controlled
   browser executables replace only the external browser application; the real
   Zentty product, GTK input, authenticated CLI, Ghostty PTY, listener, registry,
   ranking, and URL remain real. Prove preferred selection, explicit CLI
   selection, exact URL argv, unavailable/forged rejection, and system fallback
   under X11 and Wayland.
5. Preserve Docker as an explicit environment-dependent cell and all existing
   hostile URL, ownership, PID, stop, watch, and port-policy evidence. Run the
   focused mutation gate and every presently executable qualification cell
   before promoting `utilities.dev-servers`.

Acceptance criteria:

- [x] Browser discovery/configuration has one deterministic, source-compatible
      catalog with no shell launch or alternate preferences store.
- [x] UI, primary open, and authenticated CLI use the same selected target and
      reject forged/unavailable explicit browser IDs.
- [x] Real X11 and Wayland journeys prove exact target/URL behavior without
      substituting Zentty, Ghostty, PTYs, sockets, or listeners.
- [x] Strict Clippy, workspace tests, governed mutation, and every presently
      executable matrix cell pass before inventory promotion.
