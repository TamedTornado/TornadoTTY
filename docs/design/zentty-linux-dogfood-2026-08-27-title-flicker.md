# Linux dogfood: stable Codex title rendering (GH-132)

Date: 2026-08-27

## Operator discovery

Live dogfood showed a changing Codex title such as
`Working ⠋ consulting | Tasks 6/7` visibly flickering in the window chrome,
worklane projection, and pane row. The window chrome could also display the
stable worklane fallback and animated pane spelling side by side, making one
semantic title appear twice. This was reported from the installed GNOME build,
not inferred from a unit test.

## Root cause and design correction

GH-123 correctly separated raw spinner frames from persisted pane identity,
but its Linux renderer was an inaccurate approximation of upstream
`SidebarShimmerTextView`. Upstream retains a stable native label/layout and
draws animation independently. Linux called `gtk::Label::set_text` with the
complete title every 100 ms. GTK therefore remeasured/repainted an identity
label for a one-glyph animation. The same callback also reset unchanged
worklane context each frame. Finally, chrome compared the animated spelling to
the stable fallback literally, so the spinner made equivalent identities look
different.

The repair keeps workspace state, persistence, history, title parsing, agent
status, and Ghostty unchanged. A focused GTK activity-title widget now owns:

- a stable label used for layout, ellipsis, accessibility, and idle display;
- static prefix and suffix labels used only while eligible; and
- one fixed-width spinner label, the only text changed per animation frame.

Worklane context is not touched by animation frames. Focused window chrome is
kept stable and changes only for semantic title updates such as task progress.
Chrome deduplication compares stable pane identity to the worklane fallback.
No debounce, sleep, timeout, second clock, or second agent-status system was
introduced; the existing shared GTK frame-clock coordinator remains.

## Failures retained as evidence

1. `xvfb-run -a cargo test ...activity_changes...` failed before widget
   construction because GTK could not initialize inside the GUI sandbox.
2. Repeating with `GDK_BACKEND=x11` failed at the same initialization boundary.
3. The identical focused test was then run with approved GUI access in a
   private Xvfb display and passed. The two environmental failures were not
   converted into passes or hidden by skipping initialization.
4. The first staged build attempt failed before compilation when the sandbox
   blocked DNS for the pinned Ghostty verification fetch. The same reviewed
   build was rerun with network permission; Ghostty remained pinned at
   `80054768edbffd5df8568782e528363033a49192`.
5. Diff review found that `render_codex_activity_titles` still returned
   `focused-chrome=true` after chrome repainting had been removed. That would
   have produced a dishonest integration receipt. The method now returns
   `false`, and the existing journey requires both
   `sidebar=true focused-chrome=false` and the exact stable chrome title.

## Focused evidence

- `cargo check -p zentty-linux`: PASS.
- `cargo test -p zentty-core --test codex_title`: PASS, 5 tests.
- `cargo test -p zentty-linux codex_title_animation::tests --no-fail-fast`:
  PASS, 4 tests.
- `cargo test -p zentty-linux window_chrome::tests --no-fail-fast`: PASS,
  3 tests including unnamed-worklane semantic deduplication.
- Private-Xvfb `activity_title` exact widget test: PASS. It inspects the
  stable label, static prefix/suffix, one-character spinner width and text,
  and stable-child restoration.
- `bash -n` and `shellcheck -x linux/tests/rust-multi-window`: PASS.
- `linux/scripts/build-local`: PASS; Cargo publication-age audit reported 91
  packages and zero exceptions, and package notice collection passed.
- Existing `ZENTTY_MULTI_WINDOW_FLEET_ONLY=true` controlled X11 journey:
  PASS in its real staged application with two GTK windows, real Ghostty PTYs,
  physical input, authenticated agent IPC, exact spinner frame, stable chrome,
  unchanged sidebar hierarchy, surviving Agent Status popover, and idle
  teardown.

No full qualification was run or claimed for this dogfood repair. Operator
visual confirmation on the installed GNOME build remains required after the
next coordinated deployment.
