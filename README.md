<h1 align="center">
  <img src="assets/icon.png" alt="Zentty" width="128"><br>
  Zentty for Linux
</h1>

<p align="center">
  <strong>An unofficial Linux port of Zentty, built with Rust, GTK 4, and Ghostty.</strong><br>
  Worklanes, real terminal panes, workspace restoration, and coding-agent awareness
  in a native Linux application—without Electron or a web view.
</p>

<p align="center">
  <a href="#install-on-ubuntu-2404">Install</a> ·
  <a href="#build-and-run-for-development">Build</a> ·
  <a href="#project-status">Status</a> ·
  <a href="docs/cli.md">CLI</a> ·
  <a href="CONTRIBUTING.md">Contributing</a>
</p>

> [!IMPORTANT]
> This is a community Linux port maintained in Jason Maskell's public fork.
> The original Zentty project is created and maintained by
> [dedene](https://github.com/dedene/zentty) for macOS. This fork is not an
> official Linux release from the upstream project.

![Zentty for Linux running Codex with worklanes and agent status](assets/zentty-linux-workspace.png)

## What works

- **Worklanes and panes.** Create, rename, reorder, color, navigate, move,
  split, close, restore, and arrange real Ghostty terminals with independent
  PTYs.
- **Durable workspaces.** Window, worklane, pane, layout, working-directory,
  scrollback, and supported coding-agent restore state survive ordinary
  relaunches and crash recovery.
- **Keyboard-first control.** Bindable commands, command palette, pane and
  worklane navigation, Worklane Peek, pane-local search, and Global Find.
- **Agent awareness.** Managed hooks, authenticated local IPC, sidebar status,
  attention routing, notifications, and safe resume behavior for supported
  coding agents including Codex, Claude Code, Gemini CLI, GitHub Copilot CLI,
  OpenCode, Amp, Cursor, Droid, Kimi, Grok, Antigravity, Hermes Agent, Mistral
  Vibe, Pi, OMP, and Small Harness. Explicit custom-agent status is supported
  through the same protocol.
- **Project context.** Git branch and pull-request state, bookmarks and
  workspace presets, Open With actions, development-server discovery, task
  runners, and a process Task Manager.
- **Terminal workflows.** Ghostty themes, light and dark appearance, clean/raw
  and Markdown copy, URL activation, local and SSH file transfer, scrolling,
  selection, IME input, and Wayland/X11 support.
- **Scriptable automation.** The packaged `zentty` CLI uses the same
  authenticated command API as the GUI. See [`docs/cli.md`](docs/cli.md).

The source-backed feature inventory is
[`docs/design/zentty-linux-feature-inventory.json`](docs/design/zentty-linux-feature-inventory.json).

## Project status

The Linux port is in **daily-dogfood / pre-release** status. It is useful now,
but it has not yet published a signed stable Linux release.

- The initial product feature inventory is implemented.
- The currently qualified package target is **Ubuntu 24.04 LTS (Noble),
  amd64**.
- Both GTK Wayland and X11 paths are exercised by controlled real-product
  integration journeys. Desktop- and GPU-specific defects may still exist.
- Linux update discovery and installation UI is intentionally deferred until
  release preparation. Install upgrades explicitly with a newer `.deb`.
- The optional terminal performance diagnostics overlay is deferred.
- Task Manager network/container telemetry is a future enhancement.
- The qualification matrix still contains explicit blocked, expected-failure,
  and deferred cells. This README does **not** claim exhaustive or full Linux
  qualification.

Open work is tracked in the fork's
[GitHub issues](https://github.com/TamedTornado/zentty/issues). The machine-readable
qualification authority is
[`linux/qualification-matrix.json`](linux/qualification-matrix.json).

## Install on Ubuntu 24.04

There is no public release download yet. Build the native Debian package from
the exact checked-out revision, then install it with APT.

### 1. Install the reviewed build environment

The repository owns a pinned Ubuntu environment manifest and bootstrap. Run
the package installation as root, but install the pinned language tools as
your normal user:

```bash
sudo apt-get update
sudo apt-get install -y jq
mapfile -t packages < <(linux/ci/bootstrap-ubuntu-24.04 --print-apt)
sudo apt-get install -y "${packages[@]}"
linux/ci/bootstrap-ubuntu-24.04 --no-system-install
```

The bootstrap downloads content-pinned toolchains and verifies their hashes.
It includes qualification dependencies as well as the smaller build-time set,
so it is intentionally substantial and reviewable rather than minimal.

### 2. Build the Debian package

Package construction requires a clean checkout:

```bash
linux/scripts/prepare-ghostty-source
ZENTTY_GHOSTTY_PREPARED=true linux/scripts/build-deb
```

Verify and install the resulting artifact:

```bash
cd build/linux-package
sha256sum --check --strict SHA256SUMS
dpkg-deb --info ./zentty_*_amd64.deb
cd ../..
sudo apt install ./build/linux-package/zentty_*_amd64.deb
```

Launch Zentty from the desktop application menu or run:

```bash
zentty-linux
```

The command API is available as:

```bash
zentty --help
```

The complete package ownership, upgrade, removal, and retained-user-data
contract is documented in
[`linux/packaging/README.md`](linux/packaging/README.md).

## Build and run for development

After completing the reviewed environment bootstrap above:

```bash
linux/scripts/build-local
linux/scripts/run-local
```

`build-local` prepares the pinned Ghostty fork and builds a ReleaseSafe Rust/GTK
application bundle under `build/linux/`. `run-local` launches that exact staged
bundle using its packaged-style relative library paths.

Use an isolated state directory when experimenting without your normal
workspace:

```bash
linux/scripts/run-local \
  --state-directory /tmp/zentty-state \
  --no-session-restore
```

The binary exposes launch options and exact build provenance without starting
GTK:

```bash
build/linux/bin/zentty-linux --help
build/linux/bin/zentty-linux --version
```

Linux configuration lives at `$XDG_CONFIG_HOME/zentty/config.toml`, falling
back to `~/.config/zentty/config.toml`. See
[`docs/configuration.md`](docs/configuration.md).

## Architecture and testing

The Linux application is not a re-skinned web terminal:

- product and portable model code are written in Rust;
- the native shell uses `gtk4-rs`;
- terminal rendering and PTY behavior come from a narrow embedding API in a
  maintained Ghostty GTK fork;
- Zentty-specific worklane, agent, settings, and persistence policy stays out
  of Ghostty; and
- the macOS Swift implementation and tests remain the behavioral source for
  port parity.

Real-product integration tests launch the delivered executable with real GTK,
Ghostty surfaces, PTYs, processes, filesystems, local IPC, and controlled X11
or Wayland environments. External coding-model responses are controlled where
necessary. Focused tests cover deterministic models and parsers.

For the architecture decision, test policy, and chronological field evidence,
see:

- [`docs/architecture/0001-rust-gtk4-linux-product.md`](docs/architecture/0001-rust-gtk4-linux-product.md)
- [`linux/test-policy/README.md`](linux/test-policy/README.md)
- [`linux/README.md`](linux/README.md)
- [`docs/design/zentty-linux-dogfood-2026-08-01.md`](docs/design/zentty-linux-dogfood-2026-08-01.md)

The full local qualification entry point is available for release work, but is
not required for ordinary development or every bug fix:

```bash
linux/tests/qualify-local
```

## macOS and upstream Zentty

For the official macOS application, signed `.dmg`, Sparkle updates, macOS build
instructions, and upstream feature development, visit
[`dedene/zentty`](https://github.com/dedene/zentty).

This fork keeps its `main` branch as an upstream/macOS tracking branch. The
Linux product is developed on `linux/port`, which is the intended default
landing branch for this fork.

## Contributing

Contributions are welcome. Start with [`CONTRIBUTING.md`](CONTRIBUTING.md).
Before a non-trivial contribution can be merged, contributors must agree to
[`CLA.md`](CLA.md).

Please keep Linux product policy in Zentty unless Ghostty itself owns the
behavior. Changes to the maintained Ghostty fork should remain minimal,
independently tested, and suitable for upstream review.

## License, attribution, and trademarks

Zentty is available under the GNU General Public License v3.0 only
(`GPL-3.0-only`). See [`LICENSE`](LICENSE).

The original Zentty design and macOS implementation are by
[dedene](https://github.com/dedene/zentty) and its contributors. Ghostty is by
Mitchell Hashimoto and the Ghostty contributors. Third-party notices for the
packaged Linux artifact are generated and verified during packaging.

The GPL license covers the code. It does not grant rights to use the Zentty
name, logos, icons, or other branding for unrelated distributions. See
[`TRADEMARKS.md`](TRADEMARKS.md).
