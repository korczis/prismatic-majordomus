# The execution plane, proven from outside the executable: a capability started over HTTP,
# streamed over a real WebSocket, read back identically by the snapshot, the history, the
# Cockpit page and the command line.
#
# What this case exists for is the property no unit test can hold: that the *same*
# execution is one thing seen through five interfaces. A regression that gave the browser
# its own answer, or the command line its own list, would leave every Rust test green.
#
# The WebSocket client is python3 and thirty lines: the handshake of RFC 6455 and enough
# frame decoding to read what this server writes. A vendored client would prove that the
# vendored client works.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
command -v curl >/dev/null 2>&1 || { echo "    skip: no curl"; exit 0; }
command -v python3 >/dev/null 2>&1 || { echo "    skip: no python3"; exit 0; }
command -v jq >/dev/null 2>&1 || { echo "    skip: no jq"; exit 0; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj100.XXXXXX")"
SRV=""; trap 'rm -rf "$S"; [ -n "$SRV" ] && kill "$SRV" 2>/dev/null' EXIT

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm layer

# ---------------------------------------------------------------- the server
"$RB" serve --repo "$PWD" --port 0 > "$S/out.txt" 2> "$S/err.txt" & SRV=$!
i=0
until grep -q 'listening on http://' "$S/err.txt" 2>/dev/null; do
  i=$((i+1)); [ "$i" -lt 300 ] || { echo "    the server never listened"; cat "$S/err.txt"; exit 1; }
  kill -0 "$SRV" 2>/dev/null || { echo "    the server exited before listening"; cat "$S/err.txt"; exit 1; }
  sleep 0.1
done
U="$(sed -n 's#.*listening on \(http://127\.0\.0\.1:[0-9]*\).*#\1#p' "$S/err.txt" | head -n 1)"
[ -n "$U" ] || { echo "    no URL on the listening line"; exit 1; }

post() { curl -s -X POST -H 'Content-Type: application/json' -d "$2" "$U$1"; }
get()  { curl -s "$U$1"; }

# ---------------------------------------------------------------- the contract, self-described
get /api/v1/executions/protocol > "$S/protocol.json"
jq -e '.protocol_version == "1"' "$S/protocol.json" >/dev/null \
  || { echo "    the protocol version is not 1"; cat "$S/protocol.json"; exit 1; }
CHANNEL="$(jq -r .websocket "$S/protocol.json")"
[ -n "$CHANNEL" ] && [ "$CHANNEL" != null ] || { echo "    the protocol names no live channel"; exit 1; }
jq -e '(.event_types | index("execution.completed")) and (.stream_types | index("stream.ready"))' "$S/protocol.json" >/dev/null \
  || { echo "    the protocol does not describe its own messages"; jq -c '.event_types' "$S/protocol.json"; exit 1; }
# nothing here is written down twice: the schemas are the Rust types
jq -e '.event_schema.oneOf | length > 0' "$S/protocol.json" >/dev/null \
  || { echo "    the event schema is not derived"; exit 1; }

# ---------------------------------------------------------------- a WebSocket client
cat > "$S/follow.py" <<'PY'
import base64, json, os, socket, sys
base, target = sys.argv[1], sys.argv[2]
host, port = base.split("//")[1].split(":")
s = socket.create_connection((host, int(port)), timeout=30)
s.settimeout(30)
key = base64.b64encode(os.urandom(16)).decode()
s.sendall(("GET %s HTTP/1.1\r\nHost: %s:%s\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
           "Sec-WebSocket-Key: %s\r\nSec-WebSocket-Version: 13\r\n\r\n" % (target, host, port, key)).encode())
head = b""
while b"\r\n\r\n" not in head:
    b = s.recv(1)
    if not b:
        sys.exit("the server closed during the handshake: " + head.decode("utf-8", "replace"))
    head += b
if b" 101 " not in head.split(b"\r\n")[0] + b" ":
    sys.exit("no 101: " + head.decode("utf-8", "replace"))
buf = b""
def need(n):
    global buf
    while len(buf) < n:
        chunk = s.recv(65536)
        if not chunk:
            return False
        buf += chunk
    return True
while True:
    if not need(2):
        break
    op = buf[0] & 0x0f
    ln = buf[1] & 0x7f
    off = 2
    if ln == 126:
        need(4); ln = int.from_bytes(buf[2:4], "big"); off = 4
    elif ln == 127:
        need(10); ln = int.from_bytes(buf[2:10], "big"); off = 10
    if not need(off + ln):
        break
    payload, buf = buf[off:off+ln], buf[off+ln:]
    if op != 0x1:
        continue
    frame = json.loads(payload.decode())
    print(frame["type"])
    sys.stdout.flush()
    if frame["type"] in ("execution.completed", "execution.failed", "execution.cancelled"):
        break
PY

# ---------------------------------------------------------------- start, follow, agree
post /api/v1/executions/start '{"capability":"executions.demonstrate","input":{"steps":2,"delay_ms":150}}' > "$S/started.json"
ID="$(jq -r .id "$S/started.json")"
[ -n "$ID" ] && [ "$ID" != null ] || { echo "    the execution was not accepted"; cat "$S/started.json"; exit 1; }
[ "$(jq -r .state "$S/started.json")" = queued ] \
  || { echo "    a fresh execution is not queued: $(jq -r .state "$S/started.json")"; exit 1; }
# the links come from the registry, not from this case
[ "$(jq -r .links.cockpit "$S/started.json")" = "/cockpit/executions/$ID" ] \
  || { echo "    the Cockpit link is not derived"; jq -c .links "$S/started.json"; exit 1; }

python3 "$S/follow.py" "$U" "$CHANNEL?execution=$ID" > "$S/stream.txt" 2> "$S/stream.err" \
  || { echo "    the live channel failed"; cat "$S/stream.err"; exit 1; }
for t in stream.ready execution.created execution.started execution.step.started execution.log execution.progress execution.completed; do
  grep -qx "$t" "$S/stream.txt" || { echo "    the stream carried no $t"; cat "$S/stream.txt"; exit 1; }
done

get "/api/v1/executions/get?id=$ID" > "$S/snapshot.json"
[ "$(jq -r .state "$S/snapshot.json")" = succeeded ] \
  || { echo "    the snapshot disagrees with the stream: $(jq -r .state "$S/snapshot.json")"; exit 1; }
[ "$(jq -r .output.steps "$S/snapshot.json")" = 2 ] || { echo "    the output is not the handler's"; exit 1; }
[ "$(jq -r '.steps | length' "$S/snapshot.json")" = 2 ] || { echo "    the steps were not recorded"; exit 1; }

get "/api/v1/executions/events?id=$ID" > "$S/history.json"
[ "$(jq -r .state "$S/history.json")" = succeeded ] || { echo "    the history disagrees"; exit 1; }
# the stream and the history are the same events in the same order
jq -r '.events[].type' "$S/history.json" > "$S/history.txt"
grep -v '^stream\.' "$S/stream.txt" > "$S/stream-events.txt"
diff "$S/stream-events.txt" "$S/history.txt" >/dev/null \
  || { echo "    the stream and the retained history differ"; diff "$S/stream-events.txt" "$S/history.txt" | head; exit 1; }
# and the sequences are dense, which is what a client deduplicates on
jq -e '[.events[].sequence] as $s | ($s | length) > 0 and ($s == ([range(($s|min); ($s|max)+1)]))' "$S/history.json" >/dev/null \
  || { echo "    the sequences are not dense"; jq -c '[.events[].sequence]' "$S/history.json"; exit 1; }

# ---------------------------------------------------------------- a reconnection asks for the gap
CURSOR=2
python3 "$S/follow.py" "$U" "$CHANNEL?execution=$ID&after=$CURSOR" > "$S/replay.txt" 2>/dev/null \
  || { echo "    the replay connection failed"; exit 1; }
grep -qx stream.ready "$S/replay.txt" || { echo "    a reconnection is not told what it joined"; exit 1; }
grep -qx execution.completed "$S/replay.txt" || { echo "    a reconnection did not receive the end"; exit 1; }
grep -qx execution.created "$S/replay.txt" && { echo "    the cursor was ignored: an event before it was written again"; exit 1; }

# ---------------------------------------------------------------- the Cockpit renders it
curl -s -H 'Accept: text/html' "$U/cockpit/executions" > "$S/list.html"
grep -q "$ID" "$S/list.html" || { echo "    the Cockpit list does not carry the execution"; exit 1; }
curl -s -H 'Accept: text/html' "$U/cockpit/executions/$ID" > "$S/page.html"
grep -q 'data-mj-socket' "$S/page.html" || { echo "    the page carries no live channel"; exit 1; }
grep -q 'succeeded' "$S/page.html" || { echo "    the page does not show the state"; exit 1; }
# the page is complete without a script: the output is in the markup the server rendered
grep -q 'elapsed_ms' "$S/page.html" || { echo "    the page does not render the output"; exit 1; }

# ---------------------------------------------------------------- the command line agrees
"$RB" executions list --repo "$PWD" --format json > "$S/cli-list.json" 2> "$S/cli.err" \
  || { echo "    executions list failed"; cat "$S/cli.err"; exit 1; }
jq -e --arg id "$ID" '[.executions[].id] | index($id)' "$S/cli-list.json" >/dev/null \
  || { echo "    the command line does not see the server's executions"; jq -c '[.executions[].id]' "$S/cli-list.json"; exit 1; }
"$RB" executions show "$ID" --repo "$PWD" --format json > "$S/cli-show.json" 2>/dev/null
[ "$(jq -r .state "$S/cli-show.json")" = succeeded ] || { echo "    the command line disagrees about the state"; exit 1; }

# and it runs its own, in its own process, through the same capability
"$RB" run executions.demonstrate --repo "$PWD" --input '{"steps":1,"delay_ms":0}' --format json > "$S/cli-run.json" 2> "$S/run.err" \
  || { echo "    majordomus run failed"; cat "$S/run.err"; exit 1; }
[ "$(jq -r .state "$S/cli-run.json")" = succeeded ] || { echo "    the run did not succeed"; cat "$S/cli-run.json"; exit 1; }
[ "$(jq -r .output.observed "$S/cli-run.json")" = true ] || { echo "    the run was not observed"; exit 1; }

# ---------------------------------------------------------------- cancellation is real
post /api/v1/executions/start '{"capability":"executions.demonstrate","input":{"steps":40,"delay_ms":300}}' > "$S/long.json"
LONG="$(jq -r .id "$S/long.json")"
sleep 0.6
post /api/v1/executions/cancel "{\"id\":\"$LONG\"}" > "$S/cancel.json"
[ "$(jq -r .outcome "$S/cancel.json")" = requested ] || { echo "    cancelling was not accepted"; cat "$S/cancel.json"; exit 1; }
[ "$(jq -r .cancellable "$S/cancel.json")" = true ] || { echo "    the task does not declare itself cancellable"; exit 1; }
i=0
until [ "$(get "/api/v1/executions/get?id=$LONG" | jq -r .state)" = cancelled ]; do
  i=$((i+1)); [ "$i" -lt 100 ] || { echo "    the execution never reached cancelled"; exit 1; }
  sleep 0.1
done
[ "$(get "/api/v1/executions/get?id=$LONG" | jq -r '.steps | length')" -lt 40 ] \
  || { echo "    it ran to the end rather than stopping"; exit 1; }

# ---------------------------------------------------------------- the boundary
# The plane runs registered capabilities. Nothing takes a command, and nothing that is not
# in the registry is run.
for attempt in '{"capability":"sh -c echo"}' '{"capability":"../../bin/sh"}'; do
  CODE="$(curl -s -o "$S/refused.json" -w '%{http_code}' -X POST -H 'Content-Type: application/json' -d "$attempt" "$U/api/v1/executions/start")"
  case "$CODE" in 400|404) ;; *) echo "    '$attempt' was answered $CODE, not a refusal"; cat "$S/refused.json"; exit 1 ;; esac
done
get /openapi.json > "$S/openapi.json"
jq -e '[.paths | keys[] | select(test("shell|/exec/"))] | length == 0' "$S/openapi.json" >/dev/null \
  || { echo "    a shell-shaped route is in the document"; exit 1; }
# a plain GET of the live channel says what it is rather than upgrading
[ "$(curl -s -o /dev/null -w '%{http_code}' "$U$CHANNEL")" = 426 ] \
  || { echo "    a plain GET of the live channel did not answer 426"; exit 1; }

echo "    the execution plane: one execution through the socket, the snapshot, the history, the Cockpit and the command line"
