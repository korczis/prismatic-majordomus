# majordomus-covers: none
# /cockpit/peers renders the blackboard and follows it live (ADR 0176, I2998).
#
# The Cockpit's Peers page read peers.list once: the boards of one repository's checkouts,
# nothing of the mesh, and never again. It now renders blackboard.snapshot, the projection
# /ui/peers renders, and follows the same snapshot frames. What is decided here:
#
#   1. share/cockpit/peers.js reads the page again from its own address on every snapshot frame,
#      a connection's first one included, one reading at a time, so a later reading is the one
#      that stays; it keeps a page it could not read again and says so, reads a hidden page once
#      it is shown, and its state line says live, reconnecting or offline;
#   2. on a running server with a declared mesh, /cockpit/peers lists a worker announced over MCP
#      with its intent and claim and a mesh session with its intent and exclusive claim, names
#      the blackboard as what it read and the channel it follows, and asks no peers.list; the
#      script it loads and the one socket client that script imports are served;
#   3. a second announcement is pushed on the page's own channel as a snapshot frame caused by
#      the change, and the page read again after that frame shows it, without a reload.
. "$ROOT/test/lib.sh"
command -v node >/dev/null 2>&1 || skip "no node"
command -v python3 >/dev/null 2>&1 || skip "no python3"
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v curl >/dev/null 2>&1 || skip "curl not installed"

# ---------------------------------------------------------------- 1. the script
M="$T/modules"
mkdir -p "$M"
cp "$ROOT/share/cockpit/peers.js" "$ROOT/share/cockpit/socket.js" "$M/"
cat > "$M/run.mjs" <<'JS'
const mode = process.argv[2];
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
const sockets = [];
class FakeSocket {
  constructor(url) { this.url = url; this.listeners = {}; this.closed = false; sockets.push(this); }
  addEventListener(name, fn) { (this.listeners[name] ||= []).push(fn); }
  emit(name, event) { for (const fn of this.listeners[name] || []) fn(event); }
  send(frame) { this.emit('message', { data: JSON.stringify(frame) }); }
  close() { this.closed = true; this.emit('close'); }
}
// a runtime of its own may carry a WebSocket, so a browser without one is made explicitly
globalThis.WebSocket = mode === 'no-socket' ? undefined : FakeSocket;

// the page as the server renders it: the blackboard of one reading, with its own state line
const blackboard = (reading) => ({
  reading,
  note: { textContent: 'as rendered' },
  dataset: { mjPeersLive: '/events?follow=blackboard&frames=snapshot' },
  replaceWith(next) { shown = next; },
});
let shown = blackboard('G0');
const listeners = {};
globalThis.document = {
  visibilityState: 'visible',
  querySelector: (q) => (q === '[data-mj-peers]' ? shown : q === '[data-mj-peers-note]' ? shown.note : null),
  addEventListener: (name, fn) => { (listeners[name] ||= []).push(fn); },
};
globalThis.window = {
  location: { href: 'http://127.0.0.1:8741/cockpit/peers', pathname: '/cockpit/peers', search: '' },
};
const requests = [];
globalThis.fetch = (url, options) => new Promise((resolve) => requests.push({ url, options, resolve }));
// a page of a reading holds that reading's blackboard; the page of a failure holds none
globalThis.DOMParser = class {
  parseFromString(text) {
    return { querySelector: (q) => (q === '[data-mj-peers]' && /^G\d+$/.test(text) ? blackboard(text) : null) };
  }
};
const { SAID, UNREAD } = await import('./peers.js');
const fail = (why) => { console.error(why); process.exit(1); };
const settle = async () => { for (let i = 0; i < 20; i++) await new Promise((r) => setImmediate(r)); };
const answer = async (i, body, ok = true) => {
  requests[i].resolve({ ok, status: ok ? 200 : 500, text: async () => body });
  await settle();
};
const line = () => shown.note.textContent;
const snapshot = (trigger) => ({ type: 'blackboard.snapshot', data: { trigger, blackboard: {}, cursor: 'm1' } });

if (mode === 'no-socket') {
  if (line() !== SAID.offline) fail(`a browser with no WebSocket is not said to be offline: ${line()}`);
  if (requests.length) fail('a page that cannot follow its channel read itself');
  console.log('peers.js: no-socket');
  process.exit(0);
}

if (line() !== SAID.connecting) fail(`the line does not say it is connecting: ${line()}`);
let s = sockets.at(-1);
if (!s) fail('the page opened no channel');
const channel = new URL(s.url).searchParams;
if (channel.get('follow') !== 'blackboard' || channel.get('frames') !== 'snapshot') fail(`not the page's channel: ${s.url}`);
s.emit('open');
if (line() !== SAID.live) fail(`an open channel is not said to be live: ${line()}`);
s.send({ type: 'stream.ready', data: { heartbeat_seconds: 20, scope: 'blackboard', epoch: 'e1' } });
if (requests.length) fail('the page was read before a snapshot');

// the connection's first snapshot reads the page again, from its own address, as HTML
s.send(snapshot('connect'));
if (requests.length !== 1) fail(`the connection's first snapshot did not read the page again: ${requests.length}`);
if (requests[0].url !== '/cockpit/peers') fail(`the page was not read from its own address: ${requests[0].url}`);
if (requests[0].options.headers.Accept !== 'text/html') fail('the page was not asked for as HTML');

// two changes while that reading is in flight: one more reading after it, never two at once
s.send(snapshot('change'));
s.send(snapshot('change'));
await settle();
if (requests.length !== 1) fail(`two readings were in flight at once: ${requests.length}`);
await answer(0, 'G1');
if (shown.reading !== 'G1') fail(`the reading did not replace the page's blackboard: ${shown.reading}`);
if (requests.length !== 2) fail(`the changes during a reading were not read after it: ${requests.length}`);
await answer(1, 'G2');
if (shown.reading !== 'G2' || requests.length !== 2) fail(`the later reading is not the one shown: ${shown.reading}`);
if (line() !== SAID.live) fail(`the line of a new reading does not say live: ${line()}`);

// a reading that fails keeps the page and says so; the next one that is read clears it
s.send(snapshot('change'));
await answer(2, 'the capability behind this page did not answer', false);
if (shown.reading !== 'G2') fail('a failed reading replaced the page');
if (line() !== SAID.live + UNREAD) fail(`a failed reading is not said: ${line()}`);
s.send(snapshot('refresh'));
await answer(3, 'a page that holds no blackboard');
if (shown.reading !== 'G2' || line() !== SAID.live + UNREAD) fail(`a page with no blackboard was taken for one: ${line()}`);
s.send(snapshot('change'));
await answer(4, 'G4');
if (shown.reading !== 'G4' || line() !== SAID.live) fail(`a reading after a failure: ${shown.reading}, ${line()}`);

// a frame that is no snapshot reads nothing
s.send({ type: 'blackboard.signals', data: { trigger: 'change', signals: [], cursor: 'm2' } });
s.send({ type: 'stream.heartbeat', data: {} });
if (requests.length !== 5) fail(`a frame that is no snapshot read the page: ${requests.length}`);

// a hidden page is not read; it is read once, when it is shown again
document.visibilityState = 'hidden';
s.send(snapshot('change'));
s.send(snapshot('change'));
if (requests.length !== 5) fail('a hidden page was read');
document.visibilityState = 'visible';
for (const fn of listeners.visibilitychange || []) fn();
if (requests.length !== 6) fail(`a page shown again was not read once: ${requests.length}`);
await answer(5, 'G5');
for (const fn of listeners.visibilitychange || []) fn();
if (requests.length !== 6) fail('a page shown again with nothing missed was read');

// the channel drops: reconnecting, and the reconnection's first snapshot reads the page again
s.close();
if (line() !== SAID.reconnecting) fail(`a dropped channel is not said to be reconnecting: ${line()}`);
advance(1000);
s = sockets.at(-1);
if (sockets.length !== 2) fail('a dropped channel did not reconnect');
s.emit('open');
s.send({ type: 'stream.ready', data: { heartbeat_seconds: 20, scope: 'blackboard', epoch: 'e1' } });
s.send(snapshot('connect'));
if (requests.length !== 7) fail(`a reconnection's first snapshot did not read the page: ${requests.length}`);
await answer(6, 'G6');
if (shown.reading !== 'G6' || line() !== SAID.live) fail(`after a reconnection: ${shown.reading}, ${line()}`);

// the server closes the stream: offline, not live
s.send({ type: 'stream.closing', data: {} });
if (line() !== SAID.offline) fail(`a closed stream is not said to be offline: ${line()}`);
console.log('peers.js: follows');
JS
for mode in follows no-socket; do
  node "$M/run.mjs" "$mode" > "$T/peers-$mode.out" 2>&1 \
    || { echo "    peers.js ($mode):"; sed 's/^/    | /' "$T/peers-$mode.out"; exit 1; }
  grep -q "^peers.js: $mode" "$T/peers-$mode.out" || { echo "    peers.js ($mode) did not finish"; cat "$T/peers-$mode.out"; exit 1; }
done
echo "    peers.js reads the page again on every snapshot, one reading at a time, and says live, reconnecting or offline"

# ---------------------------------------------------------------- 2. the page
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
  printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 4\n  repository: case-3386\n' >> .ai/repo/mesh/majordomus.yaml
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
url="$(jq -r '.url' "$R/.ai/local/state/mcp/server.json")"
cd "$R" || exit

mcp() { # <session or ""> <json>
  if [ -n "$1" ]; then
    curl -fsS --max-time 30 -X POST "$url/mcp" -H "mcp-session-id: $1" \
      -H 'content-type: application/json' -H 'accept: application/json, text/event-stream' -d "$2"
  else
    curl -fsS --max-time 30 -D "$T/headers" -X POST "$url/mcp" \
      -H 'content-type: application/json' -H 'accept: application/json, text/event-stream' -d "$2"
  fi
}
mcp "" '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case-3386","version":"1"}}}' >/dev/null \
  || { echo "    the MCP client could not attach"; exit 1; }
SID="$(tr -d '\r' < "$T/headers" | awk 'tolower($1)=="mcp-session-id:" {print $2}')"
announce() {
  mcp "$SID" "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"majordomus_announce\",\"arguments\":{\"intent\":\"$1\",\"scope\":[\"$2\"]}}}" >/dev/null \
    || { echo "    the MCP client could not announce"; exit 1; }
}
announce "the first reading of 3386" "docs"
"$RB" mesh session open --session s1 --worker 3d --intent "the mesh session of 3386" >/dev/null 2>&1 \
  || { echo "    a mesh session could not be opened"; exit 1; }
"$RB" mesh claim --session s1 --intent "the mesh claim of 3386" src >/dev/null 2>&1 \
  || { echo "    the mesh session could not claim"; exit 1; }

page() { curl -fsS --max-time 120 -H 'Accept: text/html' "$url/cockpit/peers"; }
page > "$T/peers.html" || { echo "    /cockpit/peers did not answer"; exit 1; }
for want in '<title>Peers' 'the first reading of 3386' 'case-3386' '>docs<' \
  'the mesh session of 3386' '>src<' '>exclusive<' '>mesh<' \
  'data-capability="blackboard.snapshot" data-field="workers"' \
  'data-mj-peers-live="/events?follow=blackboard&amp;frames=snapshot"' 'data-mj-peers-note' \
  'src="/cockpit/assets/peers.js?v='; do
  grep -qF "$want" "$T/peers.html" || { echo "    /cockpit/peers lacks $want"; exit 1; }
done
grep -qF 'peers.list' "$T/peers.html" && { echo "    /cockpit/peers names peers.list"; exit 1; }
echo "    /cockpit/peers lists the announced worker and the mesh session from the blackboard, and names its channel"

SCRIPT="$(grep -o 'src="/cockpit/assets/peers.js?v=[^"]*"' "$T/peers.html" | head -1 | sed 's/^src="//; s/"$//; s/&amp;/\&/g')"
curl -fsS --max-time 30 "$url$SCRIPT" > "$T/peers.js" || { echo "    $SCRIPT is not served"; exit 1; }
grep -q 'data-mj-peers' "$T/peers.js" || { echo "    $SCRIPT is not peers.js"; exit 1; }
# the server names an import by its digested URL, beside the module that imports it
SOCKET="$(grep -o "\./socket\.js?v=[0-9a-f]*" "$T/peers.js" | head -1)"
[ -n "$SOCKET" ] || { echo "    peers.js imports no digested socket client"; grep -n 'import' "$T/peers.js"; exit 1; }
curl -fsS --max-time 30 "$url/cockpit/assets/${SOCKET#./}" > "$T/socket.js" || { echo "    $SOCKET is not served"; exit 1; }
cmp -s "$T/socket.js" "$ROOT/share/cockpit/socket.js" \
  || { echo "    the socket client peers.js imports is not share/cockpit/socket.js"; exit 1; }
echo "    peers.js and the one socket client it imports are served"

# ---------------------------------------------------------------- 3. followed, read again
LIVE="$(python3 -c 'import html,re,sys; m=re.search(r"data-mj-peers-live=\"([^\"]+)\"", open(sys.argv[1]).read()); print(html.unescape(m.group(1)) if m else "")' "$T/peers.html")"
[ -n "$LIVE" ] || { echo "    the page names no live channel"; exit 1; }
cat > "$T/follow.py" <<'PY'
import base64, json, os, socket, sys
base, target, limit = sys.argv[1], sys.argv[2], int(sys.argv[3])
host, port = base.split("//")[1].split(":")
s = socket.create_connection((host, int(port)), timeout=60)
key = base64.b64encode(os.urandom(16)).decode()
s.sendall(("GET %s HTTP/1.1\r\nHost: %s:%s\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
           "Sec-WebSocket-Key: %s\r\nSec-WebSocket-Version: 13\r\n\r\n" % (target, host, port, key)).encode())
head = b""
while b"\r\n\r\n" not in head:
    b = s.recv(1)
    if not b:
        break
    head += b
print(json.dumps({"status": head.split(b"\r\n")[0].decode("utf-8", "replace")}), flush=True)
buf = b""
def need(n):
    global buf
    while len(buf) < n:
        chunk = s.recv(65536)
        if not chunk:
            return False
        buf += chunk
    return True
seen = 0
while seen < limit:
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
    print(payload.decode(), flush=True)
    seen += 1
PY
python3 "$T/follow.py" "$url" "$LIVE" 1000 > "$T/live.jsonl" 2>"$T/live.err" &
FOLLOWER=$!; PIDS="$PIDS $FOLLOWER"
wait_for() { # <file> <jq filter> <seconds>
  local n=0
  while [ "$n" -lt $(( $3 * 10 )) ]; do
    grep -v '"status"' "$1" 2>/dev/null | jq -e -s "map(select($2)) | length > 0" >/dev/null 2>&1 && return 0
    sleep 0.1; n=$((n + 1))
  done
  return 1
}
wait_for "$T/live.jsonl" '.type == "blackboard.snapshot" and .data.trigger == "connect"' 60 \
  || { echo "    the page's channel gave no snapshot:"; cat "$T/live.jsonl" "$T/live.err" | head -20; exit 1; }
announce "the second reading of 3386" "test"
wait_for "$T/live.jsonl" '.type == "blackboard.snapshot" and .data.trigger == "change" and ([.data.blackboard.workers[]? | select(.intent == "the second reading of 3386")] | length == 1)' 25 \
  || { echo "    the second announcement was not pushed on the page's channel:"; jq -c '{type, trigger: .data.trigger}' "$T/live.jsonl" 2>/dev/null | tail -8; exit 1; }
kill "$FOLLOWER" 2>/dev/null || true
grep -q '"blackboard.signals"' "$T/live.jsonl" && { echo "    the page's channel, which asks for snapshots, was sent signals"; exit 1; }
page > "$T/again.html" || { echo "    /cockpit/peers did not answer when it was read again"; exit 1; }
grep -qF 'the second reading of 3386' "$T/again.html" \
  || { echo "    the page read again after the frame does not show the change:"; grep -o 'reading of 3386' "$T/again.html" | head; exit 1; }
echo "    a second announcement is pushed on the page's channel, and the page read again after it shows it"
echo "    /cockpit/peers renders the blackboard and follows it live"
