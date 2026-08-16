#!/usr/bin/env bash
set -euo pipefail

# Minimal language-neutral client for the authenticated application API.
# It intentionally performs one read-only operation and obtains its capability
# from the calling pane environment; the token is never placed in argv.
for dependency in jq socat; do
    command -v "${dependency}" >/dev/null || {
        echo "zentty-api-example: ${dependency} is required" >&2
        exit 69
    }
done

socket="${ZENTTY_INSTANCE_SOCKET:?not running inside a Zentty instance}"
[[ "${ZENTTY_PANE_TOKEN:-}" =~ ^[0-9a-f]{64}$ ]] || {
    echo "zentty-api-example: ZENTTY_PANE_TOKEN is missing or malformed" >&2
    exit 77
}
request_id="shell-example-$$"

# jq reads the capability from its environment. Do not replace this with
# --arg, which would expose the token through the process argument vector.
response="$({
    jq -cn \
        --arg id "${request_id}" \
        '{
          version: 1,
          applicationApiVersion: 1,
          id: $id,
          kind: "discover",
          arguments: ["--json"],
          standardInput: null,
          environment: {ZENTTY_PANE_TOKEN: env.ZENTTY_PANE_TOKEN},
          expectsResponse: true,
          subcommand: "panes"
        }'
} | env -u ZENTTY_PANE_TOKEN socat -t 3 - "UNIX-CONNECT:${socket}")"

jq -e --arg id "${request_id}" '
  .version == 1 and .applicationApiVersion == 1 and .id == $id and
  (.capabilities | index("panes") != null) and
  ((.ok == true and .error == null and (.result.stdout | type == "string")) or
   (.ok == false and .result == null and
    (.error.category | type == "string") and
    (.error.code | type == "string") and
    (.error.message | type == "string")))
' <<<"${response}" >/dev/null || {
    echo "zentty-api-example: malformed or mismatched response" >&2
    exit 76
}

if [[ "$(jq -r '.ok' <<<"${response}")" != true ]]; then
    jq -r '"\(.error.category)/\(.error.code): \(.error.message)"' <<<"${response}" >&2
    exit 1
fi
jq -j '.result.stdout' <<<"${response}"
