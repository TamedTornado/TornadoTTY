# Zentty Linux worklane shell closeout dogfood

Date: 2026-08-22
Tracking: GH-4, GH-16

This append-only record starts before GH-4 closeout changes.

## Discovery: the tracker defect mixed delivered and missing behavior

- GH-4 still owns five `PARTIAL` inventory entries and two `NOT_IMPLEMENTED`
  matrix cells, so it is not currently closeable.
- The matrix descriptions still named physical divider gestures, double-click
  equalization, and contextual worklane transfer as missing. Maintained
  controlled X11 and Wayland journeys delivered those behaviors in later
  commits, but the GH-4 cells were intentionally not promoted by unrelated
  workspace/transfer issues.
- The remaining substantive gap is not another pane/worklane model operation.
  It is proof that real dynamic progress/failure and action state reaches the
  staged product's actual GTK accessibility tree on both compositor stacks.
- The controlled display wrappers deliberately disable ambient accessibility
  bridging. Closeout must create a private AT-SPI session explicitly; developer
  desktop accessibility state cannot be assumed and absence cannot become a
  pass.

## Decision

Use a separate Rust AT-SPI inspector process and extend the existing
`rust-agent-ipc` actor. Keep all topology, pointer, persistence, and PTY evidence
in the existing source-UX, sidebar-management, multi-window, and session-restore
actors. No parallel shell actor, accessibility model, or agent fixture is
permitted.

## Failure: the staged product was absent from a valid private AT-SPI tree

- A private session bus and AT-SPI registry were started inside controlled
  X11. Both `org.a11y.Status.IsEnabled` and `ScreenReaderEnabled` were set
  before product launch, `GTK_A11Y=atspi` reached the product, and the product
  remained mapped and responsive.
- The registry nevertheless had no Zentty application child and the product
  held no AT-SPI bus socket. This is a qualification failure, not an
  environmental pass.
- The same session was then exercised with the stock GTK 4 `zenity` product.
  It registered an application root and window on that AT-SPI bus. That
  controlled comparison rules out the private bus, registry, X server, and
  host accessibility packages as the cause.
- The material architectural difference is that Zentty initialized GTK and
  owned raw `gtk::Window` instances without registering a `gtk::Application`.
  The next repair is deliberately narrow: register the packaged application
  ID and associate every top-level window with that application. No workspace
  or accessibility shadow model is being added.

## Discovery: embedded Ghostty installed the wrong default application identity

- The first registration repair still failed. A direct window receipt exposed
  the reason: `GhosttyRuntime::new` had installed
  `com.mitchellh.ghostty-debug` as GIO's default application before GTK shell
  construction. Zentty's newly registered `com.zentty.zentty` object therefore
  existed, but new shell windows continued attaching to Ghostty's unregistered
  default.
- The repair must explicitly make Zentty's registered packaged identity the
  default before constructing shell windows. This changes application/window
  ownership only; terminal surfaces remain owned by the embedded Ghostty
  runtime.

## Repair: register the single embedding application instead of competing with it

- Creating a second registered `gtk::Application` and making it the default
  corrected the shell window's printed ID but still did not register an AT-SPI
  root. Ghostty had already initialized GTK's application/accessibility
  boundary around its embedding application.
- The corrected repair reuses that one embedding application, changes its ID
  to the packaged `com.zentty.zentty` identity before registration, preserves
  multi-process operation with `NON_UNIQUE`, and registers it exactly once.
  Shell windows and embedded terminal surfaces now share one GTK application
  rather than two competing application objects.

## Rejected repair and bounded test-policy correction

- Registering and activating the single Ghostty embedding application still
  produced no AT-SPI root, even though its application ID and shell-window
  association were correct. Keeping that lifecycle change would add Ghostty
  startup, global-shortcut, and standalone-window side effects without proving
  accessibility, so the experiment was reverted rather than normalized into
  product code.
- This identifies an embedding-boundary limitation: Ghostty constructs its own
  custom GTK application before the host may initialize GTK, while its embed
  API neither accepts a host application nor runs the normal application loop.
  Fixing that boundary belongs in a separate minimal Ghostty proposal, not in
  GH-4's worklane behavior.
- GH-4 will use GTK's official `GTK_A11Y=test` backend against the actual GTK
  widget constructors, while the existing staged-product actors continue to
  prove real X11/Wayland input, PTYs, persistence, and authenticated dynamic
  state. This is not described as external AT-SPI qualification, and the
  remaining embed limitation stays explicit in the matrix/dogfood evidence.

## Initial GTK accessibility-contract failure

- The first controlled X11 test correctly failed on its first named-node
  assertion. The widget existed; the test omitted the established `zentty-`
  namespace applied by `widget_name`. The production name was not weakened to
  satisfy the test. The assertion was corrected to the real stable widget
  identity before continuing to role, property, state, action, and sensitivity
  checks.

## Second GTK accessibility-contract failure

- The next run rejected an assertion that expected both **Split Right** and
  **Add Pane Right** controls to coexist. The product intentionally reuses one
  contextual right-arrow button and changes its command, label, and stable name
  with the configured insertion behavior.
- The test now proves both mutually exclusive states in sequence and asserts
  that the stale identity disappears after the behavior change. No duplicate
  button was added to make the test pass.

## Repair: actual-widget contract and durable controlled runner

- Worklane and pane selector/menu widgets now have stable names in addition to
  their existing source-derived labels, roles, selected states, and named GTK
  actions. Pane-local controls and dividers retained their established stable
  identities.
- One single-threaded test constructs the real sidebar card, pane frame, and
  divider widgets. It rejects missing nodes, wrong roles, absent label/selected
  metadata, wrong action routing, an enabled unavailable cross-window action,
  stale right-insertion identity, and a non-focusable/non-separator divider.
- `linux/tests/rust-worklane-accessibility` compiles the test artifact outside
  the isolated display, then executes it under `GTK_A11Y=test` on controlled
  X11 or Wayland. This repaired an early harness mistake that caused the
  disposable display HOME to download and rebuild the Rust toolchain on every
  run. No additional workspace or agent actor was created.

## Focused qualification result

- Controlled X11: authenticated real Unix-socket events drove real-process,
  real-PTY panes through running, needs-input, failed, progress, completed, and
  clear states; the actual-widget accessibility contract then passed.
- Controlled Wayland: the same real product journey and actual-widget contract
  passed.
- Architecture, feature-inventory, and qualification-matrix validators and
  their negative runner tests passed.
- The five GH-4 inventory entries are now `IMPLEMENTED`. The two authoritative
  product-worklane cells are now `PASS`; the matrix totals are **191 PASS, 3
  XFAIL, 2 NOT_IMPLEMENTED**. This is not full Linux qualification.
- External staged-product AT-SPI inspection remains a known Ghostty embedding
  limitation and is not described as passing. GH-16 already owns the broader
  installed-product accessibility-tree, screenshot, scale, motion, and visual
  parity acceptance. GH-4's widget semantics are closed without pretending
  that GH-16 is complete.
