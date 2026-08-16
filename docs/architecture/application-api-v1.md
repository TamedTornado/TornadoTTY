# Zentty application API v1 (extraction contract)

Status: **implemented for authenticated in-pane clients and owner-local XDG
runtime discovery**. The machine authority is
[`zentty-application-api-v1.json`](zentty-application-api-v1.json). The JSON
schemas describe what the current producer writes; they do not turn the
  private socket path or pane capability into an unprotected discovery
  mechanism.

## Ownership

```text
GTK actions ─┐
             ├── application command service ── existing product owners
API request ─┘
      ^
      └── authenticated Unix transport
```

The API owns operation, result, and error meaning. The application command
service owns dispatch. The Unix transport owns framing, authentication,
timeouts, and socket lifetime. The CLI owns source-compatible argument parsing,
response rendering, and exit status. None of those adapters owns workspace or
terminal state.

The transport passes the service a closed `ApplicationAuthority` and
`ApplicationTarget` from `zentty-api`. The target is canonical authenticated
context, not a serialized client claim. The v1 request's bounded string vector
is a language-neutral normalized parameter sequence: the CLI validates source
syntax and constructs it, while each operation owner performs the one semantic
interpretation. The transport never branches on those parameters.

## Versions and compatibility

- Application API version: `1`.
- Current Unix envelope version: `1`.
- Application requests state `applicationApiVersion`. Responses state that
  version and advertise the currently callable external operation IDs in
  `capabilities`. `shell-signal` is deliberately absent because it is an
  internal shell-integration route, not a public automation capability.
- Readers must reject an unsupported major version before dispatch.
- Within v1, producers may add optional response fields and new error codes in
  an existing category. Readers must ignore unknown optional fields.
- During extraction, the server accepts a legacy in-pane v1 request that omits
  `applicationApiVersion`, and the Rust client accepts a legacy response that
  omits negotiation fields. Current producers always emit them, and the
  published producer schemas require them.
- Removing or changing a field, operation identity, category, authentication
  rule, or existing result meaning requires a new major version.
- An operation not implemented by a running instance is an explicit
  `unsupported_operation` result; it is never silently treated as success.

The request and response producer schemas are in
[`schemas/zentty-application-request-v1.schema.json`](schemas/zentty-application-request-v1.schema.json)
and
[`schemas/zentty-application-response-v1.schema.json`](schemas/zentty-application-response-v1.schema.json).
The request schema expresses per-argument and count ceilings. The additional
128 KiB aggregate-argument ceiling is enforced by the API constructor because
JSON Schema cannot express the aggregate UTF-8 byte count directly. A
serialized structured result is limited to 256 KiB and an error message to 4
KiB. The five closed result kinds are `empty`, `discovery`, `selection`,
`topology`, and `theme`; product owners never choose terminal tables, shell
exports, or pretty-printed JSON. The CLI renders those presentations from the
structured result and the original parsed command.

## Authentication and target identity

The socket and its runtime directory are owner-private. Every request carries
a 256-bit pane capability in the JSON payload. The token must not be placed in
argv, filenames, logs, diagnostics, or normal output. The server authenticates
that capability and derives the canonical window/worklane/pane target from its
registry. Claimed target environment fields are diagnostic compatibility data,
not routing authority.

Using a valid capability against another instance fails authentication. A
selector that disagrees with the capability fails authorization. A target
removed after authentication returns `stale_target`.

For owner-local automation, each live product publishes non-secret
`instance.json` metadata and a separate `automation.token` beneath its private
0700 instance directory in `$XDG_RUNTIME_DIR/zentty`. Both artifacts and the
socket are 0600. Discovery rejects symlinks, loose modes, incompatible schema
or API versions, a mismatched socket path, and stale PID/start-time identity.
The credential has instance authority only for `discover` operations: it
cannot submit agent events, compatibility requests, server mutations, or pane
mutations. An automation client explicitly selects a pane and requests shell
exports to transition to that pane's narrower capability. For source
compatibility, the opt-in `--include-control-token` spelling and
`controlToken` result field are retained, but the value is now an opaque
`@file:/...` reference to a separate 0600 credential file under the
instance's 0700 `pane-credentials` directory. It is never the capability
itself. The delivered CLI resolves and validates that reference immediately
before transport; external clients must do the same. Each pane credential
file has an independently random non-secret name, is removed when the pane is
unregistered, and the entire directory is removed at instance shutdown.

With one valid instance, the CLI discovers it automatically. With multiple
instances it fails closed and requires `ZENTTY_INSTANCE_ID`; it never chooses
one by recency. There is no `/tmp` scanning fallback when `XDG_RUNTIME_DIR` is
absent.

## Threat model

The protected assets are pane capabilities, the instance discovery
capability, canonical topology identity, terminal input/state, and sensitive
payloads. The local adversary model includes another process with the same
login's observable argv/process metadata, stale clients after restart, forged
selectors, symlink or permission substitution in the runtime tree, partial or
oversized frames, slow clients, and a valid capability replayed against the
wrong instance. The API does not claim isolation from a process already able
to read the owner's 0600 files or ptrace the owner under the host's security
policy.

Mitigations are owner-private runtime artifacts, independently random scoped
capabilities, canonical server-side target derivation, PID plus `/proc` start
identity, exact socket-path validation, bounded framing and waits, explicit
version/capability negotiation, typed stale/retry failures, credential-file
indirection for opt-in pane selection, and revocation on pane/instance
teardown. Tests exercise each of these boundaries through the real Unix
transport and the staged product; environmental absence is never a pass.

## Lifecycle

1. The product creates one owner-private runtime directory and Unix listener.
2. A terminal receives its socket path, instance identity, canonical target
   metadata, and pane capability in its launch environment.
3. A client sends one bounded JSON request, shuts down its write half, and
   receives one bounded JSON response.
4. Socket replacement or application restart invalidates the old instance and
   all of its capabilities. Clients must rediscover rather than replay them.
5. Shutdown stops acceptance, bounds in-flight work, removes the socket and
   private runtime directory, and never transfers product ownership to a
   client.

## Error handling

Clients branch on `error.category`, not prose. `error.code` preserves a more
specific stable product reason. The ten closed categories are in the machine
inventory. In particular, retry is only appropriate for
`retryable_instance_replacement`; `stale_target` requires target discovery,
and `authorization_failure` must not be retried with the same capability.

## Non-Rust example

[`../../examples/application-api-v1.sh`](../../examples/application-api-v1.sh)
uses only Bash, jq, socat, the published schema, and the authenticated pane
environment. It performs the read-only `discover/panes` operation. jq reads the
token from its environment instead of an argv option, and socat is launched
with the token removed from its environment. Its integration test runs the
script as a separate process against the real Unix listener and server
authentication path.

```sh
./examples/application-api-v1.sh
```

This example is usable inside a Zentty pane. The delivered CLI additionally
supports owner-local discovery; the example remains in-pane so it demonstrates
the wire contract without duplicating discovery and credential validation.
