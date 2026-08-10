# Zentty Linux remote paste dogfood — 2026-08-10

Tracking: GH-17

## Source audit before implementation

- The feature is real and source-owned. Commits `afa145b` and `d89b531` added
  remote clipboard-image and arbitrary-file upload, and `14ee35a` later
  extracted the controller without changing the workflow.
- The source does not use `scp` despite an outdated controller comment. It
  launches `/usr/bin/ssh`, sends bytes on standard input, and runs remote
  `sh -c 'umask 077; cat > ...'`.
- The source automatically uploads remote-pane file/image paste and drop. There
  is progress/failure feedback but no separate approval prompt. The earlier
  conversational description of an approval step was an extrapolation and is
  not source parity; Linux must not silently add it.
- The source has focused fake-process, decision, path, request-resolution,
  clipboard, cancellation, and controller integration tests. It does not have
  the real SSH end-to-end evidence required by the Linux plan.
- Collision rejection, checksums, partial-upload cleanup, explicit local
  symlink handling, and completion-time SSH identity revalidation are not fully
  implemented by the source. They remain explicit Linux hardening work rather
  than being misrepresented as existing macOS behavior.
- The source destination model retains user, host, and port but not the
  connection options that may be essential to the already-running session.
  Reconnecting with only those fields can fail for sessions launched with
  `-F`, `-i`, `-J`, bind options, or selected `-o` settings. Linux now captures
  a bounded reusable connection-only subset from the process argv. Options
  capable of launching commands or requesting a remote session are discarded,
  and `BatchMode` is product-forced rather than inherited.
- OpenSSH reuses letters with different meanings between `ssh` and `scp`:
  notably SSH `-B`/`-b` are bind interface/address while SCP `-B` is batch mode
  and `-b` is a bandwidth limit; SSH `-p` becomes SCP `-P`. The pure transport
  policy translates bind settings through `-o`, maps the port explicitly, and
  preserves every value as a separate argv element. Passing the original SSH
  flags directly to SCP would have been a functional and safety bug.
- The first tests-first implementation checkpoint covers source size limits,
  deterministic source filename sanitization, separate partial/final paths,
  shell-safe insertion, strict fallback classification, reusable SSH option
  filtering, and exact SSH/SCP argument translation. Focused tests and strict
  Clippy pass. This is policy evidence only; no transfer or product claim is
  made yet.
- The initial mutation run found eight survivors. Four exposed missing boundary
  examples (`-a` must not leak into SCP, long stem budgeting, dotfiles, empty
  extensions); the remaining four showed that branches copied from the Swift
  filename splitter were observationally redundant after sanitization. Tests
  were expanded and the splitter simplified rather than preserving dead source
  structure. The final focused run completed 58 mutants in two minutes: 55
  caught, three unviable, zero missed, and zero timed out.

## Transactional transport and first real product journey

- The first local preparation test failed because its hand-entered expected
  SHA-256 was wrong (`0d6b…` versus the independently observed `fb11…`). The
  implementation was not changed to fit the fixture; the receipt was checked
  with the system hash tool and the fixture was repaired.
- A direct `cargo test -p zentty-linux --test remote_transfer` initially linked
  the stale default `build/linux-deps/ghostty/zig-out` library, which did not
  yet export `ghostty_gtk_embed_surface_foreground_process_id`. The maintained
  build flow already selects the pinned/current library through
  `GHOSTTY_LIB_DIR`; focused tests were rerun against `build/linux/lib`, and a
  subsequent `build-local` refreshed the default staged library. This was a
  stale native artifact, not a test skip or a product pass.
- The first real disposable-OpenSSH transport run was blocked inside the
  filesystem/network sandbox with `socket: Operation not permitted`. It was
  rerun elevated rather than converted into a fake process test. The real
  server proves key-authenticated SCP, atomic final-path collision rejection,
  byte-for-byte SHA-256 equality, and the hardened SSH-stream fallback when
  the server has no SFTP subsystem.
- SCP cannot safely reopen an attacker-substitutable original pathname after
  local validation. The implementation therefore opens the source with
  `O_NOFOLLOW`, hashes that retained regular-file descriptor, copies those
  bytes into a mode-0600 `create_new` staging file, and gives only that private
  staging path to SCP. The remote partial name is independently hardened from
  the source-compatible eight-hex visible name to a 128-bit nonce.
- Publication uses a remote hard link from the verified private partial path
  to the final path, so an existing final object is not overwritten. Both
  `sha256sum` and macOS/BSD-compatible `shasum -a 256` are supported. The
  remaining uncertainty is the usual same-UID remote-account adversary: POSIX
  shell commands cannot make the sequence of stat, hash, and link operations
  indivisible against another process running as the same remote user.
- The initial cancellation unit proof unexpectedly took ten seconds. Killing
  `sh` left its local `sleep` grandchild holding the captured stderr pipe, so
  the reader correctly waited for EOF. The test now uses `exec sleep`, making
  the real child itself the cancellation target and reducing the proof to the
  intended bounded interval. Product SSH/SCP processes do not use the test's
  extra local shell layer.
- The application now has one `RemotePaneContext`, not parallel SSH and upload
  registries. It owns the existing SSH probe source, exact foreground PID plus
  destination identity, and at most one cancellable upload per pane. Identity
  changes cancel the shared transport token; completion independently checks
  the live surface PID, current projected identity, and a fresh `/proc` SSH
  probe before inserting a path.
- The first controlled X11 product journey failed with no remote-paste log.
  Physical `Ctrl+Shift+V` delivered `GDK_KEY_V`, while the capture controller
  accepted only lowercase `GDK_KEY_v`. Supporting both physical translated
  key values repaired the real event path. The rerun passed through the real
  GTK `GdkFileList` clipboard, real Ghostty pane, real OpenSSH/SFTP transport,
  remote SHA-256 comparison, verified path insertion, Enter submission to the
  remote actor, SSH exit, and return to the original PTY.
- File drops use the same GTK frame controller and the same coordinator,
  transport, cancellation, and completion logic as file clipboard paste.
  Clipboard textures are encoded by GDK to PNG, bounded at 10 MiB, staged in a
  private local file, and then use that same path. Ordinary text remains
  Ghostty-owned because the capture controller proceeds unless the remote pane
  clipboard advertises `GdkFileList` or `GdkTexture`.
- Both controlled compositor journeys then passed a second real clipboard
  gesture carrying `image/png`. GDK decoded and re-encoded the texture, the
  product uploaded it through the same real server, and the actor asserted the
  remote PNG signature before the SSH session exited. This is real image
  clipboard evidence on X11 and Wayland; drag/drop remains explicitly
  unqualified rather than being inferred from sharing the controller.
- The architecture contract now describes `zentty-linux` truthfully as a
  shipped product package (binary plus the reusable Linux transport library),
  inventories the remote-paste coordinator and its single authority, and lets
  the existing non-shipped `zentty-test-support` actor call the production
  transport. No second transfer implementation or embedded scenario mode was
  introduced.
- The first Linux-transport mutation run left 34 survivors because ordinary
  Cargo discovery intentionally skips the real OpenSSH tests. The maintained
  mutation command now includes the two ignored, qualification-owned tests;
  those tests reuse `linux/tests/lib/disposable-sshd` rather than introducing a
  second SSH fixture. A deliberately failing SFTP subsystem proves that only a
  missing/unavailable subsystem permits stream fallback: an SFTP transfer
  failure remains a failure and its partial is removed.
- Intermediate mutation runs exposed boundary and classification gaps: an
  exact-size file must be accepted, only `ENOENT` while spawning `scp` means
  the local program is absent, and cancellation must remain observably
  distinct from timeout. Exact assertions now cover each branch. The final
  focused Linux transport run tested 50 mutants in about two minutes: 40
  caught, ten unviable, zero missed, and zero timed out.
- The final standalone transport receipt passed against real loopback OpenSSH:
  SCP publication, collision preservation, no-SFTP stream fallback, and
  no-local-SCP stream fallback. Strict formatting, Clippy, focused unit tests,
  matrix-runner self-tests, feature-inventory validation, and both architecture
  contracts also pass. An unelevated full matrix attempt failed only while its
  `prepare-ghostty` cell tried to resolve GitHub from the restricted network;
  this environmental absence is recorded as a failure and will be rerun with
  the required network permission, not converted into a pass.
- A post-build compositor rerun caught a serious evidence bug: the
  `nested-wayland-input` wrapper created and inventoried a real private Cage
  Wayland socket, but inherited `GDK_BACKEND=x11` from the Xvfb transport that
  hosts Cage. Consequently its child product could still use X11 while the
  wrapper described the environment as Wayland. The inner wrapper now forces
  `GDK_BACKEND=wayland` after Cage is ready. Its isolation self-test passes,
  and the consolidated product receipt now reports `rust-session-restore
  passed: wayland` while exercising the real file and PNG remote-paste paths.
  The earlier Wayland product-paste receipts are superseded; only this corrected
  receipt is accepted as Wayland evidence.
- Invoking the matrix runner directly then failed both agent cells because it
  intentionally received ambient Gemini 0.54.4, not the reviewed 0.53.0
  prerequisite selected by `qualify-local`. This reconfirmed that
  `qualify-local`, not its internal runner, is the authoritative operator
  entrypoint. The next authoritative run selected 0.53.0 and its Wayland agent
  cell passed.
- That authoritative run still failed X11 when GDK returned a transient
  `org.freedesktop.DBus.Error.AccessDenied: Invalid transfer` while converting
  the real `text/uri-list` clipboard into `GdkFileList`. A fresh isolated X11
  rerun passed unchanged, initially suggesting a clipboard-provider race. A
  one-time 75 ms file-list retry passed three isolated journeys but failed again
  in the authoritative combined cell; that attempted repair is therefore not
  accepted as sufficient evidence. The underlying issue is GDK's portal-backed
  conversion of an X11 `text/uri-list` into `GdkFileList`. Clipboard providers
  already advertising the standard URI-list MIME type now use GDK's raw
  `InputStream` path, bounded to 64 KiB, decoded as UTF-8, restricted to local
  file URIs, and never confused with ordinary terminal text. Native GTK file
  lists remain the fallback for providers without that MIME type. The exact
  five-journey X11 agent cell that had failed twice then passed end to end.
  The previous failed qualification summaries remain valid failed evidence
  until the full authoritative gate is rerun.
- Adding the asynchronous clipboard reader exposed an ownership-validator gap:
  its function inventory regex recognized only synchronous `fn` declarations.
  The validator now inventories optional `async fn` declarations too, the
  reviewed source hash and explicit function inventory were updated, and all
  positive and negative architecture contract tests pass.
- The final authoritative `qualify-local` rerun passed every presently
  executable support and matrix cell in 363,370 ms. The implemented local
  suite is therefore passed, with Valgrind described only as **PASS with
  reviewed suppressions**. Release and full Linux qualification remain
  explicitly not passed because the authoritative declarations still contain
  seven BLOCKED, one XFAIL, and 21 NOT_IMPLEMENTED cells. Declared totals are
  PASS=92, FAIL=0, BLOCKED=7, XFAIL=1, and NOT_IMPLEMENTED=21; no exhaustive-QA
  claim is made.
