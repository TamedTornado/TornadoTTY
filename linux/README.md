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

## Qualification commands

The complete local matrix is one command:

```sh
linux/tests/qualify-local
```

It runs the ReleaseSafe semantic, staged-bundle, and ten-process lifecycle
matrix on both backends; rebuilds Debug for both memory gates; then restores a
ReleaseSafe bundle and reruns both semantic checks. The individual commands
below are useful while diagnosing a specific gate.

ReleaseSafe product boundary:

```sh
linux/scripts/build-local
GDK_BACKEND=wayland linux/tests/single-terminal
GDK_BACKEND=x11 linux/tests/single-terminal
GDK_BACKEND=wayland linux/tests/staged-bundle
GDK_BACKEND=x11 linux/tests/staged-bundle
GDK_BACKEND=wayland linux/tests/repeated-lifecycle
GDK_BACKEND=x11 linux/tests/repeated-lifecycle
```

Debug memory boundary:

```sh
GHOSTTY_OPTIMIZE=Debug linux/scripts/build-local
GDK_BACKEND=wayland linux/tests/memory-safety
GDK_BACKEND=x11 linux/tests/memory-safety
```

The memory script refuses an artifact whose build metadata does not say
`optimize=Debug`. ReleaseSafe currently has a separately recorded open
Valgrind value-report investigation; a Debug pass must not be represented as a
ReleaseSafe memory result.

See `docs/design/zentty-linux-dogfood-2026-08-01.md` for commands, failures,
repairs, receipts, environment limits, and the complete qualification record.
