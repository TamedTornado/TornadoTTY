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

### D. Real product journeys

- In staged ReleaseSafe Zentty, launch separate real dynamic-port HTTP servers
  from real Ghostty PTYs in at least two panes/worklanes.
- Verify sidebar attribution and ranking, focus-dependent primary selection,
  exact browser argv, refresh/removal, ignore/unignore, graceful stop, hostile
  URL rejection, unrelated listener isolation, and terminal focus afterward.
- Repeat representative GUI-sensitive behavior in controlled X11 and Wayland.
- Exercise one real container-published server when the controlled host provides
  a supported runtime; preserve an explicit blocked prerequisite otherwise.

### E. Qualification and completion

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

