# Zentty Linux SSH identity dogfood record — 2026-08-10

Tracking: GH-17 (`terminal.remote-ssh-transfer`)

## Frozen scope

- The next source-backed terminal feature is SSH pane identity, not remote
  upload yet. A trustworthy upload decision first needs a live destination tied
  to the exact pane-owned process tree.
- Existing Linux restoration preserves workspace topology and restores approved
  agent drafts. Recently closed ordinary panes may prefill a previous command;
  they do not execute it. SSH will preserve that safety boundary: this slice
  never silently reconnects a network session.
- Source inspection found that macOS does not trust terminal title text alone.
  `PaneSSHProcessProbe` walks the pane root process tree, requires a process
  whose actual name is `ssh`, reads its argv, and prefers the deepest nested SSH
  destination. Linux must preserve that ownership check using `/proc`.
- Ghostty already has the necessary engine-owned primitive internally:
  `Termio.getProcessInfo(.foreground_pid)`. The proposed ABI addition exposes
  only that generic PTY fact. SSH parsing, polling, presentation, and all product
  policy remain in Zentty.

## Discoveries and repairs

- The first local prerequisite check found `/usr/bin/ssh` but no `sshd`. The
  initial package-install escalation was rejected because installing a network
  daemon is a persistent system change. Jason then explicitly approved
  `openssh-server` provided package service startup was blocked and the daemon
  was used only as a private disposable test fixture.
- Installation used a temporary `policy-rc.d` returning 101. Apt confirmed that
  the service and socket were not started. Ubuntu nevertheless created systemd
  enablement symlinks for `ssh.socket`; these were explicitly disabled, and
  both `ssh.socket` and `ssh.service` were verified disabled and inactive. The
  unrelated Unity apt source also reported a pre-existing missing signing key;
  package installation came from the signed Ubuntu Noble update archive.
- A first unprivileged loopback-server proof failed inside the filesystem/network
  sandbox because local socket creation and netlink inspection were prohibited.
  The approved unsandboxed proof then authenticated successfully against an
  unprivileged `127.0.0.1` high-port daemon using generated Ed25519 host/client
  keys. No system service was enabled or used.
- The first controlled-product actor used `ss` to infer listener readiness.
  `ss` returned `RTNETLINK answers: Invalid argument` inside the nested desktop
  environment even though direct SSH authentication worked. The fixture now
  treats a successful key-authenticated SSH command as readiness evidence. This
  is both more portable and closer to the behavior the product actually needs.
- Source parity required more than checking Ghostty's immediate foreground PID:
  macOS scans the owned process tree and prefers the deepest `ssh`. Linux now
  follows bounded `/proc/<pid>/task/<pid>/children` edges, caps the tree at 256
  nodes, checks exact `comm == ssh`, bounds every proc read to 64 KiB, and
  rechecks process start times to reject PID reuse.
- The first Linux projection performed that bounded `/proc` walk directly from
  the GTK timeout callback. The source implementation explicitly detaches its
  process scan, so leaving the Linux scan on the UI thread would have created a
  periodic responsiveness risk even with strict read bounds. The coordinator
  now snapshots only pane IDs and Ghostty foreground PIDs on the GTK thread,
  performs `/proc` inspection through GLib/GIO's existing blocking worker pool,
  and applies the result back on the default main context. An explicit
  `ssh_probe_in_flight` guard permits at most one scan per window; a slow scan
  skips subsequent timer ticks rather than accumulating work. This preserves
  the architecture's one-runtime rule and adds neither a thread runtime nor a
  second process registry.
- Typing the full dynamic SSH command through Wayland virtual-keyboard input
  exposed a physical-input limitation: capital letters in long OpenSSH option
  names were corrupted before they reached Bash. The test was not weakened to
  programmatic terminal injection. Instead, the controlled external fixture
  installs a lowercase `zentty-ssh-fixture` executable on the pane's ordinary
  `PATH`; physical input launches it, and it execs the real OpenSSH client.
- One manual Wayland attempt set `GDK_BACKEND=wayland` outside
  `nested-wayland-input`. Its internal Xvfb transport deliberately overwrote
  that outer value, so the actor actually ran on X11 and correctly failed the
  Wayland journey. The matrix contract sets `GDK_BACKEND=wayland` inside the
  wrapped command. Repeating that exact form reported Ghostty's windowing
  protocol as Wayland.
- The Debug Wayland consolidated actor completed the SSH transition but later
  failed an existing exact focus receipt because Ghostty Debug output
  interleaved bytes into Zentty's stderr line. The canonical ReleaseSafe actor,
  which is the matrix's agent-integration profile, passed the complete journey
  on controlled Wayland and X11. This does not convert the Debug receipt
  interleaving into a pass; it remains an observed test-output limitation.
- The generic sidebar receipt previously printed every pane label verbatim.
  Once an SSH identity became a label, this leaked `user@host` despite the
  coordinator's redacted state-only receipt. The receipt now reports only
  `label-present` and `custom`; exact destination parsing and presentation
  precedence remain asserted in pure state tests, while the real actor proves a
  post-transition render occurred without logging the destination.
- The first focused mutation run found two real parser coverage gaps and three
  timeout mutants: an argv boundary was not exercised, the distinction between
  value-consuming and flag-only SSH options was not asserted, and mutations of
  manual index increments could create an infinite loop. Tests now cover a
  trailing flag and a flag-only option before the destination. The parser scan
  is bounded structurally by the finite argv slice while still using checked
  indexing, so malformed option bookkeeping cannot hang the parser. The rerun
  completed all 32 generated mutants in 54 seconds: 31 caught, one unviable,
  zero missed, and zero timed out.
- `qualify-local` still has no help-only mode. Invoking it with `--help` began
  the real support suite. That partial attempt failed before the matrix because
  the managed Ghostty checkout had not yet fetched the newly pushed lock
  revision; the isolated async-ABI self-test cannot clone a commit absent from
  its source checkout. This is not qualification evidence. Running
  `linux/scripts/prepare-ghostty-source` fetched and detached the managed
  checkout at the exact locked commit before the authoritative rerun.
- The first authoritative matrix rerun executed all cells but correctly failed
  two audit cells and rejected the tracked enum XFAIL as an unexpected skip.
  The Ghostty API snapshot had been recorded from the developer worktree, which
  has an `upstream/*` remote-tracking namespace; the reproducible managed clone
  has only the locked fork's `origin/*` refs. Restoring the snapshot's canonical
  `upstream_remote_tracking_ref_available=false` makes the audit independent of
  developer-only remotes while retaining the exact official-base object check.
- The enum ABI probe compared the source header with
  `<ghostty-source>/zig-out`, but qualification deliberately builds Debug and
  ReleaseSafe into isolated install prefixes. The old default prefix therefore
  contained a valid but stale pre-feature header and produced prerequisite exit
  77 rather than the required tracked-defect exit 99. The probe now accepts an
  explicit install prefix, and the matrix binds its Debug XFAIL cell to the
  exact Debug install produced by its build dependency. The self-test retains
  the default-prefix fixture, including its stale-header negative case. Focused
  reruns passed both API audits, the matrix runner tests, and the self-test; the
  real Debug installed header then reproduced only the reviewed enum-size
  mismatch and exited 99 as declared.
- The enum probe also accepted a missing installed header and silently fell
  back to compiling only the source header. That could have converted a missing
  build artifact into the tracked XFAIL. The runner now requires the installed
  header, and its self-test creates an exact installed copy before proving both
  the current XFAIL and the stale-header prerequisite failure.
- A subsequent complete matrix run reached the repaired audits and XFAIL, but
  an existing `rust-pane-search` assertion failed after creating pane 4 during
  a frozen Global Find. A focused rerun passed, exposing a timing-dependent
  harness assertion rather than a product failure: it required a duplicate
  unchanged-state receipt after pane creation, while the product is entitled
  not to emit state when the frozen result set does not change. The actor now
  uses its immediately following physical Return navigation as the causal
  observation and requires `selected=1 total=3`; admitting pane 4 or losing the
  selection anchor fails the same assertion without relying on a redundant
  log emission. The failed matrix run is retained as discovery evidence and is
  not a qualification pass.

## Evidence obtained so far

- Ghostty change `c92768fae16bff6fc849f53176c1c77813e6135a` adds one
  generic scalar query in 11 lines across the public header, Zig wrapper, and
  version allowlist. Its Debug GTK embed build, C/C++ signature checks, exact
  export allowlist, and null/foreign/uninitialized C contracts pass.
- The consolidated session actor starts a real disposable OpenSSH daemon,
  launches the real client through physical compositor input and a real
  Ghostty PTY, observes the remote identity and rendered projection, exits the
  SSH command, observes local identity, and proves the original pane and shell
  continue without a second terminal-ready event.
- The complete ReleaseSafe actor passes under private X11 and private
  input-capable Wayland. The SSH server is stopped and its generated keys,
  config, logs, and launcher are removed by the existing actor cleanup path.
- The feature inventory advances `terminal.remote-ssh-transfer` from
  `NOT_IMPLEMENTED` to `PARTIAL`. Actual remote upload/paste, collision,
  cancellation, hostile filename, and checksum behavior remain unimplemented
  and must not be inferred from identity detection.

## Final qualification receipt

- The final exact-tree `linux/tests/qualify-local` run completed in 369,400 ms
  and passed every presently executable support test and matrix cell. Declared
  totals are `PASS=91`, `FAIL=0`, `BLOCKED=7`, `XFAIL=1`, and
  `NOT_IMPLEMENTED=21`. The implemented local suite and product boundary pass;
  release qualification and full Linux qualification remain false.
- Debug Valgrind is **PASS with reviewed suppressions**, not an unsuppressed
  clean result. The preserved raw receipt contains 427 errors in 427 contexts,
  6,240 definite bytes, and 41,428 indirect bytes. The suppression-enabled
  receipt contains zero post-suppression errors, contexts, definite bytes, and
  indirect bytes, with 427 reviewed suppressed errors/contexts. ReleaseSafe
  Valgrind remains the declared XFAIL and no suppression was broadened.
- The final real SSH journey passed in the consolidated ReleaseSafe session
  actor on controlled X11 and input-capable Wayland. Focused parser mutation
  remained zero-missed/zero-timeout, strict workspace Clippy passed with
  warnings denied, and the pinned full Ghostty regression remained the
  qualification floor.
- A supplemental `cargo test --workspace --locked` attempt inside the network-
  restricted command sandbox denied the agent IPC tests' private Unix-socket
  creation with `EPERM`; that run is an environmental failure, not a pass. The
  identical workspace command was rerun with the required controlled host
  permission and passed every unit, integration, and doc test.
