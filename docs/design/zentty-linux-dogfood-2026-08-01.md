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

## Current next gate

The Ghostty spike remains historical evidence and must not evolve into the
product. The current product-boundary gaps are ReleaseSafe Valgrind value
correctness, multiple simultaneous terminals, programmatic and physical
keyboard input, bidirectional clipboard, focus, resize, IME composition, and a
genuine scaled compositor on both display backends. A public CI environment
with controlled nested compositors is also still required; local desktop
receipts alone are not the final automation standard.

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
