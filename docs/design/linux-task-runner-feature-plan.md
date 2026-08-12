# Linux project task-runner feature plan

Date: 2026-08-11
Tracking: GH-19, GH-22

## Product outcome

Opening the command palette in a real project pane discovers the same task
sources as source Zentty, presents each runnable or diagnostically disabled
task with its source and exact command, and launches an enabled task in a real
Ghostty terminal rooted at the source directory. Zentty discovers and launches
tasks; it does not interpret or replace the project task runner.

## Source authority

The port follows `Zentty/TaskRunners`, `CommandPaletteTaskRunnerTests`, and the
task-runner path in `RootViewController`. Required sources are:

- `package.json` scripts with package-manager metadata/lockfile selection;
- `Taskfile.yml`/`Taskfile.yaml`, local includes, descriptions, and required
  variable diagnostics;
- JSONC `.vscode/tasks.json`, supported workspace/CWD variables, environment,
  and unsupported-variable diagnostics;
- public `justfile` recipes;
- public or documented `Makefile` targets;
- `mise.toml`, `mise-tasks`, and `.mise/tasks` tasks.

Nearest project sources precede ancestor sources and duplicate labels remain
individually addressable. Hidden/private and unsupported parameterized tasks
must not silently become runnable.

## Architecture constraints

1. Models, bounded ancestry discovery, parsers, quoting, ordering, and stable
   identity live in one focused `zentty-core` task-runner module.
2. Discovery is a read-only snapshot created when the palette opens. There is
   no watcher, alternate workspace authority, task daemon, or second command
   registry.
3. Dynamic entries use the existing `CommandPaletteItem` and GTK action router.
4. Execution uses the existing `WorkspaceState`, `PaneRuntimeCoordinator`, and
   Ghostty surface lifecycle. It must not run a host-side child behind the
   terminal or fake terminal output.
5. The dynamic action parameter identifies a discovered snapshot entry, not a
   shell command supplied by GTK. Activation revalidates the source file and
   task identity before launch so stale or forged parameters cannot execute.
6. Commands are source-derived and shell-quoted. No task-file contents become
   an outer shell program except through the named task runner's normal CLI.
7. An action with environment overrides or a busy/unknown focused prompt opens
   a new pane with the source working directory and real environment. Reusing
   an idle focused pane is allowed only after Linux has a trustworthy prompt
   activity signal; absence of that signal must prefer a new pane.

## Test-first construction order

### A. Core red tests

- Port source discovery cases for all six formats, including ancestry,
  malformed child sources, duplicate titles, ordering, descriptions, command
  quoting, package-manager precedence, JSONC, includes, parameters, variables,
  environment, and disabled reasons.
- Add Linux safety cases for bounded files, symlink escape, non-regular files,
  hostile names, path boundaries, include cycles/escapes, invalid UTF-8, and
  deterministic stable IDs.
- Test snapshot revalidation against changed/deleted sources and forged IDs.

### B. Product integration

- Add task entries to the existing palette snapshot and route them through one
  parameterized action.
- Launch a new pane using the existing topology/runtime transaction, exact
  source CWD, environment, and task command. Failed surface creation must roll
  back without a phantom pane.
- Disabled tasks remain visible/searchable and do not execute.

### C. Real-system journey

- In staged ReleaseSafe Zentty under controlled X11 and Wayland, open a real
  project containing representative task files.
- Drive the real command palette using physical input, select a task, and prove
  its exact CWD/environment/argv outcome through the real Ghostty PTY.
- Change a source after palette discovery and prove stale activation is refused.
- Prove a hostile/unrelated parameter cannot launch a command and terminal
  focus/lifecycle remains correct.

### D. Qualification

- Add the focused and product tests to the authoritative matrix without
  introducing another orchestration layer.
- Run strict formatting/Clippy, the full workspace, controlled compositor
  journeys, mutation tests with repository copy safety, and every presently
  executable matrix cell.
- Update the feature inventory and dogfood report with every failure, repair,
  receipt, and remaining limitation before promotion from `NOT_IMPLEMENTED`.

## Explicit exclusions

- Task Manager, development-server control, Open With, editor integration, and
  arbitrary user-defined commands are separate features.
- Zentty does not install npm/pnpm/yarn/bun/task/just/make/mise for the user.
- The port does not execute disabled tasks by guessing missing parameters.

## Parity closeout — disabled task sources and full-format product evidence

GH-18 has now delivered the single Open With authority that the initial task
runner slice depended on. The remaining task-runner parity work will therefore
be completed without adding another launcher or palette route.

Construction order:

1. Add red Open With policy tests for a real regular file: desktop and editor
   executable targets receive its canonical URI/path, while directory-only
   terminal options reject it. Existing directory behavior must remain exact.
2. Make a disabled task palette item activatable, revalidate its opaque task
   snapshot, then open only its discovered source file through the configured
   primary Open With target. It must never execute the disabled command, guess
   parameters, or trust a GTK-supplied path.
3. Expand the one existing staged-product journey—do not create a second
   harness—to materialize package.json, Taskfile include, VS Code JSONC, just,
   make, mise.toml, and mise script tasks. Drive representative enabled and
   disabled entries through the real palette and prove exact file/CWD/command
   boundaries with real compositor input and Ghostty PTYs.
4. Preserve the existing changed/deleted/forged, symlink, hostile-text, bounded
   file, ordering, and mutation evidence. Promote `utilities.task-runners` only
   after both controlled compositor journeys and the complete matrix pass.

Acceptance criteria:

- [x] Disabled task selection opens its exact canonical source in the primary
      configured application and cannot execute task text.
- [x] Every source-advertised format is represented in the staged-product
      fixture and discoverable through the real command palette.
- [x] Representative enabled tasks from non-VS-Code formats launch with their
      exact source CWD and runner command in real Ghostty panes.
- [x] X11 and Wayland journeys, strict Clippy, workspace tests, governed
      mutation evidence, and every presently executable matrix cell pass.
