# ADR 0001: Rust and gtk4-rs Linux product architecture

- **Status:** Accepted and normative
- **Decision date:** 2026-08-02
- **Architecture contract:**
  [`linux-product-architecture-v1.json`](linux-product-architecture-v1.json)
- **Workspace-state contract:**
  [`workspace-state-v1.schema.json`](workspace-state-v1.schema.json)
- **Applies from:** Zentty `8c08e7ed987d46fcda65d716cf02845a2c98b285`
- **Engine boundary:** Ghostty
  `5fc8fa2cf4b27bfe27072d561de98f33b2c16636`

`MUST`, `MUST NOT`, `SHOULD`, and `MAY` are used normatively. This ADR is a
contract for issues #3 through #13, not permission to add product behavior to
the current C host.

## Decision

The Linux product is a Rust application with:

1. a platform-neutral, pure-Rust product core;
2. a `gtk4-rs` Linux shell;
3. a raw Ghostty C-ABI crate owned by Zentty;
4. a safe Rust/GObject adapter owned by Zentty; and
5. Ghostty remaining a generic Zig terminal engine behind its
   language-neutral C ABI.

The GLib default main context is the UI and event-loop authority. The product
does not have a second general-purpose async runtime. All workspace concepts
remain in Zentty; no worklane, pane-topology, persistence, agent, or platform
service concept may cross into Ghostty.

`linux/src/main.c` and `linux/src/host_options.*` are the **C qualification
host**. They prove the extracted engine boundary and are neither the product
architecture nor a base to extend. Product behavior begins only in the Rust
workspace established by #13.

## Why this boundary

Rust gives the product model explicit ownership, exhaustive command/state
transitions, a strong serialization ecosystem, and a reviewable place to
concentrate the small amount of unavoidable FFI. `gtk4-rs` uses the same
GTK/GDK/GLib object and event-loop model as Ghostty's existing Linux surface,
so the shell can embed the real widget without an extra rendering or IPC
layer. A pure Rust core keeps deterministic state, commands, migrations, and
agent-state projection testable without a display server.

### Rejected choices

| Choice | Reason rejected for this product boundary |
| --- | --- |
| Extend the C host | It would turn qualification scaffolding, test-only environment layouts, and manual ownership into the product. It provides no good home for the workspace model, typed commands, migrations, or platform seams. |
| Swift/AppKit | AppKit is macOS-only. A second Swift implementation would neither host the GTK widget naturally nor share the Linux dependency/tooling ecosystem. No new Swift/AppKit Linux-port code is permitted. |
| Electron/web UI | It adds a second renderer, runtime, packaging/security surface, and non-native input/IME/accessibility boundary while still requiring native Ghostty embedding. |
| A non-GTK Rust UI framework | Ghostty's qualified surface is a GTK widget. A different toolkit requires foreign-window embedding, texture copying, or terminal reimplementation and invalidates the proven GTK/GDK lifecycle. |

This choice means contributors must understand Rust ownership and GLib
thread-affinity, and debugging must cross Rust, C ABI, GTK, and Zig frames.
That complexity is accepted but confined. Release symbols MUST retain enough
information to attribute Rust and native frames; Debug and ReleaseSafe-style
qualification MUST exercise the same public ABI. Valgrind remains the current
cross-language memory gate. Sanitizer jobs MAY supplement it only with a
pinned compatible toolchain and MUST NOT replace the real GTK/Ghostty product
tests.

## Existing macOS responsibility inventory

The Linux port copies contracts, not AppKit implementation. This inventory
anchors parity decisions to real macOS responsibilities rather than a visual
approximation.

| Responsibility | macOS evidence | Shared semantic contract | Linux owner |
| --- | --- | --- | --- |
| Workspace/worklanes | `WorklaneStore`, `WindowWorkspaceState`, `PaneStripState` | Stable identity, ordering, selection, topology, legal transitions | `zentty-core` |
| Pane layout and focus | `PaneStripState`, `PaneLayoutSizing`, `WorklaneStore+Reordering` | Column/row placement, weights, deterministic focus target | `zentty-core`; GTK projection in `zentty-linux` |
| Persistence/recovery | `WorkspaceRecipe`, `SessionRestoreStore`, `AppDelegate` save orchestration | Versioning, migration, atomic save, corruption and restart policy | `zentty-core` plus `PersistenceCoordinator` |
| Terminal host | `LibghosttyRuntime`, `LibghosttySurface`, `LibghosttyAdapter`, `TerminalPaneHostView` | Create/configure/focus/resize/input/exit/close semantics | `zentty-ghostty`; composition in `zentty-linux` |
| Commands/actions | `PaneCommand`, `AppActionRouter`, `PaneCommandExecutor` | Typed intent, availability, pure transition before UI effect | `zentty-core`; GTK action binding in `zentty-linux` |
| Agent state | `WorklaneStore+AgentStatus`, agent hook/IPC types | Adapter identity and durable non-secret resume reference; live status is transient | `zentty-core`; Linux IPC/process services in `zentty-linux` |
| Server discovery | `Servers/*`, `WindowServerCommandService` | Normalization, ranking, pane association, explicit stop intent | Pure algorithms in `zentty-core`; `/proc`, Docker, opener, and signals in Linux services |
| Configuration | `AppConfig`, `AppConfigStore`, `AppConfigTOML` | Typed defaults, validation, user intent | Pure representation in `zentty-core`; XDG I/O and GTK application in `zentty-linux` |
| Platform integrations | `OpenWith/*`, notifications, `NSPasteboard`, `Process`, Sparkle | Capability-oriented service results and failures | Linux service implementations; update/desktop policy under #7/#9 |

macOS remains the existing Swift/AppKit application and is not required to
import Rust crates or adopt the Linux state file. Cross-platform means shared
behavioral contracts and fixtures. Any future on-disk interchange is a
separate migration decision and must not regress current macOS restoration.

## Cargo workspace and dependency direction

#13 creates one root Cargo workspace and exactly these initial responsibility
boundaries. A crate split may change only through a superseding ADR and an
updated machine contract.

| Crate | Responsibility | Allowed workspace dependencies | Forbidden |
| --- | --- | --- | --- |
| `zentty-core` | Workspace/worklane/pane state, typed commands, invariants, schema/migrations, agent-state projection, platform-service traits and pure request/result types | None | GTK, GLib/GObject, Ghostty FFI, Linux-only APIs, process/global environment reads |
| `zentty-ghostty-sys` | Audited raw declarations matching Ghostty's pinned exported downstream header, opaque native types, constants, and native link metadata | None | Product types, gtk4-rs wrappers, policy, convenience behavior |
| `zentty-ghostty` | Safe `GhosttyRuntime`/`GhosttySurface` API, GObject transfer, callbacks, thread-affinity, error mapping | `zentty-ghostty-sys` | Workspace state, commands, agent/worklane concepts |
| `zentty-linux` | Shipped binary, GTK application/windows/views, composition root, Linux platform-service implementations and lifecycle orchestration | `zentty-core`, `zentty-ghostty` | Direct `zentty-ghostty-sys`, test-support, product behavior selected only by test environment variables |
| `zentty-test-support` | Non-shipped controlled child programs, isolated XDG layouts, receipts, and external product drivers | `zentty-core`, `zentty-ghostty` | Direct raw FFI, production feature branches, being linked by shipped crates |

The only allowed workspace edges are:

```text
zentty-linux ------------> zentty-core
      |                         (no workspace dependencies)
      +------------------> zentty-ghostty --> zentty-ghostty-sys

zentty-test-support -----> zentty-core
      +------------------> zentty-ghostty
```

Crate features MUST be additive capabilities. A test feature MUST NOT switch
the shipped executable to another implementation. Integration tests launch the
same `zentty-linux` artifact delivered to users.

`#![forbid(unsafe_code)]` applies to `zentty-core`, `zentty-linux`, and
`zentty-test-support`. The sys crate contains declarations, not safe
abstractions. `zentty-ghostty` permits `unsafe` only in private, named FFI and
GObject-transfer modules, requires a `// SAFETY:` justification for every
block, and denies unsafe operations inside unsafe functions. Its public API is
safe. No raw Ghostty or GObject pointer is public or stored in product state.

The Ghostty repository MUST NOT gain a Cargo workspace, Rust crate, gtk4-rs
dependency, MSRV promise, or Rust release duty for Zentty. A proposed Ghostty
API operation requires a failing product contract test naming the semantic
need and misuse behavior. Convenience alone is insufficient. No Ghostty API
addition is proposed by this ADR.

## Event loop, threads, and cancellation

The thread that owns the acquired default `glib::MainContext` is the UI
thread. `GtkApplication`, every GTK object, `GhosttyRuntime`,
`GhosttySurface`, and their callbacks are `!Send` and `!Sync` by contract and
are created, used, and destroyed on that thread.

Acquiring/referring to the GLib main context is not GTK initialization. The
pinned Ghostty ABI requires `ghostty_gtk_embed_runtime_new*` to run in a fresh
process **before `gtk_init`, `GtkApplication` construction, or any GTK
object**. The composition root therefore acquires only the GLib context,
creates the Ghostty runtime, and only then initializes GTK. The positive and
GTK-first misuse cases run in separate processes because runtime creation is
unique and irreversible.

GLib sources and `spawn_local` futures are the default async mechanisms.
Blocking filesystem, discovery, or subprocess waits run only through a
bounded worker service. Each job has:

- an owner and cancellation token;
- a bounded completion/shutdown deadline;
- a result channel back to the GLib context;
- no GTK, GObject, or Ghostty reference in worker-owned data; and
- a join/reap obligation during owner teardown.

Adding Tokio or another runtime requires a separate ADR naming threads,
reactor ownership, cancellation propagation, wakeup bridging, resource caps,
and shutdown/join order. It may not become a second UI authority.

## Ownership and lifetime contract

### Owner tree and creation order

```text
ApplicationRuntimeOwner [one per process, GLib UI thread]
├── acquired default MainContext [no GTK initialization]
├── GhosttyRuntimeOwner [created before gtk_init; runtime lease + tick SourceId]
├── GtkApplication [created only after Ghostty runtime]
├── PlatformServicesOwner
│   └── PlatformTaskOwner* [bounded, cancellable, joined]
├── PersistenceCoordinator [workspace lock + pure WorkspaceStore]
└── WindowOwner* [stable WindowId + strong gtk::ApplicationWindow]
    └── WorklaneOwner* [stable WorklaneId + model projection]
        ├── PaneRecord* [durable core state; survives terminal failure]
        └── PaneOwner* [transient projection + retry/failure state]
            └── GhosttySurfaceOwner? [absent when construction failed]
                ├── GtkObjectOwner [one proven/adopted surface-widget reference]
                ├── CallbackRegistrationOwner* [signal/source IDs + state]
                ├── GlibSourceOwner* [tick/timer/idle SourceId]
                ├── GhosttyPtyOwner [engine-internal]
                └── GhosttyChildProcessOwner [engine-internal]
```

The strict creation order is: acquire the GLib context without initializing
GTK; create the one Ghostty runtime; initialize GTK/create `GtkApplication`;
create platform services and load/migrate/validate durable state; then create
windows, worklanes, pane projections, and terminal instances. An isolated
misuse test MUST initialize GTK first and prove that late runtime creation is
rejected without a surface, callback, or partial runtime; another fresh
process proves the required positive order.

Restoration installs the complete durable `PaneRecord` topology before any
terminal constructor runs. A pane creates its GTK placeholder/container, then
attempts its Ghostty surface. Surface creation remains internal and unsafe
until the exact returned GObject transfer has been proved. A restoration
failure leaves the `PaneRecord`, ordering, layout, and active IDs intact and
projects `TerminalInstanceState::Failed { retry }`; it never silently deletes
the pane. A partial native failure unwinds only native/transient children.

### Named contracts

| Object | Owner and valid lifetime | Creation contract | Teardown contract |
| --- | --- | --- | --- |
| `ApplicationRuntimeOwner` | Process main owns exactly one from startup through final GLib shutdown | Acquires only the GLib main context, creates Ghostty runtime, then GTK/application and remaining services in the fixed machine-contract order | Delegates the explicit `ShutdownCoordinator` sequence below; ordinary close transitions are disabled once shutdown starts |
| `GhosttyRuntimeOwner` | Application owner; unique and non-recreatable in a process under the pinned ABI; a counted lease is attached to every native surface GObject | Created after GLib-context acquisition but before `gtk_init`, `GtkApplication`, or any GTK object; safe adapter accepts the full-transfer non-null runtime handle and installs one GLib tick source only after GTK becomes available | Mark closing and remove the tick source after surface shutdown. Never block the UI thread awaiting finalization: native free is deferred until the last surface GObject's qdata destroy notify releases its lease on the GLib UI thread; never attempt recreation |
| `PlatformServicesOwner` | Application owner | Builds explicit Linux service implementations from validated XDG/config inputs | Stop accepting requests, cancel tasks, collect results, join/reap, release D-Bus/portal resources |
| `PersistenceCoordinator` | Application owner while workspace lock is held | Loads bytes, selects version, migrates in memory, validates, then exposes a pure model | Complete or reject the last atomic save before releasing lock; never save over rejected/corrupt input automatically |
| `WindowOwner` | Window registry, keyed by stable ID | Creates a strong `gtk::ApplicationWindow`, then projects already-loaded worklanes/panes | User close routes a model close transaction; application shutdown only tears down projections/native objects and MUST NOT remove durable topology |
| `WorklaneOwner` | One window; valid only while its stable model entry exists in that window | Projects ordered core state; does not become the source of truth | Close/remove its panes before removing the model projection and GTK container |
| `PaneRecord` | `zentty-core` workspace model; durable independent of GTK/terminal success | Restore applies records first. A new-pane command creates a draft record and attempts the transient terminal before one atomic model commit | User close explicitly removes it. Restoration/native failure and application shutdown retain it |
| `PaneOwner` | One worklane projection; stable pane ID links to a `PaneRecord` while terminal instance is optional | Restoration projects the existing record and transitions Starting→Running or Failed/retry. New-pane creation uses a draft ID; constructor success commits the record, constructor failure destroys the draft projection and leaves the model unchanged | User close routes exactly one model close transition. Shutdown mode suppresses that transition, stops callbacks/input, closes the optional surface, and removes only GTK projection |
| `GhosttySurfaceOwner` | Optional transient terminal instance for one pane; its native surface GObject owns a stable `SurfaceLeaseState` in qdata | Adapter validates runtime/arguments, invokes the unsafe constructor internally, adopts the returned GObject exactly once after transfer is proven, and immediately attaches one counted runtime lease with qdata/destroy-notify semantics before exposing a safe terminal API | Quiesce callbacks, disconnect sources/signals, detach the hidden widget, and drop the adapter reference. Any other native reference may delay finalization; only the GObject qdata destroy notify releases the lease, never `GhosttySurfaceOwner::drop` |
| `GtkObjectOwner` | Surface owner plus GTK parent hierarchy or child traversal may each hold strong references | The current C header says only “normal GTK container ownership rules apply”; that is not an exact floating/full/borrowed transfer contract. #11 must publish an explicit header contract plus native ref/finalize tests before #13 uses the one matching gtk4-rs conversion: sink a floating reference, adopt a full reference, or take a new reference from a borrow—never guess or combine them. The product API does not return the widget, but the qdata lease remains correct even if native GTK traversal obtains another strong reference | Remove from the adapter-controlled parent, disconnect signal handlers, and drop the adapter reference on the UI thread. Borrowed callback parameters are never unrefed, cloned, or retained; native finalization invokes the qdata destroy notify and releases the finalization-coupled runtime lease |
| `CallbackRegistrationOwner` | Surface or platform task; never longer than target object | Stores every `SignalHandlerId` and native registration; native userdata points to stable adapter-owned state | Set closing flag, disconnect signals/unregister native callback, wait for the ABI's callback-quiescence guarantee, then free userdata |
| `GlibSourceOwner` | Runtime, surface, window, or service that scheduled a tick, timeout, or idle callback | Stores the nonzero `SourceId` beside a weak/stable-ID callback target; repeating callbacks have a bounded stop condition where applicable | Remove the source before its target or callback state is released; a callback already on-stack observes `closing` and returns `Break`. Source IDs are cleared exactly once |
| `GhosttyPtyOwner` | Ghostty engine inside a live surface | Created only by Ghostty surface creation from a transient resolved launch profile/CWD | Ghostty closes file descriptors during surface destruction; Zentty never closes engine-owned descriptors or treats an observed fd as ownership |
| `GhosttyChildProcessOwner` | Ghostty engine inside a live surface | Spawned by Ghostty from a transient resolved launch profile/CWD; Zentty observes exit only | Surface close requests engine shutdown; Ghostty must terminate/reap according to its contract. Zentty does not signal an observed PID as a substitute. Bounded close/reap is a contract test |
| `PlatformTaskOwner` | Platform service registry | Starts a non-terminal process with explicit argv/cwd and an allow-listed environment | Cancel, close pipes, apply documented graceful/forced termination deadline, wait/reap, then deliver at most one completion |
| `ShutdownCoordinator` | Application runtime; active exactly once after the running→shutting-down transition | Closes all user-input/action gates and owns the final-event/snapshot boundary | Cancel/join producers; quiesce callbacks/remove recurring sources; drain or explicitly discard queued typed events; freeze and validate one final `PaneRecord` snapshot; atomically save it; then destroy terminal/GTK objects without ordinary close mutations; free Ghostty runtime last and release GLib context |

“Owned” above is literal. Stable IDs do not own objects, maps do not confer
native ownership, and a GTK parent reference does not replace the adapter's
explicit surface lifetime.

### Restore, new-pane, and shutdown transactions

Durable and transient state have different failure semantics:

- **Restore:** load/migrate/validate the whole workspace, install all
  `PaneRecord`s and active-ID references, then create terminal instances. A
  constructor failure changes only transient projection state to
  `Failed { retry }`; topology stays eligible for a later retry and next save.
- **New pane:** core prepares a valid draft transition and stable ID, but the
  durable transition is not committed yet. The UI builds a transient terminal
  against that draft. Success atomically commits the record and publishes the
  pane; failure tears down the draft terminal/container and returns an error
  with no model or persistence change.
- **Ordinary pane close:** the user command removes one `PaneRecord` through a
  core transition, then tears down the associated terminal projection.
- **Application shutdown:** ordinary pane/window close commands are disabled.
  Teardown must not translate window destruction or child exit into record
  deletion.

Shutdown has one owner and this mandatory order:

1. enter `ShuttingDown`; reject commands and stop keyboard, paste, drag, and
   other user input;
2. cancel and join platform/agent/server/filesystem producers;
3. mark native callbacks closing, disconnect signal handlers, and remove
   recurring GLib sources;
4. drain already-queued typed events to a bounded quiescence point, explicitly
   classifying any event discarded because its producer was cancelled;
5. freeze one immutable snapshot from the remaining `PaneRecord` topology and
   validate it;
6. atomically save that frozen snapshot (or report the exact durability error);
7. close terminal instances and children in shutdown mode without routing
   ordinary `ClosePane`/`CloseWindow` transitions;
8. detach hidden surface widgets and destroy GTK windows/application; do not
   block the UI thread waiting for native finalization;
9. let each surface GObject's qdata destroy notify release its counted runtime
   lease; the last release schedules/performs native runtime free on the GLib
   UI thread, then release the GLib context.

No producer may mutate the model after step 4. A save failure does not reopen
event routing and does not justify deleting the prior valid state.

### FFI, GObject, and callback rules

- `zentty-ghostty-sys` models runtime handles as opaque pointers and GTK
  pointers without constructing Rust references. Null, foreign, stale, wrong
  runtime, invalid enum, duplicate close, and use-after-close cases are tested
  at the C boundary and through the safe adapter where representable.
- The safe runtime/surface wrappers are UI-thread-affine and non-cloneable.
  Consuming `close` is idempotent at the product level; the raw destructor is
  invoked at most once.
- `GhosttySurface` does not implement `IsA<gtk4::Widget>`, return a
  `gtk4::Widget`, or otherwise let a cloneable GTK wrapper escape. The adapter
  owns the native widget and mounts it only into an adapter-controlled child
  slot. Closing removes it from the GTK parent before dropping the adapter's
  only adopted reference. GTK signal parameters are translated immediately
  into typed events and are never passed to product callbacks as objects.
- Immediately after the proven GObject adoption, the adapter installs a stable
  per-surface `SurfaceLeaseState` in native GObject qdata with a destroy notify.
  The dedicated qdata key is immutable after attachment: adapter code MUST NOT
  replace, steal, or clear it. That state owns one counted, non-public runtime
  lease. `GhosttySurfaceOwner` drop detaches and drops the adapter reference but
  does **not** release the lease. Native finalization invokes the destroy
  notify, so the lease survives parent/child traversal or any
  otherwise-unexpected strong native reference.
  Dropping the application-facing runtime only marks it closing. The last lease
  release defers/performs native runtime free on the GLib UI thread; no owner
  blocks that thread waiting for a weak-ref/finalize callback.
- A GTK callback parameter is borrowed for the dynamic callback extent.
  Retention requires an explicit gtk4-rs clone/ref. An owned return is adopted
  exactly once; a proven floating object is sunk exactly once. Until
  `ghostty_gtk_embed_surface_new` has an exact header transfer contract and a
  matching ref/finalize contract test, its Rust constructor remains private
  unsafe machinery and no safe `GhosttySurface` constructor may claim it.
  This proof is a scoped #11 deliverable and a hard prerequisite to #13's safe
  constructor—not an assumption #13 may paper over locally.
- Native userdata is a stable heap allocation owned by its registration. A
  trampoline takes only a temporary borrow, checks the closing flag, posts
  model events to the GLib context, and contains panics with `catch_unwind`.
  Rust unwinding MUST NOT cross C.
- Callbacks MUST NOT synchronously mutate the owner collection while Ghostty
  is on the stack. They enqueue typed events containing stable IDs. The event
  resolver safely drops events for closed IDs.
- Surface destruction is not considered complete until the pinned Ghostty ABI
  guarantees no later callback and the adapter has disconnected every GLib
  source/signal. `ZL-13-RUST-GHOSTTY-ADAPTER` /
  `TEST-RUST-GHOSTTY-CALLBACK-DROP` must include fresh-process positive
  runtime-before-GTK, isolated GTK-first misuse, no-cloneable-widget escape,
  qdata destroy-notify lease release only at native finalization, nonblocking
  runtime-owner drop, native finalize-before-runtime-free, reentrancy, and
  late-callback tests.

## Workspace state version 1

The JSON Schema is normative for shape and allow-listed fields. Its
`x-zentty-invariants` plus the rules below are normative semantic constraints.
The dependency-light validator and fixtures are design-time executable
evidence; #3 reimplements the same rules in `zentty-core` with unit/property
and migration tests.

Published version 1 is immutable; both the schema and executable semantic-rule
artifact SHA-256 values are pinned in the architecture contract. An
incompatible shape or semantic change creates schema version 2,
a v1-to-v2 migration, and fixtures; it never silently edits what “version 1”
means. A correction to the v1 artifact itself requires an ADR amendment and a
checksum, validator, fixture, and compatibility review in the same change.

### Durable content

Version 1 stores stable, canonical-lowercase RFC 4122-variant UUIDv4 IDs;
explicit contiguous ordering; active window, worklane, and pane selection;
worklane/pane layout weights and coordinates; local absolute CWD; an approved
durable launch-profile reference representing the pane command; optional user
title; and an optional non-secret agent adapter/resume identifier. Identity is
globally unique, every active ID resolves inside its containing object, and
every persisted string rejects NUL.

It intentionally excludes window-system handles, GTK/GObject data, runtime
pointers, PIDs/PTY fds, focus history, renderer state, scrollback, live server
discovery, live agent status, notification bodies, and timestamps that do not
change restoration behavior.

`command` contains only `launch_profile_id`, matching a restricted non-secret
identifier grammar. Workspace state has no free-form program, argv, header,
environment, or shell-command field. The profile definition is separately
validated durable configuration and may contain executable/argument templates
but never credential values; a profile that requires authentication contains
only an approved secret-provider handle. `ProcessLauncher` resolves the
profile and secret handle into transient argv/environment immediately before
spawn, outside `PaneRecord` and the serializer. Agent/PTY output, shell
history, process discovery, and transient resolved launch data have no write
path into `command`.

The schema MUST NOT persist credentials, API keys/tokens, authorization headers,
clipboard contents, agent prompts/responses/transcripts, arbitrary environment
variables, or any unapproved secret. This is enforced by a strict allow-list,
the profile-reference data-flow boundary above, NUL rejection, and a
defense-in-depth credential-pattern validator covering authorization schemes,
headers, URLs, and known provider forms—not a redact-after-serialization pass.
`agent.resume_id` is allowed only when
the adapter has classified it as a non-secret opaque identifier. Unknown or
unclassified agent material stays transient.

### Read, migration, corruption, and write behavior

1. Read a bounded regular file from the platform state path without following
   an attacker-controlled symlink. Do not log raw bytes.
2. Parse JSON and inspect `schema_version` before decoding version-specific
   fields.
3. Version 1 with any unknown field is rejected. A newer version is rejected.
   In either case leave bytes untouched, disable automatic saves, and expose a
   visible read-only recovery path; silently dropping fields is forbidden.
4. The committed v0 fixture is the only recognized old design fixture. Its
   migration renames the three `active_*` keys, sets `revision` to zero,
   assigns array-index `order`, supplies unit layout weights/zero-based pane
   rows, renames the already-approved `command_profile` reference to
   `command.launch_profile_id`, and adds null titles/agents. It never migrates
   free argv or secret material into v1.
   Each future version adds one explicit sequential migration. Skipping a
   version is forbidden.
5. Validate shape, ordering, uniqueness, references, CWD, and confidentiality
   after every migration. Retain a mode-0600 pre-migration backup before
   replacing the original.
6. Malformed JSON, invalid state, I/O failure, or failed migration leaves the
   original untouched and automatic saves disabled. It must not become an
   empty workspace overwrite.
7. An ordinary save holds an exclusive workspace lock; serializes and
   validates in memory; creates a mode-0600 exclusive temporary file in the
   same directory; writes all bytes; flushes and fsyncs the file; atomically
   renames it over the primary; and fsyncs the directory. Any pre-rename
   failure removes only the temporary file. Post-rename fsync failure is an
   explicit indeterminate durability error, not success.

The fixtures cover a valid state, malformed state, recognized v0 input,
unknown-field rejection, and an explicit no-secret example. #6 owns crash and
interrupted-write product qualification; this ADR fixes the invariant it must
prove.

## Platform-service boundary

`zentty-core` defines capability traits and immutable request/result types.
Implementations are injected at the `zentty-linux` composition root. Core
commands never call global environment, filesystem, GTK, D-Bus, or process
APIs directly.

| Interface | Pure contract | Linux implementation responsibility |
| --- | --- | --- |
| `PlatformPaths` | Return typed config/state/cache/runtime paths | XDG resolution, secure directory modes, missing-variable fallbacks |
| `ResourceOpener` | Open a validated URL or local file and report acknowledged/failure | Portal-first open, documented `gio` fallback, no shell interpolation |
| `NotificationService` | Post/replace/withdraw by stable notification ID | Freedesktop/portal semantics and action callback routing |
| `ClipboardService` | Explicit read/write request with MIME and user gesture context | GDK clipboard on UI thread; clipboard contents never enter persistence/receipts |
| `PortalService` | Typed portal operations and parent-window token | D-Bus portal request lifecycle, denial/cancel/error distinction |
| `SettingsService` | Load/watch validated typed settings | XDG config and desktop settings; changes return typed events |
| `ProcessLauncher` | Structured executable/argv/cwd, allow-listed environment delta, cancellation and exit | No shell concatenation; pipes, process group, signal, deadline, and reap ownership |

Server discovery and agent integration split at the same seam: normalization,
ranking, command decisions, and status projection are pure core logic;
`/proc`, sockets, Docker, signals, wrapper installation, IPC, and process
observation are Linux services. Production behavior MUST work from ordinary
user input/state. Environment variables may configure test harnesses outside
the product, but a test-only environment layout cannot be the only way to
create worklanes or panes.

## Linux/macOS responsibility map

| Contract | Linux | macOS | Compatibility rule |
| --- | --- | --- | --- |
| Workspace transitions and commands | New `zentty-core` | Existing Swift models/actions | Same stable-ID/topology outcomes; implementations need not share code |
| Durable restoration | Strict schema v1 under XDG state | Existing `WorkspaceRecipe`/`SessionRestoreStore` | No automatic cross-import; platform fixtures assert semantic cases |
| UI/event loop | GTK4/GLib | AppKit main actor/run loop | Each remains sole UI authority |
| Terminal engine | Safe wrapper over Ghostty GTK C ABI | GhosttyKit/AppKit adapter | Ghostty stays generic; platform surface types stay isolated |
| Windows/worklanes/panes | `zentty-linux` GTK projection | Existing controllers/views | Multiple worklanes and panes, focus and restoration are shared behaviors |
| Clipboard/open/notifications/settings | Linux capability services | Existing AppKit/Foundation services | Core observes typed success/failure, not platform object types |
| Agents/servers | Pure projection + Linux process/IPC services | Existing hook/IPC/server services | Credentials/transcripts remain out of workspace persistence |
| Packaging/updates | Distribution work in #9 | DMG/Sparkle unchanged | Linux work MUST NOT regress or replace macOS delivery |

## Test and qualification strategy

All implementation follows #12's red/environment-proof/green/fault/mutation/
matrix order. The boundary claimed determines the test:

| Tier | Primary evidence | Doubles allowed | Claim prohibited |
| --- | --- | --- | --- |
| Unit/property | `zentty-core` transitions, ordering, active-ID invariants, migrations, command construction | Deterministic IDs/clocks and in-memory trait implementations | GTK, Ghostty, process, or desktop behavior |
| Contract/component | Raw/safe ABI misuse, schema fixtures, atomic-write fault points, platform protocol parsing | Controlled child/protocol server or fault injector outside claimed boundary | Real external service/product qualification |
| Product-boundary integration | Shipped `zentty-linux` executable, pinned Ghostty, real GTK/GL, PTY/process/filesystem/D-Bus | Controlled child command and deterministic external protocol actor, labelled | Installed artifact or representative desktop/hardware |
| Display/compositor | Same product under controlled nested native Wayland and X11/Xwayland | Input/IME/portal driver only after its environment self-check | Untested native compositor behavior |
| Memory/lifecycle | Repeated create/close and app shutdown under governed Valgrind; sanitizer supplement where pinned | Reviewed external-library suppressions with raw receipts | “Raw clean” when suppressions are used |
| Packaging | Installed artifact in isolated root/VM, desktop entry, resources, loader, upgrade/uninstall | Isolated root is allowed and declared | Staged bundle equals installed package |
| Public/representative | Controlled CI plus GNOME/KDE/IME/GPU cells and dogfood | None below the claimed system boundary | Missing environment as PASS |

The schema/architecture validator uses only Bash and `jq`. Its self-test must
prove rejection of malformed/current-invalid state, old state as current,
unknown fields, secret-like keys/values, invalid dependency direction, unsafe
leakage, and missing owners. It does not claim that Rust product persistence
exists.

Until the Rust product replaces it, the C qualification host is mechanically
frozen by `linux/test-policy/qualification-host-freeze.json`. The validator
requires the exact closed-world `linux/src` inventory plus byte-for-byte
SHA-256 identities for those sources and `linux/scripts/build-local`. Extending
that host to implement product behavior is not an allowed shortcut; a reviewed
policy update may repair qualification truth, while retirement removes the host.

The machine contract uses #12's stable `ZL-*` requirement and `TEST-*` test
vocabulary exactly. `linux/qualification-matrix.json` owns authoritative
granularity and status. The architecture contract retains a non-authoritative
mirror of its architecture-owned subset so the validators can reject drift
between the two artifacts. The reconciled subset names these exact IDs and
axes:

- `ZL-2-PRODUCT-BOUNDARY` / `TEST-PRODUCT-BOUNDARY`: capability
  `product_boundary`, cells `product-boundary-wayland` and
  `product-boundary-x11`;
- `ZL-2-ARCHITECTURE-CONTRACT` / `TEST-ARCHITECTURE-CONTRACT`: capability
  `architecture_contract`, cell `architecture-contract-v1`;
- `ZL-3-WORKSPACE-PERSISTENCE` / `TEST-WORKSPACE-SCHEMA`,
  `TEST-WORKSPACE-PERSISTENCE`, `TEST-WORKSPACE-RESTORE`: capabilities
  `workspace_schema`, `workspace_persistence`, and `workspace_restore`, with
  cells `workspace-schema-v1-contract`, `workspace-persistence-unit`, and the
  two `product-workspace-restore-{wayland,x11}` cells;
- `ZL-4-WORKLANE-SHELL` / `TEST-WORKLANE-LIFECYCLE`: capability
  `product_worklanes`, cells `product-worklanes-wayland` and
  `product-worklanes-x11`;
- `ZL-5-PANE-TERMINAL-LIFECYCLE` /
  `TEST-PANE-TERMINAL-LIFECYCLE`: capability
  `product_pane_terminal_lifecycle`, an explicit 24-cell family covering
  Debug/ReleaseSafe, Wayland/X11, default/epoll/io_uring, and single/multi
  terminal behavior. Every row remains `NOT_IMPLEMENTED` under GH-5;
- `ZL-6-RECOVERY` / `TEST-WORKSPACE-RECOVERY`: capability `recovery`, cells
  `workspace-recovery-interrupted-write` and
  `workspace-recovery-corrupt-state`;
- `ZL-7-PLATFORM-SERVICES` / the six `TEST-PLATFORM-*` IDs: separate
  capabilities `platform_xdg_paths`, `platform_open`,
  `platform_notifications`, `platform_clipboard`, `platform_settings`, and
  `platform_process_launch`, with the exact seven `platform-*` cells in the
  machine contract; and
- `ZL-13-RUST-GHOSTTY-ADAPTER` with `TEST-RUST-GHOSTTY-PRODUCT-USAGE`,
  `TEST-RUST-GHOSTTY-CALLBACK-DROP`, and `TEST-RUST-GHOSTTY-CONFIG` is the
  cross-stream prerequisite/test vocabulary for the safe adapter.

Issue #12 has now added and owns these cells. Only the two design-contract
cells are `PASS` with executable commands; every implementation/product cell
remains `NOT_IMPLEMENTED` and linked to its owning issue. The
`authoritative:false` flags in this architecture artifact mean that this mirror
cannot override the matrix; they do not make the corresponding matrix rows
non-authoritative. A cell with `display: both` does not claim that each backend
axis ran. The architecture validator compares every mirrored field with the
authoritative matrix and fails on drift.

## Toolchain, dependencies, and repository policy

- Rust edition is **2024**. The bootstrap pins the normal/release `rustc` and
  Cargo to **1.97.1**, the current stable patch listed by the official
  [Rust releases](https://blog.rust-lang.org/releases/) on the decision date.
  The separate project MSRV is **1.85.0**, the first stable release supporting
  edition 2024; every shipped crate declares `rust-version = "1.85"`. This
  floor comes from Zentty's language-edition decision, not a transitive
  dependency: current
  [gtk4-rs 0.11.3 documentation](https://gtk-rs.org/gtk4-rs/stable/latest/docs/gtk4/)
  reports its own MSRV as 1.83, but edition-2024 Zentty cannot promise 1.83.
  CI has distinct jobs. Pinned 1.97.1 runs the locked shipped/default feature
  graph plus reviewed all-feature/clippy coverage. MSRV 1.85.0 MUST compile and
  test the **same committed lockfile and exact shipped/default feature graph**,
  including the `zentty-linux` release binary; a minimal-feature-only build is
  not an MSRV claim. The observed development default (1.93.0) is neither
  substituted for the pin nor evidence for the MSRV. Changing either
  toolchain requires an ADR amendment, dependency review, and both green jobs.
- Rust MSRV and native library compatibility are separate. The initial native
  target is **GTK 4.14 or newer**, and #13 MUST enable no gtk4-rs API feature
  newer than `v4_14`; this keeps the bootstrap compatible with the observed
  Ubuntu 24.04 GTK 4.14.5 host. The floor job MUST prove with `pkg-config` that
  its native GTK is 4.14.x, build the locked shipped/default graph with exactly
  the gtk4-rs `v4_14` API feature (no `v4_16` or later), and run the real
  product/adapter smoke available at that milestone. Merely compiling on a
  newer GTK does not prove the floor. Using GTK 4.16+ API requires an explicit
  Linux platform-floor decision and matching controlled environment/package
  evidence. A newer Rust compiler never implies a newer GTK requirement.
- One workspace-root `Cargo.lock` is committed. CI, release, packaging, and
  documented reproduction use `--locked`. Library crates do not carry nested
  lockfiles. Lockfile changes are isolated and reviewed with dependency
  provenance/advisories/licenses.
- crates.io is the default permitted registry. Git/path dependencies outside
  the workspace are forbidden unless a reviewed, issue-linked, commit-pinned,
  time-bounded exception records source and replacement plan. Ghostty is a
  pinned native input, not a Cargo dependency. Release source/vendor handling
  must be reproducible and license-complete.
- Runtime dependencies are minimized, default features disabled when unused,
  and versions chosen only after MSRV/license/source review. `cargo deny check`
  enforces sources, duplicate/banned crates, advisories, and the reviewed
  license allow-list; `cargo audit` is the independent advisory gate. An
  unmaintained/yanked/advisory exception needs owner, rationale, expiry, and
  issue. Dependency licenses must be GPL-3.0-compatible or receive explicit
  legal review; generated third-party notices ship with the artifact.
- `cargo fmt --all --check` is mandatory. `cargo clippy --workspace
  --all-targets --all-features -- -D warnings`, workspace tests, and rustdoc
  with warnings denied are required. Workspace lint tables deny unsafe code by
  default and the adapter opts into only its documented stricter FFI policy.
- Build scripts and proc macros are executable supply-chain code and receive
  the same source/license audit. Binding generation must be reproducible from
  the pinned Ghostty header; release builds do not download code.
- `cargo-mutants` is not accepted or pinned by this ADR. #12 must evaluate it
  against the chosen toolchain before adding it; curated FFI/lifecycle mutants
  remain mandatory because generated mutants cannot prove native ownership.

No JavaScript dependency is introduced here. If later tooling introduces one,
it uses pnpm and repository configuration `minimumReleaseAge: 10080` (or
`.npmrc` `minimum-release-age=10080`); scope exclusions require Jason's
explicit authorization.

## C qualification-host retirement

The C host stays frozen except for repairs necessary to keep its qualification
evidence truthful. It MUST NOT gain workspace UI/state, commands, persistence,
agent awareness, server discovery, settings, or platform product services.

It leaves the product build and staged bundle only when all of these are true:

1. #11 first publishes the exact surface GObject transfer contract and native
   finalize proof; #13's resulting safe adapter then has positive,
   null/foreign/stale/misuse, transfer,
   callback-quiescence, multiple-surface, and reverse-teardown tests against
   the exact pinned real library.
2. The Rust product creates one and multiple panes through ordinary product
   state/actions, not `ZENTTY_LINUX_*` layout controls, under both Wayland and
   X11/Xwayland.
3. Product cells supersede every C-host lifecycle/interaction/configuration/
   focus/resize/input/output/exit/teardown and staged-bundle claim without
   losing Debug/ReleaseSafe, backend, or single/multi axes.
4. Repeated lifecycle and governed raw plus suppression-enabled candidate
   memory evidence pass for the product, and final results are described only
   as “PASS with reviewed suppressions” after governance accepts them. Existing
   ReleaseSafe XFAILs remain failures until fixed, not reasons to retire
   evidence.
5. The authoritative matrix records explicit `superseded_by` links from each
   retired host-only cell, and its self-test rejects a product/release claim
   whose command still launches the C host.
6. The distributed package contains `zentty-linux`, not the C host; staged and
   installed launch, loader, resources, hardening, upgrade, and uninstall
   ownership are qualified.
7. `linux/src/main.c` and `host_options.*` are removed from product build
   inputs. Small C ABI fixtures MAY remain under `linux/tests/` as raw-boundary
   tests and must be labelled as such.

Retirement is a replacement of evidence, never deletion of a difficult cell.

## Follow-ups, dependency order, and open risks

No newly discovered product work is left as an unowned prose requirement.
Existing public issues own it: #13 bootstrap/adapter; #3 state/persistence; #4
GTK shell; #5 pane/terminal lifecycle; #6 recovery; #7 workflow/agent/server
parity; #8 input/IME/resize/scaling; #9 packaging; #10 controlled public CI;
#11 Ghostty API decision; and #12 qualification architecture/matrix
traceability.

The accepted order is #2 + #12, then #13 bootstrap may establish Cargo and raw
declarations, but #11's scoped transfer/header/finalize proof MUST land before
#13 exposes a safe surface constructor. #3, #4, and #5 then build the product;
#6 follows the real lifecycle. #7 inventory may continue early but its
delivery depends on that foundation. #8–#10 expand real product evidence.
#11's broader final API-retention/upstream decision still waits for #5's real
callers. This refines the epic order only for the concrete safety prerequisite
and does not create hidden work.

Open, tracked risks are:

- #11 owns the exact header transfer/native-finalize proof that gates #13's
  safe constructor. #13 owns the hidden-widget/runtime-lease wrapper and
  callback-quiescence tests. This ADR deliberately does not infer
  floating/full/borrowed ownership merely from the qualification host's
  current `g_object_ref_sink` use.
- #3/#6 must turn the atomic/migration contract into real fault-injection and
  interrupted-write evidence.
- #7 must decide the initial-release parity inventory and explicitly classify
  each adapter resume identifier before any is persisted.
- #8/#10 still own controlled native Wayland input/IME/scaling infrastructure;
  the architecture validator is not display qualification.
- #9 must settle distro/package and third-party-notice delivery details while
  preserving the lock/source/license policy.
- #12 owns reconciliation of the proposed capability/cell IDs with the
  authoritative matrix and mutation/receipt traceability.

Passing this ADR's self-tests means only that the architecture artifacts are
internally consistent. It does not mean the Rust workspace, Linux product,
release qualification, or full Linux qualification exists.
