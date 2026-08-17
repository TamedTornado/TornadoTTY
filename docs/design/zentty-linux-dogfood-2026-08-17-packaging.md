# Zentty Linux dogfood: installable Debian package

Date: 2026-08-17
Epic: GH-9
Current child: GH-51

This record begins before packaging changes. The existing
`linux/tests/staged-bundle` proves that a relocated build tree can run, but it
is not an installable artifact and has no package-manager ownership contract.
GH-51 freezes that contract before GH-52 constructs a package.

## Initial observations

- The first supported target is the machine already used for controlled Linux
  qualification: Ubuntu 24.04 LTS (Noble), amd64, glibc 2.39, GTK 4.14, and
  libadwaita 1.5. RPM, Flatpak, and AppImage are deliberately deferred rather
  than being implied by a generic claim of “Linux support.”
- The staged application already relies on a private relative layout:
  `bin/zentty-linux` has an `$ORIGIN/../lib` RPATH, the product locates the
  sibling `zentty` executable, and product resources live below
  `share/zentty`. Installing the binaries directly into `/usr/bin` would break
  that relationship and put private libraries in the global `/usr/lib`
  namespace. The package will therefore preserve the tree under
  `/usr/lib/zentty` and publish relative symlinks from `/usr/bin`.
- `libghostty-gtk-embed.so` directly needs the system GTK, GLib/GIO,
  libadwaita, X11, Wayland, libc, and libm stack. Zentty must use Debian ELF
  dependency generation for the final dependency set. Only the pinned Ghostty
  embedding library and its non-system `libgtk4-layer-shell.so` dependency are
  approved private bundled libraries.
- Ghostty uses the installed `xterm-ghostty` terminfo database as the sentinel
  for resource discovery on Linux. The GTK embedding build currently emits
  only its headers and libraries, so packaging must explicitly generate and
  install the compiled terminfo entries. Treating terminfo as optional would
  silently degrade `$TERM` behavior and remote shell integration.
- Inspection of Ghostty's real generated resource tree corrected an initial
  manifest assumption: `x/xterm-ghostty` is the compiled file and
  `g/ghostty` is the relative symlink `../x/xterm-ghostty`, not the reverse.
  The manifest now preserves that generator-owned topology exactly.
- `readelf` exposed a build-tree leak in the current embedding library:
  `RUNPATH` contains a relative `.zig-cache/o/...` directory in addition to
  `$ORIGIN` and a system library directory. GH-52 must reject and repair that
  leak rather than copying it into the package. `$ORIGIN` itself is required
  so the private layer-shell library remains discoverable.
- Debian maintainer scripts cannot safely enumerate home directories. Ordinary
  uninstall and `dpkg --purge` therefore must not delete per-user XDG config,
  state, data, or cache. An explicit per-user cleanup operation may be added
  later, but it is not a package-manager side effect.

## Ratified construction boundary

1. `.deb` is the only first-class artifact in this epic.
2. `/usr/lib/zentty` owns the relocatable private application tree;
   `/usr/bin/zentty` and `/usr/bin/zentty-linux` are relative symlinks into it.
3. Standard desktop, AppStream, icon, terminfo, copyright, and changelog files
   use their normal `/usr/share` locations.
4. A clean Git tree and exact Zentty and Ghostty revisions are mandatory.
   Debian versions identify the source commit; timestamps come from
   `SOURCE_DATE_EPOCH`, never the wall clock.
5. The checked-in install manifest is closed-world. The package builder must
   expand every declared tree to a file-level receipt and fail on undeclared,
   missing, duplicate, unsafe, or incorrectly mode-owned paths.
6. Runtime libraries are system-owned unless explicitly listed in the two-item
   bundled-library allowlist. Final `Depends` must be generated from actual
   ELF linkage, not maintained as a guessed handwritten list.
7. Install, upgrade, uninstall, and purge tests use isolated package-manager
   roots. Controlled Wayland and X11 installed-product journeys follow only
   after the real artifact exists; environmental absence cannot become PASS.

## Qualification order

1. Validate the policy and manifest, including negative mutations.
2. Build a deterministic package and audit its expanded contents, ELF paths,
   dependencies, metadata, modes, licenses, and provenance.
3. Exercise install, same-version reinstall, upgrade, uninstall, and purge in
   disposable roots, including preservation of user XDG data.
4. Launch the installed product through public entry points under controlled
   Wayland and X11, exercising a real Ghostty PTY and installed resources.
5. Reconcile the authoritative matrix and release documentation. No packaging
   cell becomes PASS merely because the staged bundle passes.

## GH-51 implementation and first qualification attempt

- Added the versioned policy and an 18-entry closed ownership manifest. The
  contract runner ties the manifest to the existing staged build outputs and
  rejects unknown keys/kinds, missing resources or licenses, duplicate paths,
  unsafe destinations and traversal, absolute links, wrong modes, undeclared
  libraries, unknown formats/architectures, package-script home deletion, open
  manifests, and build-script contradictions.
- Added the runner's negative suite to `qualify-local` support qualification.
  Focused policy tests, qualification-matrix tests, ShellCheck, JSON parsing,
  and diff hygiene passed.
- The first full qualification invocation was run inside the filesystem/network
  sandbox and failed, correctly rather than skipping: controlled nested display
  self-tests could not create Unix/X11 sockets, and `prepare-ghostty` could not
  resolve GitHub. The matrix recorded `prepare-ghostty` as FAIL and made no
  qualification claim. This is environmental evidence, not a product failure
  and not a pass; the same gate must be rerun with its established permissions.
- The permitted full run executed all cells but did not pass. The Claude-team
  fixture deliberately terminated the controlled model endpoint after seeing
  its required teammate record, but another request could be inside the next
  JSONL append at termination. The dead writer therefore left a truncated
  trailing record; this was a crash-consistency defect in the evidence writer,
  not a live reader/writer race. The Wayland Open With journey also sent its
  first physical chord after observing application focus but without an
  end-to-end compositor input acknowledgement; that chord was lost under load.
  Their X11 twins passed. An initial focused agent rerun accidentally resolved
  ambient Gemini 0.55.1 rather than the matrix-pinned 0.53.0 and correctly
  failed its executable-identity check. None of these failures was converted
  into a pass or a packaging exception.
- A supervisor invocation also carried `CARGO_BUILD_JOBS=1` from an obsolete
  workaround for an already-fixed memory-amplification bug. That limit is not
  product policy and will not be applied to subsequent qualification runs;
  bounded matrix scheduling remains the load-control owner.

## Concurrency repairs

- Replaced direct JSONL append in the controlled Anthropic endpoint with an
  atomic same-directory transaction: read only committed newline records,
  write and `sync_all` the complete next receipt, then rename it over the
  public receipt. Deliberate process termination can now expose either the old
  complete receipt or the new complete receipt, never a partial public record.
  A deterministic pre-rename interruption test proves that the old receipt
  remains parseable and that the next publication recovers the abandoned
  transaction.
- Added a bounded real Wayland input-readiness handshake to the existing Open
  With journey. It sends the physical command-palette chord, observes the
  product transition, dismisses it through physical Escape, and only then runs
  the single-chord feature assertion. It retries the readiness probe at most
  three times but does not retry or weaken the feature assertion. Absence of
  real compositor delivery remains a failure.
- The controlled endpoint unit suite passed 6/6 with loopback permission. Four
  concurrent real Claude-team journeys passed with real Claude 2.1.201,
  Ghostty, PTYs, staged CLI/tmux shim, controlled model endpoint, teammate
  input, deliberate endpoint termination, and teardown. Four concurrent real
  Open With journeys passed in separate nested Cage/Wayland environments; all
  four established physical input on the first readiness attempt and then
  completed the real desktop/custom/terminal/SSH/canonical-target journey.
  Strict Clippy, formatting, ShellCheck, policy negatives, matrix focused
  tests, and diff hygiene pass after the repair.

## Second parallel qualification findings

The next normal-parallelism matrix run proved the first two repairs: both
`agent-integration-wayland` and `open-with-wayland` passed. It exposed two more
pre-existing evidence races rather than producing a clean receipt:

1. Debug Ghostty and Zentty write diagnostics to one redirected descriptor from
   multiple threads. A `terminal-ready-pane=pane-agent` marker interleaved with
   a renderer diagnostic at byte granularity, so exact-line grep reported that
   the source pane was absent even though both real PTYs had started. The
   closed-pane journey now waits for the two PTY children's independent CWD
   receipt instead of treating a shared debug log as an atomic event stream.
2. The Wayland notifications journey traversed settings focus with rapid Tab
   events and inspected an asynchronously written focus log after each send.
   Under load it could observe that Import Audio had been focused after an
   additional Tab was already queued, then send Return to the wrong control.
   The settings window now reports every completed GTK focus-widget change.
   The journey sends exactly one Tab and waits for its corresponding focus
   transition before sending another; Return is sent only while the unique
   `notification-sound-import` widget is the latest acknowledged focus owner.

Four concurrent real Debug/io_uring closed-pane restore journeys passed after
the PTY-receipt repair. Four concurrent notification journeys passed with real
GTK settings, physical input, private D-Bus, freedesktop notification daemon,
native audio chooser, `aplay` through the controlled ALSA device, restart
persistence, removal, and explicit unavailable behavior. A harmless grep
diagnostic on a not-yet-created restart log was also silenced at the shared
bounded-wait helper; absence still times out and fails.

The focus receipt initially used a nonexistent convenience accessor and then
an ambiguous `focus` method shared by GTK traits; both attempts failed at
compile time and were not retained. The final implementation explicitly calls
`GtkWindowExt::focus` and names the focused widget. A strict whole-binary
Clippy probe subsequently reached unrelated, pre-existing lint debt in
`agent_runtime`, `application`, `application_commands`, `attention_inbox`,
`sleep_inhibitor`, and `window_chrome`; its one new `map_unwrap_or` finding in
the focus receipt was repaired with `map_or_else`. The ordinary build, focused
real journeys, format gate, and the existing qualification policy remain the
acceptance gates for this packaging change; unrelated lint debt is neither
silently repaired here nor represented as a passing strict whole-binary lint.

## Third parallel qualification findings

The next normal-parallelism full run passed the repaired Claude-team,
Open With, closed-pane, and notifications cells, but it was still not a clean
receipt. The X11 consolidated agent cell completed its adapter journey and
then returned 124 before the remaining journeys completed. Isolating its real
components showed that the installed Codex lifecycle had inherited the
user-facing close-confirmation defaults: physical Ctrl+Q correctly presented
`Quit Zentty?`, while the test incorrectly expected an unconditional exit.
The harness now writes the same explicit no-confirmation test policy already
used by the other lifecycle journeys. The corrected installed Codex journey
and the complete five-part X11 agent cell pass with real Codex 0.147.0, Claude
2.1.201, Ghostty PTYs, staged wrappers and CLI, controlled model endpoints,
physical lifecycle input, team teardown, tmux compatibility, and crash/clean
session restoration.

The same full run also exposed another Wayland focus race in bookmark preset
export/import. The harness treated a historical focus receipt as current and
advanced Tab traversal on a 50 ms timer; under load it could queue input ahead
of GTK, or activate before the compositor acknowledged the focused button.
Bookmark traversal now checks the latest focus owner, sends one physical Tab,
and waits for an actual focus transition before advancing. A dropped nested
compositor chord retries the same step rather than assuming progress. Save
activation now emits a product-owned receipt; the harness may retry the real
activation only until that receipt appears, and then waits for the modal entry
focus without sending another activation. The corrected real nested-Wayland
journey passed export through the GTK chooser, physical deletion, file import,
and portable persistence. These were test-policy defects, not converted skips.

The following full run passed both corrected bookmark cells and both
consolidated agent-integration cells, then found one more X11-only identity
assumption in the agent-fleet journey. While the fleet popover was visible,
GTK exposed its native X11 transient with the same `Zentty` title and process
ID as the two application windows. The harness counted all three visible
titled windows and failed before routing, even though the product still had
exactly two real toplevels. X11 routing now accepts only candidates whose EWMH
window type is `_NET_WM_WINDOW_TYPE_NORMAL`; titled popup/transient windows
remain real but cannot masquerade as product toplevels. The corrected journey
passed with two real windows, moved live Ghostty PTY state, cross-window agent
fleet lifecycle, physical routing, and controlled StatusNotifier publication.

## Accepted GH-51 qualification receipt

After the final repair, `linux/tests/qualify-local` completed under normal
matrix parallelism in 715,830 ms. Every presently executable support test and
matrix cell passed: 152 declared `PASS`, zero `FAIL`, five `BLOCKED`, one
`XFAIL`, and fifteen `NOT_IMPLEMENTED`. The machine summary correctly claims
only the implemented local suite and product boundary; release and full Linux
qualification remain not passed because the declared gaps still exist.

Debug IBus-focus Valgrind is **PASS with reviewed suppressions**, not an
unsuppressed clean result. Its preserved raw receipt reports 427 errors in 427
contexts, 6,240 definite bytes, and 41,461 indirect bytes. The reviewed
post-suppression receipt reports zero errors, contexts, definite bytes, and
indirect bytes, with all 427 contexts accounted for by the governed effective
suppression set. ReleaseSafe Valgrind remains `XFAIL`. The final run also
passed policy negatives, matrix-runner tests, suppression governance, real
nested Wayland and X11 journeys, Ghostty regression, and both Debug and
ReleaseSafe product coverage. No blocked, expected-failing, or unimplemented
cell is described as exhaustive QA.
