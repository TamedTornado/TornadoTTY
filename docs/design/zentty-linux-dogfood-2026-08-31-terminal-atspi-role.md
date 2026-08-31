# Embedded terminal AT-SPI role

## Dogfood finding

While diagnosing intermittent `tt-stt-flow` focus protection failures, the
speech-to-text companion reported the exact focused Zentty object as:

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

Zentty now assigns host-owned accessibility metadata immediately after each
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

The controlled external AT-SPI actor was attempted twice. Its stock GTK control
and in-process GTK accessibility assertions passed, but its private registry
returned `Object does not exist at path /org/a11y/atspi/cache` and captured zero
Zentty applications on both attempts. Those environmental failures are not
counted as either a product pass or a role-specific pre-fix failure. The actor
retains the stronger terminal assertions and will reject a regression when the
private registry is available again.

## Human validation remaining

After installing and restarting the fixed Zentty build, a normal dictation can
confirm the real desktop integration by inspecting `tt-stt-flow`'s focus
receipt. GH-141 remains open until that external consumer reports role
`terminal` for the focused embedded surface.
