# majordomus-covers: none
# The blackboard is one projection, and every interface answers it (ADR 0176, I2980).
#
# Who works on what was three answers — the board, the mesh, the monitor — that every surface
# would have joined again. `blackboard.snapshot` joins them once. What is decided here, on a
# running server with two real MCP clients attached:
#
#   1. the command line, HTTP and MCP answer the same blackboard (the command line asks the
#      server holding the lease, because a board is that server's memory);
#   2. each worker is keyed by its checkout and board position, never by the bare position,
#      and says what it announced — or that it announced nothing;
#   3. two workers whose claims meet are a conflict, named by both keys;
#   4. a worker that goes away with an announcement is departed, not present.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v curl >/dev/null 2>&1 || skip "curl not installed"
command -v jq >/dev/null 2>&1 || skip "no jq"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj3380.XXXXXX")"; trap 'rm -rf "$S"' EXIT

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm layer >/dev/null

R="$PWD"
trap '"$RB" serve stop --repo "$R" >/dev/null 2>&1; rm -rf "$S"' EXIT
"$RB" serve ensure --port 0 --idle 120 --wait 40 --format json > "$S/ensure.json" 2>"$S/ensure.err" \
  || { echo "    the server did not come up:"; tail -3 "$S/ensure.err" | sed 's/^/      /'; exit 1; }
url="$(jq -r '.url // .server.url // empty' "$S/ensure.json")"
[ -n "$url" ] || { echo "    the server reported no url"; cat "$S/ensure.json"; exit 1; }

# One MCP client over HTTP: initialize, keep the session id the server hands out.
attach() { # <name> → prints the session id
  curl -fsS --max-time 10 -D "$S/h.$1" -o "$S/init.$1" -X POST "$url/mcp" \
    -H 'content-type: application/json' -H 'accept: application/json, text/event-stream' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"$1\",\"version\":\"1\"}}}" \
    || return 1
  tr -d '\r' < "$S/h.$1" | awk 'tolower($1)=="mcp-session-id:" {print $2}'
}
call() { # <session> <id> <tool> <arguments-json> → the tool's text result
  curl -fsS --max-time 60 -X POST "$url/mcp" -H "mcp-session-id: $1" \
    -H 'content-type: application/json' -H 'accept: application/json, text/event-stream' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":$2,\"method\":\"tools/call\",\"params\":{\"name\":\"$3\",\"arguments\":$4}}" \
    | sed -n 's/^data: //p; /^{/p' | jq -r '.result.content[0].text'
}

a="$(attach case-alpha)" && [ -n "$a" ] || { echo "    the first MCP client could not attach"; exit 1; }
b="$(attach case-beta)" && [ -n "$b" ] || { echo "    the second MCP client could not attach"; exit 1; }
call "$a" 2 majordomus_announce '{"intent":"alpha reads the docs","scope":["docs/guide"],"claim":"docs"}' >/dev/null \
  || { echo "    the first client could not announce"; exit 1; }
call "$a" 3 majordomus_announce '{"intent":"alpha fixes the site","scope":["site"],"claim":"site"}' >/dev/null \
  || { echo "    the first client could not announce its second claim"; exit 1; }

# ---------------------------------------------------------------- 1. one answer, three interfaces
# What differs between two reads taken a moment apart is the clock: the time of the read, the
# seconds since a peer was last seen, and the signals and cursor of a monitor that may have
# heard something in between. Everything else is the projection and must be identical.
same='del(.at, .signals, .cursor, .for_you) | .workers |= map(del(.last_seen_seconds_ago, .last_beat_ms))'
curl -fsS --max-time 60 "$url/api/v1/blackboard?checkouts=this" > "$S/http.json" \
  || { echo "    GET /api/v1/blackboard failed"; exit 1; }
call "$b" 4 majordomus_blackboard '{"checkouts":"this"}' > "$S/mcp.json" \
  || { echo "    the MCP tool majordomus_blackboard failed"; exit 1; }
"$RB" blackboard --this-checkout --format json > "$S/cli.json" 2>"$S/cli.err" \
  || { echo "    majordomus blackboard failed:"; tail -3 "$S/cli.err" | sed 's/^/      /'; exit 1; }
for f in http mcp cli; do jq -S "$same" "$S/$f.json" > "$S/$f.norm" || { echo "    $f answered no JSON"; exit 1; }; done
diff -u "$S/http.norm" "$S/mcp.norm" > "$S/d1" || { echo "    HTTP and MCP answer different blackboards:"; head -20 "$S/d1" | sed 's/^/      /'; exit 1; }
diff -u "$S/http.norm" "$S/cli.norm" > "$S/d2" || { echo "    HTTP and the command line answer different blackboards:"; head -20 "$S/d2" | sed 's/^/      /'; exit 1; }
echo "    the command line, HTTP and MCP answer the same blackboard"

# ---------------------------------------------------------------- 2. workers, keyed and announced
checkout="$(jq -r '.checkout' "$S/http.json")"
alpha="$(jq -c --arg c "board:$checkout/" '.workers[] | select(.client == "case-alpha")' "$S/http.json")"
beta="$(jq -c '.workers[] | select(.client == "case-beta")' "$S/http.json")"
[ -n "$alpha" ] && [ -n "$beta" ] || { echo "    an attached client is not a worker:"; jq -c '.workers' "$S/http.json" | sed 's/^/      /'; exit 1; }
printf '%s' "$alpha" | jq -e --arg c "$checkout" '.key | startswith("board:" + $c + "/p")' >/dev/null \
  || { echo "    a worker's key is not its checkout and board position: $(printf '%s' "$alpha" | jq -r .key)"; exit 1; }
[ "$(printf '%s' "$alpha" | jq -r '.claims | length')" = 2 ] \
  || { echo "    the first client's two named claims are not both on the blackboard"; exit 1; }
[ "$(printf '%s' "$alpha" | jq -r '.presence')" = attached ] || { echo "    an attached client is not attached"; exit 1; }
[ "$(printf '%s' "$beta" | jq -r '.intent // "none"')" = none ] \
  || { echo "    a client that announced nothing is given an intent"; exit 1; }
jq -e '.complete == (.sources | map(select(.source == "board" or .source == "mesh")) | all(.state == "ok" or .state == "skipped"))' "$S/http.json" >/dev/null \
  || { echo "    complete disagrees with how the board and the mesh were read"; jq -c '.sources' "$S/http.json"; exit 1; }
echo "    each worker is its checkout and position, with every claim; one that announced nothing has no intent"

# ---------------------------------------------------------------- 3. claims that meet
call "$b" 5 majordomus_announce '{"intent":"beta rewrites the guide","scope":["docs"]}' >/dev/null \
  || { echo "    the second client could not announce"; exit 1; }
curl -fsS --max-time 60 "$url/api/v1/blackboard?checkouts=this" > "$S/met.json" || { echo "    GET /api/v1/blackboard failed"; exit 1; }
akey="$(printf '%s' "$alpha" | jq -r .key)"; bkey="$(printf '%s' "$beta" | jq -r .key)"
jq -e --arg a "$akey" --arg b "$bkey" \
  '[.conflicts[] | select([.first.worker, .second.worker] | sort == ([$a, $b] | sort))] | length == 1 and .[0].certainty == "potential"' \
  "$S/met.json" >/dev/null \
  || { echo "    the two clients' meeting claims are not one potential conflict:"; jq -c '.conflicts' "$S/met.json" | sed 's/^/      /'; exit 1; }
jq -e '.conflicts_total >= (.conflicts | length)' "$S/met.json" >/dev/null || { echo "    conflicts_total undercounts"; exit 1; }
echo "    two workers whose claims meet are one potential conflict, named by both"

# ---------------------------------------------------------------- 4. gone is not present
curl -fsS --max-time 10 -X DELETE "$url/mcp" -H "mcp-session-id: $a" >/dev/null 2>&1 || true
curl -fsS --max-time 60 "$url/api/v1/blackboard?checkouts=this" > "$S/gone.json" || { echo "    GET /api/v1/blackboard failed"; exit 1; }
jq -e --arg a "$akey" '.workers[] | select(.key == $a) | .presence == "departed"' "$S/gone.json" >/dev/null \
  || { echo "    a client that went away holding claims is not departed:"; jq -c --arg a "$akey" '.workers[] | select(.key == $a)' "$S/gone.json"; exit 1; }
[ "$(jq -r '.present' "$S/gone.json")" -lt "$(jq -r '.present' "$S/met.json")" ] \
  || { echo "    a departed worker is still counted present"; exit 1; }
echo "    a worker that went away is departed and no longer counted present"

echo "    the blackboard is one projection, answered the same on every interface"
