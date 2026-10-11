# majordomus-covers: none
# majordomus-timeout: 300
# A caller on another host reads, and changes nothing it cannot authenticate (ADR 0126, I2123).
#
# On 2026-10-09 three servers on the owner's machine listened on every interface, and the
# only guard on a state-changing request was the `Origin` check, which stops a browser and
# no program. Any host on the LAN or the tailnet could transition a plan record, start any
# capability through `executions.start`, or post a mesh claim the node then signed with its
# own trusted key.
#
# The server is started from a copy of the executable on every interface, on a port the
# system picks, and only that process is stopped, by the pid this case started. Requests
# are sent to this machine's own address off loopback, which is what another host reaches:
#
#   POST plan/transition, executions/start, mesh/claims, peers/announce  -> 403 forbidden
#   GET  repository                                                     -> 200
#   POST plan/transition from 127.0.0.1                                 -> not 403
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v curl >/dev/null 2>&1 || skip "no curl"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; MAJORDOMUS_LOG=error; export MAJORDOMUS_SHARE MAJORDOMUS_LOG
unset MAJORDOMUS_HTTP_HOST
S="$(mktemp -d "${TMPDIR:-/tmp}/mj1022.XXXXXX")"
PID=""
trap '[ -n "$PID" ] && kill "$PID" 2>/dev/null; rm -rf "$S"' EXIT

# this machine's address off loopback: the source the kernel picks for a documentation-only
# destination. Nothing is sent. A machine with no route off loopback cannot run this case,
# and says so rather than passing.
own_address() {
  if command -v ip >/dev/null 2>&1; then
    ip -4 -o route get 192.0.2.1 2>/dev/null | sed -n 's/.* src \([0-9.]*\).*/\1/p'
  else
    ifc="$(route -n get 192.0.2.1 2>/dev/null | awk '/interface:/{print $2}')"
    [ -n "$ifc" ] && ipconfig getifaddr "$ifc" 2>/dev/null
  fi
}
LAN="$(own_address)"
case "$LAN" in
  ''|127.*) echo "    this machine has no address off loopback to send from (got '$LAN')"; exit 1 ;;
esac

cp "$RB" "$S/majordomus"
"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm layer

( exec "$S/majordomus" serve --host 0.0.0.0 --port 0 --idle 0 --discovery filesystem </dev/null >"$S/serve.log" 2>&1 ) &
PID=$!
PORT=""
for _ in $(seq 1 150); do
  PORT="$(jq -r --argjson pid "$PID" 'select(.pid == $pid) | .url // empty' .ai/local/state/mcp/server.json 2>/dev/null | sed -n 's/.*:\([0-9]*\)$/\1/p')"
  [ -n "$PORT" ] && break
  sleep 0.2
done
[ -n "$PORT" ] || { echo "    the server did not publish an address"; cat "$S/serve.log"; exit 1; }

# status and body of one request
call() { # method host path body
  curl -sS --max-time 30 -o "$S/body.json" -w '%{http_code}' -X "$1" \
    -H 'content-type: application/json' ${4:+--data "$4"} "http://$2:$PORT$3"
}

for route in plan/transition executions/start mesh/claims mesh/handovers/consume peers/announce; do
  status="$(call POST "$LAN" "/api/v1/$route" '{}')"
  if [ "$status" != 403 ] || ! jq -e '.error.code == "forbidden"' "$S/body.json" >/dev/null 2>&1; then
    echo "    POST /api/v1/$route from $LAN answered $status, not 403 forbidden:"
    sed 's/^/      /' "$S/body.json"; exit 1
  fi
done

status="$(call GET "$LAN" /api/v1/repository)"
[ "$status" = 200 ] || { echo "    a remote read answered $status"; sed 's/^/      /' "$S/body.json"; exit 1; }

status="$(call POST 127.0.0.1 /api/v1/plan/transition '{}')"
[ "$status" != 403 ] || { echo "    the same write from loopback was refused as remote"; exit 1; }
exit 0
