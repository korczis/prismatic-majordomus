# majordomus-timeout: 300
# A monitor source is declared once (I2944): the real executable refuses an unknown source
# with exactly the declared names, on the command line and over HTTP, and takes a declared one.
# The unit tests prove that a test-only row reaches every surface; this case proves the
# shipped executable reads the same table on both transports.
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
  git add -A && git commit -qm base
)
mkdir -p "$T/state"
( cd "$R" && XDG_STATE_HOME="$T/state" exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$T/serve.log" 2>&1 ) &
SRV_PID=$!
PIDS="$PIDS $SRV_PID"
ready() {
  local url
  url="$(jq -r '.url // empty' "$R/.ai/local/state/mcp/server.json" 2>/dev/null)"
  [ -n "$url" ] && curl -sS --max-time 10 "$url/api/v1/ready" 2>/dev/null | jq -e '.ready == true' >/dev/null 2>&1
}
i=0; until ready; do
  kill -0 "$SRV_PID" 2>/dev/null || { echo "    the server exited before it was ready"; cat "$T/serve.log"; exit 1; }
  i=$((i + 1)); [ "$i" -lt 600 ] || { echo "    the server was not ready in 120 s"; cat "$T/serve.log"; exit 1; }
  sleep 0.2
done
cd "$R" || exit
URL="$(jq -r '.url' "$R/.ai/local/state/mcp/server.json")"
DECLARED='the sources are mesh, executor and github'

# ---------------------------------------------------------------- 1. HTTP: an unknown name is refused with the declared list
BODY="$(curl -sS --max-time 20 "$URL/api/v1/monitor?sources=slack")"
grep -qE "unknown source .?\"slack.?\": $DECLARED" <<<"$BODY" \
  || { echo "    HTTP refusal does not name the declared sources:"; echo "$BODY" | head -5; exit 1; }

# ---------------------------------------------------------------- 2. the command line: the same refusal
expect_exit 10 "$RB" monitor --sources slack
expect_grep "unknown source .?\"slack.?\": $DECLARED"

# ---------------------------------------------------------------- 3. a declared name is taken, and every declared source is reported
PAGE="$("$RB" monitor --sources mesh,executor --format json 2>/dev/null)" || { echo "    monitor with declared sources failed"; exit 1; }
jq -e '[.sources[].source] == ["mesh","executor","github"]' <<<"$PAGE" >/dev/null \
  || { echo "    the page does not report every declared source in order:"; echo "$PAGE" | jq -c '.sources'; exit 1; }
jq -e '.sources[] | select(.source == "github") | .state == "skipped"' <<<"$PAGE" >/dev/null \
  || { echo "    a source not asked for is not reported skipped"; exit 1; }
echo "    ok: one declaration answers both transports"
