# Zentty Linux dogfood — shell integration

## 2026-08-15 — GH-45 baseline

The source already contains substantial Bash, Zsh, Fish, and Nushell
integration and retained Swift coverage. Linux stages those exact resources and
injects their discovery environment through the existing pane runtime. The
current qualification is nevertheless incomplete: Bash and Zsh have only a
standalone staged-resource process test, while Fish and Nushell are declared
BLOCKED. The standalone actor uses a fake CLI and no real Zentty/Ghostty pane,
so none of those cells alone meets GH-45's end-to-end standard.

The existing real `rust-cli-contract` actor proves the staged CLI, private
socket, pane capability, integration directory, and a real Bash child, but it
does not launch each supported shell or prove their hooks and user-file safety.
The repair will extend that actor rather than accrete another product harness.

Local prerequisites discovered before implementation:

- Bash 5.2.21 at `/usr/bin/bash`;
- Zsh 5.9 at `/usr/bin/zsh`;
- Fish 4.8.1 at `/home/jason/Projects/.tools/fish-4.8.1/bin/fish`;
- Nushell 0.114.1 at `/home/jason/Projects/.tools/nu-0.114.1/nu`.

Ubuntu 24.04's repository offers Fish 3.7 and no Nushell package, so blindly
installing distribution packages would test unsupported or absent versions.
The portable tools were already placed outside the repository. Their presence
is local evidence only; the public qualification contract must use explicit
prerequisites and may not silently pass when they are absent.

## First real-pane run: the test driver failed before the shell

The first controlled X11 Bash journey found a harness defect before it could
exercise Bash. The driver combined util-linux `script --log-out FILE` with the
legacy positional output-file argument `/dev/null`; this host's `script`
correctly rejected the extra argument as `unexpected number of arguments`.
Because the ready marker preceded `exec`, the input helper briefly found a
window that was already withdrawing and reported X11 `BadWindow` on the first
attempt. The scenario now waits for the shell-specific prompt in the real PTY
transcript rather than treating driver startup as shell readiness, prints the
transcript when a child dies early, and uses exactly one output-file form.
This was a test-driver failure, not product evidence, and is recorded as FAIL
until the repaired journey passes.

## Product discovery: the staged shell hooks had no Linux receiver

All four source integrations invoke `zentty ipc agent-signal`, suppressing any
failure so a terminal remains usable outside Zentty. The Rust CLI implemented
`agent-event` but not `agent-signal`; therefore every Bash/Zsh/Fish/Nushell
prompt, activity, root-PID, and CWD report had silently no-op'd on Linux. This
was a real product omission, not a test gap. The repair adds one authenticated
product IPC subcommand, reuses the existing private instance socket and pane
capability, rejects malformed/duplicate/unknown signal fields, and remains
silent when the complete live-pane environment is absent. It does not add a
socket, daemon, event store, shell implementation, or Rust-specific Ghostty
binding. Full agent-session bookkeeping remains owned by GH-46.

The parser was initially added to the already broad product CLI module. Before
qualification it was moved into the focused `shell_signal` module so this
feature did not reverse the completed application-shell decomposition. A real
CLI-process/Unix-socket test proves canonical server-side targeting despite
forged environment topology, and a separate process test proves non-invasive
success outside a pane. As elsewhere in this repository, the restricted tool
sandbox returned `EPERM` for every real Unix-socket case; the exact suite then
passed outside that sandbox (8 passed, 0 failed). The sandbox result is not
product evidence.

## Harness discoveries and repairs

- Hashing the entire temporary home incorrectly attributed Ghostty's generated
  config and the desktop session's `.dbus` cache to shell integration. The
  reviewed boundary now hashes every ordinary home file while excluding only
  application-owned Ghostty/Zentty config, the desktop-owned D-Bus cache, and
  Nushell's ordinary command-history file. User Bash, Zsh, Fish, and Nushell
  config files remain included byte-for-byte. Ubuntu's global Zsh startup also
  creates `.zcompdump` without Zentty, so the journey establishes that native
  baseline before hashing and still detects any subsequent change.
- The first X11 Unicode path was sometimes truncated by synthesized keyboard
  input. The path is now supplied through the real pane environment and the
  physically typed command expands it. This still drives the real PTY and
  proves spaces plus Unicode at the CWD/IPC/CLI boundaries without pretending
  XTEST is a reliable Unicode input method. The first Wayland command also
  lost repeated letters through virtual-keyboard delivery; a short environment
  name removes that irrelevant transport hazard while all command keystrokes
  remain compositor-visible physical input.
- Bash and Zsh intentionally retain the XDG discovery entry so a user can move
  into Fish or Nushell and let that shell consume the integration. Fish and
  Nushell remove the entry once their vendor autoload has run. Treating the
  Bash/Zsh value as a leak contradicted the source design; the journey now
  asserts the two distinct contracts rather than weakening both.
- Nested integration markers are deliberately shell-specific. Bash exports
  its idempotence marker and Nushell retains an environment marker after
  consuming XDG discovery. Zsh's global marker and Fish's global variable are
  not exported, and their consumed/direct discovery mechanisms prevent
  recursive loading. The first uniform `loaded=1` assertion was wrong; exact
  per-shell nested receipts now pin the source behavior.
- The first version-gate test found that the inherited Nushell regular
  expression accepted 0.99 even though the diagnostic promised 0.100+. One
  shared prerequisite/version authority now rejects 0.99 and accepts 0.100+.
  Its focused test covers explicit-path precedence, PATH discovery, the
  documented controlled-host `.tools` layout, absent tools, all four minimum
  versions, and an old Fish executable that must return 77 without launching.

## Controlled real-product evidence

After the repairs, Bash 5.2.21, Zsh 5.9, Fish 4.8.1, and Nushell 0.114.1 each
passed in a separate real staged Zentty/Ghostty pane under private Xvfb/X11 and
private Cage/Wayland. Every journey used a real PTY and physical compositor
input and proved prompt readiness, hostile Unicode/space CWD, authenticated
pane identity and socket capability, staged CLI discovery and management
command passthrough, opted-in tmux shim ordering, prompt/activity/CWD/root-PID
signals, nested-shell semantics, unchanged user config, clean child exit, and
socket removal. The PASS receipt includes the exact shell version plus byte
count and SHA-256 identities for startup, environment, and nested-shell
receipts; raw temporary files are intentionally removed after assertions.

The authoritative matrix adds eight named product cells (four shells by two
display systems), one prerequisite-policy cell, and promotes the existing Fish
and Nushell staged-process cells from BLOCKED to PASS. Declared totals are now
152 PASS, 0 FAIL, 5 BLOCKED, 1 XFAIL, and 15 NOT_IMPLEMENTED (173 cells).
These are declaration totals, not a claim that the full aggregate has passed.

## Mutation-run resource incident and containment

The first parallel parser mutation attempts made GNOME intermittently
unresponsive, and a later two-worker run terminated the complete WezTerm
application. The initial hypothesis that ordinary parallel GTK/Rust builds
overwhelmed the workstation was wrong. Kernel evidence identified the exact
failure at 2026-08-16 06:05:37: mutant test process `zentty_linux-62` reached
about 93.4 GiB anonymous RSS, exhausted all 8 GiB swap, and was selected by the
global OOM killer. The active mutation had changed the positional parser's
`index += 1` into `index *= 1`, so the loop never advanced while continually
appending cloned arguments. The 90-second wall timeout could not prevent it
from exhausting memory first.

The mutant inherited WezTerm's GNOME application scope. That scope has
`OOMPolicy=stop` and `KillMode=control-group`, so systemd stopped the remaining
GUI and mux-server processes when their descendant was OOM-killed. Two normal
compiler workers were not the cause; the missing fault-containment boundary
was. Interrupted scratch directories were removed explicitly.

An exploratory cargo-reapi 0.1.1 adapter was withdrawn before commit at the
operator's direction. The attempted performance comparison ran on this host's
ext4 project filesystem rather than cargo-reapi's qualified reflink-enabled
XFS Linux profile, so its timings are not valid evidence for or against that
product. Zentty contains no cargo-reapi integration or policy; that work is
deferred rather than smuggled into the shell-integration feature.

`mutate-rust` now refuses to execute without resource isolation. On a systemd
desktop it re-enters itself in a dedicated user scope with 8 GiB `MemoryHigh`,
12 GiB `MemoryMax`, 1 GiB `MemorySwapMax`, `OOMPolicy=continue`, and reduced CPU
and I/O weights. It also applies a 6 GiB per-process virtual-address ceiling;
hosts without a user systemd manager must at least provide `prlimit` for that
same ceiling. Automated tests pin every cgroup property, re-entry argument,
and inherited process limit.

The exact `index *= 1` mutant was then rerun alone through the repaired real
runner. Its test aborted at the 6 GiB ceiling with `memory allocation of 4
bytes failed`, cargo-mutants correctly classified it as caught in 56 seconds,
the kernel recorded no OOM event, WezTerm remained alive, and no scratch tree
survived. This is the required containment receipt, not a waiver for mutation
coverage still to be completed.

The completed focused parser mutation pass tested 35 mutants in two minutes:
31 were caught and four were unviable. One surviving guard mutation exposed a
missing exact diagnostic assertion for an unknown `pane-root-pid` event with
an extra positional argument; the contract test was tightened and caught the
mutant. The separate truecolor-default helper pass caught all three mutants.
No surviving viable mutation was waived.

## Final journey and orchestration review

The first controlled Wayland Bash rerun revealed that a long compositor-typed
absolute probe path could receive an encoded key-release sequence (`6;5u`) in
the middle of the command. That was a Cage virtual-keyboard transport defect,
not shell or product behavior. The journey now types the short command
`"$P" shell` and lets the real shell expand `P` to the same hostile absolute
path. Physical compositor input, the real PTY, hostile argv boundaries, and
the absolute-path execution boundary all remain exercised. Bash then passed
on Wayland, and all eight shell/compositor cells passed in their final form.

The shell journey initially created a second inline fake `codex` executable.
The repository's orchestration contract correctly rejected that accretion.
The journey now routes the boundary probe through the single reviewed
`linux/tests/fixtures/controlled-agent` using its focused `shell-boundary`
profile. The actor's own fast contract pins hostile arguments and pane/worklane
identity, while the real product journey still traverses the installed Codex
wrapper. `test-orchestration-contract` passes with one actor and no new launch,
PTY, socket, or agent-test layer.

Final real-product shell receipts:

- Bash: X11 startup receipt passed; Wayland startup SHA-256
  `8b794b...`, environment `a07358...`, nested `aa9254...`.
- Zsh: X11 receipt `1a2c7c...`; Wayland `8eadef...`.
- Fish: X11 receipt `c42108...`; Wayland `6b91c0...`.
- Nushell: X11 receipt `0f1678...`; Wayland `87ef2a...`.

These abbreviated identities are the concise human report emitted during the
controlled run; the runner validates and emits the full hashes in its receipt.
Generic pane persistence remains owned by the existing canonical session-
restore journey rather than being reimplemented eight times. Each shell cell
does prove its own real process launch, prompt, state transitions, nested shell,
clean exit, and product teardown.

`cargo test --workspace --all-targets --locked` passed after the final product
repair, including 255 `zentty-linux` tests and every CLI/socket contract. The
workspace-wide strict Clippy run remains blocked by unrelated existing
current-toolchain pedantic findings, led by a 118-line function in
`crates/zentty-core/tests/workspace_state.rs` plus pre-existing long GTK
composition functions. It also found one new 102-line product CLI dispatcher;
that feature-owned issue was repaired by returning directly from the
`shell-signal` arm. The baseline findings are recorded rather than silently
fixed or misreported as feature regressions.

The first commit-ready aggregate ran 151 declared-PASS cells successfully and
failed the existing Wayland bookmark import/export cell. Its real GTK save
chooser created `Portable.zenttypreset.zenttypreset`: the harness had typed the
full default filename into a save-name field whose GTK selection covers only
the basename, retaining the existing suffix. This was neither converted to a
pass nor blamed on the desktop. The harness now accepts the exact product-
provided default filename as a real user would, while continuing to exercise
the real chooser, physical acceptance, file write, deletion, physical import,
and portable persisted envelope. Focused controlled Wayland and X11 reruns
both passed. The aggregate failure receipt remains in the dogfood history; a
new full aggregate is required before commit.

The repaired full aggregate then passed every presently executable support and
matrix cell in 646.820 seconds. Its machine receipt is
`build/linux/qualification-summary.json`; declared totals are **152 PASS, 0
FAIL, 5 BLOCKED, 1 XFAIL, and 15 NOT_IMPLEMENTED**. The implemented local suite
passed and suppression governance was accepted. Release qualification and full
Linux qualification did **not** pass because the explicitly declared BLOCKED,
XFAIL, and NOT_IMPLEMENTED cells remain. This result is therefore not described
as exhaustive QA or an unsuppressed-clean Valgrind result.
