# Zentty Linux dogfood — development-server discovery and control

Date: 2026-08-10
Issues: GH-19, GH-20, GH-22

## Slice contract

The authoritative construction and acceptance order is
[`linux-development-server-feature-plan.md`](linux-development-server-feature-plan.md).
Every discovery, red test, failure, repair, real-system receipt, platform
difference, and remaining limitation is recorded here as the feature is built.

## Initial source audit

- The source feature is not merely “find port 5173.” It has distinct models for
  normalized candidates, manual/watch/Docker/scanner origins, explicit/PID/CWD/
  worklane confidence, first-seen-preserving registry merge, deterministic
  relevance, ignored port rules, browser targets, output URL detection, passive
  listener polling, Docker publication, menu modeling, IPC commands, and safe
  process termination.
- Source stop safety permits only scanner observations with PID ancestry. It
  rescans at activation time, rejects missing/unowned listeners, avoids the pane
  shell's process group, sends SIGINT, and schedules bounded SIGKILL escalation
  only if the same process remains alive.
- Source URL policy accepts HTTP/HTTPS only, requires a port, canonicalizes
  loopback/wildcard hosts to localhost, allows private/link-local hosts, retains
  paths/query/fragments, and rejects public or hostile schemes.
- Linux already has the required real ownership inputs without another Ghostty
  API: each live `GhosttySurface` exposes the foreground process ID, and the
  existing workspace owns pane IDs/CWDs. The missing platform boundary is a
  bounded `/proc` listener/inode/process-tree observer plus product projection.
- The existing GH-19 acceptance criteria also require container-published
  discovery. That obligation remains explicit in the plan instead of quietly
  treating a host socket as Docker evidence.
- The first source-derived core tests were written before implementation. The
  required red run failed to compile because no server URL, port-rule,
  detection, or relevance types/functions existed in `zentty-core`; no hidden
  Linux implementation satisfied the new product contract.
- The Linux `/proc` fixture tests were likewise written before the observer.
  Their red run failed at compile time because neither `PaneProcessContext` nor
  `scan_listeners_at` existed. The fixtures use real proc-file formats and Unix
  socket-inode symlinks rather than a mocked observer interface.
- The source `[server_detection]` config test was added before extending the
  new process config authority. Its red run failed because `AppConfig` had no
  `server_detection` field; the feature did not create an ad-hoc environment or
  preferences path to bypass GH-20.
- The real loopback-listener observer test initially failed before executing
  its assertions because the restricted development sandbox denied `bind(2)`
  with `EPERM`. The identical test was then run with the existing elevated
  `cargo test` boundary and passed against the host's real `/proc` socket and
  process data. Environmental socket denial was recorded rather than treated
  as product evidence or converted into a pass.
- The registry merge test was written against the source precedence and
  first-seen contracts before a Linux registry existed. Its red compile failed
  on the missing `ServerRegistry` export. The implementation is a pure keyed
  registry: a source refresh replaces only that worklane/source, preserves
  surviving first-seen timestamps, and merges equal origins by source
  precedence rather than whichever poll completes last.
- The first orchestration-contract run rejected the new real TCP fixture as an
  unreviewed controlled-model endpoint because its allowlist intentionally
  catches every loopback `TcpListener`. The repair does not weaken that scan:
  it names and requires this one real development-server fixture, excludes
  exactly that path from the model-endpoint comparison, and leaves every other
  unexpected listener implementation failing the contract.
- Termination authorization was added red-first. The compile failure proved no
  prior helper could bless a stop request. The pure gate now requires a
  scanner/PID-confidence record, the same pane on both record and fresh
  observation, an owned current listener, a listed port, and valid non-init
  process IDs. Like the source, a listener may equal the current pane process;
  the safety rule is that Zentty must never target the pane process *group*.
  Signaling remains outside this pure decision.
- Adding the source Stop Server verb made the earlier product query by bare
  port correctly return two results (Open and Stop), so the journey's stale
  one-result assertion failed. The repair queries the user-visible verb plus
  port for each action. It does not weaken execution: both dynamic parameter
  routes still cross the real command palette, and Stop is additionally proved
  by the owned fixture PID exiting after an observed SIGINT delivery.
- The ignored-port persistence test was written before a config mutation API
  existed and failed to compile on the missing method. The write path edits the
  existing TOML document rather than serializing `AppConfig`, so comments and
  unknown keys survive. A config-file symlink remains a symlink while its
  resolved target is replaced by a private, fsynced, same-directory temporary
  file; the test exercises that real filesystem behavior.
- Both controlled compositor journeys now pass with a staged ReleaseSafe
  product, real Ghostty PTY, real dynamic TCP listener, exact external browser
  argv receipt, durable ignore/unignore, and graceful owned-process stop. The
  X11 Docker variant additionally starts a real disposable `busybox` HTTP
  container with a Compose project-path label and dynamic published port;
  Zentty attributes it as `Docker`/`Cwd` while leaving Stop Server unavailable
  for that non-owned process.
- Docker discovery uses the source's bounded CLI-inspection approach. Docker
  commands have a two-second deadline, capped stdout, no stdin, exact argv, and
  are run only every third listener poll. A pure inspect fixture rejects an
  unrelated project and a database while retaining the matching web service.
  Scanner and Docker records merge through the one `ServerRegistry`; there is
  no second product/process registry.
- The source audit after passive discovery exposed a material remaining path:
  output-reported URLs are delivered by `zentty server watch`, not inferred
  from the listener alone. The CLI has source commands `set`, `clear`, `list`,
  `open`, and `watch`, plus internal `watch-set`/`watch-clear`. This was added to
  the feature plan before implementation rather than silently promoting the
  passive subset to feature parity.
- The authenticated route was added to the existing pane-token Unix socket and
  existing process `AgentRuntime`. It has its own bounded server request/reply
  contract and receiver but no second socket, listener thread, token registry,
  workspace registry, or GLib scheduler. A real-socket test proves a spoofed
  client target loses to the token's canonical window/worklane/pane.
- Final diff review found the first server-route extraction retained the pane
  token registry lock while waiting for the GTK product reply. It did not
  deadlock the current tick order, but it could unnecessarily delay independent
  authentication. The route now authenticates, drops the lock, and only then
  waits. A real concurrent-socket test holds a server reply pending while a
  separate authenticated agent event is accepted and delivered.
- The first complete workspace run of the new registry-clear test failed only
  because the assertion assumed reverse lexical ordering. The registry's
  deterministic origin ordering was correct; the fixture expectation was
  repaired and the rerun passed 8/8. No product behavior changed to satisfy the
  mistaken assertion.
- The staged X11 journey now starts two real HTTP listeners beneath the real
  Ghostty pane. One is wrapped by the staged `zentty server watch` CLI, which
  tees its real stdout, detects the `/fixture` URL, registers through the
  authenticated socket, and appears as `Watch`/`Explicit`. A real `server list
  --json` receipt retains that path, and `server open` crosses the controlled
  `xdg-open` boundary with the exact same path. The independent scanner-owned
  listener still exercises safe SIGINT stop, so watch coverage did not replace
  the original ownership journey.
- Watch launches exact argv through `Command`, inherits stdin, concurrently
  forwards stdout/stderr, bounds its incremental detection tail, deduplicates
  reported URLs, clears only its pane's watch records on entry/exit, and
  preserves ordinary child exit codes. The browser is the only substituted
  external component in the product journey.
- The first copy-safe, file-scoped core mutation run tested 131 mutants and
  found 21 survivors. They exposed missing boundary assertions for empty host/
  port errors, IPv6 unique-local/link-local masks, range-edge removal, init PID
  rejection, and each relevance-score contribution. After adding those tests,
  one survivor remained because the test asserted only failure rather than the
  source-specific `MissingPort` result. Pinning that diagnostic produced the
  final receipt: **131 tested, 117 caught, 14 unviable, 0 missed**. Every run
  used `.cargo/mutants.toml` with `gitignore = true` and `copy_target = false`;
  the ignored multi-gigabyte build tree was never copied.
- The 2026-08-11 full qualification run exposed a load-sensitive assertion in
  the controlled Wayland journey. The first scanner pass correctly published
  the listener with CWD confidence, while the test immediately demanded the
  later PID-attributed observation after waiting only for the independent
  authenticated watch record. A standalone rerun passed, confirming a
  synchronization race rather than environmental absence or a product waiver.
  The journey now waits, with a bounded ten-second deadline, for the exact
  listener port to reach `Scanner`/`Pid` confidence before asserting it. It
  does not accept the weaker CWD observation as a pass.

## Remaining limitation

- Linux currently implements the source's system-default browser target via
  exact `xdg-open URL` argv. The source's multi-browser catalog/Open With UI is
  not yet ported; non-system `--browser` values fail explicitly instead of
  silently falling back. This remains product scope and must not be described
  as full browser-target parity.

## Browser-target parity resumed — 2026-08-12

- The task-runner and project Open With closeouts triggered a fresh source
  server audit rather than treating `xdg-open` as sufficient. Source Zentty has
  a distinct `ServerBrowserCatalog`, preferred browser, enabled browser list,
  custom browser records, menu selection, and explicit authenticated CLI
  browser selection. Linux's single-target rejection is therefore the concrete
  remaining development-server parity gap, not a merely cosmetic setting.
- Server browsers will not be merged into project Open With. They accept a
  normalized HTTP(S) URL rather than a canonical local path, have distinct
  configuration and source verbs, and include the system-default handler.
  Reusing the reviewed discovery/direct-argv pattern is appropriate; reusing
  the project catalog would erase the source boundary and create surprising
  editor/file-manager entries in a server menu.
- The closeout extends `linux/tests/rust-development-servers`; it does not add a
  browser-only scenario or a second server registry. Only the controlled
  external browser executable may be substituted in the real-product journey.
