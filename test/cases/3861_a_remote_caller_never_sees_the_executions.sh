# majordomus-covers: capability:executions.list capability:executions.get capability:executions.events capability:executions.demonstrate
# majordomus-timeout: 300
# A caller on another host never sees this process's executions, live or recorded (I3842,
# ADR 0220 decision 4).
#
# I3826 refused the mesh's reads to a remote caller, but an execution of one of them that a
# worker on this machine started still streamed over /events and was listed by
# executions.list to anyone who could reach the server; on a hosted hub that is every
# visitor. An anonymous caller reads the repository and liveness, and nothing of what runs.
#
# The server is started from a copy of the executable on every interface, on a port the
# system picks, and only that process is stopped, by the pid this case started. Requests are
# sent to this machine's own address off loopback, which is what another host reaches (the
# harness of case 1086):
#
#   GET /api/v1/executions, /executions/get, /executions/events, /executions/demonstrate -> 403
#   GET /cockpit/executions, and the /events upgrade                                     -> 403
#   GET /api/v1/live, /api/v1/repository                                                -> 200
#   the same execution reads and the /events upgrade from 127.0.0.1                      -> answered
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v curl >/dev/null 2>&1 || skip "no curl"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; MAJORDOMUS_LOG=error; export MAJORDOMUS_SHARE MAJORDOMUS_LOG
unset MAJORDOMUS_HTTP_HOST
S="$(mktemp -d "${TMPDIR:-/tmp}/mj3861.XXXXXX")"
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
LAN="$(own_address || true)"
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
  PORT="$(jq -r --argjson pid "$PID" 'select(.pid == $pid) | .url // empty' .ai/local/state/mcp/server.json 2>/dev/null | sed -n 's/.*:\([0-9]*\)$/\1/p' || true)"
  [ -n "$PORT" ] && break
  sleep 0.2
done
[ -n "$PORT" ] || { echo "    the server did not publish an address"; cat "$S/serve.log"; exit 1; }

# status of one GET, its body in $S/body.json
get() { # host path
  curl -sS --max-time 30 -o "$S/body.json" -w '%{http_code}' "http://$1:$PORT$2" || true
}
# status of a WebSocket upgrade of the live channel; an accepted one is held open until the
# bound, so only the status line is read
upgrade() { # host
  curl -s --max-time 3 -o /dev/null -w '%{http_code}' \
    -H 'Connection: Upgrade' -H 'Upgrade: websocket' -H 'Sec-WebSocket-Version: 13' \
    -H 'Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==' "http://$1:$PORT/events" || true
}

# ---------------------------------------------------------------- a remote caller sees no execution
for path in /api/v1/executions '/api/v1/executions/get?id=e-none' \
  '/api/v1/executions/events?id=e-none' '/api/v1/executions/demonstrate?steps=1'; do
  status="$(get "$LAN" "$path")"
  if [ "$status" != 403 ] || ! jq -e '.error.code == "forbidden"' "$S/body.json" >/dev/null 2>&1; then
    echo "    GET $path from $LAN answered $status, not 403 forbidden:"
    sed 's/^/      /' "$S/body.json"; exit 1
  fi
done
status="$(get "$LAN" /cockpit/executions)"
[ "$status" = 403 ] || { echo "    /cockpit/executions answered a remote caller $status"; exit 1; }
status="$(upgrade "$LAN")"
[ "$status" = 403 ] || { echo "    the live channel's upgrade answered a remote caller $status, not 403"; exit 1; }

# ---------------------------------------------------------------- what an anonymous caller still reads
for path in /api/v1/live /api/v1/repository; do
  status="$(get "$LAN" "$path")"
  [ "$status" = 200 ] || { echo "    GET $path from $LAN answered $status, not 200"; sed 's/^/      /' "$S/body.json"; exit 1; }
done

# ---------------------------------------------------------------- this machine is unchanged
status="$(get 127.0.0.1 /api/v1/executions)"
[ "$status" = 200 ] && jq -e 'has("executions")' "$S/body.json" >/dev/null 2>&1 \
  || { echo "    this machine's executions.list answered $status:"; sed 's/^/      /' "$S/body.json"; exit 1; }
status="$(upgrade 127.0.0.1)"
[ "$status" = 101 ] || { echo "    the live channel did not open for this machine (status $status)"; exit 1; }
echo "    a remote caller is refused every execution read, the live channel and its page; liveness and the repository still answer it"
exit 0
