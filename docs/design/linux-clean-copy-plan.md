# Linux Clean Copy feature plan

Tracking: [GH-35](https://github.com/TamedTornado/zentty/issues/35)

## Source contract

Port the existing Clean Copy family, not a generic clipboard formatter. Ghostty
continues to own selection and the first desktop-clipboard write. Zentty may
read that completed text, apply the source-ordered pure transformation, and
replace the clipboard only when the requested action requires it.

Source authority is the `CleanCopy*`, `MarkdownReformatter`, terminal clipboard,
command registry, and pane-menu code and tests named in GH-35. Where source
classifiers deliberately bail out, Linux must preserve bytes rather than make a
more aggressive aesthetic guess.

## Ownership

- `zentty-core`: immutable options, transformation result, conservative
  classifiers, Clean Copy pipeline, and Markdown reformatter.
- focused `zentty-linux` clipboard module: request identity and asynchronous GTK
  clipboard read/replace after the real Ghostty copy binding.
- existing `ActionRouter` and command palette: exact actions and vocabulary.
- existing product actor: real selection, clipboard service, action, and paste
  round-trip. No new application or clipboard harness.
- existing configuration owner: eventual automatic-clean preference. This
  slice must not create an interim settings file.

## Test construction order

1. Port source-derived pure golden and bailout fixtures as red tests.
2. Implement the smallest ordered pure pipeline that satisfies those fixtures;
   expand fixtures whenever a source branch or mutation survives.
3. Add focused GTK action/presentation tests and asynchronous request policy.
4. Wire raw/clean/Markdown actions through the real Ghostty selection binding.
5. Extend the existing controlled X11/Wayland actor to assert external desktop
   clipboard bytes and a real paste round-trip.
6. Mutation-test pure policy, then run all workspace and qualification gates.

## Claim limits

- Pure fixtures do not prove Ghostty selection or desktop clipboard ordering.
- An internal GTK receipt does not prove compositor clipboard ownership; the
  controlled actor must read the external clipboard and paste it into a PTY.
- X11 does not establish Wayland.
- Automatic-clean configuration remains incomplete until the one settings
  owner exists; accessible explicit actions can ship independently if this is
  visible in inventory and issue state.

## Evidence-driven architecture correction

The first real implementation disproved the assumption that Ghostty's binding
establishes a fresh clipboard owner: with no selection it succeeds without
changing the clipboard. A later GTK read can therefore observe unrelated stale
bytes. The accepted implementation replaces steps 3–4's asynchronous
post-copy path with one minimal Ghostty-owned synchronous selection-read ABI,
a safe Rust copy of its borrowed callback bytes, and a Zentty-owned clipboard
write. The real actor must prove that no-selection leaves an independent
clipboard sentinel untouched. This correction removes asynchronous lifecycle
state; it does not relax any external compositor or PTY round-trip evidence.
