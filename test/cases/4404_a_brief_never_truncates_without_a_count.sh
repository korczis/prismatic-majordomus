# majordomus-covers: none
# claims: mcp-peers
# The brief cuts nothing it does not count (I4390).
#
# On 2026-10-29 the peers section cut every announced intent at 72 characters, mid-word, and
# said nothing of what it had left out. The lines now wrap each named worker's whole intent at
# 100 columns under the label column; past eight workers they say `+N more: majordomus peers`;
# and the one line, the only form whose width is bounded, drops whole tokens from the right and
# counts them as `+N`.
#
# What it proves, over a real server and twelve real MCP clients:
#   1  the multi-line brief carries every named worker's 400-byte intent whole
#   2  it names eight and says `+4 more: majordomus peers`
#   3  nothing in the lines is cut with an ellipsis, and no line is wider than 100 columns
#   4  the one line is at most 160 columns, and whatever it dropped it counted
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v curl >/dev/null 2>&1 || skip "curl not installed"
command -v python3 >/dev/null 2>&1 || skip "no python3"
export MAJORDOMUS_SHARE="$ROOT/share"
fail() { printf '    %s\n' "$*"; exit 1; }

PIDS=""
trap 'for p in $PIDS; do kill "$p" 2>/dev/null || true; done' EXIT
R="$T/repo"; mkdir -p "$R"
(
  cd "$R" || exit
  git init -q . && git config user.email t@example.com && git config user.name t
  "$MJ" init >/dev/null && "$MJ" update >/dev/null
  git add -A && git commit -qm base
)
mkdir -p "$T/bin" "$T/state"
printf '#!/bin/sh\necho "gh: not here in this case" >&2\nexit 4\n' > "$T/bin/gh"; chmod +x "$T/bin/gh"
( cd "$R" && PATH="$T/bin:$PATH" XDG_STATE_HOME="$T/state" exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$T/serve.log" 2>&1 ) &
PIDS="$PIDS $!"
ready() { [ -n "$(jq -r '.url // empty' "$R/.ai/local/state/mcp/server.json" 2>/dev/null)" ]; }
i=0; until ready; do i=$((i + 1)); [ "$i" -lt 200 ] || { cat "$T/serve.log"; exit 1; }; sleep 0.2; done
url="$(jq -r '.url' "$R/.ai/local/state/mcp/server.json")"

# twelve workers, each announcing an intent of 400 bytes that ends in a word of its own
intent() { printf 'worker %02d ' "$1"; i=0; while [ "$i" -lt 44 ]; do printf 'measured '; i=$((i + 1)); done; printf 'end%02d' "$1"; }
n=1
while [ "$n" -le 12 ]; do
  H="$T/h.$n"
  curl -fsS --max-time 10 -D "$H" -o /dev/null -X POST "$url/mcp" \
    -H 'content-type: application/json' -H 'accept: application/json, text/event-stream' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"case-$n\",\"version\":\"1\"}}}" \
    || fail "client $n could not attach"
  SID="$(tr -d '\r' < "$H" | awk 'tolower($1)=="mcp-session-id:" {print $2}')"
  body="$(jq -nc --arg i "$(intent "$n")" '{jsonrpc:"2.0",id:2,method:"tools/call",params:{name:"majordomus_announce",arguments:{intent:$i,scope:["docs"]}}}')"
  curl -fsS --max-time 30 -X POST "$url/mcp" -H "mcp-session-id: $SID" \
    -H 'content-type: application/json' -H 'accept: application/json, text/event-stream' -d "$body" >/dev/null \
    || fail "client $n could not announce"
  n=$((n + 1))
done
[ "$(intent 1 | wc -c | tr -d ' ')" -ge 390 ] || fail "the intents are not long enough to prove anything"

text="$(cd "$R" && PATH="$T/bin:$PATH" "$RB" brief --this-checkout 2>/dev/null)" || fail "brief failed"
json="$(cd "$R" && PATH="$T/bin:$PATH" "$RB" brief --this-checkout --format json 2>/dev/null)" || fail "brief --format json failed"
[ "$(printf '%s' "$json" | jq -r .answered_by.process)" = leaseholder ] || fail "the brief was not the leaseholder's"
[ "$(printf '%s' "$json" | jq -r .peers.attached)" = 12 ] || fail "the board does not hold twelve workers"

# ---------------------------------------------------------------- 1  whole intents
flat="$(printf '%s' "$text" | tr '\n' ' ' | tr -s ' ')"
named="$(printf '%s' "$json" | jq -r '.peers.people[].intent')"
[ "$(printf '%s\n' "$named" | grep -c .)" = 8 ] || fail "the brief does not name eight workers"
printf '%s\n' "$named" | while IFS= read -r want; do
  case "$flat" in *"$want"*) ;; *) fail "an intent is not whole in the lines: $(printf '%.60s' "$want")" ;; esac
done

# ---------------------------------------------------------------- 2  the rest is counted
case "$text" in *'+4 more: majordomus peers'*) ;; *) fail "the lines do not count the four workers left out" ;; esac

# ---------------------------------------------------------------- 3  nothing clipped, nothing wide
case "$text" in *'…'*) fail "something in the lines is cut with an ellipsis" ;; esac
# counted in characters, not bytes: an em dash is one column
wide="$(printf '%s\n' "$text" | sed 1d | python3 -c 'import sys
for l in sys.stdin.read().splitlines():
    if len(l) > 100: print(l); break')"
[ -z "$wide" ] || fail "a line is wider than 100 columns: $wide"

# ---------------------------------------------------------------- 4  the one line
line="$(cd "$R" && PATH="$T/bin:$PATH" "$RB" brief --this-checkout --line 2>/dev/null)"
[ "$(printf '%s' "$line" | python3 -c 'import sys; print(len(sys.stdin.read()))')" -le 160 ] || fail "the line is wider than 160 columns: $line"
tokens="$(printf '%s' "$line" | awk -F' [|] ' '{ print NF }')"
case "$line" in
  *' | +'[0-9]*) ;;
  # nothing dropped: every section's token is there
  *) for want in 'ci ' 'prs ' 'disk ' 'mesh ' 'peers 12/' 'release '; do
       case "$line" in *"$want"*) ;; *) fail "the line dropped '$want' and did not count it: $line ($tokens tokens)" ;; esac
     done ;;
esac
exit 0
