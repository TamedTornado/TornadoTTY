# Zentty Linux Dogfood: Navigation Recents

Date: 2026-08-22  
Issue: GH-79

## Record

### Source reinspection corrected the initial issue draft

- **Discovery:** The initial issue draft proposed durable recent-command
  persistence and recording every palette target. Direct inspection of
  `RecentCommandsTracker` and `CommandPaletteController.executeItem` showed
  that the macOS source keeps history only for the controller session and
  deliberately excludes pane destinations, restored commands, and task runners.
- **Decision:** Correct GH-79 before implementation. Linux will port the actual
  selective, session-scoped behavior instead of treating an earlier inventory
  note as stronger authority than source.
- **Risk avoided:** Persisting or recording all targets would make Linux look
  superficially richer while changing source UX, retaining unsafe stale dynamic
  destinations, and repeating the prior failure mode of inventing features.

Further test-first failures, repairs, receipts, and residual limitations will be
appended as implementation proceeds.

### Existing fleet Quit route re-entered a mutably borrowed shell

- **Discovery:** Requalifying the existing physical multi-window fleet journey
  aborted in `ApplicationCoordinator::application_close_evidence`. The generic
  simple-action adapter held a mutable `ApplicationShell` borrow while
  `request_quit` synchronously asked the coordinator to collect close evidence
  from every shell, including the already mutably borrowed owner.
- **Evidence:** Controlled X11 session
  `dbe49354871fb68389862e377603351d6d214f5de18f0093670b68723350dad4`
  reached the rendered fleet footer and physical Quit activation, then failed
  with `RefCell already mutably borrowed` at `application.rs:1017`.
- **Repair:** Register Quit as a read-only simple action rather than routing it
  through the mutable action adapter. Multiple immutable borrows are valid
  while the evidence walker snapshots all windows; the existing deferred
  coordinator shutdown remains unchanged.

### Palette-focused Wayland keys bypassed the window activation clock

- **Discovery:** The existing exact cross-window fleet route succeeded in
  selecting the correct real pane but twice failed its Wayland activation-token
  contract. Sessions
  `5e9d8596cb9cdf05cca2b57ad4d79124735cb3596d2cc64148ad262fdd53d403`
  and `c4a295f816c6d204a2ffc3bff18edef9b23b3ac70ba9a57cd71d02b055869d00`
  both logged `event-time=none startup-id=absent` after physical Return.
- **Cause:** Once the palette search entry owned focus, its capture-phase key
  controller could consume Return without the window-level controller seeing a
  usable timestamp. The controlled Cage virtual-keyboard profile also reports
  `CURRENT_TIME`; it cannot qualify `xdg-activation` and was the wrong profile
  for this matrix cell. Sessions
  `08d114f93470861ecb457051417672a4d2fe9a22a75a28b6a1dc682c1d4f19f0`
  and `d520e235acf4f4ebab91803e2c6dde6454567d0bff1f39a8e1f5b25e22eac4e0`
  confirmed that limitation rather than product failure.
- **Repair:** Feed the palette controller's actual GDK event-object timestamp
  into the existing window-owned `UserActivationClock`, while retaining its
  strict next-idle expiry and single-use semantics. Requalify only with the
  matrix-owned `nested-wayland-activation-v1` Labwc profile, which supplies
  real outer-X11 input and advertises `xdg-activation`; do not convert Cage's
  missing timestamp into a pass.

### Mutation testing exposed four initially weak assertions

- **First run:** 41 mutants produced 28 caught, four missed, and nine
  compile-unviable mutations. The misses changed the eligibility guard, all
  section titles, and the active-query branch.
- **Repair:** Tests now inspect the tracker before resolution for disabled and
  ineligible recording, pin every source section title, and require an active
  query to return only the `Results` section.
- **Final run:** 41 mutants produced 32 caught, zero missed, and nine
  compile-unviable mutations. The invocation used the permanent
  `.cargo/mutants.toml` `gitignore = true` and `copy_target = false` policy plus
  `--gitignore=true`; no ignored `build/linux-deps` tree was copied.

### Final real-product evidence

- Controlled X11 source UX session
  `52f453a992b1c8ade4f965beec0a93cc16ab656294db07860f55ad80bdbe04b1`:
  physical palette pane jump, Recent Panes/Recent Actions separation,
  source-eligible command recording/deduplication, restored sidebar geometry,
  exact real-PTY focus, and the established pointer pane journey passed.
- Controlled Wayland Open With session
  `148a77b0a1745f6b2df8ab5adaced6860039c979c557d6a8a036c72aaafcda65`:
  three successful real external-app routes appeared in the rendered Recent
  Actions section; physical input, real desktop entries, real executable
  launch, canonical paths, and disposable SSH rejection passed.
- Controlled X11 fleet session
  `ccd39c7c297232ebc1f726a36499c0f53c546c895e0515fa011f96f553a32598`:
  physical fleet lifecycle, repaired Quit path, cross-window exact routing, and
  real PTY focus passed.
- Matrix-owned Labwc Wayland activation session
  `028ee4c24528c242ffcfe8ae9f2ac1d7b310e8e2e5f554161f752845387d5ec6`:
  real event-authorized cross-window fleet activation and exact PTY routing
  passed. Cage timestamp absence remains environmental and was not called a
  pass for activation qualification.
- Workspace formatting, pedantic Clippy with warnings denied, all workspace
  unit/integration/doc tests, application-shell ownership validation and its
  negative tests, feature inventory and its negative tests, shell syntax and
  ShellCheck, and qualification-matrix schema/coverage validation passed.
- The first sandboxed workspace run failed only because the sandbox denied
  real Unix-socket creation with `EPERM`. The identical complete workspace run
  passed at the established socket-capable permission boundary; the sandbox
  failure is not product evidence.

### Qualification and remaining boundaries

- Feature inventory is now 28 `IMPLEMENTED`, 20 `PARTIAL`, and 12
  `NOT_IMPLEMENTED`; `application.navigation-history-recents` is implemented.
- Authoritative matrix declarations remain 188 `PASS`, 3 `XFAIL`, and 4
  `NOT_IMPLEMENTED`. This feature changed no matrix status and makes no full
  Linux qualification claim.
- Recent Actions remain deliberately session-scoped, matching the source.
  Hardware mouse Back/Forward qualification and unrelated GH-16 visual,
  animation, and comprehensive accessibility work remain outside GH-79.
