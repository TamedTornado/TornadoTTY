# Embedded terminal AT-SPI role

## Dogfood finding

While diagnosing intermittent `tt-stt-flow` focus protection failures, the
speech-to-text companion reported the exact focused pre-rebrand object as:

```text
application="com.zentty.zentty"
window=""
role="panel"
element="path:/org/gtk/application/com_zentty_zentty/a11y/..."
```

The object accepted terminal input and was the embedded Ghostty surface, but
GTK exposed it as a generic panel. This deprived assistive clients of terminal
semantics and made cross-application focus diagnostics less meaningful.

## Repair

Tornado TTY now assigns host-owned accessibility metadata immediately after each
Ghostty surface is created and before terminal initialization callbacks run:

- role: `terminal`;
- label: `Terminal pane <durable-pane-id>`.

The label is deliberately independent of shell titles, agent status, widget
indexes, and sidebar rendering. Initial panes and panes created later use the
same sole surface-construction path. Keyboard focus, PTY behavior, and Ghostty's
native lifecycle are unchanged. No Ghostty fork change was required.

## Focused evidence

The external AT-SPI product contract was strengthened first to require the
initial `pane-1` and runtime-added `pane-2` as distinct terminal nodes. The
pre-fix real dogfood receipt already established the incorrect `panel` role.

Focused automated receipts:

```text
terminal_accessibility_label_is_stable_and_pane_specific: 1 PASS
actual_terminal_widget_exposes_terminal_accessibility_semantics: 1 PASS
ReleaseSafe build and package-age audit: PASS
rustfmt on touched Rust: PASS
actor shell syntax and git diff checks: PASS
```

## External qualification closure (2026-09-01)

The earlier external failure was reproducible, but its diagnosis was obscured
by two stale test assumptions. The inspector filtered for the pre-rebrand
application name `zentty`, and the contract expected pre-rebrand and
pre-working-directory accessible labels. Repeated one-second probes therefore
rejected a live tree until the 30-second shell exited. A PID-only inspector mode
now captures the real application first; the contract independently asserts
the public root identity `com.tamedtornado.tornadotty`, and later actions reuse
the observed name. Exact PID matching, wrong-name rejection, and stale-PID
rejection remain mandatory.

The captured product tree established:

- application `com.tamedtornado.tornadotty`, exact launched PID;
- frame `Tornado TTY` in the active state;
- initial `Terminal pane pane-1` with role `terminal`;
- focus on Ghostty's descendant inside that terminal node;
- a real external `Add Pane Right` action;
- runtime `Terminal pane pane-1` and `Terminal pane pane-2`, both with role
  `terminal` after the action.

Additional discoveries and repairs:

1. The sandbox filesystem namespace presented `/tmp/.X11-unix` with the wrong
   effective owner to Xvfb even after the host directories were verified as
   root-owned mode `1777`. The controlled display journey therefore runs at
   host scope; absence inside the namespace was not converted to a pass.
2. The runner's supposedly valid JSON fixture did not contain the terminal
   node required by the authoritative contract. The fixture now models the
   terminal and its focused Ghostty descendant, and its missing-node,
   missing-state, false-identity, invalid-PID, missing-registry, fabricated
   input, and uncontrolled-display rejection cases pass.
3. The backend-independent widget suite contains a pointer ownership assertion
   whose deterministic controlled display is X11. It is executed once in the
   X11 cell instead of being mislabeled as Wayland accessibility coverage. Both
   X11 and Wayland still launch independent real product processes and private
   AT-SPI registries and exercise the live external action/topology contract.
4. The nested Wayland process inherited `XDG_CURRENT_DESKTOP=GNOME` from the
   developer session. That made the private Cage environment repeatedly launch
   and crash `xdg-desktop-portal-gnome`. The private wrapper now supplies an
   isolated portal policy selecting `xdg-desktop-portal-gtk`; environmental
   absence was not accepted as success.

Final focused receipts:

```text
external-atspi-runner-test: PASS
rust-worklane-accessibility: PASS backend=x11 widget-semantics=executed external-atspi=real control=real-gtk live-pane-change=1 terminal-semantics=1
rust-worklane-accessibility: PASS backend=wayland widget-semantics=covered-by-x11-cell external-atspi=real control=real-gtk live-pane-change=1 terminal-semantics=1
```

The occasional stock-control warning about a missing initial
`/org/a11y/atspi/cache` object is emitted while AT-SPI populates its cache; the
subsequent exact-PID control and product receipts pass and are the authoritative
result. GH-141 is externally qualified and no human-only validation remains for
its acceptance criteria.
