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
command -v curl >/dev/null 2>&1 || skip "no curl"
mj_runs python3 || skip "$(mj_runs_why python3)"
command -v jq >/dev/null 2>&1 || skip "no jq"
mj_runs node || skip "$(mj_runs_why node)"

# ---------------------------------------------------------------- the Cockpit's client
# share/cockpit/socket.js is the one client of the channel, and it is where a gap, a
# duplicate and a dead connection are noticed. It is run here against a WebSocket and a
# clock this case controls: no browser, no server, no wait. It must drop an event at or
# below the last sequence seen, reconnect from that sequence when one is skipped (the
# server replays the gap), retire a channel silent for 2.5 heartbeats (the server writes
# `stream.heartbeat` on every quiet interval), and not ask the same gap twice.
SOCKET_CHECK="$(mktemp "${TMPDIR:-/tmp}/mj100-socket.XXXXXX")"
node --input-type=module - "$ROOT/share/cockpit/socket.js" > "$SOCKET_CHECK" 2>&1 <<'NODE' \
  || { echo "    the Cockpit's live client does not keep the stream whole:"; sed 's/^/    | /' "$SOCKET_CHECK"; rm -f "$SOCKET_CHECK"; exit 1; }
import { readFileSync } from 'node:fs';
const source = readFileSync(process.argv[2], 'utf8');

// a clock this script moves, so a watchdog of fifty seconds is proved in no time
let now = 0;
let timers = [];
let nextTimer = 1;
globalThis.setTimeout = (fn, ms) => {
  const id = nextTimer++;
  timers.push({ id, at: now + Math.max(0, ms || 0), fn });
  return id;
};
globalThis.clearTimeout = (id) => { timers = timers.filter((t) => t.id !== id); };
function advance(ms) {
  const until = now + ms;
  for (;;) {
    timers.sort((a, b) => a.at - b.at || a.id - b.id);
    const due = timers[0];
    if (!due || due.at > until) break;
    timers.shift();
    now = due.at;
    due.fn();
  }
  now = until;
}
Math.random = () => 0.5;

// a WebSocket that is a list of what was opened and a way to speak for the server
const sockets = [];
class FakeSocket {
  constructor(url) { this.url = url; this.listeners = {}; this.closed = false; sockets.push(this); }
  addEventListener(name, fn) { (this.listeners[name] ||= []).push(fn); }
  emit(name, event) { for (const fn of this.listeners[name] || []) fn(event); }
  send(frame) { this.emit('message', { data: JSON.stringify(frame) }); }
  close() { this.closed = true; }
}
globalThis.WebSocket = FakeSocket;
globalThis.window = { location: { href: 'http://127.0.0.1:8741/cockpit/executions/x-1' } };

const { follow } = await import('data:text/javascript,' + encodeURIComponent(source));

const fail = (why) => { console.error(why); process.exit(1); };
const eq = (a, b, why) => {
  if (JSON.stringify(a) !== JSON.stringify(b)) fail(`${why}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`);
};
const event = (sequence) => ({ type: 'execution.log', execution_id: 'x-1', sequence, data: {} });
const after = (s) => new URL(s.url).searchParams.get('after');

const delivered = [];
const resyncs = [];
follow('/events?execution=x-1&after=2', {
  message: (f) => delivered.push(f.sequence ?? f.type),
  resync: (why) => resyncs.push(why),
});
let s = sockets.at(-1);
s.emit('open');
s.send({ type: 'stream.ready', data: { heartbeat_seconds: 20, truncated: false } });

// duplicates are dropped, and the heartbeat is the channel's own business
[3, 4, 4, 3].forEach((n) => s.send(event(n)));
s.send({ type: 'stream.heartbeat', data: { at: 'now' } });
eq(delivered, [3, 4], 'an event at or below the last seen is a duplicate');

// a gap on a one-execution channel reconnects from the last sequence seen
s.send(event(6));
eq(delivered, [3, 4], 'the event after a gap is not handed over');
if (!s.closed) fail('a gap did not retire the socket');
advance(1000);
s = sockets.at(-1);
eq(sockets.length, 2, 'a gap opens one new connection');
eq(after(s), '4', 'the reconnect asks for what it missed');
s.emit('open');
s.send({ type: 'stream.ready', data: { heartbeat_seconds: 20, truncated: false } });
[5, 6].forEach((n) => s.send(event(n)));
eq(delivered, [3, 4, 5, 6], 'the replay filled the gap');

// a quiet channel with heartbeats stays; a silent one is retired after two and a half
advance(40_000);
s.send({ type: 'stream.heartbeat', data: { at: 'now' } });
advance(40_000);
eq(sockets.length, 2, 'a channel that heartbeats is not reconnected');
advance(11_000);
if (!s.closed) fail('a silent channel was not retired');
advance(1000);
s = sockets.at(-1);
eq(sockets.length, 3, 'a silent channel reconnects');
eq(after(s), '6', 'from its cursor');
s.emit('open');
s.send({ type: 'stream.ready', data: { heartbeat_seconds: 20, truncated: false } });

// a gap the replay did not fill is not asked again: the page reads the history instead
s.send(event(8));
advance(1000);
s = sockets.at(-1);
eq(sockets.length, 4, 'the first gap at 6 reconnects');
s.emit('open');
s.send({ type: 'stream.ready', data: { heartbeat_seconds: 20, truncated: false } });
s.send(event(8));
eq(sockets.length, 4, 'the same gap twice does not reconnect again');
eq(delivered.at(-1), 8, 'it is handed over');
if (!resyncs.includes('gap')) fail(`the page was not told to read the history: ${resyncs}`);

// a replay whose cursor's history was dropped starts the picture where it can
s.send({ type: 'stream.ready', data: { heartbeat_seconds: 20, truncated: true } });
s.send(event(40));
eq(delivered.at(-1), 40, 'the first event after a truncated replay is not a gap');
eq(sockets.length, 4, 'and does not reconnect');

// a channel that follows every execution drops repeats per execution
const all = [];
follow('/events', { message: (f) => all.push(`${f.execution_id}#${f.sequence}`) });
s = sockets.at(-1);
s.emit('open');
s.send({ type: 'stream.ready', data: { heartbeat_seconds: 20 } });
s.send({ ...event(1), execution_id: 'a' });
s.send({ ...event(1), execution_id: 'b' });
s.send({ ...event(1), execution_id: 'a' });
s.send({ ...event(2), execution_id: 'a' });
eq(all, ['a#1', 'b#1', 'a#2'], 'a repeat of one execution is dropped, another execution is not');
if (new URL(s.url).searchParams.has('after')) fail('an unscoped channel names a cursor');
console.log('socket.js: duplicates dropped, a gap reconnects from the cursor, a silent channel is retired');
NODE
grep -q '^socket.js: duplicates dropped' "$SOCKET_CHECK" \
  || { echo "    the client check did not reach its end:"; sed 's/^/    | /' "$SOCKET_CHECK"; rm -f "$SOCKET_CHECK"; exit 1; }
rm -f "$SOCKET_CHECK"

RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj100.XXXXXX")"
SRV=""; trap 'rm -rf "$S"; [ -n "$SRV" ] && kill "$SRV" 2>/dev/null' EXIT

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm layer

# ---------------------------------------------------------------- the server
serve_up "$S/out.txt" "$S/err.txt" || exit 1

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
# `shell.check` is the one route whose name says shell, and it runs nothing: it reads the
# tree against the automation inventory, so it must stay a GET with no parameter and no body.
jq -e '[.paths | keys[] | select(. != "/api/v1/shell/check") | select(test("shell|/exec/"))]
       | length == 0' "$S/openapi.json" >/dev/null \
  || { echo "    a shell-shaped route is in the document"; exit 1; }
jq -e '.paths["/api/v1/shell/check"]
       | (keys == ["get"]) and (.get.requestBody == null) and ((.get.parameters // []) == [])' \
  "$S/openapi.json" >/dev/null \
  || { echo "    the shell inventory check is not a read-only GET that takes nothing"; exit 1; }
# a plain GET of the live channel says what it is rather than upgrading
[ "$(curl -s -o /dev/null -w '%{http_code}' "$U$CHANNEL")" = 426 ] \
  || { echo "    a plain GET of the live channel did not answer 426"; exit 1; }

echo "    the execution plane: one execution through the socket, the snapshot, the history, the Cockpit and the command line"
