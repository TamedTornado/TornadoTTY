# Linux Ghostty appearance and live reload feature plan

Status: active implementation plan for GitHub issue #20
Date: 2026-08-12

## Source contract

The macOS source owns a Ghostty configuration stack, theme resolution, theme-mode
memory, opacity, and live runtime reload. It writes through symlinked Ghostty
configuration, preserves unrelated lines, resolves recursive `config-file`
includes, and updates every surface through Ghostty rather than recreating PTYs.

Linux will preserve those product behaviors while mapping `followMacOS` to the
desktop-neutral name `automatic`. Compositor blur is not part of this slice and
must remain an explicit later platform alternative.

## Feature slice

1. Add the smallest upstream-reviewable Ghostty GTK embedding operation needed to
   hard-reload default configuration into the existing embedded runtime and all
   live surfaces. It must reject null/stale handles and report load/update failure.
2. Wrap that operation in the existing safe Rust adapter; do not create a second
   runtime, surface registry, or configuration parser for terminal behavior.
3. Add a pure Rust appearance model for dark/light/automatic theme specifications,
   theme-name sanitization, opacity normalization, and comment-preserving Ghostty
   key updates.
4. Persist Zentty-owned appearance memory through the existing `ConfigStore`, and
   write terminal-facing values through the real XDG Ghostty configuration without
   replacing symlinks.
5. Wire the four already-audited appearance commands and Reload Configuration into
   the one shortcut/action registry. Apply reload to the process-global Ghostty
   runtime so every open window and existing PTY receives it.
6. Extend the settings surface with an Appearance section only after the model and
   runtime boundary are proven. Theme catalog/gallery and background-image UI are
   later slices within issue #20, not invented in this slice.

## Bundled fallback-theme slice

Before the gallery can honestly advertise the source default, make that theme a
real distributable resource:

1. Stage the checked-in `GitHub-Dark-Personal` theme under Zentty's versioned
   application data directory and require packaging/install tests to retain it.
2. Resolve the resource relative to the delivered executable, not the repository
   checkout or current working directory.
3. When—and only when—a persisted Ghostty theme specification references the
   source fallback, install an exact private copy into the user's XDG Ghostty
   theme directory if absent. Never overwrite or follow an existing theme entry.
4. Drive `Use Dark Theme` through the real staged command palette on controlled
   X11 and Wayland, then assert the installed bytes, permissions, persisted theme,
   native reload, and surviving PTYs. Plain Ghostty compatibility is part of the
   feature: the copied theme must be discoverable without Zentty running.
5. Mutation-test the pure reference/resource policy, then rerun all presently
   executable qualification cells before committing.

## Theme catalog and Appearance projection slice

The next feature slice makes the source theme library usable rather than merely
shipping its default:

1. Stage the complete checked-in Ghostty theme library and make the staged-bundle
   tests reject a missing, extra, or byte-different theme.
2. Add one bounded theme-catalog owner that parses Ghostty theme files, applies
   deterministic bundled/user precedence, exposes dark/light classification and
   preview colors, and skips invalid entries without treating them as themes.
3. Replace the shortcuts-only settings presentation with one settings shell and
   distinct Appearance and Shortcuts pages. Appearance must expose source theme
   mode, dark/light slots, searchable/filterable real themes, live selection,
   and background opacity; it must use the existing `ConfigStore` and Ghostty
   reload path rather than a second persistence system.
4. Drive the real staged UI with physical input under controlled X11 and nested
   Wayland. Assert search/filter, a user theme overriding a bundled name, theme
   persistence and live reload, opacity persistence and live effect, process
   continuity, settings navigation, and restart.
5. Mutation-test catalog precedence/parsing/filtering and appearance application,
   then rerun the complete presently executable matrix before committing.

## Test construction order

1. Ghostty Zig contract tests and exported-symbol/old-new ABI mismatch tests.
2. Rust adapter null/lifecycle and runtime reload tests.
3. Pure appearance/config writer tests, including malicious names, bounds,
   comments, duplicate keys, symlinks, permissions, and preserved unknown lines.
4. Controlled X11 and Wayland journeys using real Ghostty surfaces and PTYs. Change
   the real config, invoke reload through the real action/shortcut path, and prove
   the existing terminal process survives while a visible runtime property changes
   across multiple existing surfaces. Add the cross-window form before the wider
   issue #20 appearance work claims every-window reload coverage.
5. Focused mutation testing with repository-enforced `gitignore=true` and
   `copy_target=false`.
6. Full presently executable qualification only after focused failures are fixed.

## Acceptance boundary

This slice is complete only when the Ghostty change is independently reviewable,
the Zentty work uses the existing ownership architecture, both controlled
compositors pass real product journeys, and no failing cell is hidden. Issue #20
remains open for the complete settings navigation, theme gallery/resources,
background images, external watcher/partial reload, and remaining sections.
