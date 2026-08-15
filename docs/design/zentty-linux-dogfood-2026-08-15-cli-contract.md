# Zentty Linux dogfood: CLI compatibility contract

Date: 2026-08-15
Owner: GH-42

## Trigger

GH-22 was decomposed after its implemented foundations and remaining parity
scope became impossible to report honestly as one feature. GH-42 is the first
child because previous port work repeatedly demonstrated that implementing
from memory misses source behavior. The operator required this to be one
complete issue slice rather than another sequence of tiny parser changes.

## Starting state

The original Swift product already has an ArgumentParser command tree. Linux
has a handwritten Rust parser, an authenticated bounded product protocol, and
substantial staged-product coverage inside the tmux/product journey. What it
does not yet have is a detailed versioned compatibility contract connecting
each source leaf to its Linux parser, output fields, errors, schemas, goldens,
and executable receipts.

The authoritative matrix at the start of GH-42 is `PASS 138`, `FAIL 0`,
`BLOCKED 7`, `XFAIL 1`, and `NOT_IMPLEMENTED 16` (162 total). Implemented local
and product-boundary qualification pass; release and full Linux qualification
do not.

## Initial discoveries

- The broad feature audit owns every Swift CLI command symbol, but intentionally
  maps them to feature families. It is not detailed enough to serve as a
  syntax/output compatibility contract.
- The Rust parser has one authority and the application has one product
  dispatch authority; GH-42 does not need a new router.
- JSON discovery fields already match the source Codable structures closely,
  but there is no committed schema or explicit output version policy.
- The Rust option validator accepts duplicate flags/value options and returns
  the first value. That makes conflicting explicit selectors ambiguous rather
  than fail-closed.
- Shell export rendering uses the correct POSIX embedded-single-quote pattern,
  but only ordinary values are presently exercised by the product journey.
- The staged product journey already invokes many topology families, but it
  does not prove every alias, text/JSON contract, hostile selector/export
  value, or incompatible output-version request.

Further failures, repairs, mutation results, and qualification receipts will
be appended as the issue is executed.

## Source closure and contract construction

- The source audit contains 40 non-root CLI symbols. The new contract maps all
  40 exactly once to 40 command records; the validator rejects missing,
  duplicated, stale, or newly unaccounted source symbols.
- The command records cover 13 output contracts, six JSON schemas, reviewed
  text goldens, option/default metadata, stable errors and exit behavior.
- Source `ipc` and managed `launch` are only partial on Linux. They remain
  visible and are owned by GH-46 and GH-47 rather than being mislabeled as
  complete or omitted.
- Linux adds `--output-version` to discovery and selection. Version 1 is
  accepted and preserved over the product protocol; every other value fails
  before IPC. This is a Linux extension needed to make external scripting
  compatibility enforceable.
- Source `theme auto` follows macOS appearance. The Linux platform alternative
  follows the freedesktop/GTK color scheme and is documented as intentional.

## Failures and repairs

1. **Duplicate explicit options were accepted.** The Rust parsers returned the
   first value for duplicate selector/value options and treated repeated flags
   as harmless. That could turn contradictory scripting input into an ambient
   action. Shared product parsing and server parsing now reject duplicates;
   focused tests cover `--json`, selector, PID, and browser duplicates.
2. **Topology output had drifted from the source.** Linux overview JSON was a
   flat object containing independent windows/worklanes/panes arrays, and text
   output was a count-only summary. Source output is nested and its individual
   list commands have reviewed column vocabulary. The single existing render
   authority now emits the source shapes; no second compatibility formatter
   was introduced.
3. **Optional JSON values were emitted as `null`.** Swift `encodeIfPresent`
   omits them. Linux discovery now omits absent title, focused pane, working
   directory, agent, and control-token keys. Schemas explicitly distinguish
   omission from a required field.
4. **Pane-index selection could cross worklanes.** A source pane index is
   scoped to the caller worklane unless a worklane is explicit. Linux now uses
   that default and rejects zero or multiple matches instead of taking the
   first ambient match.
5. **The first topology schema used a relative `$ref` beneath an HTTPS `$id`.**
   The local `jsonschema` executable correctly resolved it as a network URL,
   making an offline receipt fail. The topology schema is now self-contained;
   qualification does not depend on network schema retrieval.
6. **A split-created pane reran the test actor.** Every new real pane inherits
   the product `--command`; the first harness let those children repeat the
   whole CLI journey, race on shared receipts, and produce invalid capability
   tokens. The actor now executes assertions only in `pane-1`, while child
   panes remain real shells. This is a harness repair, not a product bypass.
7. **The shell-export comparison initially used unauthenticated discovery.**
   That response intentionally omits `controlToken`, so the expected value was
   invalid. The journey now independently obtains authenticated discovery,
   sources the emitted shell program in an empty environment, checks the
   exported endpoint is a live Unix socket, and compares all topology IDs and
   the token byte-for-byte.
8. **A fixed 30-second actor sleep made a short journey slow.** The child now
   waits for a bounded release marker from the outer verifier. Controlled X11
   and Wayland journeys complete in seconds without racing product shutdown.
9. **The first mutation campaign exposed 15 surviving branches.** Missing
   cases included no-argument server success, discovery dispatch arms,
   cross-window topology nesting, multiple-window formatting, truncation,
   HOME abbreviation, agent summaries, and the non-shell selection branch.
   Focused tests were added for the behavior rather than excluding mutants.
10. **The first full rerun exposed one unrelated transient Wayland bookmark
    failure.** `bookmarks-presets-import-export-wayland` could not observe the
    bookmark-name dialog during a heavily parallel run. Its product log showed
    the physical shortcut and create-menu focus preceding the timeout. The
    exact matrix command immediately passed in a fresh controlled Wayland
    session with the real export chooser, file, physical delete, import chooser
    and persisted portable data. This is recorded as a failed qualification
    attempt, not converted into a pass; a clean complete matrix rerun is still
    required before commit.
11. **Reducing matrix concurrency from four to three moved, rather than fixed,
    the unrelated timing failures.** The bookmark cell passed, but the legacy
    staged X11 tmux journey timed out after a child assertion at `send-keys`,
    and the X11 fleet journey failed to observe exactly two X11 window
    identities after its product log recorded both windows. Neither path was
    modified by GH-42. The failed receipt remains in the qualification summary;
    the final audit is being rerun serially so a heavily loaded host cannot
    consume physical-input timing budgets. This is evidence that these older
    journeys need a separate deterministic synchronization review, not grounds
    for retries inside a cell or for treating failure as success.

## Real-system and mutation receipts

- Controlled X11: `rust-cli-contract-x11: PASS staged-cli=true
  real-product=true authenticated-socket=true aliases=true schemas=4
  text-goldens=true hostile-shell-unit=true fail-closed=true`.
- Controlled Wayland: `rust-cli-contract-wayland: PASS staged-cli=true
  real-product=true authenticated-socket=true aliases=true schemas=4
  text-goldens=true hostile-shell-unit=true fail-closed=true`.
- The journeys use the actual staged `zentty` subprocess, actual
  `zentty-linux`, real Ghostty surfaces and PTYs, private authenticated Unix
  socket, and controlled nested compositors. Environmental absence is not a
  pass.
- Final focused mutation receipt: 106 mutants tested; 94 caught, 12 unviable,
  zero missed. The campaign used `.cargo/mutants.toml` with `gitignore = true`
  and `copy_target = false`; scratch trees did not copy `build/linux-deps`.
- The source-contract runner reports `PASS commands=40 source-symbols=40
  output-contracts=13 schemas=6`. Its negative suite rejects a missing command,
  unknown status, absent executable evidence, stale source symbol, unknown
  option/output contract, duplicate path, and false completion.

## Remaining boundary

GH-42 establishes and qualifies the compatibility contract; it does not claim
that partial source agent-event or launcher cases are implemented. The broader
feature inventory therefore moves the CLI family from `NOT_IMPLEMENTED` to
`PARTIAL`, reducing that inventory total from 20 to 19. GH-46 and GH-47 must
finish those recorded cases, while GH-43 through GH-45 and GH-49 own deeper
control, layout, shell, and installed-product journeys. Full Linux
qualification remains false while any matrix cell is blocked, XFAIL, or not
implemented.

## Final qualification

The authoritative final run used `ZENTTY_QUALIFICATION_JOBS=1
linux/tests/qualify-local` after the two parallel timing failures described
above. It executed every support test and every presently executable matrix
cell without an unexpected skip or retry inside a cell:

- declared matrix totals: `PASS 140`, `FAIL 0`, `BLOCKED 7`, `XFAIL 1`,
  `NOT_IMPLEMENTED 16` (164 total);
- implemented local suite: passed;
- product-boundary qualification: passed;
- release qualification: not passed;
- full Linux qualification: not passed;
- wall time: 2,057,100 ms; the 350,280 ms upstream Ghostty regression was the
  floor and longest cell;
- Debug Valgrind: **PASS with reviewed suppressions**, not an unsuppressed clean
  result. Raw totals were 427 errors/contexts, 6,080 definite bytes and 41,363
  indirect bytes. Post-suppression totals were zero errors/contexts and zero
  definite/indirect bytes, with all 427 errors/contexts explicitly accounted
  for by the reviewed effective suppression set;
- ReleaseSafe Valgrind remains XFAIL and was not made green by broadening
  suppressions.

The matrix gained two explicit release cells,
`cli-source-compatibility-x11` and `cli-source-compatibility-wayland`; both use
the staged ReleaseSafe product under controlled compositors and passed. The
matrix still cannot be described as exhaustive or fully qualified while its
seven blocked, one XFAIL and 16 not-implemented cells remain.
