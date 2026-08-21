# Linux key-repeat and modifier-order dogfood record — 2026-08-21

Issue: GH-72, child of GH-8. Plan:
`docs/design/linux-key-repeat-qualification-plan.md`.

## Scope and authority

This slice qualifies server/compositor-driven repeat, physical modifier
ordering, focus transfer while a key remains down, and repeat termination on
key-up. It uses the staged ReleaseSafe product, real Ghostty surfaces and PTYs,
the existing private Xvfb/Cage environments, and the public external Zentty
CLI. No application component is replaced by a fake.

The X11 server was configured to a 200 ms delay and 20 Hz rate. Cage advertised
a 600 ms delay and 25 Hz rate through `wl_keyboard.repeat_info`. Those captured
values, rather than desktop preferences, define each run's repeat-count bound.

## Discoveries, failures, and repairs

- The shared product-input helper accepts complete `xdotool key` chords; using
  `keydown` or `keyup` as though they were chord names is ambiguous and does not
  preserve a held physical key. The repeat journey now owns small XTEST
  key-down/key-up helpers while retaining the shared focus preparation.
- An early harness revision delegated focus transitions to child-side marker
  watchers. Its product log showed a transition before the intended barrier,
  so that evidence was rejected. The final journey makes every transition from
  the orchestrator through the documented public CLI.
- An instance-discovery credential is intentionally read-only for pane
  mutations. Calling `pane focus --pane-id` directly from an external process
  failed closed with `AuthorizationFailure`. The repaired journey first calls
  `zentty select pane --shell --include-control-token`, then uses the selected
  pane's capability for `pane focus`, exactly as the public CLI contract
  documents.
- The raw Wayland driver now exposes only four reviewed actions: tap, shifted
  tap, a hold bounded to 250–2000 ms, and Enter. It still accepts only reviewed
  maps and the single physical keycode. It cannot inject arbitrary text.
- Adding Shift to the reviewed Wayland XKB payload changed the GH-71 evidence
  hashes. The complete Wayland runtime-layout journey was rerun and its dogfood
  record now contains the current hashes.

No Ghostty defect or Ghostty patch was required.

## Focused real-system receipts

X11 passed in private Xvfb session
`843f295decbd9aabc736462ed6866b80f033acafc90dc07ad5e6cd4f7615014f`:

```text
rust-keyboard-repeat-x11: PASS repeat=10+14/24 rate=20 delay=200 modifier-order focus-transfer key-up-stop real-pty
```

Wayland passed in private Cage session
`bd658c1ad4f110c742a4e77ab009f955c4bc1d18954c36d408f15c721f54c8bb`
over private Xvfb transport session
`864ff828399d53bad03a85ccc8331a7db97fddcfd420989bbad0a1b1aa489ef8`:

```text
rust-keyboard-repeat-wayland: PASS repeat=1+13/18 rate=25 delay=600 modifier-order focus-transfer key-up-stop real-pty
```

In both runs pane 1 received uppercase `Y`, then lowercase `y`, proving ordered
modifier release. The held physical key produced non-empty receipts on both
sides of the public-CLI focus transfer. Combined counts stayed within bounds
derived from the captured repeat configuration, the receipt had exactly four
lines, and its hash remained stable after key-up.

## Qualification scope

The authoritative matrix adds explicit `keyboard-repeat-x11` and
`keyboard-repeat-wayland` PASS cells. Driver unit tests, harness contract tests,
ShellCheck, Clippy, matrix validation, matrix-runner tests, and feature-inventory
tests pass. This is focused GH-72 evidence, not a claim that the entire Linux
matrix or GH-8 epic passes.

## AI disclosure

Investigation and implementation assistance were provided by OpenAI Codex
under Jason Maskell's direction.
