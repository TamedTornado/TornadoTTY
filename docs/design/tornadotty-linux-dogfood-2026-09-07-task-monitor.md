# Task Manager monitoring — 2026-09-07

Issue #163, following the
[IPC dispatch budget](tornadotty-linux-dogfood-2026-09-07-ipc-dispatch-budget.md).

## Confirmed gaps and repair

- Task Manager construction ran `getconf CLK_TCK` and `getconf PAGESIZE` on
  GTK, although subsequent `/proc` sampling was already off-thread. Removed
  those subprocesses. Reused already-pinned rustix 1.1.4 with its `param`
  feature; initialization, including any auxv fallback read, now runs inside
  the existing single in-flight sampling worker. No unsafe application code,
  new worker pool, queue, product timer or release-age exception.
- Completed samples published their captured pane list without checking current
  topology. They now project current sources, accepting metrics only when
  window/worklane/pane/root-PID/remote ownership still matches. Closed panes
  cannot reappear; moved/respawned panes cannot inherit stale metrics or peaks;
  renamed panes retain current labels with valid metrics. New/changed sources
  show unavailable metrics until sampled. Rejections identify pane/window in
  diagnostics, not private process contents. Remote roots are not sampled.
- A panicked scan no longer leaves a permanently absent sampler: the next
  admitted scan can initialize it again. Shutdown/visibility gates remain.

## Focused evidence

- RED then GREEN: real GTK controller + real sampler reading a private proc
  fixture, with one FIFO stat read held in flight. GTK idle heartbeat completes
  while blocked; repeated refresh stays single-flight. Close, move, respawn,
  rename and add happen before release. Old code republishes the closed pane;
  repaired code uses current ownership and drops stale metrics. Extended the
  passing check to reject stale memory peaks too.
- 16 focused Task Manager tests PASS (private X11), including existing parser,
  tree-bound, CPU history, PID reuse and projection tests. Two selected ownership
  mutations (always accept / always reject) caught by the real GTK regression;
  baseline PASS. Scratch-safe wrapper, two workers, per-process 6 GiB VM limit.
  This is targeted mutation evidence, not a whole-subsystem kill-rate claim.
- Existing two-window real GTK/Ghostty/PTY Task Manager journey PASS with
  `getconf` deliberately replaced by an exit-86 fixture. Previous clean
  `7d205cf1` build FAILS opening the monitor with that exact getconf error.
  Journey retains live CPU/memory, grouping, search, expand, clipboard,
  cross-window focus and End Task assertions. The first attempt stopped earlier:
  its old "Add Pane Right" query now has two matches. Updated to the full
  "Add Pane Right Without Resizing" action name, then obtained actual RED/GREEN.
- First private-display Cargo invocation started rustup setup because HOME was
  isolated. Canceled it; subsequent commands explicitly reused CARGO_HOME and
  RUSTUP_HOME. No system packages or live-client state changed.
- Build/notices/dependency-age audit PASS. Rust formatting, Bash syntax and
  diff checks PASS. Clippy with tests found a similar-name lint in the previous
  dispatch test; renamed the local counter. Five existing product findings
  plus the existing catalog test-module placement finding remain; no new
  suppressions. No full qualification or fresh aggregate matrix claim.

Reproduce the focused GTK checks with the installed toolchain homes preserved:

```sh
CARGO_HOME=/home/jason/.cargo RUSTUP_HOME=/home/jason/.rustup \
linux/tests/nested-x11 cargo test -p zentty-linux --bin zentty-linux task_manager

ZENTTY_LINUX_BINARY="$PWD/build/gh163-task-monitor/bin/zentty-linux" \
linux/tests/nested-x11 linux/tests/rust-task-manager
```

Staged only at `build/gh163-task-monitor`; no install/restart, live QA pending.
#163 remains open: this does not repair the agent lifecycle `/proc` sweep on
GTK, all server-monitor stale-result paths, global monitor scaling, or the PTY
overload/capture acceptance criteria. The ownership guard compares current
source identity, not a newly introduced universal pane-generation framework.
