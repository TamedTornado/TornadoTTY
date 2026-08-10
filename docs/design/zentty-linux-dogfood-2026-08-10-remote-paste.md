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
