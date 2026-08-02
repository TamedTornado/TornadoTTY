# Zentty Linux port dogfood field report

Date: 2026-08-01

Status: **in progress**. This is a contemporaneous field report. Entries record
what was observed while extracting Ghostty's GTK surface, the consequence for
the Linux port, the repair, and the evidence available at the time. A patch is
not called successful until focused tests and the live Linux host demonstrate
the intended behavior.

## Scope and topology

The project is building a Linux Zentty application around Ghostty's existing
GTK/OpenGL terminal surface rather than reimplementing terminal emulation or
rendering.

- Product fork: `TamedTornado/zentty`, branch `linux/port`
- Engine fork: `TamedTornado/ghostty`, branch `zentty/gtk-embed`
- Zentty baseline: `dedene/zentty` at `6e021b0`
- Ghostty baseline: `dedene/ghostty` at `4e9fe4bb5`
- Official Ghostty base recorded by Zentty: `19e20f766`
- Required Zig version: `0.15.2`
- Development host: Ubuntu 24.04, native Wayland session
- Alternate display backend target: X11 through GDK/Xwayland

The upstream Ghostty application and the downstream embedding host are separate
claims. An unchanged Ghostty build establishes the baseline. The alternate host
must independently prove surface creation, PTY operation, input, resizing,
multiple surfaces, focus, and teardown.

## Acceptance and evidence standard

This port uses integration qualification as its primary credibility gate. Each
milestone must publish reproducible commands and concise result receipts for
the upstream Ghostty regression suite, a real alternate GTK host, Wayland and
X11, multi-surface lifecycle and stress behavior, and applicable live desktop,
IME, scaling, clipboard, and GPU checks. Later Zentty milestones must exercise
the packaged application end to end.

An environment that was not run is a documented gap, not a pass. Screenshots
may supplement evidence but cannot replace semantic assertions, process state,
test receipts, leak checks, or observed live behavior.

## Qualification baseline

Baseline qualification is complete for the unchanged Ghostty application.

- The public Zentty and Ghostty forks exist and are locally configured with
  their source remotes.
- Zentty pins Ghostty revision `4e9fe4bb5` on `zentty/smooth-scroll`.
- Static inspection found `GhosttySurface` implemented as an `Adw.Bin` with a
  `GtkGLArea` and a full GTK terminal interaction stack.
- Static inspection found 18 direct `Application.default()` dependencies in
  `src/apprt/gtk/class/surface.zig` plus a direct `GhosttyWindow` ancestor
  dependency.
- The development host did not initially provide Zig, GTK4 development
  metadata, or libadwaita development metadata. The baseline was built with
  checksum-verified Zig 0.15.2, Blueprint Compiler 0.16.0 at tag commit
  `04ef0944`, GTK 4.14.5, and libadwaita 1.5.0.
- Unchanged debug build passed:
  `zig build -Doptimize=Debug -Dcpu=baseline -fno-sys=gtk4-layer-shell`.
  `ghostty +version` reported GTK runtime 4.14.5, libadwaita runtime 1.5.0,
  OpenGL, X11, and Wayland support.
- The full unchanged regression command passed:
  `zig build test -Doptimize=Debug -Dcpu=baseline -fno-sys=gtk4-layer-shell`.
  The warning output was produced by intentional negative-path tests; the
  command exited zero.
- A native Wayland smoke test created and realized the GTK surface, created an
  OpenGL 4.6 EGL context, started a real PTY child, observed its zero exit after
  2002 ms, unrealized the surface, and exited zero.
- An X11/Xwayland smoke test exercised the same path with
  `GDK_BACKEND=x11`; it selected the X11 protocol, realized OpenGL 4.6,
  observed the PTY child's zero exit after 1999 ms, unrealized, and exited
  zero.

## First embedding milestone

Ghostty commit `32f631d5f` is the first working extraction spike. A plain
`GtkApplication` owns the window while an explicit, non-default Ghostty GTK
runtime owns `GhosttySurface`. The legacy Ghostty constructor remains and
delegates to the explicit constructor, limiting compatibility risk.

- `Surface` no longer reads `Application.default()` after construction; its
  allocator, core app, runtime app, configuration, window protocol, clipboard,
  notification, and teardown paths resolve through its retained owner.
- The alternate host selected native Wayland, constructed and realized the
  widget, initialized OpenGL 4.6, started `/bin/sh` through a real PTY, observed
  exit 0 after 1999 ms, destroyed and finalized the surface, and exited 0.
- The identical host selected X11/Xwayland, ran the same PTY path with exit 0
  after 1998 ms, finalized the surface, and exited 0.
- Normal Ghostty still selected Wayland, ran a real PTY child, finalized the
  surface, and exited 0 through the unchanged `Surface.new` call path.
- The complete post-patch `zig build test` command passed. Intentional
  negative-test warnings matched the baseline class of output.
- Ghostty commit `e753619f3` promoted the spike from log inspection to an
  assertion-based integration probe. It fails unless the plain host is the
  process-default application, the Ghostty core surface initializes, the PTY
  child-exit notification arrives, and every core tick succeeds. Both Wayland
  and X11/Xwayland printed `embed-spike: PASS` and exited 0.

## Lifecycle and memory qualification

Ghostty commits `44743e839` and `94fe18e78` extend the spike into a repeatable
lifecycle and memory-safety gate without adding Zentty application code.

- The default host now creates four simultaneous `GhosttySurface` widgets,
  each backed by an independent real PTY child. It asserts the exact number of
  core initializations and child-exit notifications, continues checking every
  core tick, and drains finalization before destroying the Ghostty core.
- One four-surface debug run passed on native Wayland and one passed through
  X11/Xwayland. Both observed four initializations, four child exits, four
  `surface closed` events, and `embed-spike: PASS`.
- Ten consecutive process runs passed per backend: 40 constructed, initialized,
  exercised, and finalized terminal surfaces on Wayland, plus another 40 on
  X11/Xwayland. No run timed out or returned a nonzero status.
- The complete post-change debug regression command passed:
  `zig build test -Doptimize=Debug -Dcpu=baseline
  -fno-sys=gtk4-layer-shell`.
- A focused regression test frees the source configuration arena immediately
  after building a POSIX shell subprocess command, then verifies that all
  generated `argv` strings remain valid.
- The ReleaseSafe Valgrind gate runs one real surface and PTY with the epoll
  libxev backend. Both native Wayland and X11/Xwayland passed with zero
  definite leaks, zero indirect leaks, and `ERROR SUMMARY: 0 errors` after
  narrowly suppressing confirmed process-global allocations in the dynamic
  loader, GTK GIO module scan, and fontconfig/Expat caches. Possibly-lost and
  reachable third-party process-global state remains reported, not classified
  as a test failure.

## Interaction qualification

Ghostty commit `8f5141d7e` adds a separate `gtk-embed-interaction` target. It
keeps the lifecycle probe simple while using four specialized real PTYs to
test cross-surface behavior.

- A programmatic embedder text submission plus a distinct Enter key event was
  sent through Ghostty core to a child. The child verified the exact bytes and
  returned an OSC title acknowledgement; the host asserted that acknowledgement
  before accepting child exit.
- Another child issued an OSC 52 clipboard write. The host asserted the exact
  standard-clipboard contents, asynchronously read them back through GDK, and
  pasted them into a second PTY. That child verified the exact bytes and sent a
  separate title acknowledgement. Both `clipboard-write` and `clipboard-read`
  signals were required.
- The host moved focus through all four embedded surfaces and checked each
  surface's focus state after GTK delivered the transition.
- A live window resize changed every terminal allocation and caused the resize
  child to receive `SIGWINCH`. The host independently asserted that the tested
  widget width changed from 1000 to 1200 pixels.
- Post-map content scale was read and required to be positive for every
  surface. The development display reported 1.00 on both axes.
- The ReleaseSafe interaction target passed with all semantic assertions and
  four clean child exits on native Wayland and X11/Xwayland. The full debug
  Ghostty regression suite passed afterward.

This is not yet a claim that compositor-generated physical keyboard events or
an external IME were automated. Wayland intentionally prevents ordinary
clients from globally synthesizing trusted input. The target instead proves
the input contract an embedding host calls after its toolkit has interpreted
the event. Real GTK controller and IME coverage remains a separate gate.

## Incidents and repairs

### Spike root was initially below the Zig module boundary

- **Observation:** The first spike compile rejected
  `@import("../class/surface.zig")` as an import outside the module path.
- **Consequence:** The alternate host did not compile, so it had not yet
  exercised Ghostty code.
- **Diagnosis:** Zig scopes relative imports to the root module's directory;
  placing the executable root below `src/apprt/gtk` prevented it from
  importing the rest of Ghostty's source tree.
- **Repair:** Moved only the executable root to `src/gtk_embed_spike.zig` and
  retained the GTK-specific implementation and build step isolation.
- **Evidence:** The next build compiled 91 of 93 steps and launched the host.
- **Outcome:** Harness construction issue resolved; no production API change.

### Plain GtkApplication cannot construct GhosttySurface

- **Observation:** The minimal host activated a plain `GtkApplication`, then
  panicked while executing `Surface.new(.none)`.
- **Consequence:** Ghostty's existing GTK widget cannot currently be embedded
  by another GTK application, despite GTK, GObject, Blueprint, and OpenGL
  already being portable.
- **Diagnosis:** Template initialization queried the surface's `key-sequence`
  property. `getKeySequence` called `Application.default()`, which runtime-
  checked the process default application as `GhosttyApplication`; the actual
  default was the host's `GtkApplication`, so the optional cast was null and
  the forced unwrap panicked at `class/application.zig:232`. This is the first
  dynamically proven instance of the 18 static process-global dependencies.
- **Repair:** Added a retained explicit owner and `newWithApplication`; the
  existing `new` delegates with `Application.default()` for compatibility.
  All later surface operations use the retained owner. Template getters use
  the C allocator only during the narrow GObject construction interval before
  the owner can be attached.
- **Evidence:** The debug stack passed through `getKeySequence`, the GObject
  property getter, `gtk.Widget.initTemplate`, `Surface.new`, and the spike's
  activation callback. The process exited 1, as expected for this gate.
- **Outcome:** Repaired at Ghostty commit `32f631d5f`; qualified on Wayland,
  X11/Xwayland, the normal Ghostty application, and the full regression suite.

### Anonymous constructor option structs were not interchangeable

- **Observation:** The first compatibility wrapper failed to compile because
  Zig treated the textually identical anonymous option structs on `new` and
  `newWithApplication` as distinct types.
- **Consequence:** Normal Ghostty could not build with the first draft.
- **Diagnosis:** Anonymous structs have type identity per declaration rather
  than structural interchangeability.
- **Repair:** Introduced one named `Surface.NewOptions` type and used it for
  both constructors.
- **Evidence:** The unchanged Ghostty build and subsequent full test suite
  compiled and passed.
- **Outcome:** Source compatibility preserved with a smaller shared contract.

### GLib callback constants translated as booleans

- **Observation:** The spike's first periodic core-tick callback did not
  compile when returning `G_SOURCE_CONTINUE` from a C-ABI function declared to
  return `c_int`.
- **Consequence:** The host could not yet drive the Ghostty core mailbox from
  the plain GTK event loop.
- **Diagnosis:** Zig's C translation exposed the GLib macros as booleans while
  `GSourceFunc` uses `gboolean`, an integer ABI type.
- **Repair:** Returned explicit ABI values `1` for continue and `0` for remove.
- **Evidence:** The callback compiled and drained core surface messages during
  both compositor runs.
- **Outcome:** Harness ABI corrected; no production behavior was weakened.

### X11 teardown retained the surface past core-app destruction

- **Observation:** The first successful X11 PTY run unrealized its widget but
  then asserted that `font_grid_set.count()` was nonzero in `App.deinit`.
- **Consequence:** X11 functionality worked, but lifecycle qualification
  failed and the command exited 1.
- **Diagnosis:** Destroying the X11 window was not sufficient to synchronously
  finalize every GObject before the host destroyed the Ghostty core. The
  `GtkApplication` still owned pending teardown work, and the repeating tick
  source also remained registered.
- **Repair:** The host now removes its tick source, destroys the window, quits,
  unreferences the host application, and drains the default GLib context
  before terminating the Ghostty runtime and core.
- **Evidence:** Two subsequent X11 runs finalized renderer and IO threads,
  logged `surface closed`, produced no panic, and exited 0. The matching
  Wayland rerun also exited 0.
- **Outcome:** Repaired in the integration host; this ordering becomes a
  required lifecycle contract for the eventual embedding API.

### Default-application assertion initially depended on GLib ordering

- **Observation:** The first semantic probe reported
  `default_plain=false` even though surface initialization, PTY exit, and
  teardown all succeeded.
- **Consequence:** The probe correctly refused to certify that the explicit
  owner was independent of process-default application state.
- **Diagnosis:** The host created its non-running Ghostty runtime before
  running the plain `GtkApplication`; relying on implicit GLib default-
  application selection made the assertion dependent on construction and
  startup order.
- **Repair:** The activation callback now explicitly installs the plain host
  with `g_application_set_default` before constructing `GhosttySurface`.
- **Evidence:** This recreates the original failing condition deliberately,
  but `newWithApplication` now initializes and runs the surface. Both backend
  probes asserted the plain default, core initialization, child exit, and
  tick health, printed `PASS`, and exited 0.
- **Outcome:** Test assumption repaired; process-default independence is now a
  deterministic assertion instead of an inference from logs.

### Baseline display access was unavailable inside the filesystem sandbox

- **Observation:** The first Wayland smoke command exited 1 with
  `Gtk: Failed to open display`.
- **Consequence:** No compositor claim could be made from the sandboxed run.
- **Diagnosis:** The managed command sandbox could not access the user's
  compositor socket. The Ghostty binary had already built successfully.
- **Repair:** Re-ran the identical executable and backend selection with
  explicit permission to access the live graphical session.
- **Evidence:** Both the Wayland and X11/Xwayland runs then selected their
  requested protocol, realized the OpenGL surface, ran and reaped a real PTY
  child, unrealized the surface, and exited zero.
- **Outcome:** Harness/environment issue resolved; no Ghostty source change.

### Four-surface stress exposed a shell-command lifetime bug

- **Observation:** Valgrind reported that `execve` read a command argument from
  memory already freed when the temporary surface configuration was destroyed.
- **Consequence:** The live PTY commonly appeared to work because the stale
  bytes had not yet been overwritten, but the POSIX shell command path had a
  genuine use-after-free.
- **Diagnosis:** `execCommand` cloned strings for a direct command but appended
  the `.shell` command pointer without cloning it. The pointer usually belonged
  to the temporary `Config` arena, while the subprocess retained `argv` in its
  own longer-lived arena.
- **Repair:** The shell branch now duplicates its zero-terminated command into
  the subprocess allocator, matching the existing direct-command ownership
  rule. A focused test destroys the command arena before inspecting `argv`.
- **Evidence:** The focused test passed, the full Ghostty regression suite
  passed, both four-surface compositor runs passed, and both final Valgrind
  gates completed with zero errors.
- **Outcome:** Repaired in isolated Ghostty commit `44743e839`, separate from
  the embedding API work so it can be reviewed independently.

### Debug Valgrind was too slow for a useful integration gate

- **Observation:** A four-surface Debug build under Valgrind did not complete
  within 180 seconds and had only partially initialized OpenGL.
- **Consequence:** The first memory-check command was not deterministic enough
  for CI or credible repeated qualification.
- **Diagnosis:** Instrumenting four debug OpenGL terminal stacks multiplied
  startup cost without increasing the memory-lifetime coverage needed for this
  gate.
- **Repair:** The dedicated target uses ReleaseSafe and one real surface while
  the normal debug target retains four simultaneous surfaces and repeated-run
  stress coverage.
- **Evidence:** The final one-surface Valgrind runs completed on both backends,
  exercised OpenGL, PTY creation and exit, and teardown, and exited zero.
- **Outcome:** Test matrix split by purpose rather than weakening either gate.

### Valgrind and io_uring stalled after the child exited

- **Observation:** The first ReleaseSafe Valgrind run reached PTY reader exit
  but did not deliver the child watcher event before a 300-second timeout.
- **Consequence:** The host could not complete its child-exit assertion or
  teardown under Memcheck.
- **Diagnosis:** The ordinary io_uring path passes live tests, but its kernel
  interface is not reliably driven under Valgrind instrumentation. This was a
  tool/backend interaction, not evidence that the normal runtime had stalled.
- **Repair:** Only the Valgrind target requests libxev's epoll backend before
  surface IO loops are created. Normal embedding tests continue to exercise
  Ghostty's default io_uring selection.
- **Evidence:** With epoll, Valgrind observed child exit and complete teardown
  on both Wayland and X11/Xwayland.
- **Outcome:** Deterministic instrumentation path added without changing
  production backend selection.

### Platform-global allocations initially failed the leak gate

- **Observation:** After the Ghostty command use-after-free was repaired,
  Valgrind still returned 99 for small definite and indirect allocations whose
  stacks terminated in the ELF loader, GTK's GIO module scan, or
  fontconfig/Expat configuration parsing.
- **Consequence:** Treating all process-exit allocations alike produced a
  failing gate even though none were owned by the surface or embedding patch.
- **Diagnosis:** These libraries intentionally retain process-global caches and
  dynamically loaded module metadata. Ghostty's existing suppression file did
  not match several Ubuntu 24.04 stripped-library stack shapes.
- **Repair:** Added a spike-local suppression file with stack-specific rules;
  it is applied in addition to Ghostty's existing file. Definite and indirect
  Ghostty-owned allocations, invalid reads/writes, and other address errors
  remain fatal.
- **Evidence:** Final Wayland summary: 0 definite, 0 indirect, 0 errors. Final
  X11/Xwayland summary: 0 definite, 0 indirect, 0 errors. Each run also printed
  the host's semantic `PASS` result.
- **Outcome:** Leak policy now distinguishes confirmed platform lifetime from
  product regressions without a blanket leak suppression.

### The managed sandbox made Zig's global cache read-only

- **Observation:** A post-change full regression invocation inside the
  filesystem sandbox failed before compilation with `ReadOnlyFileSystem` for
  `/home/jason/.cache/zig` and secondary `manifest_create Unexpected` errors.
- **Consequence:** That invocation supplied no source-code test result.
- **Diagnosis:** Ghostty's already-populated Zig package and compiler caches are
  outside the workspace write roots.
- **Repair:** Re-ran the identical command with explicit permission to use the
  existing cache, just as live compositor tests require explicit desktop
  access.
- **Evidence:** The elevated command completed the entire suite and exited 0.
- **Outcome:** Environment failure recorded separately; no cache deletion,
  dependency repinning, or source workaround was used.

### The first keyboard child waited for the wrong byte count

- **Observation:** The first interaction run sent keyboard input but timed out
  with only three of four children exited.
- **Consequence:** A boolean "input sent" assertion was insufficient evidence
  that the PTY received the intended data.
- **Diagnosis:** The harness counted `ghostty-embed-keyboard` as 23 bytes; it is
  22. The child therefore consumed the following carriage return as data,
  rejected the value, and deliberately stayed alive.
- **Repair:** Corrected the byte count and added a child-to-host OSC title
  acknowledgement. The target now requires both successful API submission and
  exact child verification.
- **Evidence:** Subsequent runs logged `keyboard input acknowledged by child`,
  observed that child's exit, and passed on both backends.
- **Outcome:** Harness defect repaired and the semantic assertion strengthened.

### Clipboard paste remained buffered by the PTY line discipline

- **Observation:** The host observed exact OSC clipboard write and asynchronous
  clipboard-read signals, but the receiving child remained blocked.
- **Consequence:** Toolkit clipboard success still did not prove delivery to
  the PTY.
- **Diagnosis:** The shell PTY was in canonical mode. The clipboard contained
  no newline, so the kernel correctly buffered it rather than satisfying the
  child's read.
- **Repair:** After successful asynchronous clipboard completion, the probe
  sends a distinct Enter key event to release the canonical line. The child
  verifies only the 23 clipboard bytes and acknowledges via its title.
- **Evidence:** Both backends logged clipboard write, read, exact child
  acknowledgement, and child exit before `PASS`.
- **Outcome:** Test now reflects real terminal line-discipline behavior instead
  of weakening the clipboard assertion.

### Fixed-time focus and resize checks were briefly flaky

- **Observation:** One debug run confirmed three of four focus transfers and
  another sampled the old width before Wayland applied the resize.
- **Consequence:** The first timing scheme could reject correct asynchronous
  compositor behavior.
- **Diagnosis:** The focus cycle and resize were scheduled at overlapping fixed
  deadlines, and a failed focus sample advanced rather than retrying the same
  widget.
- **Repair:** Focus retries the same surface until GTK reports it focused;
  resize runs after focus qualification and repeats its observation until the
  allocation changes or the overall deadline expires.
- **Evidence:** Stabilized Wayland and X11/Xwayland runs confirmed all four
  focus transfers and the 1000-to-1200 width change.
- **Outcome:** Harness timing repaired without extending or bypassing semantic
  failure conditions.

### Forced 2x scale did not produce a 2x Wayland content scale

- **Observation:** Running the full interaction target with `GDK_SCALE=2` and
  a required minimum content scale of 2 still reported 1.00 for every mapped
  surface and correctly failed `scales=0/4`.
- **Consequence:** High-DPI behavior is not qualified on this host; setting an
  environment variable is not accepted as proof of a compositor scale change.
- **Diagnosis:** In the native Wayland session, the compositor continued to
  advertise scale 1 to GTK. Moving the check from initialization to after map
  confirmed that early widget timing was not the cause.
- **Repair:** No production workaround was introduced. The probe retains an
  optional minimum-scale assertion for a genuine scaled compositor/monitor.
- **Evidence:** The forced run exercised and passed every other interaction
  assertion but failed only the explicit 2x scale requirement. Normal-scale
  Wayland and X11/Xwayland reruns passed with post-map scale 1.00.
- **Outcome:** Open environment gap. Qualify with a real 2x output or controlled
  nested compositor rather than manufacturing a pass.

## Product-boundary integration milestone

The disposable Ghostty-owned spike is now frozen. Product work moved to a
separate C/GTK host under Zentty, consuming an experimental shared-library and
C-header boundary from the Ghostty fork. The first real host creates a plain
`GtkApplication`, embeds the returned Ghostty widget, runs a real PTY command,
acknowledges exact terminal output through an OSC title transition, observes
initialization and child exit, continuously drives the Ghostty core mailbox,
and tears the widget and runtime down in documented order.

- The actual Zentty host passed the single-terminal semantic test on native
  Wayland and on X11/Xwayland.
- Ten independent process lifecycles passed per backend. Each run required
  activation, terminal initialization, exact child output acknowledgement,
  child exit, successful core ticks, and clean process exit.
- A staged bundle containing only the Zentty executable, Ghostty embedding
  library, and private layer-shell dependency resolved all libraries from its
  own relative `bin`/`lib` layout and passed on both backends.
- The boundary is compiled by a normal C17 compiler with warnings as errors
  against the installed public header; the test does not import Ghostty Zig
  implementation files or execute the old spike.
- The complete post-boundary Ghostty Debug regression command passed with the
  same intentional negative-path warnings as the qualified baseline:
  `zig build test -Doptimize=Debug -Dcpu=baseline
  -fno-sys=gtk4-layer-shell`.
- The engine fork commit `b8d920495` is public and pinned by full revision in
  `linux/ghostty.lock`. A clean no-override build cloned that public fork into
  the ignored dependency directory, fetched and detached at the exact lock,
  built the ReleaseSafe bundle, and passed the real host test on both display
  backends.
- The reproducible `linux/tests/qualify-local` orchestration completed from the
  pinned checkout: ReleaseSafe single-terminal, staged-bundle, and ten-process
  lifecycle gates passed on both backends; Debug Valgrind gates passed on both;
  the script then restored ReleaseSafe and passed both final semantic checks.

### The first reusable Ghostty library selected the wrong runtime

- **Observation:** The initial shared-library artifact did not compile with
  the GTK embedding implementation even though the equivalent executable
  spike did.
- **Consequence:** A working in-tree executable was not evidence of a usable
  external product boundary.
- **Diagnosis:** Ghostty selected its embedded runtime for every library
  artifact, and its shared dependency builder omitted GTK dependencies for
  libraries unconditionally.
- **Repair:** Library artifacts now select GTK only when the configured
  application runtime is GTK, and only such libraries receive the native GTK
  dependency set. Existing non-GTK libraries retain their prior behavior.
- **Evidence:** `zig build gtk-embed-lib` succeeds and exports the initial
  runtime/surface entry points through `libghostty-gtk-embed.so`.
- **Outcome:** Minimal build-system distinction added; the complete Ghostty
  Debug regression passed before the engine change was committed and pushed.

### Building Ghostty from Zentty's working directory broke resources

- **Observation:** The first Zentty build script invoked Ghostty with
  `zig build --build-file .../ghostty/build.zig` while its current directory
  remained the Zentty repository. Ghostty's resource generator failed with
  `FileNotFound`.
- **Consequence:** The local integration build was not reproducible across
  repository boundaries.
- **Diagnosis:** Parts of Ghostty's resource build intentionally resolve paths
  relative to the repository working directory, not only the build-file path.
- **Repair:** The script enters the selected Ghostty checkout in a subshell and
  invokes its ordinary build command there.
- **Evidence:** Resource generation and the embedding-library build complete
  through the Zentty build script.
- **Outcome:** Host build now respects the engine repository's build contract
  rather than patching resource paths downstream.

### Bundling the private layer-shell library exposed ELF lookup mistakes

- **Observation:** The Zentty executable linked, but its first real Wayland
  run exited 127 because `libgtk4-layer-shell.so` was not found, even though
  the file had been copied beside the Ghostty library.
- **Consequence:** A successful link falsely appeared to be a distributable
  integration.
- **Diagnosis:** Ghostty's `-fno-sys=gtk4-layer-shell` build produces a private
  dynamic library inside Zig's cache. The first resolver also parsed ldd's
  `=> not found` text as a path, and the executable's RUNPATH does not supply a
  transitive lookup path when the dependent Ghostty library has its own
  RUNPATH.
- **Repair:** The build identifies the exact built dependency, copies it into
  the product `lib` directory, supplies a link-time `rpath-link`, and gives the
  Ghostty embedding library a Linux `$ORIGIN` RUNPATH so its private dependency
  is discoverable beside it.
- **Evidence:** `ldd` reports no missing dependency; isolated staged bundles
  resolve both non-system libraries from their own relative layout and pass
  the full semantic host test on Wayland and X11/Xwayland.
- **Outcome:** Relocatable local bundle qualified. System packaging and ABI
  versioning remain open work.

### The actual host reproduced the Valgrind io_uring stall

- **Observation:** The first product-boundary Valgrind run initialized OpenGL,
  started the real PTY, and saw its read thread exit, but it never received the
  child watcher notification and timed out after 300 seconds.
- **Consequence:** Reusing the spike's memory result would not qualify the real
  shared-library boundary or Zentty teardown.
- **Diagnosis:** The real host reproduced the previously observed Valgrind and
  io_uring incompatibility. An environment variable invented by the first
  Zentty test script had no effect because the library exposed no way to make
  the required pre-event-loop backend selection.
- **Repair:** The experimental C boundary now offers explicit default, epoll,
  and io_uring runtime construction. The memory test requests epoll through
  that API; ordinary product runs retain automatic selection.
- **Evidence:** With explicit epoll, the instrumented real host observes exact
  terminal output, child exit, teardown, and semantic `PASS` in seconds rather
  than timing out.
- **Outcome:** Runtime capability exposed instead of hiding a test-only switch
  inside the Zentty process.

### The first product Valgrind completion found unresolved value reports

- **Observation:** After the epoll repair, Valgrind completed with zero
  definite and indirect leaks but returned 99 for uninitialized-value and
  conditional-branch reports. The tail of the first receipt showed Ubuntu's
  stripped `librsvg` while GTK initialized its SVG icon loader.
- **Consequence:** The actual-host memory gate is still red despite successful
  product lifecycle assertions and zero owned leaks.
- **Diagnosis:** The first diagnosis was incomplete: Ghostty's existing
  suppression does expect an librsvg symbol absent from this stripped system
  library, but a follow-up run with Valgrind suppression generation showed
  earlier reports in Ghostty's `ensureLocale` path through Zig's writer and
  memcpy implementation. A first bounded librsvg-to-GDK suppression therefore
  could not and should not hide all 32 contexts.
- **Repair:** No broad suppression and no passing claim has been made. Exact
  suppression candidates were captured for diagnosis. The next run must
  separate optimizer/tool false positives from an actual initialization bug;
  product-owned `ensureLocale` frames are not classified as external noise.
- **Evidence:** Current receipt records `integration PASS`, 0 bytes definitely
  lost, 0 bytes indirectly lost, and `61 errors from 32 contexts`.
- **Outcome:** Open ReleaseSafe instrumentation investigation. This is recorded
  as a failure, not converted into a pass.

### Debug instrumentation separated the ReleaseSafe reports

- **Observation:** Rebuilding the identical shared-library boundary in Debug
  mode eliminated the Ghostty `ensureLocale` value reports. With the narrow
  external SVG-loader suppression, the actual Zentty host passed Valgrind on
  both Wayland and X11/Xwayland.
- **Consequence:** The memory gate needs to state which artifact it qualifies;
  silently substituting Debug for the ReleaseSafe product would overclaim.
- **Diagnosis:** The remaining product-owned reports are specific to optimized
  Zig writer/memcpy code generation under Valgrind on this host. This narrows
  the issue but does not yet prove those reports harmless.
- **Repair:** The build accepts an explicit optimization mode and writes build
  metadata containing that mode and the exact Ghostty revision. The memory
  script refuses to run unless the current host is a recorded Debug build.
- **Evidence:** Debug Wayland and X11/Xwayland receipts each contain semantic
  `integration PASS`, zero definite leaks, zero indirect leaks, and
  `ERROR SUMMARY: 0 errors`.
- **Outcome:** Debug memory safety qualified for the real product boundary.
  ReleaseSafe value correctness remains a separate open gate and is not
  covered by this pass.

### The first memory script still defaulted to the sibling checkout

- **Observation:** Review after the full matrix found that the build defaulted
  to the pinned ignored checkout, while the memory script still defaulted its
  suppression-file path to `../ghostty`.
- **Consequence:** The test happened to pass on the development machine but
  would fail—or consume files from the wrong revision—on a clean public clone.
- **Diagnosis:** The pinned-source change updated the build path but missed one
  test-only default. The recorded binary metadata was correct; the auxiliary
  suppression source was not guaranteed to match it.
- **Repair:** The memory test now defaults to the same pinned
  `build/linux-deps/ghostty` checkout and retains an explicit
  `GHOSTTY_SOURCE_DIR` override for engine development.
- **Evidence:** Static path audit now gives the build and memory gate the same
  no-override source root. A clean-clone CI run remains a required independent
  confirmation.
- **Outcome:** Reproducibility defect repaired before the Zentty commit.

## Multi-terminal product milestone

Status: **qualified locally**. This reviewable increment moves simultaneous
surface, focus, and resize qualification out of the frozen Ghostty spike and
into the real Zentty executable. Acceptance requires four terminal widgets in
a product-owned GTK grid, four independent PTYs with exact output and exit
acknowledgements, focus transfer through every surface, a product layout reflow
observed by a terminal allocation, clean teardown, and identical
semantic results on Wayland and X11/Xwayland. The one-terminal, staged-bundle,
repeated-lifecycle, memory, and Ghostty regression gates must remain green.

The host now accepts a bounded integration-only surface count. Invalid counts
must fail before GTK or Ghostty initialization with a stable usage exit status;
normal product startup remains one terminal until actual Zentty workspace state
owns pane creation.

### Focusing the wrapper widget did not focus Ghostty's terminal area

- **Observation:** The first four-terminal Wayland run initialized all four
  OpenGL surfaces, acknowledged all four exact outputs, and observed all four
  child exits, but timed out with only the initially focused terminal
  confirmed and no resize qualification.
- **Consequence:** Generic `gtk_widget_grab_focus()` on the returned public
  widget cannot implement pane selection, so the product boundary is
  insufficient even though terminal rendering and PTYs work.
- **Diagnosis:** `GhosttySurface` is a composite widget. Its Zig `grabFocus`
  method deliberately focuses the private `GtkGLArea`; asking GTK to focus the
  outer `AdwBin` does not transfer focus into that private child.
- **Repair:** Added one narrow embedding operation that delegates focus to the
  terminal surface without exposing the private GTK child or Ghostty Zig type.
  The Zentty test verifies the result through GTK's public root focus ancestry
  rather than trusting the operation call.
- **Evidence:** Failed receipt records `initialized=4 titles=4 children=4
  focus=1 resize=0` and a semantic `FAIL` after the 12-second deadline.
- **Outcome:** Subsequent Wayland runs confirmed all four focus transfers; no
  timing increase or weakened focus assertion was accepted.

### A fixed resize target matched the four-pane minimum allocation

- **Observation:** After adding the narrow focus operation, the next Wayland
  run confirmed all four focus transfers but timed out with resize still
  unchanged.
- **Consequence:** The focus repair was proven independently, but the combined
  milestone remained red.
- **Diagnosis:** Four rendered terminal widgets caused GTK to choose a window
  allocation at least as large as the fixed 1200-pixel test target. Reapplying
  that default therefore did not guarantee a size transition.
- **Repair:** The test now reads the mapped window allocation and requests an
  explicit increase of 200 by 100 pixels. It still requires the compositor to
  apply the request and a terminal allocation to change before passing.
- **Evidence:** The failed intermediate receipt records `focus=4 resize=0`;
  a second relative-default-size attempt also left the allocation unchanged.
- **Outcome:** GTK documentation confirms default size is an initial-size hint,
  not a deterministic live resize mechanism. The product test now reflows its
  four live panes from a two-column grid to a one-column grid and requires the
  terminal allocation to change. A genuine externally driven compositor
  resize remains a separate environment gate rather than a manufactured pass.

### Four live product panes passed on both display backends

- **Observation:** With the focus boundary and deterministic product reflow,
  the real Zentty host completed its four-terminal scenario on native Wayland
  and X11/Xwayland.
- **Consequence:** Simultaneous surfaces, pane focus, and allocation changes no
  longer rely on the disposable in-tree Ghostty spike.
- **Evidence:** The receipt requires and records four initializations, four
  distinct OSC output acknowledgements, four focus confirmations, a changed
  terminal width after live grid reattachment, four child exits, successful
  core ticks, semantic `PASS`, and process exit zero.
- **Outcome:** Multi-terminal behavior qualified on both backends. The existing
  single-terminal and staged-bundle gates were rerun on both and remained
  green; stress, memory, and full Ghostty regressions remain pending.

### Product-boundary input and clipboard qualification

Status: **qualified locally**. A separate four-pane scenario targets operations
the product needs but generic `GtkWidget` does not expose for Ghostty's private
terminal child. The proposed boundary adds only terminal focus, UTF-8 input,
and an asynchronous standard-clipboard paste request.

The test does not accept successful API return values as proof. One real PTY
must verify exact submitted keyboard text and acknowledge it through an OSC
title. A second PTY must issue an OSC 52 clipboard write; Zentty must place the
exact value onto the real GDK clipboard, request an asynchronous paste into a
different surface, observe Ghostty's completion signal, release the canonical
line with a carriage return through the same text path, and receive a second
exact child acknowledgement. All prior
multi-pane focus, reflow, lifecycle, and backend assertions remain active.

#### Key injection initially ignored Ghostty's input-effect result

- **Observation:** The first ReleaseSafe library build failed because the new
  Enter-key wrapper discarded `CoreSurface.keyCallback`'s non-void
  `InputEffect` result implicitly.
- **Consequence:** The C boundary did not compile; no interaction result was
  claimed.
- **Diagnosis:** The wrapper needs only success/failure for its boolean ABI,
  but Zig requires every semantic return value to be handled explicitly.
- **Repair:** Explicitly discard the effect after successful dispatch while
  retaining error-to-false conversion.
- **Evidence:** The corrected ReleaseSafe library and C host compile with
  warnings as errors.
- **Outcome:** Compile-time contract caught and repaired before host testing.
  The key-specific wrapper was subsequently removed entirely: carriage return
  through the generic text path exercises the required PTY behavior without a
  one-value, test-shaped public key enum.

#### Exact input and clipboard round trips passed on both backends

- **Observation:** The real four-pane Zentty interaction scenario passed on
  native Wayland and X11/Xwayland.
- **Evidence:** The keyboard API reported submission, but the gate passed only
  after the target PTY read and compared `zentty-product-keyboard` and emitted
  its acknowledgement. A different PTY emitted OSC 52 containing
  `zentty-product-clipboard`; Zentty observed the exact write, stored it in the
  GDK standard clipboard, started Ghostty's asynchronous paste on another
  surface, observed `clipboard-read`, sent carriage return, and received the
  child's exact acknowledgement. Four focus transitions, live pane reflow,
  four child exits, core ticks, and teardown also passed.
- **Outcome:** Programmatic input and real GDK clipboard round trips qualified
  through the product boundary on both display backends. Physical key-event
  translation and IME remain separate gates.

The public header is also compiled from an independent C17 contract program
with warnings as errors. It verifies invalid async-backend rejection, null-safe
runtime destruction, null runtime tick and surface creation failure, null-safe
focus, and false-returning null input/clipboard operations before any desktop
integration test runs.

The complete Ghostty Debug regression suite passed after the focus, text, and
clipboard operations were added. Its warning output remained the same
intentional negative-path class recorded at baseline.

Ghostty commit `7fa70f310` contains only the three minimal surface operations
and their C declarations. It is public on `zentty/gtk-embed` and is pinned by
full revision in Zentty. The initially proposed one-value key API was not
committed.

The final `linux/tests/qualify-local` run at this milestone passed from the
public full-revision lock. It covered the C ABI contract, single terminal,
four simultaneous terminals, exact keyboard and bidirectional GDK clipboard
round trips, focus, live pane reflow, staged relocation, ten process
lifecycles, and final ReleaseSafe semantic checks on both display backends;
the pinned full Ghostty Debug regression and both Debug Valgrind gates also
passed. The orchestrator restored a ReleaseSafe artifact before exiting.

#### Shell syntax check glob included the new C contract source

- **Observation:** A pre-regression `bash -n linux/tests/*` invocation tried to
  parse `api-contract.c` as shell and failed at `int main(void)`.
- **Consequence:** The regression command did not start; this supplied no
  engine test result.
- **Diagnosis:** Adding a C source beside executable test drivers made the old
  broad shell glob invalid.
- **Repair:** Syntax checks enumerate executable shell drivers rather than all
  files in the directory. The C source continues to compile with `-Werror` in
  the normal build.
- **Evidence:** The corrected driver ran against exact pinned commit
  `7fa70f310` and the complete suite exited zero.
- **Outcome:** Test invocation error recorded separately from product status;
  the regression itself is green.

## Current next gate

The Ghostty spike remains historical evidence and must not evolve into the
product. The next product stage is a real Linux worklane UI and persistent
workspace model using the now-qualified multi-surface boundary; normal startup
intentionally remains one terminal until that product model owns pane creation.
The current gap inventory and status are authoritative only in
`linux/qualification-matrix.json`. This historical checkpoint predates the
controlled X11 physical-key and external-resize harnesses added on 2026-08-02;
it must not be read as the current result. Local desktop receipts alone are not
the final automation standard.

## 2026-08-02 exhaustive-coverage audit

Status: **implemented local suite PASS; release and full Linux qualification
NOT PASSED**. A second-pass audit rejected the premise that the milestone was
already exhaustively tested. The live feature paths were strong, but several
public-boundary misuse and reproducibility cases had only comments or implicit
behavior rather than executable contracts.

### Runtime and widget handle misuse was not defended

- **Observation:** The first C contract checked null handles and an invalid
  backend, but did not exercise concurrent runtimes, stale runtime reuse,
  runtime recreation, uninitialized surfaces, or ordinary GTK widgets passed
  to Ghostty-specific operations.
- **Consequence:** The public API could overwrite process-global Ghostty state
  on a second constructor call and blindly cast a foreign `GtkWidget`, making
  accidental client misuse capable of corruption rather than a clean failure.
- **Diagnosis:** Header prose said one runtime was allowed and functions
  accepted `GtkWidget *`, but the implementation did not enforce either
  contract.
- **Repair:** In progress: track the one active runtime, reject foreign/stale
  handles without dereferencing them, allow recreation only after teardown,
  and validate every surface operation with Ghostty's registered GObject type.
  Expand the independent C17 contract to cover all these states, including an
  uninitialized real Ghostty surface.
- **Evidence:** The independent C17 contract now passes with both epoll and
  io_uring construction under Wayland and X11. Each process checks invalid,
  null, foreign, concurrent, stale, and uninitialized states plus active tick,
  real surface construction, idempotent free, and recreation rejection.
- **Outcome:** Public handle and process-lifecycle contract hardened and
  executable on all four backend combinations.

### A pinned revision did not guarantee a clean source tree

- **Observation:** The build and regression scripts compared `HEAD` with the
  lock but did not reject modified tracked or untracked files in the managed
  dependency checkout.
- **Consequence:** A nominally pinned qualification could compile local edits
  while its metadata reported only the public commit, undermining
  reproducibility.
- **Diagnosis:** Git revision identity and worktree identity were conflated.
- **Repair:** Managed builds now validate lock syntax, exact origin URL, clean
  status before checkout, full revision after checkout, and record
  `ghostty_tree=clean`. Explicit development overrides may build a dirty tree
  but record it as dirty; the pinned regression gate always rejects dirt.
- **Evidence:** The managed public build completed with exact origin/revision
  and wrote `ghostty_tree=clean`; the regression driver intentionally pointed
  at the dirty development checkout failed before invoking Zig with the
  expected clean-tree diagnostic.
- **Outcome:** Pin integrity and dirty-tree rejection qualified.

### Host option parsing had branch gaps and no unit boundary

- **Observation:** Surface-count parsing lived as a private function inside the
  500-line GTK host, accepted libc whitespace/sign variants implicitly, and
  tested only the value `5`. An explicitly invalid async backend silently fell
  back to automatic selection, while explicit io_uring was not wired at all.
- **Consequence:** Configuration failure behavior was neither strict nor
  exhaustively enumerable, and successful public async constructors did not
  prove the product selected them.
- **Diagnosis:** Pure option policy and GTK process orchestration were mixed.
- **Repair:** Extract strict pure C parsers for canonical counts 1 through 4
  and `auto`/`epoll`/`io_uring`. Add table-driven unit coverage for nulls,
  boundaries, signs, whitespace, leading zeroes, overflow-shaped strings,
  spelling/case errors, and null outputs. Add host exit-64 wiring checks and
  real mapped PTY runs for both async backends on both display backends.
- **Evidence:** Pure parser tests compile with `-Werror` and pass every table
  row. The actual host rejects invalid count/backend wiring with exit 64.
  Mapped real-PTY runs pass for epoll and io_uring under both Wayland and X11.
- **Outcome:** Option policy separated from GTK orchestration and all declared
  branches covered at unit and process-integration levels.

### The C host build used only baseline warnings and implicit hardening

- **Observation:** Host and contract programs used `-Wall -Wextra -Werror`,
  but relied on distribution defaults for optimization, PIE, RELRO, immediate
  binding, stack protection, executable-stack policy, and fortified libc.
- **Consequence:** A ReleaseSafe Ghostty library could be packaged beside an
  unoptimized host whose hardening was accidental rather than reviewable.
- **Diagnosis:** The build distinguished Ghostty optimization modes but did not
  apply the same intent to the C boundary.
- **Repair:** Centralize strict C flags including pedantic, conversion, shadow,
  format, prototype, and missing-prototype diagnostics. Build Debug hosts with
  `-O0 -g3`; build release hosts with `-O2`, fortification, and assertions
  disabled. Link PIE with stack protector, RELRO, NOW, and non-executable stack,
  and record host optimization in metadata.
- **Evidence:** Strict release rebuild passed. Metadata records
  `host_optimize=Release`; ELF inspection reports PIE (`DYN`/`FLAGS_1 PIE`),
  GNU RELRO, `BIND_NOW`, and a read/write non-executable GNU stack.
- **Outcome:** Compiler policy and binary hardening made explicit and verified.

#### GTK's public header failed whole-program pedantic mode

- **Observation:** The first strict build failed because GTK 4.14's
  `gdkdmabufformats.h` expands an autoptr macro with an extra file-scope
  semicolon rejected by GCC `-Wpedantic -Werror`.
- **Consequence:** Applying project diagnostics indiscriminately to dependency
  headers made a valid host impossible to compile.
- **Diagnosis:** `pkg-config` supplies GTK include directories as ordinary
  `-I` paths, so GCC attributes dependency-header pedantic diagnostics to the
  application compilation.
- **Repair:** Retain all actionable strict warnings globally; apply pedantic
  error mode to the pure host-options module/test that has no GTK header
  expansion. Do not suppress or patch GTK downstream.
- **Evidence:** Hardened GTK host/API builds pass without pedantic suppression;
  the pure option module/test passes with pedantic errors enabled.
- **Outcome:** Dependency diagnostic isolated without weakening project-owned
  pure C coverage.

#### GObject validation initially passed an untyped pointer

- **Observation:** The first hardened ReleaseSafe build failed in
  `gobject.ext.cast` because `anyopaque` is not statically guaranteed to be a
  `GTypeInstance`.
- **Consequence:** The new foreign-widget defense did not compile; no contract
  result was claimed.
- **Diagnosis:** The public ABI erases the pointer type, while the generated
  GObject helper intentionally requires an explicit type-instance assertion
  before it performs the runtime type check.
- **Repair:** Convert the non-null ABI pointer to `GTypeInstance` explicitly,
  then use the generated checked cast to `GhosttySurface`.
- **Evidence:** Corrected ReleaseSafe build completed and foreign `GtkButton`
  operations were rejected without a crash in all contract runs.
- **Outcome:** Compile-time type boundary preserved rather than bypassed with a
  blind `Surface` cast.

#### Runtime recreation crashed in process-global signal setup

- **Observation:** The expanded contract safely rejected a concurrent runtime
  but panicked in Zig's POSIX signal setup when it attempted to create another
  runtime after fully freeing the first.
- **Consequence:** The header's initial "one active runtime" wording implied a
  sequential lifecycle that Ghostty process-global state does not support.
- **Diagnosis:** `GlobalState.deinit` is sufficient for process shutdown, not
  for reinitializing every signal and native subsystem in the same process.
- **Repair:** Enforce one successful runtime construction for the entire
  process lifetime. Freeing clears the active handle so stale operations fail,
  but does not reopen construction. The header now states this explicitly and
  the contract requires recreation rejection.
- **Evidence:** Recreation is rejected cleanly in four independent contract
  combinations; no second Ghostty initialization occurs.
- **Outcome:** Real lifecycle defect found by negative coverage; no attempt was
  made to broaden Ghostty's global reinitialization semantics for Zentty.

#### Constructing Ghostty after `gtk_init` violated initialization order

- **Observation:** The first recreation repair still panicked during the
  contract's initial valid runtime construction because the expanded C test
  created a GTK button—and therefore called `gtk_init`—before Ghostty.
- **Consequence:** The contract exposed another undocumented process-level
  precondition rather than reaching its lifecycle assertions.
- **Diagnosis:** Ghostty runtime initialization owns native signal and GTK
  setup ordering. The real Zentty host already constructs it first, but the
  public header did not state that requirement.
- **Repair:** Construct the one Ghostty runtime before `gtk_init` or any other
  GTK object, document the ordering in the public header, then create the
  foreign GTK widget used for type-rejection checks.
- **Evidence:** Both async backends passed under both display backends when the
  runtime was constructed first.
- **Outcome:** Initialization order is now an explicit ABI contract instead of
  an accidental property of the working host.

### DOGFOOD-2026-08-02-QUALIFICATION-MATRIX: prose exclusions were invisible to automation

- **Observation:** Several known gaps were listed only in narrative text. The
  old `qualify-local` shell sequence could pass without representing physical
  key translation, IME, external resize, real compositor scaling,
  ReleaseSafe Valgrind, installation ownership, or controlled public CI.
- **Consequence:** “The local script passed” was too easy to misread as
  release or full Linux qualification. A missing command was indistinguishable
  from a deliberately blocked requirement.
- **Diagnosis:** There was no executable inventory against which prose and the
  runner could be reconciled.
- **Repair:** `linux/qualification-matrix.json` now owns every cell and one of
  five explicit states: `PASS`, `FAIL`, `BLOCKED`, `XFAIL`, or
  `NOT_IMPLEMENTED`. The runner validates the complete ReleaseSafe/Debug ×
  Wayland/X11 × default/epoll/io_uring × single/multi product grid and required
  non-functional capabilities, executes every PASS/XFAIL command in order,
  treats exit 77 as an unexpected skip, and writes JSON plus a short report.
  It independently reports implemented-local, release, and full-Linux claims.
- **Evidence:** Runner self-tests deliberately remove a required cell, invent
  a status, skip unexpectedly, make an XFAIL stale, fail a PASS command, and
  assert an impossible full-qualification claim. Every malformed fixture is
  rejected and the valid fixture reports local-only success.
- **Outcome:** No known gap can disappear from qualification silently. The
  authoritative matrix supersedes older gap prose in this report.

### The first ABI audit found 14,259 exported implementation symbols

- **Observation:** The experimental shared library exported the eight intended
  `ghostty_gtk_embed_*` functions plus thousands of statically linked FreeType,
  Fontconfig, XML, ImGui, and other dependency symbols.
- **Consequence:** A nominally tiny C ABI accidentally exposed a huge unstable
  surface and could interpose dependency symbols in a host process.
- **Diagnosis:** Zig's dynamic library link included static dependencies with
  default symbol visibility and no ELF export policy.
- **Repair:** Add one Linux version script beside the experimental Ghostty
  library and set it only on that build artifact. Zentty's `abi-surface` gate
  compares every exported text symbol with the exact eight-function allowlist.
- **Evidence:** The rebuilt library reports eight exported text symbols, all
  versioned `GHOSTTY_GTK_EMBED_1.0`; any extra or missing symbol fails the local
  matrix.
- **Outcome:** The fork's experimental ABI is genuinely minimal without
  changing normal Ghostty build products.

### GCC's analyzer could not prove the parsed terminal bound

- **Observation:** Enabling `-fanalyzer` failed the first host rebuild with a
  possible out-of-bounds write while clearing the fixed four-slot terminal
  array during teardown.
- **Consequence:** The parser does enforce one through four, but that fact lived
  in another translation unit and was not locally visible to the analyzer or a
  future maintainer changing the assignment.
- **Diagnosis:** Teardown trusted a cross-module invariant at the final memory
  write rather than defending the array locally.
- **Repair:** Bound the teardown loop by both the selected count and the actual
  array capacity. Retain `-fanalyzer` under `-Werror` for all project C builds.
- **Evidence:** Debug and release C builds pass conversion, shadow, format,
  prototype, stack-protection, and analyzer diagnostics.
- **Outcome:** Static analysis produced a useful defense-in-depth repair rather
  than being disabled as a false positive.

### Controlled X11 was locally feasible; sandbox display absence was not a pass

- **Observation:** Xvfb and xdotool were installed, but an unelevated Xvfb
  probe could not open its own display socket. The same probe outside the
  filesystem/process sandbox succeeded.
- **Consequence:** Treating the first environmental failure as either product
  failure or skipped success would have misclassified a feasible matrix cell.
- **Diagnosis:** The managed command sandbox, not GTK or Ghostty, prevented the
  nested X server socket from operating.
- **Repair:** Add a deterministic `controlled-x11` harness using Xvfb at a
  fixed 1280×800×24 screen and software OpenGL. Missing Xvfb/xdotool exits 77,
  which the matrix rejects as an unexpected skip.
- **Evidence:** The nested server independently maps the real product host and
  completes both the physical-key and external-resize drivers.
- **Outcome:** Controlled local X11 is PASS. Controlled Wayland and public CI
  remain explicit BLOCKED cells; they were not inferred from this result.

### Physical X11 key translation now crosses the native GTK event path

- **Observation:** The prior input test called the embedding C text-injection
  API and explicitly did not prove native GDK key translation.
- **Consequence:** Keyboard input could pass while real key events were broken.
- **Repair:** In controlled Xvfb, xdotool focuses the mapped Zentty window,
  types `zentty-physical-key`, and sends Return. The child PTY acknowledges only
  the exact line by changing the terminal title; the integration process then
  requires that title and clean child exit.
- **Evidence:** The X11 physical-key matrix cell passes with the real GTK
  controller, Ghostty input handling, PTY, shell, terminal parser, and title
  notification in the path.
- **Outcome:** Physical X11 key translation is PASS. Wayland remains BLOCKED on
  a controlled compositor plus virtual-input prerequisite, not silently
  skipped.

### An external X11 client now drives the resize qualification

- **Observation:** The existing multi-terminal test reflowed its own GTK grid;
  this proved live surface allocation changes but not a compositor/server-side
  window resize.
- **Repair:** The host exposes a test-only readiness point after four mapped
  surfaces and focus qualification. A separate xdotool process discovers the
  X11 window and changes it from the outside; the host passes only after its
  terminal allocation changes and all PTYs complete.
- **Evidence:** The controlled Xvfb run records `external resize ready`, an
  externally requested 700×500 window, a changed terminal width, and the exact
  four-surface integration PASS summary.
- **Outcome:** Externally driven X11 resize is PASS. The Wayland cell remains
  NOT_IMPLEMENTED until a controlled compositor-side driver exists.

### DOGFOOD-2026-08-02-RELEASESAFE-VALGRIND: optimized instrumentation is still red

- **Observation:** The real four-surface ReleaseSafe interaction completes its
  semantic PASS, but Valgrind exits 99 with optimized-build
  uninitialized-value reports. After narrowly suppressing the independently
  diagnosed system Fontconfig/IBus caches, Wayland reports zero
  definite/indirect bytes and 695 errors; X11 reports 160 definite bytes, 66
  indirect bytes, and 668 errors.
- **Consequence:** A Debug-only clean result may not be represented as memory
  qualification for the shipped optimization mode.
- **Repair:** Make ReleaseSafe Valgrind a command-backed XFAIL on both display
  backends with this tracking ID. The runner treats exit 77 as an unexpected
  skip and a zero exit as a stale XFAIL, so the defect cannot become permanent
  folklore or a false pass.
- **Evidence:** Both command-backed XFAILs ran in the final matrix. Each receipt
  includes the exact four-surface semantic PASS followed by its Valgrind
  failure; neither is an environmental skip.
- **Outcome:** Unresolved. This XFAIL prevents release and exhaustive/full
  Linux qualification.

### The first full matrix invocation was interrupted by the command session

- **Observation:** The initial long matrix invocation lost its controlling tool
  session midway through ReleaseSafe X11 interaction. The cell log was opened
  but empty, and no summary was written.
- **Diagnosis:** The independently rerun X11 interaction passed in 6.6 seconds;
  the aborted invocation left no product failure receipt.
- **Repair:** Treat an absent final summary as an incomplete run, never as a
  pass. Rerun the entire matrix under a persistently polled command session
  after all audit changes are complete.
- **Outcome:** In progress; final totals must come only from the completed JSON
  summary.

### The first completed matrix correctly failed a dirty regression checkout

- **Observation:** The first end-to-end matrix reached every cell, but the
  Ghostty regression cell rejected the explicit development checkout because
  the audited changes were still uncommitted.
- **Consequence:** The implemented-local result was FAIL even though the build
  intentionally permits a dirty developer override and records it as dirty.
- **Diagnosis:** The build override and the pinned regression gate have
  deliberately different trust policies. The latter must never qualify a
  worktree that cannot be identified by commit.
- **Repair:** Do not weaken the regression gate. Commit the reviewed Ghostty
  patch, update Zentty's exact lock, and rerun against the now-clean local
  checkout before pushing.
- **Evidence:** The failed cell stopped before Zig with `Ghostty regression
  requires a clean pinned checkout`.
- **Outcome:** Resolved by Ghostty commit
  `5fc8fa2cf4b27bfe27072d561de98f33b2c16636`; the final pinned regression cell
  passed against a clean tree.

### Expanded Valgrind exposed an Ubuntu Fontconfig metrics cache

- **Observation:** Both Debug semantic runs passed, but Valgrind found one or
  two stripped Fontconfig/Pango metrics-cache roots and their children. This
  produced 8–13 leak errors per single-surface run and similar failures for
  four surfaces.
- **Consequence:** The newly expanded Debug matrix failed; representing the old
  one-surface historical result as current would have been incorrect.
- **Diagnosis:** Ubuntu's stripped Fontconfig library prevents the existing
  upstream function-name suppressions from matching. Every allocation began
  in system Fontconfig and reached GTK through
  `pango_context_get_metrics`; no Ghostty-owned frame was present.
- **Repair:** Add test-local suppressions bounded by allocation kind,
  `libfontconfig.so`, Pango metrics consumption, and indirect-only child
  records. Do not suppress possible leaks or Ghostty-library allocation roots.
- **Evidence:** After repair, one- and four-surface Debug runs on both Wayland
  and X11 report semantic PASS, zero definite/indirect bytes, and zero errors.
- **Outcome:** External process-global cache isolated without weakening the
  Ghostty memory boundary.

### Valgrind needed a test deadline, not a semantic shortcut

- **Observation:** The first ReleaseSafe four-surface Valgrind reproduction
  hit the host's normal 12-second integration deadline before all PTYs
  completed, obscuring the memory result with a semantic timeout.
- **Repair:** The memory driver enables a test-only 240-second internal
  deadline while retaining its independent 300-second process timeout. Normal
  integrations and the product path keep their original deadlines/behavior.
- **Evidence:** The rerun completes the exact four-surface keyboard, clipboard,
  focus, resize, title, and child-exit PASS before Valgrind returns 99 for the
  tracked optimized-build reports.
- **Outcome:** ReleaseSafe remains XFAIL for memory evidence, not because the
  semantic harness timed out.

### The first controlled physical-key matrix cell raced internal focus

- **Observation:** A standalone controlled key run passed, but the first full
  matrix run later sent its line before Ghostty's internal input widget owned
  GTK focus. The shell received no matching line and exited with no title
  acknowledgement.
- **Consequence:** The physical-key cell failed nondeterministically despite a
  mapped top-level X window.
- **Diagnosis:** X11 top-level focus and Ghostty's composite-widget internal
  focus are separate states; waiting only for surface initialization was
  insufficient.
- **Repair:** The host now repeatedly requests Ghostty surface focus and emits
  a readiness marker only after `GtkRoot` confirms focus lies inside that
  surface. The external driver waits for that marker, focuses the top-level X11
  window, then injects keys. The matrix repeats the entire nested-Xvfb process
  three times.
- **Evidence:** The final matrix repeated a fresh nested-Xvfb process three
  times; all runs received the exact PTY line and title acknowledgement.
- **Outcome:** Focus synchronization made event-driven rather than timing-based.

### Repeated X11 focus exposed separate IBus and layout caches

- **Observation:** After the initial Fontconfig repair, one-surface X11 and all
  Wayland Debug Valgrind cells passed, but four X11 focus transitions retained
  one tiny IBus preedit object and a stripped Fontconfig layout cache.
- **Consequence:** The second complete matrix still correctly failed one PASS
  cell rather than averaging away a backend/scenario-specific memory result.
- **Diagnosis:** Allocation stacks were bounded to GTK's external IBus input
  module during `Surface.updateFocus`, or to Fontconfig consumed through
  `pango_layout_get_size`; the one-surface path did not create them.
- **Repair:** Add allocation-kind and library/function-bounded suppressions for
  only those external caches. Possible leaks and any Ghostty allocation root
  remain unsuppressed.
- **Evidence:** The repaired four-surface Debug X11 cell reports the exact
  semantic PASS, zero definite/indirect bytes, and zero errors in both its
  focused rerun and the final complete matrix.
- **Outcome:** Backend-specific external cache behavior isolated without
  weakening product-owned leak detection.

### Final authoritative matrix result

- **Command:** `GHOSTTY_SOURCE_DIR=/home/jason/Projects/ghostty
  linux/tests/qualify-local`
- **Pinned dependency:**
  `5fc8fa2cf4b27bfe27072d561de98f33b2c16636`, clean tree,
  ReleaseSafe artifact restored at completion.
- **Declared cells:** 53 PASS, 0 FAIL, 5 BLOCKED, 2 XFAIL, and 4
  NOT_IMPLEMENTED (64 total).
- **Execution result:** Every PASS command passed; both XFAIL commands failed
  for their tracked Valgrind defect; there were no failed PASS cells,
  unexpected skips, or stale XFAILs. The machine record is
  `build/linux/qualification-summary.json`.
- **Claims:** **implemented local suite PASSED**; **release qualification NOT
  PASSED**; **full Linux qualification NOT PASSED**.
- **Limitation:** This is intentionally not “exhaustive QA.” Any such claim is
  prohibited while the 11 non-PASS cells remain.

### DOGFOOD-2026-08-02-VALGRIND-SUPPRESSION-GOVERNANCE: zero-after-suppression was not raw cleanliness

- **Observation:** Earlier receipts retained only a suppression-enabled
  Valgrind run. Several historical statements above consequently say “zero
  errors” or “clean” without preserving the non-project-suppressed evidence
  needed to audit that claim.
- **Consequence:** A growing, stale, or over-broad suppression could hide a
  regression while the qualification report continued to look green.
- **Repair:** Every Valgrind cell now runs twice and preserves adjacent
  `*.raw.log` and `*.suppressed.log` receipts plus a JSON record. “Raw” means
  no Ghostty or Zentty suppression file; Valgrind 3.22.0 built-in
  suppressions remain active and are declared in the manifest. The matrix
  summary embeds raw and post-suppression totals and rule usage. Debug results
  are named **PASS with reviewed suppressions**. This paragraph supersedes any
  older wording above that could be read as an unsuppressed-clean result.
- **Raw evidence:** The final 40-frame Debug product receipts expose expected
  system cache findings rather than zero totals: single/interaction Wayland
  each report 242 errors, 3,296 definitely-lost bytes, and 28,606
  indirectly-lost bytes; single X11 reports 141, 2,016, and 14,406;
  interaction X11 reports 247, 3,376, and 28,643. API-only Wayland/X11 report
  4/352/132 and 5/408/132 respectively. The reviewed Debug receipts reduce those reported
  error, definite-byte, and indirect-byte totals to zero. These figures are
  evidence for reviewed suppression behavior, not a product claim of raw
  cleanliness.
- **Manifest:** `linux/tests/valgrind-suppressions.json` gives each of the nine
  project rules a tracking ID, exact finding type, affected package versions
  and environment, expected match/byte range, scenario allowlist,
  justification, and reproducer. It also pins the complete effective inherited
  set: Ghostty's 233-rule `valgrind.supp`, its six-rule embedding suppression
  file, and Valgrind's built-ins. Ghostty's main file contains duplicate names,
  so its audit identity is the pinned Ghostty revision plus exact hash and rule
  count, while per-run receipts retain actual usage.
- **Governance tests:** The validator rejects an added untracked project rule,
  increased match count or byte range, a required rule with no observed use,
  a rule used outside its scenario, a child rule without its documented root,
  changed inherited hashes/counts, and missing paired receipts. Its negative
  self-test constructs and observes failures for suppression growth, stale
  suppression, out-of-scenario matching, and untracked addition.
- **Stale receipt defense:** Before a Valgrind cell executes, the matrix runner
  removes that cell's prior JSON report. A command that fails before producing
  fresh paired evidence is therefore reported as missing/invalid evidence; it
  cannot inherit a prior run's apparently valid totals.
- **First audit rerun:** The first full governed matrix correctly failed its
  governance PASS cell: ReleaseSafe X11 used the already-narrowed metrics
  string rule six times/3,276 bytes, outside the initially reviewed 2–4-match
  range. The exact rule itself was not broadened and ReleaseSafe remained red;
  review showed the same five consecutive Fontconfig frames beside its metrics
  root. The intermediate manifest temporarily admitted 2–6 matches while
  retaining the existing byte ceiling and scenario allowlist. Because qualification had a failed PASS
  cell, its summary was rejected and the entire matrix was scheduled again.
- **Stale-rule discovery:** After reviewing the count failure, governance next
  rejected both old Pango layout-cache rules as stale: neither appeared in any
  receipt from that completed matrix. Removing them made the next full run
  correctly fail when the nondeterministic cache returned (five contexts,
  512 definite and 128 indirect bytes). Rather than waive staleness or depend
  on product timing, the standalone non-Ghostty harness now explicitly creates
  and sizes a Pango layout. This makes the root/child evidence a deterministic
  governed scenario; both rules remain tracked only because that harness uses
  them on every qualification run.
- **Apparent product cardinality variance:** At Memcheck's default caller
  depth, X11 startup appeared to produce two string-child shapes depending on
  cache order. That was not accepted as a permissive numeric range; the audit
  increased caller depth before setting a final baseline.
- **Complete-stack correction:** Raising Memcheck receipts from the default
  caller depth to 40 exposed additional deep allocations matching the already
  narrowed five/eight-Fontconfig-frame shapes. Governance rejected the old
  counts (for example, 10 rather than 2 string contexts). The manifest now
  records exact 40-frame per-scenario baselines; no suppression pattern was
  broadened. This also made both Pango roots simultaneously visible in the
  standalone harness.

### Broad Fontconfig children were narrowed and tied to their cache roots

- **Observation:** The first Fontconfig child rules admitted `malloc` or
  `strdup` followed by only one stripped `libfontconfig` frame. That was too
  broad even though the sampled stacks appeared external.
- **Repair:** The string rule now requires `strdup` followed by five
  consecutive Fontconfig frames; the node rule requires `malloc` followed by
  eight consecutive Fontconfig frames. Each child rule must appear in the
  same JSON receipt as the separately bounded Pango metrics-cache root or
  governance fails.
- **Evidence:** The standalone GTK receipt records two explicitly requested
  metrics roots alongside the narrowed node/string children. Product receipts
  record one root on X11 or two on Wayland alongside the corresponding reviewed child cardinality.
  No child-only receipt is accepted.
- **Remaining uncertainty:** Stripped distro symbols prevent assigning the
  internal Fontconfig functions by name. The repeated-library shape,
  allocation primitive, named Pango consumer on the root, external
  reproducer, fixed environment, and enforced co-occurrence are the narrowest
  deterministic evidence available on this package build.

### A non-Ghostty GTK/IBus program reproduced the focus findings

- **Observation:** A stack passing through `Surface.updateFocus` proved a
  product trigger but did not prove that Ghostty lacked lifecycle
  responsibility.
- **Attempt:** A private D-Bus plus Xvfb run did not load `libim-ibus` and did
  not reproduce the finding. That environmental absence was recorded as a
  failed reproduction attempt, never converted into a pass.
- **Repair:** `ibus-focus-reproducer.c` is a standalone GtkApplication using
  ordinary GTK entries and `GtkIMMulticontext`; it neither links nor loads
  Ghostty. On the controlled current Xwayland session with
  `GTK_IM_MODULE=ibus`, repeated focus-in/focus-out reproduces allocations
  rooted at `ibus_text_new_from_string` in `libim-ibus.so`.
- **Lifecycle check:** Adding explicit client detachment initially placed it
  after window destruction. Valgrind immediately exposed two invalid reads in
  GTK/IBus because detachment consulted the already-freed widget. Moving
  `gtk_im_context_set_client_widget(context, NULL)` before window destruction
  removes that reproducer-owned lifecycle defect; it was not suppressed.
- **Evidence:** The initial focus-only raw receipt reported 77 errors, 1,920
  definitely-lost bytes, and 14,424 indirectly-lost bytes. After the same
  harness was extended to deterministically exercise both Pango roots, the
  combined raw receipt reports 334 errors, 4,480 definite bytes, and 27,001
  indirect bytes. Its reviewed receipt reaches zero for those totals and
  records exact per-scenario rule counts. The IBus allocation stack terminates
  in the reproducer's focus driver rather than `Surface.updateFocus`.
- **Remaining uncertainty:** External reproducibility makes an IBus/GTK cache
  diagnosis credible but does **not** absolve Ghostty of lifecycle
  responsibility. The rules remain narrowly bounded to IBus construction,
  scenario-restricted, count-limited, and auditable.

### ReleaseSafe remains an evidence-bearing XFAIL

- **Decision:** The project suppression set was not broadened to make the
  optimized build green. Both ReleaseSafe Valgrind cells retain raw and
  reviewed receipts, return their nonzero Memcheck status, and remain tracked
  XFAILs. Governance treats a zero result as stale XFAIL rather than silently
  promoting it.
- **Qualification consequence:** Neither release nor full Linux qualification
  may be claimed while these or any other required matrix cells are XFAIL,
  BLOCKED, FAIL, or NOT_IMPLEMENTED.

### Suppression-governed authoritative matrix result

- **Command:** `GHOSTTY_SOURCE_DIR=/home/jason/Projects/ghostty
  linux/tests/qualify-local`
- **Pinned dependency:** Ghostty
  `5fc8fa2cf4b27bfe27072d561de98f33b2c16636`, clean; the matrix restored the
  ReleaseSafe artifacts before its final cells.
- **Declared totals:** 55 PASS, 0 FAIL, 5 BLOCKED, 2 XFAIL, and 4
  NOT_IMPLEMENTED (66 total).
- **Execution:** Every executable PASS cell passed, including suppression
  governance and its negative self-tests. Both ReleaseSafe Valgrind cells
  produced paired receipts and remained the expected nonzero XFAILs. There
  were no failed PASS cells, unexpected skips, or stale XFAILs.
- **ReleaseSafe evidence:** Wayland raw/post reports 937/695 errors,
  3,296/0 definite bytes, and 28,606/0 indirect bytes. X11 raw/post reports
  911/665 errors, 3,456/0 definite bytes, and 28,042/0 indirect bytes. These
  remain XFAIL because hundreds of post-suppression errors are unresolved.
- **Claims:** **implemented local suite PASSED**; **release qualification NOT
  PASSED**; **full Linux qualification NOT PASSED**. This result supersedes
  the earlier 53-PASS authoritative checkpoint above.
- **Machine evidence:** `build/linux/qualification-summary.json` embeds all
  nine Valgrind JSON reports, their paired receipt paths, raw/post totals, and
  suppression usage. Build receipts are intentionally ignored rather than
  committed.
- **Limitation:** This is not exhaustive QA. Eleven declared cells remain
  BLOCKED, XFAIL, or NOT_IMPLEMENTED.

The experiment remains an internal Zig/GTK integration surface rather than a
public Ghostty ABI. Any extraction proposed upstream should be smaller than the
fork's test harness, preserve the legacy constructor, and be independently
reviewable.

## AI disclosure

Initial repository analysis, implementation assistance, and this report were
prepared with OpenAI Codex under Jason Maskell's direction. Any proposed
upstream Ghostty communication or contribution must be reviewed, understood,
edited, and submitted by the human contributor in accordance with Ghostty's
contribution policy.
