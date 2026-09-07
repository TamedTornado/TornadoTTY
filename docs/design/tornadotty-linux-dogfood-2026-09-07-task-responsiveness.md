# Task discovery and launch responsiveness — 2026-09-07

Issue: [#163](https://github.com/TamedTornado/TornadoTTY/issues/163), parent #160.
Previous report: [pane-action recovery](tornadotty-linux-dogfood-2026-09-07-pane-action-recovery.md).

## Failure and repair

Task discovery at palette open and task source revalidation at activation ran
synchronously on GTK. The existing task-runner journey now has an optional
slow-I/O case: strace delays reads of its private, real VS Code task file by
three seconds. Path filtering was separately checked: unrelated reads are not
delayed. No syscall result is forged and no product fault hook was introduced.
Against staged `0cde1727`, this case fails because palette input blocks behind
discovery. This is not proof of the cause of an earlier live-client freeze.

Discovery and validation now run on Gio workers, each with one process-wide
permit, no queued backlog, and permit lifetime tied to actual worker completion
even if its window disappears. Validation retains its permit through application
of its result. Repeated requests report busy/retry instead of accumulating
threads. Other palette providers remain immediately usable.

A catalog owns both its source context and actions. Reopening clears the old
catalog. Delayed discovery requires the same palette generation and source pane;
validation requires the same focused pane/project/topology. Closed/moved/changed
targets cannot receive a late launch. Existing source fingerprint validation is
unchanged. Refresh preserves the search text and selected action without taking
focus. Existing warning UI handles rejected launch errors; no second scheduler,
task parser, or error system was added.

The first async attempt rejected valid catalogs because shell prompt hooks can
change foreground PID. Tasks are project-scoped, not owned by the momentary
foreground command, so the existing PaneContext snapshot is normalized to omit
that field for tasks only. Pane ID, worklane, topology generation, directory,
remote identity flag, and shutdown checks remain. The first slow-input test
also clicked away the palette during input setup then sent Escape to the shell;
moving setup before palette opening fixed that fixture error. No retries or
relaxed latency assertions were used to make these cases pass.

## Focused validation

```sh
ZENTTY_BUILD_OUTPUT_DIR=build/gh163-tasks linux/scripts/build-local
ZENTTY_LINUX_BINARY="$PWD/build/gh163-tasks/bin/zentty-linux" \
  linux/tests/nested-x11 linux/tests/rust-task-runners
ZENTTY_TASK_RUNNER_SLOW_IO=1 \
  ZENTTY_LINUX_BINARY="$PWD/build/gh163-tasks/bin/zentty-linux" \
  linux/tests/nested-x11 linux/tests/rust-task-runners
```

- Normal journey PASS: all advertised source formats, changed source rejected,
  disabled task opens its actual source, real PTY launches with correct CWD,
  environment and quoted argv. Repeated search waits now count new observations
  rather than matching historical results.
- Slow-read journey PASS: query input and real shell input remain responsive;
  reopened palette cannot queue another discovery or receive old results;
  fresh discovery succeeds; changing CWD during validation rejects the launch.
- Existing closed-pane-restore journey PASS after the async change, including
  all eight failed creation actions and failed Undo followed by real recovery.
- Integrated build, package notices, age audit (91 packages, zero exceptions),
  touched-file formatting, shell syntax, and diff checks PASS.
- Worker admission test PASS; shared PaneContext regression PASS. One unrelated
  sidebar GTK test matched the latter filter and retained its existing IGNORE
  because it requires a controlled display.
- Scoped worker mutations: **4 caught, 0 missed, 3 unviable** in two minutes,
  using `linux/tests/mutate-rust`, four workers, systemd resource isolation,
  gitignore=true and copy_target=false. No broad mutation-strength claim.
- Strict GTK Clippy retains six baseline findings; two new let-else/style
  findings were corrected. No suppressions or unrelated lint cleanup.

Mutation command:

```sh
linux/tests/mutate-rust -p zentty-linux \
  -f crates/zentty-linux/src/application_shell/task_runner_runtime/worker.rs \
  -j4 --cargo-test-arg=--bin=zentty-linux \
  --cargo-test-arg=task_runner_runtime --timeout 15
```

## Scope and delivery

#163 stays open: close-time process inspection, bookmark storage, other catalog
and monitor work, broader PTY-output/backpressure proof, and remaining acceptance
in the IO-boundaries report are not completed by this task-runner repair.
Workers bound concurrency, not kernel filesystem latency; a stuck filesystem
can keep its one worker occupied, but must not block GTK or accumulate workers.
No new parser limits or format semantics were invented in this batch.

No full qualification or new matrix totals, Wayland claim, install, or live
client restart. Build is staged for Jason's agreed later QA batch.
