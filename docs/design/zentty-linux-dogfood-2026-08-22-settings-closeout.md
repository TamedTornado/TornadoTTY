# Linux settings epic closeout dogfood record

Date: 2026-08-22
Tracking: GH-20, GH-7

This append-only record covers the final Settings/Appearance epic audit and
implementation. It begins before product changes so discoveries cannot be
rewritten after the fact.

## Discovery: aggregate PASS did not mean feature completion

- Every GH-20 child issue is closed and the authoritative
  `platform-settings-contract` cell is `PASS`.
- The source-backed feature inventory nevertheless still classified
  `configuration.ghostty-appearance` as `NOT_IMPLEMENTED`.
- Comparing the current GTK Appearance page with
  `AppearanceSettingsSectionViewController.swift` showed that this was not stale
  bookkeeping. Linux omitted the source **Sync OpenCode Theme** switch and the
  **Sidebar selection** Subtle/Vivid choice.
- Linux already parsed and persisted `sync_opencode_theme_with_terminal`, but no
  UI exposed it and no Linux runtime consumed it. Promoting that field based on
  serialization alone would be a false implementation claim.
- The managed OpenCode launcher already creates a private overlay and preserves
  the user's source configuration. That existing authority is the correct place
  for a generated synchronized theme; a second OpenCode configuration system is
  forbidden.
- Sidebar worklane cards already carry identity-color CSS classes and an active
  tint class. Selection emphasis can therefore be a projection policy over the
  existing card rather than a second sidebar/theme owner.
- GH-20 also promises background-image handling and a Linux blur decision.
  Background images belong to Ghostty configuration and must survive Zentty
  appearance writes/reloads; Linux has no portable compositor blur protocol and
  must document that degradation rather than claim an effect.

## Decision

The epic will not be closed by editing the inventory alone. The final coherent
feature batch ports the two missing source controls, implements real managed
OpenCode synchronization, verifies background-image preservation, records the
no-blur alternative, and extends the established controlled X11/Wayland settings
journey. Focused tests are written before implementation. The existing
`AppConfig`, `ConfigStore`, managed launcher overlay, agent status/process
identity, sidebar projection, and qualification runner remain the only owners.

## Discovery: the source shortcut catalog still had two missing commands

- A source-to-Linux command-registry comparison found that the audited source
  commands `open_with.selected_app` and `server.open_selected` were not
  bindable. The prior command-registry tests proved internal consistency, not
  source completeness.
- `open_with.selected_app` now routes to the existing primary Open With
  authority. `server.open_selected` selects the active worklane's highest-ranked
  detected server and routes through the existing server-browser authority. No
  second launcher or scanner was introduced.
- `linux/tests/feature-inventory` now parses the Linux registry and rejects
  omitted audited source commands. Its negative self-test removes the server
  command and proves that omission is fatal. Linux-only fullscreen/minimize
  commands remain explicitly allowed additions.
- The development-server journey binds the new command, sends a physical
  `Ctrl+Alt+O`, and proves the exact selected URL reaches a real controlled
  browser process on both compositors.

## Implementation: final appearance behavior

- The Appearance page now exposes the source OpenCode synchronization toggle
  and Subtle/Vivid sidebar selection policy through the single `AppConfig` and
  `ConfigStore`.
- OpenCode synchronization generates paired dark/light theme JSON from the
  actual selected Ghostty theme palettes. It publishes only to a private,
  per-pane managed overlay. The user's OpenCode configuration and theme files
  are never rewritten.
- Live refresh requires three independently checked facts: the process identity
  is OpenCode, its environment identifies Zentty's private overlay, and that
  overlay selects the managed theme. Only then, and only after theme bytes
  change, does Zentty send OpenCode `SIGUSR2`.
- Atomic theme publication uses a process-local monotonic temporary-name
  sequence. This repaired a stale-temporary collision possible when a PID was
  reused after an interrupted write.
- Subtle/Vivid changes the strength of the existing active worklane identity
  projection. It does not create another focus or selection state.
- Zentty appearance writes preserve arbitrary Ghostty configuration, including
  `background-image`. Linux blur remains an explicit degradation because GTK,
  Wayland, and X11 provide no portable compositor blur contract.

## Failures and repairs

1. The first controlled real-agent run reported no managed synchronized theme.
   The product binary had been restaged but the `zentty` launcher helper inside
   the agent-wrapper bundle was stale. Rebuilding and staging the whole bundle,
   rather than copying only `zentty-linux`, repaired the product contract.
2. Early physical attempts to select Vivid with positional Tab/arrow input
   failed after the settings page rebuilt itself. The source control's real GTK
   mnemonic is now used, making the journey independent of destroyed widget
   instances.
3. The first native export attempt used a full-path chooser location and failed
   with `Permission denied`. Both real choosers now start in the isolated home
   directory. Export drives the real filename field and action button.
4. Wayland import initially relied on the file list's initial selection and was
   scheduler-sensitive. A fixed 0.25-second mapping delay passed once and then
   failed on a later clean rerun. Cage does not publish the chooser transient
   through the foreign-toplevel protocol, so `wlrctl` could not provide a map
   receipt. The maintained journey now allows the asynchronous chooser and
   directory enumeration to settle, addresses the already-exported real file
   through the chooser's `Ctrl+L` location entry, and accepts success only after
   the application import receipt and restored binding are observed. The final
   controlled Wayland run passed.
5. The Wayland development-server journey initially observed the controlled
   browser receipt before the application emitted its result log. Waiting for
   both independently observable receipts repaired the race without weakening
   either assertion.
6. An accidental direct invocation of `linux/tests/qualification-matrix`
   attempted environment-dependent local cells. Ghostty preparation failed
   because sandbox DNS could not resolve GitHub, and Valgrind governance found
   a stale prior report. Neither was converted into a pass and neither is cited
   as product evidence. Closeout uses the bounded affected gates ratified in the
   execution plan rather than another unrelated app-wide run.

## Mutation testing

- The governed wrapper enforced `.cargo/mutants.toml` with `gitignore = true`
  and `copy_target = false`, plus the documented process address-space limit.
- An initial eight-worker OpenCode run stopped while all workers rebuilt GTK;
  it produced no outcomes and is not a pass. Four workers stayed within the
  governed resource envelope.
- Final OpenCode synchronization result: **49 mutants tested, 49 caught**.
- The first sidebar-emphasis run caught three of six mutants. The survivors
  exposed missing direct coverage for exact `subtle`/`vivid` serialization and
  parsing. Exact round-trip tests were added; the rerun result was **6 mutants
  tested, 6 caught**.

## Final focused qualification

- Formatting, workspace Clippy with warnings denied, architecture ownership,
  feature-inventory validation and negative self-tests, qualification-matrix
  runner tests, focused config/shortcut/action/OpenCode tests, and elevated real
  Unix-socket launcher tests passed.
- A complete ReleaseSafe bundle was rebuilt through `linux/scripts/build-local`;
  this stages the product, CLI helper, agent wrappers, Ghostty library, themes,
  shell integration, and notices as one artifact.
- Controlled X11 settings session:
  `7d3ba1e82242e814258c53885281f2569445a5acadb1bd87ab735cf61a751f25`.
- Controlled Wayland settings session after the chooser repair:
  `e3c35dbd1878a7bdd850a483bea527560878c0a7421fcd77aa7c9c84987c1c07`.
- Controlled development-server sessions: X11
  `4576fd14bd027c42c90c7a0d21afd4066ccfe661f91bf092074df93cda58dfa0`
  and Wayland
  `a72241debce76598864fb60c7602b5636bcae5ba9a5adefca2ff98a89639b6cd`.
- Controlled real-agent sessions: X11
  `386b468ee9580c37a1c78d383b29a24c7a24170c946e124223ebea17ead95c4a`
  and Wayland
  `5d756a8a6422f28c4e9a9ce0f9952450ee37a0564ff9bcc9968b868f4153bafc`.
- The authoritative feature inventory is now **30 IMPLEMENTED, 19 PARTIAL,
  11 NOT_IMPLEMENTED**. The qualification matrix declaration remains **188
  PASS, 0 FAIL, 0 BLOCKED, 3 XFAIL, 4 NOT_IMPLEMENTED**. Therefore this is not
  an exhaustive or full-Linux-qualification claim.
