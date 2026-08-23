#!/usr/bin/env zsh
set -euo pipefail
unsetopt BG_NICE

script_dir="${0:A:h}"
repo_root="${script_dir:h}"
tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/zentty-virtual-display-test.XXXXXX")"
trap 'rm -rf "$tmp_dir"' EXIT

bin_dir="$tmp_dir/bin"
mkdir -p "$bin_dir"
zdot_dir="$tmp_dir/zdot"
mkdir -p "$zdot_dir"
: > "$zdot_dir/.zshenv"

fake_betterdisplay="$tmp_dir/fake-betterdisplay"
fake_curl="$bin_dir/curl"
fake_screen_probe="$tmp_dir/screen-exists"
fake_main_display_probe="$tmp_dir/main-display"
fake_raw_main_display_probe="$tmp_dir/raw-main-display"
fake_resolve_display_probe="$tmp_dir/resolve-display"
fake_topology_probe="$tmp_dir/topology-status"
fake_registered_screen_probe="$tmp_dir/registered-screen"
xcodebuild_log="$tmp_dir/xcodebuild.log"
betterdisplay_log="$tmp_dir/betterdisplay.log"
curl_log="$tmp_dir/curl.log"
topology_log="$tmp_dir/topology.log"
display_state="$tmp_dir/display-created"
display_connected_state="$tmp_dir/display-connected"
main_restored_state="$tmp_dir/main-restored"
display_reconnected_state="$tmp_dir/display-reconnected"
: > "$topology_log"

legacy_identifiers='{"name":"ZenttyTests+%282%29","originalName":"ZenttyTests+%282%29","tagID":"608","displayID":"0"}'
legacy_registration_plan="$(
  CLANG_MODULE_CACHE_PATH="$tmp_dir/clang-module-cache" \
    SWIFT_MODULECACHE_PATH="$tmp_dir/swift-module-cache" \
    ZENTTY_REGISTERED_SCREEN_IDENTIFIERS="$legacy_identifiers" \
    /usr/bin/swift "$repo_root/scripts/virtual-display-state.swift" registered-screen-plan ZenttyTests
)"
if [[ "$legacy_registration_plan" != "legacy:608:0" ]]; then
  print -u2 "expected the malformed BetterDisplay name to be recognized as the registered ZenttyTests screen"
  exit 1
fi

duplicate_identifiers='{"name":"ZenttyTests","tagID":"606","displayID":"0"},{"name":"ZenttyTests (2)","tagID":"608","displayID":"0"}'
duplicate_registration_plan="$(CLANG_MODULE_CACHE_PATH="$tmp_dir/clang-module-cache" \
    SWIFT_MODULECACHE_PATH="$tmp_dir/swift-module-cache" \
    ZENTTY_REGISTERED_SCREEN_IDENTIFIERS="$duplicate_identifiers" \
    /usr/bin/swift "$repo_root/scripts/virtual-display-state.swift" registered-screen-plan ZenttyTests)"
if [[ "$duplicate_registration_plan" != $'canonical:606:0\nlegacy:608:0' ]]; then
  print -u2 "expected canonical and legacy BetterDisplay registrations to be distinguished by tagID"
  exit 1
fi

cat > "$fake_betterdisplay" <<'EOF'
#!/usr/bin/env zsh
set -euo pipefail

print -r -- "$*" >> "$ZENTTY_FAKE_BETTERDISPLAY_LOG"

case "${1:-}" in
  create)
    sleep 0.2
    touch "$ZENTTY_FAKE_DISPLAY_STATE"
    ;;
  set)
    [[ -f "$ZENTTY_FAKE_DISPLAY_STATE" ]]
    if [[ "$*" == *"-connected=on"* ]]; then
      [[ "${ZENTTY_FAKE_CONNECT_FAILURE:-}" != "1" ]]
      touch "$ZENTTY_FAKE_DISPLAY_CONNECTED_STATE"
    elif [[ "$*" == *"-connected=off"* ]]; then
      if [[ "${ZENTTY_FAKE_DISCONNECT_FAILURE:-}" != "1" ]]; then
        rm -f "$ZENTTY_FAKE_DISPLAY_CONNECTED_STATE"
        if [[ "${ZENTTY_FAKE_RECONNECT_ONCE:-}" == "1" && ! -f "$ZENTTY_FAKE_DISPLAY_RECONNECTED_STATE" ]]; then
          touch "$ZENTTY_FAKE_DISPLAY_RECONNECTED_STATE" "$ZENTTY_FAKE_DISPLAY_CONNECTED_STATE"
        fi
      fi
    elif [[ "$*" == *"-UUID=TEST-MAIN-UUID -main=on"* ]]; then
      touch "$ZENTTY_FAKE_MAIN_RESTORED_STATE"
    fi
    ;;
  discard)
    if [[ "$*" != *"-tagID=608"* ]]; then
      rm -f "$ZENTTY_FAKE_DISPLAY_STATE" "$ZENTTY_FAKE_DISPLAY_CONNECTED_STATE"
    fi
    exit 0
    ;;
esac
EOF
chmod +x "$fake_betterdisplay"
ln -s "$fake_betterdisplay" "$bin_dir/betterdisplaycli"

cat > "$fake_curl" <<'EOF'
#!/usr/bin/env zsh
set -euo pipefail

print -r -- "$*" >> "$ZENTTY_FAKE_CURL_LOG"

case "$*" in
  *"/help"*)
    exit 0
    ;;
  *"/create"*)
    if [[ "${ZENTTY_FAKE_CURL_CREATE_STATUS:-}" == "404" ]]; then
      print -u2 "curl: (22) The requested URL returned error: 404"
      exit 22
    fi
    touch "$ZENTTY_FAKE_DISPLAY_STATE"
    exit 0
    ;;
  *"/get"*)
    [[ -f "$ZENTTY_FAKE_DISPLAY_STATE" ]]
    if [[ -f "$ZENTTY_FAKE_DISPLAY_CONNECTED_STATE" ]]; then
      print -r -- "on"
    else
      print -r -- "off"
    fi
    exit 0
    ;;
  *"/set"*)
    [[ -f "$ZENTTY_FAKE_DISPLAY_STATE" ]]
    if [[ "$*" == *"connected=on"* ]]; then
      [[ "${ZENTTY_FAKE_CONNECT_FAILURE:-}" != "1" ]]
      touch "$ZENTTY_FAKE_DISPLAY_CONNECTED_STATE"
    elif [[ "$*" == *"connected=off"* ]]; then
      rm -f "$ZENTTY_FAKE_DISPLAY_CONNECTED_STATE"
    fi
    exit 0
    ;;
  *"/discard"*)
    rm -f "$ZENTTY_FAKE_DISPLAY_STATE" "$ZENTTY_FAKE_DISPLAY_CONNECTED_STATE"
    exit 0
    ;;
esac

exit 1
EOF
chmod +x "$fake_curl"

cat > "$fake_screen_probe" <<'EOF'
#!/usr/bin/env zsh
set -euo pipefail
[[ -f "$ZENTTY_FAKE_DISPLAY_CONNECTED_STATE" ]]
print -r -- "${ZENTTY_FAKE_ACTIVE_SCREEN_NAME:-ZenttyTests}"
EOF
chmod +x "$fake_screen_probe"

cat > "$fake_main_display_probe" <<'EOF'
#!/usr/bin/env zsh
set -euo pipefail
print -r -- "42"
EOF
chmod +x "$fake_main_display_probe"

cat > "$fake_raw_main_display_probe" <<'EOF'
#!/usr/bin/env zsh
set -euo pipefail
if [[ "${ZENTTY_FAKE_RAW_MAIN_CHANGED:-}" == "1" && ! -f "$ZENTTY_FAKE_MAIN_RESTORED_STATE" ]]; then
  print -r -- "9"
else
  print -r -- "42"
fi
EOF
chmod +x "$fake_raw_main_display_probe"

cat > "$fake_resolve_display_probe" <<'EOF'
#!/usr/bin/env zsh
set -euo pipefail
if [[ "${ZENTTY_FAKE_DISPLAY_ID_CHANGES:-}" == "1" && -f "$ZENTTY_FAKE_DISPLAY_CONNECTED_STATE" ]]; then
  print -r -- "84"
else
  print -r -- "42"
fi
EOF
chmod +x "$fake_resolve_display_probe"

cat > "$fake_topology_probe" <<'EOF'
#!/usr/bin/env zsh
set -euo pipefail
print -r -- "$*" >> "$ZENTTY_FAKE_TOPOLOGY_LOG"
if [[ "${ZENTTY_FAKE_MIRRORED:-}" == "1" ]]; then
  print -r -- "mirrored"
  exit 0
fi
if [[ "${ZENTTY_FAKE_MAIN_CHANGED:-}" == "1" && ! -f "$ZENTTY_FAKE_MAIN_RESTORED_STATE" ]]; then
  print -r -- "main-changed"
  exit 0
fi
print -r -- "ok"
EOF
chmod +x "$fake_topology_probe"

cat > "$fake_registered_screen_probe" <<'EOF'
#!/usr/bin/env zsh
set -euo pipefail
[[ -f "$ZENTTY_FAKE_DISPLAY_STATE" ]] || exit 1
registration_plan="${ZENTTY_FAKE_REGISTERED_SCREEN_PLAN:-canonical:606:0}"
if [[ -f "$ZENTTY_FAKE_DISPLAY_CONNECTED_STATE" ]]; then
  registration_plan="${registration_plan/canonical:606:0/canonical:606:9}"
fi
print -r -- "$registration_plan"
EOF
chmod +x "$fake_registered_screen_probe"

cat > "$bin_dir/xcodebuild" <<'EOF'
#!/usr/bin/env zsh
set -euo pipefail
print -r -- "$*" >> "$ZENTTY_FAKE_XCODEBUILD_LOG"
[[ -f "$ZENTTY_FAKE_DISPLAY_CONNECTED_STATE" ]]
sleep 0.2
[[ -f "$ZENTTY_FAKE_DISPLAY_CONNECTED_STATE" ]]
EOF
chmod +x "$bin_dir/xcodebuild"

run_harness() {
  PATH="$bin_dir:$PATH" \
    ZDOTDIR="$zdot_dir" \
    TMPDIR="$tmp_dir" \
    ZENTTY_TEST_DISPLAY_STATE_DIR="$tmp_dir" \
    ZENTTY_TEST_DISPLAY_PROVIDER=betterdisplay \
    ZENTTY_TEST_SCREEN_NAME="${2:-ZenttyTests}" \
    ZENTTY_BETTERDISPLAY_COMMAND="$fake_betterdisplay" \
    ZENTTY_TEST_SCREEN_EXISTS_COMMAND="$fake_screen_probe" \
    ZENTTY_TEST_MAIN_DISPLAY_COMMAND="$fake_main_display_probe" \
    ZENTTY_TEST_RAW_MAIN_DISPLAY_COMMAND="$fake_raw_main_display_probe" \
    ZENTTY_TEST_RESOLVE_DISPLAY_ID_COMMAND="$fake_resolve_display_probe" \
    ZENTTY_TEST_TOPOLOGY_STATUS_COMMAND="$fake_topology_probe" \
    ZENTTY_TEST_REGISTERED_SCREEN_COMMAND="$fake_registered_screen_probe" \
    ZENTTY_FAKE_BETTERDISPLAY_LOG="$betterdisplay_log" \
    ZENTTY_FAKE_TOPOLOGY_LOG="$topology_log" \
    ZENTTY_FAKE_DISPLAY_STATE="$display_state" \
    ZENTTY_FAKE_DISPLAY_CONNECTED_STATE="$display_connected_state" \
    ZENTTY_FAKE_MAIN_RESTORED_STATE="$main_restored_state" \
    ZENTTY_FAKE_XCODEBUILD_LOG="$xcodebuild_log" \
    "$repo_root/scripts/test-on-virtual-display" -only-testing:ZenttyLogicTests \
    > "$tmp_dir/harness.$1.out" 2> "$tmp_dir/harness.$1.err"
}

run_http_harness() {
  PATH="$bin_dir:$PATH" \
    ZDOTDIR="$zdot_dir" \
    TMPDIR="$tmp_dir" \
    ZENTTY_TEST_DISPLAY_STATE_DIR="$tmp_dir" \
    ZENTTY_TEST_DISPLAY_PROVIDER=betterdisplay \
    ZENTTY_TEST_SCREEN_NAME="ZenttyTests (2)" \
    ZENTTY_BETTERDISPLAY_HTTP_BASE="http://example.test" \
    ZENTTY_TEST_SCREEN_EXISTS_COMMAND="$fake_screen_probe" \
    ZENTTY_TEST_MAIN_DISPLAY_COMMAND="$fake_main_display_probe" \
    ZENTTY_TEST_RAW_MAIN_DISPLAY_COMMAND="$fake_raw_main_display_probe" \
    ZENTTY_TEST_RESOLVE_DISPLAY_ID_COMMAND="$fake_resolve_display_probe" \
    ZENTTY_TEST_TOPOLOGY_STATUS_COMMAND="$fake_topology_probe" \
    ZENTTY_TEST_REGISTERED_SCREEN_COMMAND="$fake_registered_screen_probe" \
    ZENTTY_FAKE_CURL_LOG="$curl_log" \
    ZENTTY_FAKE_TOPOLOGY_LOG="$topology_log" \
    ZENTTY_FAKE_DISPLAY_STATE="$display_state" \
    ZENTTY_FAKE_DISPLAY_CONNECTED_STATE="$display_connected_state" \
    ZENTTY_FAKE_MAIN_RESTORED_STATE="$main_restored_state" \
    ZENTTY_FAKE_XCODEBUILD_LOG="$xcodebuild_log" \
    "$repo_root/scripts/test-on-virtual-display" -only-testing:ZenttyLogicTests \
    > "$tmp_dir/harness.http.out" 2> "$tmp_dir/harness.http.err"
}

run_main_change_harness() {
  PATH="$bin_dir:$PATH" \
    ZDOTDIR="$zdot_dir" \
    TMPDIR="$tmp_dir" \
    ZENTTY_TEST_DISPLAY_STATE_DIR="$tmp_dir" \
    ZENTTY_TEST_DISPLAY_PROVIDER=betterdisplay \
    ZENTTY_BETTERDISPLAY_COMMAND="$fake_betterdisplay" \
    ZENTTY_TEST_SCREEN_EXISTS_COMMAND="$fake_screen_probe" \
    ZENTTY_TEST_MAIN_DISPLAY_COMMAND="$fake_main_display_probe" \
    ZENTTY_TEST_RAW_MAIN_DISPLAY_COMMAND="$fake_raw_main_display_probe" \
    ZENTTY_TEST_RESOLVE_DISPLAY_ID_COMMAND="$fake_resolve_display_probe" \
    ZENTTY_TEST_TOPOLOGY_STATUS_COMMAND="$fake_topology_probe" \
    ZENTTY_TEST_REGISTERED_SCREEN_COMMAND="$fake_registered_screen_probe" \
    ZENTTY_FAKE_MAIN_CHANGED=1 \
    ZENTTY_FAKE_DISPLAY_ID_CHANGES=1 \
    ZENTTY_FAKE_RECONNECT_ONCE=1 \
    ZENTTY_FAKE_BETTERDISPLAY_LOG="$betterdisplay_log" \
    ZENTTY_FAKE_TOPOLOGY_LOG="$topology_log" \
    ZENTTY_FAKE_DISPLAY_STATE="$display_state" \
    ZENTTY_FAKE_DISPLAY_CONNECTED_STATE="$display_connected_state" \
    ZENTTY_FAKE_MAIN_RESTORED_STATE="$main_restored_state" \
    ZENTTY_FAKE_DISPLAY_RECONNECTED_STATE="$display_reconnected_state" \
    ZENTTY_FAKE_XCODEBUILD_LOG="$xcodebuild_log" \
    "$repo_root/scripts/test-on-virtual-display" -only-testing:ZenttyLogicTests \
    > "$tmp_dir/harness.main-change.out" 2> "$tmp_dir/harness.main-change.err"
}

run_mirrored_harness() {
  PATH="$bin_dir:$PATH" \
    ZDOTDIR="$zdot_dir" \
    TMPDIR="$tmp_dir" \
    ZENTTY_TEST_DISPLAY_STATE_DIR="$tmp_dir" \
    ZENTTY_TEST_DISPLAY_PROVIDER=betterdisplay \
    ZENTTY_BETTERDISPLAY_COMMAND="$fake_betterdisplay" \
    ZENTTY_TEST_SCREEN_EXISTS_COMMAND="$fake_screen_probe" \
    ZENTTY_TEST_MAIN_DISPLAY_COMMAND="$fake_main_display_probe" \
    ZENTTY_TEST_RAW_MAIN_DISPLAY_COMMAND="$fake_raw_main_display_probe" \
    ZENTTY_TEST_RESOLVE_DISPLAY_ID_COMMAND="$fake_resolve_display_probe" \
    ZENTTY_TEST_TOPOLOGY_STATUS_COMMAND="$fake_topology_probe" \
    ZENTTY_TEST_REGISTERED_SCREEN_COMMAND="$fake_registered_screen_probe" \
    ZENTTY_FAKE_MIRRORED=1 \
    ZENTTY_FAKE_BETTERDISPLAY_LOG="$betterdisplay_log" \
    ZENTTY_FAKE_TOPOLOGY_LOG="$topology_log" \
    ZENTTY_FAKE_DISPLAY_STATE="$display_state" \
    ZENTTY_FAKE_DISPLAY_CONNECTED_STATE="$display_connected_state" \
    ZENTTY_FAKE_MAIN_RESTORED_STATE="$main_restored_state" \
    ZENTTY_FAKE_XCODEBUILD_LOG="$xcodebuild_log" \
    "$repo_root/scripts/test-on-virtual-display" -only-testing:ZenttyLogicTests \
    > "$tmp_dir/harness.mirrored.out" 2> "$tmp_dir/harness.mirrored.err"
}

run_failed_reconnect_harness() {
  PATH="$bin_dir:$PATH" \
    ZDOTDIR="$zdot_dir" \
    TMPDIR="$tmp_dir" \
    ZENTTY_TEST_DISPLAY_STATE_DIR="$tmp_dir" \
    ZENTTY_TEST_DISPLAY_PROVIDER=betterdisplay \
    ZENTTY_BETTERDISPLAY_HTTP_BASE="http://example.test" \
    ZENTTY_TEST_SCREEN_EXISTS_COMMAND="$fake_screen_probe" \
    ZENTTY_TEST_MAIN_DISPLAY_COMMAND="$fake_main_display_probe" \
    ZENTTY_TEST_RAW_MAIN_DISPLAY_COMMAND="$fake_raw_main_display_probe" \
    ZENTTY_TEST_RESOLVE_DISPLAY_ID_COMMAND="$fake_resolve_display_probe" \
    ZENTTY_TEST_TOPOLOGY_STATUS_COMMAND="$fake_topology_probe" \
    ZENTTY_TEST_REGISTERED_SCREEN_COMMAND="$fake_registered_screen_probe" \
    ZENTTY_FAKE_CONNECT_FAILURE=1 \
    ZENTTY_FAKE_CURL_LOG="$curl_log" \
    ZENTTY_FAKE_TOPOLOGY_LOG="$topology_log" \
    ZENTTY_FAKE_DISPLAY_STATE="$display_state" \
    ZENTTY_FAKE_DISPLAY_CONNECTED_STATE="$display_connected_state" \
    ZENTTY_FAKE_MAIN_RESTORED_STATE="$main_restored_state" \
    ZENTTY_FAKE_XCODEBUILD_LOG="$xcodebuild_log" \
    "$repo_root/scripts/test-on-virtual-display" -only-testing:ZenttyLogicTests \
    > "$tmp_dir/harness.failed-reconnect.out" 2> "$tmp_dir/harness.failed-reconnect.err"
}

run_legacy_cleanup_harness() {
  PATH="$bin_dir:$PATH" \
    ZDOTDIR="$zdot_dir" \
    TMPDIR="$tmp_dir" \
    ZENTTY_TEST_DISPLAY_STATE_DIR="$tmp_dir" \
    ZENTTY_TEST_DISPLAY_PROVIDER=betterdisplay \
    ZENTTY_BETTERDISPLAY_COMMAND="$fake_betterdisplay" \
    ZENTTY_TEST_SCREEN_EXISTS_COMMAND="$fake_screen_probe" \
    ZENTTY_TEST_MAIN_DISPLAY_COMMAND="$fake_main_display_probe" \
    ZENTTY_TEST_RAW_MAIN_DISPLAY_COMMAND="$fake_raw_main_display_probe" \
    ZENTTY_TEST_RESOLVE_DISPLAY_ID_COMMAND="$fake_resolve_display_probe" \
    ZENTTY_TEST_TOPOLOGY_STATUS_COMMAND="$fake_topology_probe" \
    ZENTTY_TEST_REGISTERED_SCREEN_COMMAND="$fake_registered_screen_probe" \
    ZENTTY_FAKE_REGISTERED_SCREEN_PLAN=$'canonical:606:0\nlegacy:608:0' \
    ZENTTY_FAKE_BETTERDISPLAY_LOG="$betterdisplay_log" \
    ZENTTY_FAKE_TOPOLOGY_LOG="$topology_log" \
    ZENTTY_FAKE_DISPLAY_STATE="$display_state" \
    ZENTTY_FAKE_DISPLAY_CONNECTED_STATE="$display_connected_state" \
    ZENTTY_FAKE_MAIN_RESTORED_STATE="$main_restored_state" \
    ZENTTY_FAKE_XCODEBUILD_LOG="$xcodebuild_log" \
    "$repo_root/scripts/test-on-virtual-display" -only-testing:ZenttyLogicTests \
    > "$tmp_dir/harness.legacy-cleanup.out" 2> "$tmp_dir/harness.legacy-cleanup.err"
}

run_legacy_active_harness() {
  PATH="$bin_dir:$PATH" \
    ZDOTDIR="$zdot_dir" \
    TMPDIR="$tmp_dir" \
    ZENTTY_TEST_DISPLAY_STATE_DIR="$tmp_dir" \
    ZENTTY_TEST_DISPLAY_PROVIDER=betterdisplay \
    ZENTTY_BETTERDISPLAY_COMMAND="$fake_betterdisplay" \
    ZENTTY_TEST_SCREEN_EXISTS_COMMAND="$fake_screen_probe" \
    ZENTTY_TEST_MAIN_DISPLAY_COMMAND="$fake_main_display_probe" \
    ZENTTY_TEST_RAW_MAIN_DISPLAY_COMMAND="$fake_raw_main_display_probe" \
    ZENTTY_TEST_RESOLVE_DISPLAY_ID_COMMAND="$fake_resolve_display_probe" \
    ZENTTY_TEST_TOPOLOGY_STATUS_COMMAND="$fake_topology_probe" \
    ZENTTY_TEST_REGISTERED_SCREEN_COMMAND="$fake_registered_screen_probe" \
    ZENTTY_FAKE_ACTIVE_SCREEN_NAME='ZenttyTests+%282%29' \
    ZENTTY_FAKE_REGISTERED_SCREEN_PLAN='legacy:608:9' \
    ZENTTY_FAKE_BETTERDISPLAY_LOG="$betterdisplay_log" \
    ZENTTY_FAKE_TOPOLOGY_LOG="$topology_log" \
    ZENTTY_FAKE_DISPLAY_STATE="$display_state" \
    ZENTTY_FAKE_DISPLAY_CONNECTED_STATE="$display_connected_state" \
    ZENTTY_FAKE_MAIN_RESTORED_STATE="$main_restored_state" \
    ZENTTY_FAKE_XCODEBUILD_LOG="$xcodebuild_log" \
    "$repo_root/scripts/test-on-virtual-display" -only-testing:ZenttyLogicTests \
    > "$tmp_dir/harness.legacy-active.out" 2> "$tmp_dir/harness.legacy-active.err"
}

run_cleanup_failure_harness() {
  PATH="$bin_dir:$PATH" \
    ZDOTDIR="$zdot_dir" \
    TMPDIR="$tmp_dir" \
    ZENTTY_TEST_DISPLAY_STATE_DIR="$tmp_dir" \
    ZENTTY_TEST_DISPLAY_PROVIDER=betterdisplay \
    ZENTTY_BETTERDISPLAY_COMMAND="$fake_betterdisplay" \
    ZENTTY_TEST_SCREEN_EXISTS_COMMAND="$fake_screen_probe" \
    ZENTTY_TEST_MAIN_DISPLAY_COMMAND="$fake_main_display_probe" \
    ZENTTY_TEST_RAW_MAIN_DISPLAY_COMMAND="$fake_raw_main_display_probe" \
    ZENTTY_TEST_RESOLVE_DISPLAY_ID_COMMAND="$fake_resolve_display_probe" \
    ZENTTY_TEST_TOPOLOGY_STATUS_COMMAND="$fake_topology_probe" \
    ZENTTY_TEST_REGISTERED_SCREEN_COMMAND="$fake_registered_screen_probe" \
    ZENTTY_FAKE_DISCONNECT_FAILURE=1 \
    ZENTTY_FAKE_RAW_MAIN_CHANGED=1 \
    ZENTTY_FAKE_BETTERDISPLAY_LOG="$betterdisplay_log" \
    ZENTTY_FAKE_TOPOLOGY_LOG="$topology_log" \
    ZENTTY_FAKE_DISPLAY_STATE="$display_state" \
    ZENTTY_FAKE_DISPLAY_CONNECTED_STATE="$display_connected_state" \
    ZENTTY_FAKE_MAIN_RESTORED_STATE="$main_restored_state" \
    ZENTTY_FAKE_XCODEBUILD_LOG="$xcodebuild_log" \
    "$repo_root/scripts/test-on-virtual-display" -only-testing:ZenttyLogicTests \
    > "$tmp_dir/harness.cleanup-failure.out" 2> "$tmp_dir/harness.cleanup-failure.err"
}

run_http_404_harness() {
  PATH="$bin_dir:$PATH" \
    ZDOTDIR="$zdot_dir" \
    TMPDIR="$tmp_dir" \
    ZENTTY_TEST_DISPLAY_STATE_DIR="$tmp_dir" \
    ZENTTY_TEST_DISPLAY_PROVIDER=betterdisplay \
    ZENTTY_BETTERDISPLAY_HTTP_BASE="http://example.test" \
    ZENTTY_TEST_SCREEN_EXISTS_COMMAND="$fake_screen_probe" \
    ZENTTY_TEST_MAIN_DISPLAY_COMMAND="$fake_main_display_probe" \
    ZENTTY_TEST_RAW_MAIN_DISPLAY_COMMAND="$fake_raw_main_display_probe" \
    ZENTTY_TEST_RESOLVE_DISPLAY_ID_COMMAND="$fake_resolve_display_probe" \
    ZENTTY_TEST_TOPOLOGY_STATUS_COMMAND="$fake_topology_probe" \
    ZENTTY_TEST_REGISTERED_SCREEN_COMMAND="$fake_registered_screen_probe" \
    ZENTTY_FAKE_CURL_LOG="$curl_log" \
    ZENTTY_FAKE_TOPOLOGY_LOG="$topology_log" \
    ZENTTY_FAKE_CURL_CREATE_STATUS=404 \
    ZENTTY_FAKE_BETTERDISPLAY_LOG="$betterdisplay_log" \
    ZENTTY_FAKE_DISPLAY_STATE="$display_state" \
    ZENTTY_FAKE_DISPLAY_CONNECTED_STATE="$display_connected_state" \
    ZENTTY_FAKE_MAIN_RESTORED_STATE="$main_restored_state" \
    ZENTTY_FAKE_XCODEBUILD_LOG="$xcodebuild_log" \
    "$repo_root/scripts/test-on-virtual-display" -only-testing:ZenttyLogicTests \
    > "$tmp_dir/harness.http-404.out" 2> "$tmp_dir/harness.http-404.err"
}

run_harness one &
pid_one=$!
run_harness two "ZenttyTests (2)" &
pid_two=$!

parallel_status=0
wait "$pid_one" || parallel_status=1
wait "$pid_two" || parallel_status=1
if [[ "$parallel_status" != "0" ]]; then
  print -u2 "parallel BetterDisplay harness run failed"
  for output in "$tmp_dir"/harness.{one,two}.{out,err}; do
    print -u2 -- "--- ${output:t} ---"
    cat "$output" >&2 2>/dev/null || true
  done
  exit 1
fi

create_count="$(grep -c '^create ' "$betterdisplay_log" 2>/dev/null || true)"
if [[ "$create_count" != "1" ]]; then
  print -u2 "expected exactly one BetterDisplay create, got $create_count"
  print -u2 -- "--- BetterDisplay log ---"
  cat "$betterdisplay_log" >&2
  exit 1
fi

disconnect_count="$(grep -c -- '-connected=off' "$betterdisplay_log" 2>/dev/null || true)"
if [[ "$disconnect_count" != "1" ]]; then
  print -u2 "expected the last harness run to disconnect the shared virtual display once, got $disconnect_count disconnect calls"
  cat "$betterdisplay_log" >&2
  exit 1
fi

discard_count="$(grep -c '^discard ' "$betterdisplay_log" 2>/dev/null || true)"
if [[ "$discard_count" != "0" ]]; then
  print -u2 "expected no discard calls (display is kept registered for reuse), got $discard_count discard calls"
  cat "$betterdisplay_log" >&2
  exit 1
fi

if grep -q 'zentty-test-display-' "$betterdisplay_log"; then
  print -u2 "expected BetterDisplay reuse to rely on the canonical name, not a synthetic serial"
  cat "$betterdisplay_log" >&2
  exit 1
fi

topology_check_count="$(wc -l < "$topology_log" | tr -d '[:space:]')"
if [[ "$topology_check_count" != "2" ]]; then
  print -u2 "expected both harness runs to verify the canonical display topology, got $topology_check_count checks"
  cat "$topology_log" >&2 2>/dev/null || true
  exit 1
fi

if grep -vq '^ZenttyTests 42$' "$topology_log"; then
  print -u2 "expected topology checks to use the canonical display name and captured main display"
  cat "$topology_log" >&2
  exit 1
fi

xcodebuild_count="$(wc -l < "$xcodebuild_log" | tr -d '[:space:]')"
if [[ "$xcodebuild_count" != "2" ]]; then
  print -u2 "expected both harness runs to invoke xcodebuild, got $xcodebuild_count"
  cat "$xcodebuild_log" >&2
  exit 1
fi

: > "$xcodebuild_log"
: > "$betterdisplay_log"
: > "$topology_log"
rm -f "$display_state" "$display_connected_state" "$main_restored_state" "$display_reconnected_state"

if ! run_main_change_harness; then
  print -u2 "main-display restoration harness run failed"
  cat "$tmp_dir/harness.main-change.err" >&2
  exit 1
fi

if ! grep -q '^set -UUID=TEST-MAIN-UUID -main=on$' "$betterdisplay_log"; then
  print -u2 "expected the previous main display to be restored before tests"
  cat "$betterdisplay_log" >&2
  exit 1
fi

if ! grep -q '^ZenttyTests 84$' "$topology_log"; then
  print -u2 "expected topology verification to resolve the physical display's new displayID after connection"
  cat "$topology_log" >&2
  exit 1
fi

main_change_disconnect_count="$(grep -c '^set -tagID=606 -connected=off$' "$betterdisplay_log" 2>/dev/null || true)"
if [[ "$main_change_disconnect_count" != "2" ]]; then
  print -u2 "expected cleanup to retry when BetterDisplay reconnects the virtual display once"
  cat "$betterdisplay_log" >&2
  exit 1
fi

main_change_xcodebuild_count="$(wc -l < "$xcodebuild_log" | tr -d '[:space:]')"
if [[ "$main_change_xcodebuild_count" != "1" ]]; then
  print -u2 "expected restored main-display harness run to invoke xcodebuild once"
  cat "$xcodebuild_log" >&2
  exit 1
fi

: > "$xcodebuild_log"
: > "$betterdisplay_log"
: > "$topology_log"
rm -f "$display_state" "$display_connected_state" "$main_restored_state"

if run_mirrored_harness; then
  print -u2 "expected mirrored display topology to abort the harness"
  exit 1
fi

if ! grep -q "is mirrored; refusing to run tests that would change another display's resolution" "$tmp_dir/harness.mirrored.err"; then
  print -u2 "expected an actionable mirrored-display error"
  cat "$tmp_dir/harness.mirrored.err" >&2
  exit 1
fi

if [[ -s "$xcodebuild_log" ]]; then
  print -u2 "expected mirrored display topology to abort before xcodebuild"
  cat "$xcodebuild_log" >&2
  exit 1
fi

if ! grep -q -- '-connected=off' "$betterdisplay_log"; then
  print -u2 "expected mirrored display topology cleanup to disconnect the test screen"
  cat "$betterdisplay_log" >&2
  exit 1
fi

: > "$xcodebuild_log"
: > "$betterdisplay_log"
: > "$topology_log"
rm -f "$display_connected_state" "$main_restored_state"
touch "$display_state"

if ! run_legacy_cleanup_harness; then
  print -u2 "legacy BetterDisplay cleanup harness run failed"
  cat "$tmp_dir/harness.legacy-cleanup.err" >&2
  exit 1
fi

if ! grep -q '^discard -tagID=608$' "$betterdisplay_log"; then
  print -u2 "expected the disconnected malformed BetterDisplay duplicate to be discarded by stable tagID"
  cat "$betterdisplay_log" >&2
  exit 1
fi

if ! grep -q '^set -tagID=606 -connected=on$' "$betterdisplay_log"; then
  print -u2 "expected the canonical BetterDisplay screen to reconnect by stable tagID"
  cat "$betterdisplay_log" >&2
  exit 1
fi

if grep -q '^create ' "$betterdisplay_log"; then
  print -u2 "expected legacy cleanup to reuse the canonical screen without creating another display"
  cat "$betterdisplay_log" >&2
  exit 1
fi

: > "$xcodebuild_log"
: > "$betterdisplay_log"
: > "$topology_log"
touch "$display_state" "$display_connected_state"
rm -f "$main_restored_state"

if run_legacy_active_harness; then
  print -u2 "expected an active legacy BetterDisplay duplicate to fail closed"
  exit 1
fi

if grep -q '^create ' "$betterdisplay_log"; then
  print -u2 "expected an active legacy display to abort without creating another display"
  cat "$betterdisplay_log" >&2
  exit 1
fi

if ! grep -q 'legacy Zentty test display tagID 608 is still connected' "$tmp_dir/harness.legacy-active.err"; then
  print -u2 "expected an actionable active-legacy-display error"
  cat "$tmp_dir/harness.legacy-active.err" >&2
  exit 1
fi

: > "$xcodebuild_log"
: > "$betterdisplay_log"
: > "$topology_log"
rm -f "$display_state" "$display_connected_state" "$main_restored_state"

if run_cleanup_failure_harness; then
  print -u2 "expected persistent BetterDisplay cleanup failure to fail the harness"
  exit 1
fi

if ! grep -q 'BetterDisplay kept reconnecting virtual display tagID 606; it was discarded for display safety' "$tmp_dir/harness.cleanup-failure.err"; then
  print -u2 "expected an actionable persistent-cleanup error"
  cat "$tmp_dir/harness.cleanup-failure.err" >&2
  exit 1
fi

if ! grep -q '^discard -tagID=606$' "$betterdisplay_log"; then
  print -u2 "expected persistent reconnects to trigger an emergency discard by stable tagID"
  cat "$betterdisplay_log" >&2
  exit 1
fi

cleanup_failure_disconnect_count="$(grep -c '^set -tagID=606 -connected=off$' "$betterdisplay_log" 2>/dev/null || true)"
if [[ "$cleanup_failure_disconnect_count" != "5" ]]; then
  print -u2 "expected five disconnect attempts before failing cleanup"
  cat "$betterdisplay_log" >&2
  exit 1
fi

if ! grep -q '^set -UUID=TEST-MAIN-UUID -main=on$' "$betterdisplay_log"; then
  print -u2 "expected failed cleanup to restore the original main display by stable UUID"
  cat "$betterdisplay_log" >&2
  exit 1
fi

: > "$xcodebuild_log"
: > "$curl_log"
: > "$topology_log"
rm -f "$display_connected_state" "$main_restored_state"
touch "$display_state"

if run_failed_reconnect_harness; then
  print -u2 "expected a failed reconnect of a registered screen to abort"
  exit 1
fi

if grep -q '/create' "$curl_log"; then
  print -u2 "expected failed reconnect to abort without creating a duplicate screen"
  cat "$curl_log" >&2
  exit 1
fi

if ! grep -q "registered virtual display tagID 606 could not be connected" "$tmp_dir/harness.failed-reconnect.err"; then
  print -u2 "expected an actionable failed-reconnect error"
  cat "$tmp_dir/harness.failed-reconnect.err" >&2
  exit 1
fi

if [[ -s "$xcodebuild_log" ]]; then
  print -u2 "expected failed reconnect to abort before xcodebuild"
  cat "$xcodebuild_log" >&2
  exit 1
fi

: > "$xcodebuild_log"
: > "$curl_log"
: > "$betterdisplay_log"
: > "$topology_log"
rm -f "$display_state" "$display_connected_state" "$main_restored_state"

if ! run_http_harness; then
  print -u2 "HTTP fallback harness run failed"
  print -u2 -- "--- stdout ---"
  cat "$tmp_dir/harness.http.out" >&2
  print -u2 -- "--- stderr ---"
  cat "$tmp_dir/harness.http.err" >&2
  print -u2 -- "--- curl log ---"
  cat "$curl_log" >&2 2>/dev/null || true
  exit 1
fi

help_count="$(grep -c '/help' "$curl_log" 2>/dev/null || true)"
if [[ "$help_count" != "1" ]]; then
  print -u2 "expected BetterDisplay HTTP fallback to probe /help once, got $help_count"
  cat "$curl_log" >&2
  exit 1
fi

if ! grep -q '/create' "$curl_log"; then
  print -u2 "expected BetterDisplay HTTP fallback to create the virtual display"
  cat "$curl_log" >&2
  exit 1
fi

if ! grep -q -- '--data-urlencode type=VirtualScreen' "$curl_log"; then
  print -u2 "expected BetterDisplay HTTP fallback to use current virtual screen parameters"
  cat "$curl_log" >&2
  exit 1
fi

if ! grep -q -- '--data-urlencode virtualScreenName=ZenttyTests' "$curl_log" || grep -q '%28' "$curl_log"; then
  print -u2 "expected HTTP transport to use the canonical URL-safe display name"
  cat "$curl_log" >&2
  exit 1
fi

http_xcodebuild_count="$(wc -l < "$xcodebuild_log" | tr -d '[:space:]')"
if [[ "$http_xcodebuild_count" != "1" ]]; then
  print -u2 "expected HTTP fallback harness run to invoke xcodebuild once, got $http_xcodebuild_count"
  cat "$xcodebuild_log" >&2
  exit 1
fi

: > "$xcodebuild_log"
: > "$curl_log"
: > "$betterdisplay_log"
rm -f "$display_state" "$display_connected_state"

if ! run_http_404_harness; then
  print -u2 "HTTP 404 fallback harness run failed"
  print -u2 -- "--- stdout ---"
  cat "$tmp_dir/harness.http-404.out" >&2
  print -u2 -- "--- stderr ---"
  cat "$tmp_dir/harness.http-404.err" >&2
  print -u2 -- "--- curl log ---"
  cat "$curl_log" >&2 2>/dev/null || true
  print -u2 -- "--- BetterDisplay log ---"
  cat "$betterdisplay_log" >&2 2>/dev/null || true
  exit 1
fi

if ! grep -q '/create' "$curl_log"; then
  print -u2 "expected HTTP 404 fallback harness run to attempt HTTP create first"
  cat "$curl_log" >&2
  exit 1
fi

if ! grep -q '^create ' "$betterdisplay_log"; then
  print -u2 "expected HTTP 404 fallback harness run to recover with BetterDisplay command transport"
  cat "$betterdisplay_log" >&2
  exit 1
fi

http_404_xcodebuild_count="$(wc -l < "$xcodebuild_log" | tr -d '[:space:]')"
if [[ "$http_404_xcodebuild_count" != "1" ]]; then
  print -u2 "expected HTTP 404 fallback harness run to invoke xcodebuild once, got $http_404_xcodebuild_count"
  cat "$xcodebuild_log" >&2
  exit 1
fi
