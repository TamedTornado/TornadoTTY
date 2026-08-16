# Zentty Linux dogfood: application API beneath the CLI

Date: 2026-08-16
Issue: GH-48
Parent epic: GH-22

This record begins before production changes. GH-48 is corrective work: the
CLI implementation preceded a formally owned application API. The goal is not
to add commands. It is to make the delivered CLI a thin client of one
language-neutral, versioned command contract shared with GUI entry points and
delegated to the existing product owners.

## Frozen architecture rule

```text
GUI actions ─┐
             ├── application command API/service ── workspace/runtime owners
CLI client ──┘
                 ^
                 └── authenticated Unix transport + discovery
```

The API is the transport-neutral request/result/error contract and application
service. The existing Unix socket is a transport. CLI parsing and rendering
are client presentation. Neither may become an owner of workspace, pane,
settings, agent, task, persistence, or platform behavior.

## Initial ownership audit

### Existing useful boundaries

- `zentty-agent-ipc::ProductIpcRequest` and `ProductIpcReply` already provide
  bounded request/reply objects separate from GTK and Ghostty.
- `AgentIpcServer` authenticates a pane capability through
  `PaneTokenRegistry` and derives the canonical `AgentTarget`; client-supplied
  window/worklane/pane environment is not used as routing authority.
- `ApplicationCoordinator::handle_product_commands` is the application-level
  receiver for general product requests. It delegates window-wide operations
  to the coordinator and per-window operations to `ApplicationShell`.
- The CLI uses the same private socket as agent events, development-server
  commands, and tmux compatibility. No second transport or application actor
  exists.

These are assets to preserve, not proof that the architecture is complete.

### Confirmed contract problems

1. The apparent API types are named `ProductIpc*` and live inside the transport
   crate. Their operation identity is an open string pair (`kind`,
   `subcommand`) plus `Vec<String>`, so parsing and product meaning cross the
   transport boundary rather than being expressed as a closed API contract.
2. The route registry is duplicated across the CLI parser, the
   `ProductIpcRequest` allowlist, transport dispatch, coordinator dispatch, and
   `ApplicationShell` string matches. A new command can drift between those
   lists without one authoritative compiler-checked or machine-validated
   inventory.
3. Development-server commands use separate `ServerIpcRequest` and reply
   types even though they cross the same authenticated product boundary.
   Tmux compatibility has another request/reply family. The differences may be
   justified, but they have not been inventoried as deliberate API families.
4. The wire envelope is private and hard-codes version `1`. Product errors are
   collapsed through partially duplicated reply types, while transport errors
   surface as `AgentIpcError` prose. External clients cannot reliably classify
   all failure and retry cases without implementation knowledge.
5. Successful general-product and server replies are primarily opaque stdout
   strings. JSON is rendered inside product handlers based on CLI-shaped
   arguments rather than returned as typed API data and rendered by the CLI.
6. `run_product_cli` obtains authentication and target context directly from
   pane environment variables and may select a different pane token from CLI
   arguments. This is useful internal behavior but is not an external instance
   discovery or least-authority contract.
7. GUI actions call existing shell/coordinator methods directly, while the CLI
   enters through `execute_product_command`. Both reach common lower-level
   owners in many cases, but there is no executable proof that GUI and API
   paths share one operation service or remain semantically identical.
8. `zentty-agent-ipc` now owns general product control, development-server
   control, launch orchestration, integrations, and agent events. Its name and
   module boundary no longer describe all of its responsibilities. A split or
   rename must follow demonstrated ownership seams, not precede them.

### Current route families

- `ipc/agent-event`: one-way canonical agent lifecycle/status input.
- `tmux_compat/*`: source-compatible tmux request/reply semantics, including
  bounded stdin and wait behavior.
- `server/*`: development-server registry and browser operations.
- `discover/*`: overview, window, worklane, pane, and pane-selection queries.
- `pane/*`: pane/worklane mutation, grids/layout, theme, notification, and
  shell lifecycle signals.
- CLI-local operations: version, color enumeration, integration installation,
  integration removal, agent launch/bootstrap, and server watch process
  supervision.

The next artifact must classify every concrete command and route into API,
transport-only, CLI-local, or compatibility-only ownership. Nothing may
disappear from that inventory merely because it is hidden or internal.

## Test-first order

1. Create a machine-readable route and ownership inventory from the existing
   behavior, plus negative runner tests for missing, duplicate, unknown, or
   side-door routes.
2. Characterize the current wire and CLI output before moving types.
3. Introduce closed, versioned application request/result/error types without
   changing product semantics.
4. Make the CLI construct API operations and render API results; make the
   transport encode/decode them without interpreting product arguments.
5. Route GUI and external requests through one application service or prove a
   documented exception for event/compatibility families.
6. Only then add public discovery, recovery, schemas, and an external non-Rust
   client.

## Qualification policy

- Existing product, CLI, tmux, agent, server, session-restore, and staged
  journeys are the integration layer; no new parallel application harness is
  permitted.
- Mutation testing targets closed routing, authentication, version/length
  gates, canonical target derivation, error classification, and CLI exit
  mapping after those decisions exist.
- Environmental absence remains BLOCKED. Valgrind passes involving
  suppressions remain described as **PASS with reviewed suppressions**.

## Slice 1: closed operation authority and crate boundary

- Added `zentty-api`, a GTK-, Unix-, CLI-, Ghostty-, and runtime-handle-free
  crate. The existing bounded general-product request/reply contract moved out
  of `zentty-agent-ipc`; Linux product handlers now depend directly on the API
  crate instead of obtaining application types through the transport crate.
- Replaced the duplicated request allowlist with the closed
  `ApplicationOperation` registry. Each of the 19 current general application
  operations owns one scope and wire name. `ApplicationRequest` stores that
  enum and derives its scope/name, so an invalid scope/name pair cannot survive
  request construction.
- Kept transitional `ProductIpc*` type aliases so this extraction does not
  combine an architecture boundary change with a broad cosmetic rename. They
  do not create another implementation; the aliases resolve to the API types.
- Added `docs/architecture/zentty-application-api-v1.json`. It accounts for all
  19 general operations, all 40 CLI contract entries, the three specialized
  wire families, four CLI-local responsibilities, product owners, result
  classes, external exposure, and known GUI entry points.
- Added one focused inventory runner and negative suite, not another product
  harness. It rejects missing/duplicate operations, unknown scopes, incomplete
  or invented CLI coverage, nonexistent GUI entry points, and false external
  classification. The compiled Rust registry independently loads the same
  machine inventory and requires exact operation identity equality.
- Initial receipts: API unit tests 3/3 PASS; existing CLI parser tests 11/11
  PASS; real Unix product transport tests 9/9 PASS; Linux product type-check
  PASS; inventory and all negative runner cases PASS. The reconciled workspace
  dependency graph, architecture contract, architecture negative self-tests,
  `ApplicationShell` ownership contract, mutation resource-isolation policy,
  strict Clippy, formatting, ShellCheck, and diff hygiene all PASS.

## Slice 2: CLI consumes an application request

- The delivered `zentty` binary previously parsed a valid
  `ApplicationRequest`, decomposed it back into the old transport tuple, and
  asked `AgentIpcClient::send_product` to validate and reconstruct it. That
  made the CLI appear to use the API while still coupling it to socket route
  fields.
- Added `AgentIpcClient::send_application`, whose client boundary is the
  already-validated `ApplicationRequest`. Both ordinary CLI commands and the
  internal shell-signal path now call it. The Unix transport alone projects
  the operation into the legacy wire envelope.
- Retained `send_product` temporarily as an explicitly documented
  source-compatibility adapter for existing internal tests/callers. It creates
  one `ApplicationRequest` and delegates immediately; it owns no behavior.
  Removing this adapter is tracked within GH-48 after all callers migrate.
- Extended the inventory validator to reject either loss of the API client
  call or reintroduction of the old tuple-shaped call in the delivered CLI.
- Real Unix-socket tests initially failed 8/9 inside the filesystem sandbox
  because creating their Unix listener returned `EPERM`. This was an
  environment restriction, not treated as a pass. The same focused suite was
  rerun with the required permission and passed 9/9, including authentication,
  canonical targets, concurrent clients/instances, partial frames, CLI shell
  signals, and bounded replies. CLI parser tests passed 11/11. Focused strict
  Clippy, inventory negative tests, ShellCheck, formatting, and diff hygiene
  pass. An exploratory `--all-targets` Clippy run exposed the pre-existing
  `similar_names` warning in `tests/launch_cli.rs`; the shipped library/binary
  targets remain clean and this slice does not suppress that warning.
