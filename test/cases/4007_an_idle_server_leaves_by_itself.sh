# majordomus-covers: none
# majordomus-timeout: 300
# An idle server leaves by itself, and a server in use stays (I4007).
#
# On 2026-10-22 fifteen `serve --idle 900` processes had run on one Mac for up to eight hours
# with no peer attached. They were not ignoring their idle life: they were never idle, because
# every HTTP request counted as use and the servers of one machine talk to each other all the
# time — the mesh links them, a board gather reads every sibling's board, and status and
# ensure probe every lease's server. This case sends exactly that traffic to a server with a
# three-second idle life and requires it to leave anyway, by itself, with exit 0, the reason
# in its log and its lease released. Then the other half: an attached MCP session and a running
# execution each keep a server past its bound, and it leaves once they are gone; a server
# started without an idle life stays.
#
# `--port 0` throughout: the default port belongs to whatever already serves this machine.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "jq not installed"
command -v curl >/dev/null 2>&1 || skip "curl not installed"

MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
MAJORDOMUS_LOG=info; export MAJORDOMUS_LOG
lease=.ai/local/state/mcp/server.json
pid=""
trap '[ -n "$pid" ] && kill "$pid" 2>/dev/null; true' EXIT

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm base

start() {   # start NAME IDLE — a server as `serve ensure` starts one: stdin not a pipe
  "$RB" serve --port 0 --idle "$2" < /dev/null > /dev/null 2> "$T/$1.log" &
  pid=$!
  url=""
  for _ in $(seq 1 150); do
    url="$(sed -n 's/.*listening on \(http:[^ ]*\).*/\1/p' "$T/$1.log" | head -1)"
    [ -n "$url" ] && break
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.2
  done
  [ -n "$url" ] || { echo "    the $1 server never logged its address:"; cat "$T/$1.log"; exit 1; }
}
alive() { kill -0 "$pid" 2>/dev/null; }
gone_within() {   # gone_within SECONDS NAME — the server ends by itself, exit 0, reason logged
  local deadline=$(( $(date +%s) + $1 ))
  while alive && [ "$(date +%s)" -lt "$deadline" ]; do sleep 0.2; done
  if alive; then
    echo "    the $2 server was still running $1 s later:"; tail -20 "$T/$2.log"; exit 1
  fi
  wait "$pid"; local code=$?
  pid=""
  [ "$code" = 0 ] || { echo "    the $2 server exited $code, not 0:"; tail -20 "$T/$2.log"; exit 1; }
  grep -q 'idle for [0-9]* s (bound [0-9]* s): no peer attached, no execution queued or running' "$T/$2.log" \
    || { echo "    the $2 server left without saying why:"; tail -20 "$T/$2.log"; exit 1; }
  if [ -f "$lease" ]; then
    echo "    the $2 server left its lease behind:"; cat "$lease"; exit 1
  fi
}
machine_traffic() {   # what another server sends: a mesh link, a probe, a sibling's board read
  curl -s -o /dev/null --max-time 2 -X POST -H 'content-type: application/json' \
    -d '{}' "$url/api/v1/mesh/link/hello" || true
  curl -s -o /dev/null --max-time 2 -X POST -H 'content-type: application/json' \
    -d '{}' "$url/api/v1/mesh/link/sync" || true
  curl -s -o /dev/null --max-time 2 -H 'X-Majordomus-Probe: 1' "$url/" || true
  curl -s -o /dev/null --max-time 2 "$url/api/v1/peers?checkouts=this" || true
}
mcp() {   # mcp ID METHOD PARAMS [CURL-ARGS...]
  local id="$1" method="$2" params="$3"; shift 3
  curl -fsS --connect-timeout 5 --max-time 30 -X POST "$url/mcp" \
    -H 'content-type: application/json' -H 'accept: application/json, text/event-stream' \
    "$@" -d "{\"jsonrpc\":\"2.0\",\"id\":$id,\"method\":\"$method\",\"params\":$params}"
}

# ---------------------------------------------------------------- 1. machine traffic is not use
# Every 0.3 s, for up to 20 s, the traffic of a busy machine's other servers. The server has a
# three-second idle life and must leave inside that window all the same.
start machine 3
began="$(date +%s)"
while alive && [ $(( $(date +%s) - began )) -lt 20 ]; do machine_traffic; sleep 0.3; done
gone_within 5 machine
[ $(( $(date +%s) - began )) -ge 3 ] || { echo "    the server left before its idle life"; exit 1; }

# ---------------------------------------------------------------- 2. an attached peer keeps it
start peer 2
mcp 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case4007","version":"0"}}' \
  -D "$T/mcp.headers" > "$T/mcp.init" || { echo "    MCP initialize failed"; cat "$T/mcp.init"; exit 1; }
sid="$(tr -d '\r' < "$T/mcp.headers" | sed -n 's/^[Mm]cp-[Ss]ession-[Ii]d: //p' | head -n 1)"
[ -n "$sid" ] || { echo "    the MCP endpoint opened no session:"; cat "$T/mcp.headers"; exit 1; }
sleep 5
alive || { echo "    a server with a peer attached left:"; tail -20 "$T/peer.log"; exit 1; }
# the peer leaves; the server follows within its idle life
curl -fsS -o /dev/null --max-time 10 -X DELETE -H "mcp-session-id: $sid" "$url/mcp" \
  || { echo "    the session could not be closed"; exit 1; }
gone_within 10 peer

# ---------------------------------------------------------------- 3. a running execution keeps it
start execution 2
curl -fsS --max-time 10 -X POST -H 'content-type: application/json' \
  -d '{"capability":"executions.demonstrate","input":{"steps":6,"delay_ms":1000}}' \
  "$url/api/v1/executions/start" > "$T/start.json" \
  || { echo "    the execution did not start:"; cat "$T/start.json"; exit 1; }
sleep 4.5
alive || { echo "    a server running an execution left:"; tail -20 "$T/execution.log"; exit 1; }
# six steps of a second each: finished by about 6 s, then two idle seconds
gone_within 15 execution

# ---------------------------------------------------------------- 4. no idle life: the owner's server stays
start owned 0
sleep 4
alive || { echo "    a server started without an idle life left:"; tail -20 "$T/owned.log"; exit 1; }
grep -q 'idle for' "$T/owned.log" && { echo "    a server without an idle life judged itself idle"; exit 1; }
kill "$pid"; wait "$pid" 2>/dev/null; pid=""
exit 0
