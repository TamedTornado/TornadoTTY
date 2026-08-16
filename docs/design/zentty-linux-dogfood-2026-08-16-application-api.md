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

## Slice 3: closed service dispatch and first shared GUI operation

- Renamed the per-window `product_cli` module to `application_commands` and
  renamed its entry point to `execute_application_request`. This is the
  application service for pane-owned operations; it is not CLI presentation
  or Unix transport code. The coordinator remains the owner of discovery,
  cross-window grid, and notification operations.
- Replaced the application-shell and coordinator route decisions based on
  command-name strings with `ApplicationOperation` matches. Human-readable
  wire names remain only for logging, rendering, compatibility parsing, and
  legacy envelope projection.
- Found that GUI directional focus and external directional focus duplicated
  the state transition plus render/scroll/native-focus sequence. Added the
  typed `PaneFocusDirection` service operation and routed both GTK actions and
  authenticated API requests through it. Directionless API focus retains its
  original select-and-present behavior.
- Extended the architecture validator to require closed operation dispatch and
  both GUI/external focus paths through the same service operation, and to
  reject restoration of open string dispatch.
- Focused Rust service tests passed 9/9; CLI parser tests passed 11/11; real
  socket transport tests passed 9/9; strict shipped-target Clippy, formatting,
  inventory negative tests, ShellCheck, and diff hygiene pass. A fresh
  ReleaseSafe staged product was built. The full delivered-CLI journey then
  passed against the real GTK/Ghostty product and private authenticated Unix
  socket in both controlled X11 and controlled Wayland, including concurrent
  instances, aliases, four schemas, text goldens, hostile shell input, and
  fail-closed cases. The first sandboxed X11 attempt was rejected because Xvfb
  could not create its private display socket; it was not counted as a pass and
  was rerun in the approved controlled environment.

## Slice 4: machine-readable application failure categories

- `ApplicationReplyError` previously exposed a stable short code but still
  required clients to maintain an undocumented code-to-meaning table. Added a
  serialized `ApplicationErrorCategory` with ten closed categories covering
  invalid arguments, unsupported operation/version, authorization failure,
  stale target/instance, retryable instance replacement, product
  unavailability/rejection, and permanent transport failure. Human prose can
  now change without changing client control flow.
- Preserved existing granular error codes for source compatibility. Reply
  construction assigns their stable category centrally in `zentty-api`; it
  does not duplicate classification in the CLI or transport.
- Added the exact category set to the machine inventory and a negative runner
  case for an unknown category. API tests exercise every category, including
  the legacy product-rejection fallback. Results: API tests 4/4 PASS, real
  Unix-socket transport tests 9/9 PASS, strict API/transport Clippy PASS, and
  inventory/schema negative tests, ShellCheck, formatting, and diff hygiene
  PASS.

## Slice 5: executable language-neutral contract and client

- The Rust reply had a machine category, but the Unix envelope still emitted
  only code and prose. Added the category to every produced error response.
  It is additive for v1 readers. The Rust client accepts an older response
  without the optional field, but rejects a producer whose category
  contradicts the stable code-derived category.
- Added draft-2020-12 request and response producer schemas. The executable
  schema runner requires exact agreement with the operation and category
  inventories, validates three positive fixtures, and rejects wrong-scope
  operations, a missing token, unknown/missing categories, and contradictory
  success/error state. The aggregate request byte ceiling remains a documented
  runtime invariant because JSON Schema cannot sum UTF-8 byte lengths.
- Added an architecture note covering ownership, version compatibility,
  ceilings, authentication, canonical targets, lifecycle, error behavior, and
  the explicit limitation that public instance discovery is not implemented.
  It does not pretend that scanning process environments or publishing pane
  tokens is an acceptable discovery design.
- Added a Bash/jq/socat client that depends only on the documented wire
  contract. It reads the pane token from environment rather than argv and
  removes it from socat's environment. Its real integration test runs the
  script as a separate process against the actual Unix listener and
  authentication registry.
- The first client run exposed a byte-accuracy bug: `jq -r` appended a newline
  to stdout that already ended in one. The integration assertion failed rather
  than normalizing it away. Replacing it with `jq -j` preserved the API result
  exactly. Corrected results: real socket tests 10/10 PASS, API tests 4/4 PASS,
  schema runner 3 positive/5 negative PASS, strict shipped-target Clippy,
  ShellCheck, inventory negatives, formatting, and diff hygiene PASS.

## Slice 6: typed transport failures

- `AgentIpcError` previously reduced every server-side envelope rejection to a
  prose-only `Rejected(String)`. Added explicit authorization, unsupported
  version, and categorized remote variants plus a public `category()` method.
  Local missing/refused sockets classify as stale instances;
  reset/aborted/broken connections classify as retryable instance replacement;
  other I/O and worker failures remain permanent transport failures.
- Authentication failures now originate as authorization errors at the token
  boundary, and envelope-version failures originate as unsupported-version
  errors. The server writes those categories without parsing its own prose.
  The application API client preserves the category in a typed remote error.
- Added real-socket cases for a missing token, unsupported version, forged
  capability before dispatch, and a removed socket path. The forged-token test
  initially demonstrated that the application client still used the legacy
  prose-only branch even though the server emitted the right category. The
  test failed (`product_rejection` versus `authorization_failure`); the client
  branch was repaired rather than weakening the assertion.
- Corrected results: real transport tests 11/11 PASS and strict shipped-target
  Clippy, formatting, and diff hygiene PASS. No retry loop was added; the API
  now supplies the classification a future bounded client policy can use.

## Slice 7: explicit API version and capability negotiation

- The envelope version governed framing but did not state which application
  contract or operations the peer implemented. Current application requests
  now carry `applicationApiVersion: 1`; every response carries that version and
  the 18 externally callable operation IDs derived from the closed registry.
  Internal `shell-signal` is intentionally not advertised.
- The server rejects an explicitly incompatible application version before
  product dispatch. The application client rejects a mismatched response
  version or a response that does not advertise its requested public
  operation. Transition compatibility remains bounded: legacy v1 in-pane
  requests/responses may omit the additive negotiation fields, but current
  producer schemas require them and current producers always write them.
- Updated the executable schemas and non-Rust client. The schema runner now
  proves that the advertised capability enum exactly equals the inventory's
  externally callable operations. Added a real-socket incompatible application
  version case alongside the existing incompatible envelope version case.
- Results: real transport tests 11/11 PASS, schema positives 3/3 and negatives
  5/5 PASS, strict shipped-target Clippy, ShellCheck, formatting, and diff
  hygiene PASS.

## Slice 8: owner-local discovery with scoped automation authority

- Designed discovery before wiring it into the CLI. Publishing a pane token
  directly would also authorize fabricated agent events and compatibility
  traffic, so `PaneTokenRegistry` now records `Pane` versus `Instance`
  authority. Instance credentials authenticate only application discovery;
  event, tmux, server, and pane-mutation routes reject them.
- Added split owner-private artifacts beneath the existing random instance
  directory: `instance.json` contains only schema/API version, instance ID,
  PID plus `/proc` start ticks, and the exact socket path;
  `automation.token` contains the credential. The directory remains 0700 and
  descriptor, credential, and socket are 0600. The credential's `Debug`
  representation is always redacted.
- Discovery validates real files/directories rather than following symlinks,
  exact modes, the expected socket path and type, API/schema versions, and
  PID/start-time identity. Invalid or stale candidates do not become usable
  endpoints. There is deliberately no insecure `/tmp` scan fallback.
- `AgentRuntime` publishes only after a real pane exists, anchors the instance
  capability to that pane, retargets it if the anchor closes, and withdraws
  discovery when the last pane disappears. The CLI preserves complete in-pane
  environments, refuses partially present endpoint variables, discovers a
  sole XDG instance, and fails closed on multiple instances unless
  `ZENTTY_INSTANCE_ID` selects one exactly.
- Added real tests for separated/redacted artifacts, hostile permissions and
  symlinks, scoped capability rejection, the delivered CLI as an outside-pane
  process, and no credential leakage to stderr. The first controlled X11 run
  failed because the journey still expected the legacy prose-only rejection
  text. The implementation correctly returned the new typed authorization
  boundary (`application capability rejected: pane token is invalid`); the
  assertion was tightened to that exact contract rather than weakening the
  implementation.
- Corrected results: core capability tests 4/4 PASS, discovery tests 2/2 PASS,
  real transport tests 12/12 PASS, Linux integration compile PASS, API schema
  positives 3/3 and negatives 5/5 PASS, and controlled real-product CLI
  journeys on X11 and Wayland PASS. Strict Clippy over the changed libraries,
  binaries, and tests, ShellCheck, formatting, architecture and inventory
  negative self-tests, and diff hygiene PASS. A broader all-tests Clippy run
  also exposed a pre-existing `similar_names` warning in `launch_cli.rs`; it
  is outside this discovery change, while the changed test targets are clean.
