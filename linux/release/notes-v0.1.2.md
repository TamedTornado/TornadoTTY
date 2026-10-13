## Highlights

- Faster server-menu updates: keep the native dropdown attached instead of
  repeatedly replacing it, avoiding the attachment path observed blocking GTK
  for several seconds during worklane switches.
- Improve pane visibility, column identities, startup widths, sidebar resizing,
  and recovery-snapshot retention. Closing the last shell closes its owning
  window rather than unrelated windows.
- Hold Ctrl to hide pane controls and click terminal content underneath them.
- Reduce duplicate coding-agent notifications, preserve remapped keyboard input,
  and correct delayed Codex working-state updates.
- Allow safe OpenCode npm-bin symlinks in private launch overlays while rejecting
  absolute, escaping, cyclic, and unresolved links.
- Improve bounded local stall diagnostics and move more discovery, persistence,
  and process-monitoring work off the GTK thread.
- Cancel stale input-method commits after pane focus changes, preventing extra
  text during Fcitx composition.
- Defer the first terminal draw until its snapshot is ready, preventing
  uninitialized shader input during pane startup.
- Use the installed `tornadotty` and `tornadotty-cli` entry points; the legacy
  Zentty executable aliases and system-tray agent-status feature are removed.

## Packages

- Ubuntu 24.04 LTS / Debian-family: `tornadotty_0.1.2_amd64.deb`
- Omarchy 4 / Arch-family: `tornadotty-0.1.2-1-x86_64.pkg.tar.zst`

Verify downloads against `SHA256SUMS`. Expanded package manifests and build
provenance accompany both native artifacts. Install with the appropriate system
package manager and restart TornadoTTY when ready; replacing files does not
update an already-running GUI.

## Qualification and limitations

This is an **unsigned prerelease**, not a signed stable-release or exhaustive
Linux-qualification claim. Checksums and provenance are not signatures.

The server-menu fix removes a measured blocking path; it does not establish
that all long-running stalls or the separately tracked Wayland display
disconnects are solved. Shell/workload containment work remains incomplete.
ReleaseSafe Valgrind cases remain expected failures, and the qualification
matrix retains explicit unimplemented capabilities.

Current release evidence and any remaining acceptance gaps are tracked in
[#206](https://github.com/TamedTornado/TornadoTTY/issues/206).

TornadoTTY is an unofficial fork of Zentty, originally created by dedene. It is
not affiliated with or endorsed by Zenjoy BV.
