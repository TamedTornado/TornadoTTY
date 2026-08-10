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
