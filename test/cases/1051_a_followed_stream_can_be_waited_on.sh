# majordomus-timeout: 300
# `majordomus monitor --follow` is a stream a script or an agent can wait on (ADR 0133,
# I2402): `--count` ends it after so many signals, `--until` at the first signal of a kind,
# `--match` keeps only what names a text, and `--timeout` gives up with exit 10. While
# following, `--format json` prints one signal per line (NDJSON), and what happens after
# the follow started reaches it without anything already printed coming back.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"
unset MAJORDOMUS_MONITOR_FOR

PIDS=""
trap 'for p in $PIDS; do kill "$p" 2>/dev/null || true; done' EXIT

R="$T/repo"
mkdir -p "$R"
(
  cd "$R" || exit
  git init -q .
  git config user.email t@example.com
  git config user.name t
  "$MJ" init >/dev/null
  "$MJ" update >/dev/null
  mkdir -p .ai/repo/mesh
  printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n' > .ai/repo/mesh/majordomus.yaml
  printf 'multicast:\n  enabled: false\ntrust:\n  policy: tofu\n' >> .ai/repo/mesh/majordomus.yaml
  printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 4\n  repository: case-1051\n' >> .ai/repo/mesh/majordomus.yaml
  git add -A && git commit -qm base
)
mkdir -p "$T/state" "$T/bin"
# GitHub is the network: a gh that is not there is the answer this case expects
printf '#!/bin/sh\necho "gh: not here in this case" >&2\nexit 4\n' > "$T/bin/gh"
chmod +x "$T/bin/gh"
( cd "$R" && PATH="$T/bin:$PATH" XDG_STATE_HOME="$T/state" exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$T/serve.log" 2>&1 ) &
PIDS="$PIDS $!"
ready() { [ -n "$(jq -r '.url // empty' "$R/.ai/local/state/mcp/server.json" 2>/dev/null)" ]; }
i=0; until ready; do i=$((i + 1)); [ "$i" -lt 200 ] || { cat "$T/serve.log"; exit 1; }; sleep 0.2; done
cd "$R" || exit

# ---------------------------------------------------------------- 1. --count ends a follow; NDJSON is one signal per line
expect_exit 0 "$RB" mesh session open --session s1 --worker 3d --intent "the runtime train"
expect_exit 0 "$RB" mesh session open --session s2 --worker cd --intent "the executor lane"
LINES="$("$RB" monitor --follow --sources mesh --count 2 --format json --timeout 20 2>/dev/null)" \
  || { echo "    a follow with --count did not end with 0"; exit 1; }
[ "$(printf '%s\n' "$LINES" | wc -l | tr -d ' ')" = 2 ] || { echo "    not two lines: $LINES"; exit 1; }
printf '%s\n' "$LINES" | jq -e -s 'length == 2 and all(.[]; .source == "mesh" and (.summary | length) > 0)' >/dev/null \
  || { echo "    the lines are not one signal each: $LINES"; exit 1; }

# ---------------------------------------------------------------- 2. --until waits for what happens next, --match narrows it
( "$RB" monitor --follow --sources mesh --since 1s --match 'release' --until session_changed \
    --interval 1 --timeout 60 >"$T/until.out" 2>"$T/until.err"; echo $? >"$T/until.rc" ) &
PIDS="$PIDS $!"
sleep 3
expect_exit 0 "$RB" mesh session open --session s2 --worker cd --intent "the executor lane, again"
expect_exit 0 "$RB" mesh session open --session s1 --worker 3d --intent "release 0.21.0"
i=0; until [ -s "$T/until.rc" ]; do i=$((i + 1)); [ "$i" -lt 300 ] || { echo "    --until never ended"; cat "$T/until.err"; exit 1; }; sleep 0.2; done
[ "$(cat "$T/until.rc")" = 0 ] || { echo "    --until ended with $(cat "$T/until.rc")"; cat "$T/until.err"; exit 1; }
grep -q '3d (s1) now: release 0.21.0' "$T/until.out" || { echo "    the awaited signal is not printed"; cat "$T/until.out"; exit 1; }
grep -q 'executor lane, again' "$T/until.out" && { echo "    --match let another signal through"; cat "$T/until.out"; exit 1; }
grep -q '^sources  mesh ok' "$T/until.out" || { echo "    a text follow does not say its sources first"; cat "$T/until.out"; exit 1; }

# ---------------------------------------------------------------- 3. --timeout gives up with 10
expect_exit 10 "$RB" monitor --follow --sources mesh --since 1s --until merged --interval 1 --timeout 2
expect_grep 'timeout reached before .*count or .*until was met'
