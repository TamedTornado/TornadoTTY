# Zentty Linux host

This directory contains the Linux product host and its external-boundary
integration tests. It does not build or import Ghostty's historical in-tree
embedding spike.

## Architecture

- `src/main.c` is the minimal GTK product executable.
- `ghostty.lock` pins the public Ghostty fork and exact embedding revision.
- `scripts/build-local` fetches that revision unless `GHOSTTY_SOURCE_DIR`
  selects an already-verified checkout, builds the shared embedding library,
  and stages a relative `bin`/`lib` bundle under ignored `build/linux`.
- `tests/` launches that product executable and asserts behavior across the C
  ABI, GTK, Ghostty, PTY, renderer, and compositor boundary.

The current host is deliberately small. Product features should enter only
with failing end-to-end tests; the old Ghostty spike remains disposable
evidence and must not become the Zentty implementation.

## Qualification

`linux/qualification-matrix.json` is the single authoritative qualification
inventory. Every row declares one of `PASS`, `FAIL`, `BLOCKED`, `XFAIL`, or
`NOT_IMPLEMENTED`, and the runner rejects missing terminal combinations,
unknown states, unexplained blockers, untracked expected failures, unexpected
skips, and stale XFAILs. Prose documentation is explanatory only and may not
override that file.

Run every presently executable cell with:

```sh
linux/tests/qualify-local
```

The command writes `build/linux/qualification-summary.json` and a log per cell
under `build/linux/matrix-logs/`. Its report deliberately distinguishes an
implemented local-suite pass from release qualification and full Linux
qualification. Non-pass required cells prevent the latter claims even when
every executable local command succeeds.

Validate matrix structure without executing product tests:

```sh
linux/tests/qualification-matrix --validate-only
linux/tests/qualification-matrix-test
```

Useful narrow diagnostic commands remain available:

```sh
GDK_BACKEND=wayland linux/tests/interaction
GDK_BACKEND=x11 linux/tests/interaction
ZENTTY_CONTROLLED_X11_SCENARIO=physical-key linux/tests/controlled-x11
ZENTTY_CONTROLLED_X11_SCENARIO=resize linux/tests/controlled-x11
```

The controlled X11 harness uses Xvfb and software rendering; missing required
tools exit as an unexpected skip rather than a pass. The matrix records the
absence of an equivalent controlled Wayland environment explicitly.

See `docs/design/zentty-linux-dogfood-2026-08-01.md` for commands, failures,
repairs, receipts, environment limits, and the complete qualification record.
