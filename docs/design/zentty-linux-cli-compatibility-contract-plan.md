# Zentty Linux CLI compatibility contract plan

Date: 2026-08-15
Owner: GH-42
Parent: GH-22

## Outcome

Close GH-42 as one issue-sized slice. The original Swift CLI is the source;
the Rust CLI is an implementation to verify, not evidence of its own parity.
The result is a versioned machine contract plus real staged-product receipts,
not a prose command list or a second routing system.

## Authority boundaries

- The existing `zentty-agent-ipc` parser owns CLI syntax and bounded wire
  requests.
- The existing application/product CLI coordinators own selector resolution,
  authentication, topology, UI mutation, and response rendering.
- The existing integration, server, agent-event, and tmux authorities retain
  their command families. GH-42 inventories and invokes them; it does not
  reimplement their behavior.
- `docs/design/zentty-cli-source-contract.json` is the only detailed command
  inventory. The older feature inventory remains the feature-family ledger.
- JSON schemas describe current v1 output. Opaque IDs are strings and clients
  must not derive meaning from their spelling.

## Construction order

1. Record every original leaf command, alias, positional, selector, option,
   default, value domain, output mode/field, error class, and exit code.
2. Add closed-world validation against the existing exhaustive Swift command
   ledger and reject missing status, owner, implementation, schema, golden, or
   executable receipt links.
3. Repair parser discrepancies found by that inventory: duplicated options,
   incompatible output-version requests, ambiguous selectors, hostile shell
   quoting, and stable error/exit behavior.
4. Validate JSON against committed schemas and text against reviewed goldens.
5. Drive the staged CLI as real subprocesses against a staged running Zentty
   process over its private authenticated socket. Exercise aliases, hostile
   values, missing/duplicate/hidden/stale targets, and parser failures.
6. Mutation-test parser, dispatch, selector, quoting, error mapping, and output
   version seams with the permanent cargo-mutants disk-safety configuration.
7. Run every presently executable matrix cell, update dogfood and GH-42 with
   exact receipts, and close only if every acceptance criterion is satisfied.

## Required failure behavior

- Unknown commands/options and malformed or duplicated values exit nonzero.
- Explicit selectors never fall back to the caller when missing, stale, or
  ambiguous.
- A positional pane reference cannot be combined with an explicit pane ID or
  index; directional focus remains source-compatible.
- Shell exports use single-quote-safe POSIX syntax and preserve Unicode,
  newlines, dollar signs, command substitutions, and embedded quotes exactly.
- Unsupported output contract versions fail before IPC rather than silently
  returning the current shape.
- Environmental absence, an unavailable source-only family, or an untested
  entry remains explicit; none becomes PASS through documentation alone.

## Test architecture

- Focused Rust tests cover pure parsing and protocol bounds.
- Inventory runner self-tests mutate copied JSON fixtures and must cover
  missing commands/options/status/tests, unknown status/schema, stale source
  symbols, duplicate aliases, untracked output fields, and false completion.
- JSON Schema validation and golden comparison operate on receipts produced by
  the actual staged CLI.
- The real-product journey uses the existing staged application, Ghostty,
  PTY, private socket, physical controlled display, and existing product
  authorities. No fake topology, terminal, or application is introduced.
- Mutation testing complements rather than replaces the staged journey.

## Completion claims

Closing GH-42 means its command-compatibility contract passes. It does not
close GH-22 and does not imply release or full Linux qualification while any
matrix cell remains BLOCKED, XFAIL, or NOT_IMPLEMENTED.
