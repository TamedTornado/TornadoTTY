# Zentty exhaustive feature audit — 2026-08-04

Status: **IN PROGRESS — implementation is frozen until the coverage gates in
this document pass.**

This audit replaces the assumption that the original 32-entry parity inventory
was complete. It was not. That inventory intentionally covered only workspace,
worklane, pane, command-routing, and agent basics, but its name and validator
allowed it to be treated as the full product contract. The missing Claude
agent-team tmux compatibility layer proved that internal consistency is not
source completeness.

## Completeness standard

A feature is in scope when any of the following presents it as current Zentty
behavior:

1. the current public documentation or product website;
2. a public release note from `v0.1.7` through the audited release;
3. a user-visible command, menu item, setting, control, notification, CLI
   command, protocol field, persistence field, or accessibility behavior in
   source;
4. a test that names a user-observable contract or regression;
5. an official screenshot or demo;
6. an external discussion that identifies behavior Zentty claims to solve.

External discussions are corroborating workflow evidence, not permission to
invent unrelated features. Requests such as token accounting, mobile control,
or automatic worktree orchestration remain explicit non-goals unless Zentty
itself ships them or Jason separately approves them.

Every discovered behavior must map to exactly one primary feature entry and may
name related entries. Every entry must state Linux feasibility, classification,
owner issue, dependency, source evidence, and a proportionate real-system test.
Nothing may be omitted merely because it is AppKit-specific: the inventory must
name a Linux-native alternative or a concrete blocker.

## Evidence reviewed

### First-party product material

- Repository `README.md`, `docs/cli.md`, `docs/agent-hooks.md`, and
  `docs/agent-status-protocol.md`.
- Current public documentation index and every linked guide: Getting Started,
  Worklanes & Panes, Agent Workflows, Shortcuts, Embedded CLI, Themes &
  Settings, tmux Compatibility, Reference, and FAQ.
- Product landing page and comparison pages, used especially to distinguish
  shipped behavior from explicit non-goals.
- All 39 public GitHub releases, `v0.1.7` through `v0.1.45`, plus the local tag
  history beginning at `v0.1.0`.
- `assets/screenshot.png`, inspected at original resolution. It shows the
  source chrome, horizontally scrolling pane canvas, nested worklane/pane
  sidebar, git branch context, agent states and attention, path/title row,
  notification badge, icon controls, terminal-native search/jump affordance,
  and command palette with Open With results.
- All Swift product and CLI files under `Zentty/` and `ZenttyCLI/`, all logic
  and integration test filenames, the complete `AppCommandID` registry, all
  settings sections, the pane command enum, sidebar/context-menu commands, and
  the Agent IPC/tmux compatibility path.

### External workflow evidence

The following discussions were read to test whether our interpretation matches
real multi-agent terminal pain. They do not add parity requirements by
themselves.

- Reddit, “I built a terminal tool to manage a fleet of Claude Code sessions
  across all my repos”: losing sessions across tabs/desktops, missed permission
  questions, accidental close, notification routing, persistence, multiple
  accounts, and no-telemetry expectations.
- Reddit, “How many terminals do you typically have open while using Claude
  Code?”: real users report multiple agents plus servers, logs, databases, and
  Docker across two to dozens of terminals; several explicitly organize
  planning, implementation, and review as separate durable sessions.
- Reddit, “I burned out my max plan for this…”: users emphasize reliable
  session detection, message delivery, exact pane jumping, agent-version
  compatibility, and distrust of superficially broad integrations.
- Reddit Warp universal-agent discussion: users object when agent features
  interfere with ordinary terminal I/O or replace established terminal UX;
  multi-account isolation, performance on large repositories, and reversible
  UI choices are recurring concerns.
- Reddit Claude terminal workflow discussion: users explicitly ask for easy
  terminal naming/coloring, use worktrees for parallel sessions, and encounter
  dev-server port conflicts. Zentty ships naming/color/git/server visibility but
  explicitly does not manage a worktree per agent.

Reddit's public search JSON endpoint returned HTTP 403 during the audit. Exact
web searches found no indexed Zentty-specific Reddit thread. This limitation is
recorded rather than converted into evidence of absence.

## Feature catalog

The machine-readable inventory is authoritative for individual entries. This
catalog is the human reconciliation checklist and must have the same coverage.

### Terminal foundation and interaction

- Real Ghostty/libghostty rendering and VT behavior, one real PTY per pane.
- Physical keys, modifier sides/layout changes, IME, mouse reporting, clipboard,
  URL actions, drag/drop, middle-click selection paste, selection autoscroll,
  smooth/elastic scrolling, scrollback preservation, and performance overlay.
- Ghostty configuration loading/reload, bundled defaults, minimum padding,
  background images, bundled terminfo, bundled themes, custom theme paths,
  live theme preview, opacity, blur/platform alternative, dark/light/automatic
  modes, and OpenCode theme synchronization.
- SSH identity display, remote-pane indication, remote file/image paste or drop,
  upload-path insertion, shell escaping, and safe local-versus-remote policy.

### Windows, worklanes, columns, and panes

- Multiple windows; new, close with confirmation, quit confirmation, native
  fullscreen, display placement restore, moving a live pane to a new/existing
  window, and grids that create windows/worklanes.
- Durable worklanes with title, rename, color, ordering, configurable insertion
  position, keyboard cycling, drag reordering, close confirmation, and exact
  window title/desktop-label presentation.
- Worklane Peek live neighboring-lane overview with hold-to-show, keyboard,
  click, swipe, project icons, input shielding, and active-window correctness.
- Horizontally scrolling columns; adaptive/forced-visible/append-scrollable right
  split behavior; configurable threshold; sidebar-change anchoring; background
  viewport stability; display-change reconciliation.
- Right/left/up/down splits, duplicate pane, grid creation, vertical stacking,
  before/after placement, pane focus history and directional/cyclic focus,
  move/swap, live cross-lane/cross-window handoff, and source-preserving drag.
- Full/halves/thirds/quarters, vertical one-to-four-per-column, four golden
  variants, explicit ratios, reset, resize by keyboard/divider, and persisted
  proportions.
- Pane custom titles, durable identity versus live process hint, labels, borders,
  inactive opacity, focus halo, CWD/path menu, copy path, run-last-command,
  zoomed-out view, and recently closed pane restoration including supported
  agent resume.

### Sidebar, chrome, navigation, and commands

- Source-accurate icon controls, tooltips, accessible names, popovers, context
  menus, header controls, sidebar width/resize, pinned-open/pinned-closed/overlay
  modes, hide/show animation, focus preservation, auto-scroll, and row diffing.
- Worklane rows with pane hierarchy, titles, paths, git branch, colors, remote
  marker, project icon, agent type/status/progress/attention, idle and shimmer
  treatment, PR badge, server/bookmark actions, and update availability.
- Navigation back/forward history, recent panes, recent commands, and exact-pane
  focus from every attention/search/menu-bar route.
- Complete command registry exposed consistently through named actions, menus,
  shortcuts, context menus, tooltips, and fuzzy command palette; dynamic task,
  server, Open With, worklane-color, recent-pane, and settings results.
- Command availability, grouping, stable layout/backdrop, outside-click dismissal,
  and no click-through into terminal content.

### Search, clipboard, files, and external tools

- Pane search, selection-as-query, next/previous, terminal-theme HUD, and
  window-wide global search in the sidebar across pane text and agent state.
- Clean Copy command detection/flattening, prompt and decoration removal,
  paragraph/indent/blank-line preservation, box drawing, slash commands, path
  quoting, URL tracking removal without functional damage, IPv6 safety, raw
  override, automatic-clean mode, and Copy as Markdown.
- Open With primary/enabled/custom targets, executable discovery, focused-CWD
  routing, project-file resolution, Linux-native file manager/editor targets,
  and branch/PR opening on the configured git remote.

### Durable recipes and project context

- Clean/live snapshots, crash-safe workspace restore, meaningful-pane skip
  rules, shell-ready restoration, multi-window geometry, pane dimensions,
  commands, labels, titles, colors, agent drafts/resume, and migration.
- Closed-pane stack with exact CWD, width/height, process hint, last command,
  navigation history, supported agent session, and close-order semantics.
- Bookmarks and presets: capture, name suggestion, create/update/edit/delete,
  unlink, import/export, JSON persistence, symlink-safe writes, live commands,
  color/layout/title/launch targets, stale-CWD healing, restore banner, and
  dotfile portability.
- Git branch/repository/remote context, automatic refresh after agent work,
  project icons, pull-request association, CI/approval/conflict/age/error state,
  adaptive refresh, manual refresh, and remote-open action.

### Agents and attention

- Consent and Ask/On/Off integration policy, grandfathering installed hooks,
  reversible install/uninstall, wrapper discovery, management-command pass
  through, nested-launch isolation, and launch-error diagnostics.
- Private per-instance Unix Agent IPC, pane-scoped tokens, canonical routing,
  topology discovery, protocol versioning, request bounds, socket recovery,
  environment injection, and manual/custom-agent protocol.
- Normalized starting/running/compacting/needs-input/idle/unresolved-stop state;
  approval/question/decision/auth/generic interaction; confidence arbitration;
  root/child sessions; task progress; PID death; stop-race debounce; artifacts;
  title/OSC/process fallbacks; and stale-state cleanup.
- Claude Code, Codex, Gemini, OpenCode, Amp, Copilot, Cursor, Droid, Kimi, Pi,
  Oh My Pi, Grok, Antigravity (`agy`), Hermes, Vibe/Mistral, Small Harness, and the
  documented custom-agent path, each with its actual hook/overlay/resume and
  capability contract rather than a generic badge.
- Claude agent-team tmux compatibility shim, command/format/store semantics,
  first golden team column, later vertical stacks, leader focus and width
  restoration, without replacing Ghostty PTY ownership.
- Sidebar/chrome/menu-bar/system-notification attention, notification inbox,
  actual question text, configurable sound/custom sound, silent/no-inbox CLI,
  badges, clear/stale cleanup, latest-attention jump, and exact pane routing.
- Optional agent caffeination while tracked work is active, implemented with a
  Linux-native inhibitor API rather than a macOS power assertion.

### Project utilities

- Passive and Docker-aware dev-server discovery, listener/process attribution,
  output URL detection, normalization, relevance ranking, ignored ports,
  browser selection, open action, stop action, refresh, and IPC/command-palette
  exposure.
- Task-runner discovery for package scripts, Taskfile, just, make, mise, and VS
  Code tasks, deduplication and safe command launch from the palette.
- Task Manager with per-pane root process, process identity, CPU history,
  memory, network, theme, refresh lifecycle, and ordinary window behavior.

### Settings, packaging, support, and trust

- Searchable settings window with grouped sidebar, section headers, back/forward
  history, keyboard navigation, appearance, shortcuts, notifications, Open
  With, dev servers, pane layout, updates/privacy, and agents sections.
- TOML persistence with defaults, validation, partial reload, last-good state,
  comments where possible, unknown-key tolerance, live watch/reload,
  symlink-safe atomic writes, and XDG-correct Linux locations.
- Fully bindable command registry; physical-key shortcut recording; conflict
  detection/resolution; left/right-hand presets; clickable previews; category
  search; import/export; reset; pass-through to terminal when unbound.
- Staged/install packaging of GUI, CLI, shim, themes, terminfo, icons, licenses,
  protocol resources, desktop metadata, and clean install/upgrade/uninstall.
- Updates with stable/beta channels, user-visible availability and failure,
  release notes, rollback-safe replacement, and a Linux-native signed package
  update policy rather than Sparkle.
- About metadata, docs/source links, third-party license catalog and source/home
  links, update state, and error diagnostics.
- No telemetry or analytics; crash reports leave the machine only after explicit
  user action. Linux error reporting must preserve that consent boundary.

## Explicit non-features and platform alternatives

- Zentty is not an IDE, editor, file tree, browser surface, cloud-agent runtime,
  token/cost accounting dashboard, phone remote, or automatic one-worktree-per-
  agent orchestrator. Ordinary shells may run in user-created worktrees.
- Zentty does not promise full tmux sessions, detach/reattach, or remote session
  persistence. Real tmux may run inside a pane. The bundled shim is narrowly for
  compatible agent-team commands.
- macOS menu bar, Dock badge, Finder, Mission Control labels, Sparkle, AppKit
  blur, haptics, caffeination, notification APIs, and Automation permissions
  require named Linux alternatives. A missing identical API is not grounds to
  delete the user outcome.

## Required governance repair

The completed audit must add automated checks for all of the following:

1. every `AppCommandID` is owned by a feature entry;
2. every settings section is owned;
3. every Zentty and ZenttyCLI product source subtree is covered by a narrow
   source rule or explicit reviewed exclusion;
4. every public release tag through the recorded audit head is represented in
   the release evidence ledger;
5. every official docs page and screenshot observation is mapped;
6. every source test file is mapped to a feature or test-infrastructure
   exclusion;
7. entries cannot cite closed/nonexistent owners or silently lose feasibility,
   acceptance criteria, or real-system scenarios;
8. prose counts and classifications are generated from the machine-readable
   inventory rather than copied by hand.

The prior validator remains useful for schema errors, duplicate IDs, missing
files, and required individual agent entries. It is not a completeness proof
until these coverage gates are implemented and mutation-tested.

## Implementation rule

All feasible entries will be scheduled. `REQUIRED_INITIAL_RELEASE` identifies
the coherent product needed before calling the Linux port usable;
`REQUIRED_LATER` is still committed parity work, not permission to drop it.
`PLATFORM_ALTERNATIVE` must state the Linux user outcome and chosen mechanism.
`BLOCKED` must name the prerequisite. `NOT_APPLICABLE` is reserved for behavior
whose user outcome genuinely has no Linux meaning, not for inconvenient APIs.

Tests are written before each slice. User workflows cross the real staged or
installed app, native compositor, Ghostty surface, PTY, processes, filesystem,
socket, notification/service boundary, and external executable wherever that
feature owns those boundaries. Deterministic pure tests and mutation testing
support these product scenarios; they do not substitute for them.
