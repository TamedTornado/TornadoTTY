# Linux IME closeout dogfood record — 2026-08-20

## Scope freeze

This record owns the GH-8 IME qualification work that begins after physical
Wayland keyboard input became a real product test at commit `e3c34624`.

The implementation boundary is deliberately narrow:

- drive a real installed IBus engine through compositor/native key events;
- deliver the composed, non-ASCII commit through GTK, Ghostty, and a real PTY;
- prove preedit cancellation, focus transfer, and pane destruction during an
  active composition before changing the corresponding matrix cells to PASS;
- reuse the existing controlled X11, controlled Wayland, product-input, and
  Rust product actors rather than creating another terminal-test authority;
- keep `ibus-focus-memory` separate. It proves the provenance of reviewed
  Valgrind suppressions; it does **not** prove product IME correctness; and
- make no Ghostty change unless a product failure establishes behavior owned by
  Ghostty rather than by Zentty's orchestration.

The first executable milestone is a deterministic X11 IBus commit. Wayland is
not inferred from it and remains independently qualified.

## Starting state

- Zentty commit: `e3c3462410df02bb15c16baa6fd4440a05ef0869`
- Matrix declarations: 172 PASS, 0 FAIL, 2 BLOCKED, 3 XFAIL,
  6 NOT_IMPLEMENTED.
- `ime-x11` and `ime-wayland` are explicitly NOT_IMPLEMENTED.
- The host has IBus 1.5.29 and the packaged `table:cangjie3`,
  `table:cangjie5`, and `table:cangjie-big` engines. The existing controlled
  wrapper previously pinned only `xkb:us::eng`.

## Discoveries and decisions

### 2026-08-20 — the existing wrapper is real but not an IME test

`linux/tests/controlled-ibus-x11` creates private Xvfb, D-Bus, IBus, XDG, and
home directories and verifies the selected engine. This is the correct service
boundary to reuse. Its hard-coded `xkb:us::eng` engine has no multi-key
composition, so it cannot make either IME cell pass.

The wrapper will accept a reviewed engine name for the new product actor while
retaining `xkb:us::eng` as the default for all suppression-governance receipts.
The READY marker must report the actual selected engine so evidence cannot
silently claim one engine while running another.

### 2026-08-20 — a stale focus receipt created a false IME diagnosis

The first focus-transfer implementation waited for the text
`focus-pane pane=pane-2`, but that receipt already existed from initial pane
creation. It therefore sent the second composition before the asynchronous
public-API focus request returned to pane 2. The empty PTY line initially looked
like an input-method semantic difference; a diagnostic receipt proved it was a
harness synchronization defect. The actor now requires the second occurrence
of the focus receipt before typing. No product or Ghostty behavior was changed.

### 2026-08-20 — Cangjie Return both commits and reaches the PTY

The first actor sent two Return keys after selecting `日`. The first Return
committed the Cangjie candidate and reached the canonical PTY as the line
terminator; the second produced an empty next line and falsely failed the later
focus assertion. Exact multi-read PTY receipts exposed the extra event. Every
composition step now sends one Return and asserts that no duplicate or empty
commit appears.

## X11 qualification result

The completed actor runs under the matrix-owned `nested-x11` Xvfb and reuses
that exact display while adding a private D-Bus and foreground IBus service. It
does not create a hidden second X server. The wrapper rejects reuse unless the
caller carries the authenticated nested-X11 session identity.

The final focused run passed:

- Bash parsing and warning-level ShellCheck for all changed actors;
- the controlled-IBus orchestration suite, including requested-engine,
  controlled-display reuse, and unsafe-reuse negative cases;
- matrix schema/coverage and matrix-runner focused tests;
- feature inventory and consolidated-orchestration contracts;
- the existing standalone GTK/IBus focus-memory journey with the default
  `xkb:us::eng` engine; and
- the real matrix-shaped product journey on nested-X11 session
  `1279c374fb7774308cda91cac6e347c12b5d7dd17f30c827eeeba11251210387`.

That product journey used the real packaged `table:cangjie5` engine, XTEST key
events, GTK's IBus context, the delivered Ghostty library, two real terminal
surfaces, authenticated public CLI focus/close commands, and exact PTY reads.
It passed explicit preedit cancellation, non-ASCII commit, focus transfer,
active-preedit surface destruction, post-destruction composition, and clean
application shutdown.

No Ghostty code changed. The declared matrix is now 173 PASS, 0 FAIL,
2 BLOCKED, 3 XFAIL, and 5 NOT_IMPLEMENTED. This is not full Linux
qualification: `ime-wayland` remains NOT_IMPLEMENTED and is the next IME
milestone.
