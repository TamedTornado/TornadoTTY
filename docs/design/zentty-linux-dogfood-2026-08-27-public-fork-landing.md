# Dogfood record: public Linux fork landing

Date: 2026-08-27

## Discovery

The public fork already described itself as a Linux port, but its GitHub
default branch was `main`. That branch and its README presented the upstream
macOS product first: the primary download was a DMG, installation described
Sparkle and notarization, requirements were macOS and Xcode, and the Linux port
appeared only as a development subsection. A visitor had to discover the
`linux/port` branch independently.

Branch ancestry showed that `origin/main` is an ancestor of `origin/linux/port`.
There were no commits unique to `main`; `linux/port` contained the complete
source plus 511 port commits. Making the port branch the public landing branch
therefore did not require rewriting, deleting, or merging upstream history.

## Provenance and presentation decision

- Preserve `main` unchanged as the macOS/upstream-tracking branch.
- Use `linux/port` as this fork's GitHub default branch.
- Lead the default README with “unofficial Linux port,” Rust/GTK/Ghostty
  architecture, and a prominent link and credit to `dedene/zentty`.
- Do not use `zentty.org` as this fork's homepage or imply that this fork owns
  the official macOS downloads, signing, Sparkle channel, or branding.
- Keep GPL, CLA, upstream attribution, and trademark boundaries visible.

## README repair

The Linux landing page now provides:

- a real, public-safe controlled-X11 screenshot copied byte-for-byte from the
  pane-search visual baseline;
- delivered worklane, pane, restoration, command, terminal, project, and
  coding-agent capabilities;
- honest daily-dogfood/pre-release status and the absence of a signed public
  Linux release;
- the qualified Ubuntu 24.04 LTS amd64 package target;
- the reviewed user/root split for environment bootstrap;
- exact Debian package build, checksum, inspection, install, launch, and CLI
  commands already owned by repository scripts;
- the faster staged development build/run route;
- explicit update-discovery, diagnostics-overlay, and Task Manager telemetry
  deferrals; and
- a warning that explicit non-pass qualification cells remain and that the
  README does not claim exhaustive or full Linux qualification.

The official macOS application and its signed distribution remain linked in a
separate upstream section instead of being silently removed from the project's
provenance.

## Focused validation

Static validation checked every relative Markdown target, every documented
repository executable, the screenshot's exact PNG dimensions/type, JSON-backed
initial-release status, whitespace, and removal of the stale primary DMG,
macOS-terminal, and Sparkle landing claims. The bootstrap's package-list mode
was executed without installing or modifying the host. No product code changed,
so a full application qualification run would add no relevant evidence and was
not performed.

## Remaining public limitation

The repository still has no signed, published Linux release artifact. Visitors
must build the native Debian package from source until release preparation and
signed update/package policy in issue #75 are completed. The README states this
directly rather than offering an unqualified developer artifact as a release.

## Public receipt

- Landing commit: `3966ddc8d90114a465abf35672c65b21b5dc45f1`.
- GitHub default branch: `linux/port` at that exact commit.
- Repository description: `Unofficial public Linux port of Zentty using Rust,
  GTK 4, and Ghostty`.
- Topics: `coding-agents`, `ghostty`, `gtk4`, `linux`, `rust`, and `terminal`.
- Homepage: empty; the upstream-owned `zentty.org` value was removed.
- GitHub's raw default-README endpoint returned the Linux heading, unofficial
  fork disclosure, upstream link, and Linux screenshot after the metadata
  change.
