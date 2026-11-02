# majordomus-covers: none
# The monitor page is pushed over the live channel, and says when it polls (ADR 0176, I2982).
#
# /cockpit/monitor used to ask the whole monitor every five seconds per tab. It now renders the
# blackboard channel with its own cursor and epoch, and the server pushes what is new. What is
# decided here:
#
#   1. share/cockpit/socket.js resumes a blackboard channel with the cursor of the last signals
#      frame and the epoch of stream.ready, never one without the other, and hands a
#      stream.resync to the page, also when a new epoch is followed by no resync;
#   2. share/cockpit/monitor.js adds a pushed signal as a row without asking /api/v1/monitor,
#      and polls — saying so — only when the channel does not open;
#   3. on a running server, the page names a channel that continues from its own cursor, and a
#      review request that names the reader reaches it marked for that reader, with no snapshot
#      frame on a channel that asked for signals only.
. "$ROOT/test/lib.sh"
mj_runs node || skip "$(mj_runs_why node)"
mj_runs python3 || skip "$(mj_runs_why python3)"
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v curl >/dev/null 2>&1 || skip "curl not installed"

# ---------------------------------------------------------------- 1. the client resumes
CHECK="$T/socket.out"
node --input-type=module - "$ROOT/share/cockpit/socket.js" > "$CHECK" 2>&1 <<'NODE' \
  || { echo "    socket.js does not resume a blackboard channel:"; sed 's/^/    | /' "$CHECK"; exit 1; }
import { readFileSync } from 'node:fs';
const source = readFileSync(process.argv[2], 'utf8');
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
globalThis.WebSocket = FakeSocket;
globalThis.window = { location: { href: 'http://127.0.0.1:8741/cockpit/monitor' } };
const { follow } = await import('data:text/javascript,' + encodeURIComponent(source));
const fail = (why) => { console.error(why); process.exit(1); };
const eq = (a, b, why) => {
  if (JSON.stringify(a) !== JSON.stringify(b)) fail(`${why}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`);
};
const param = (s, name) => new URL(s.url).searchParams.get(name);

const got = [];
const resyncs = [];
follow('/events?follow=blackboard&frames=signals&after=m1.a&epoch=e1', {
  message: (f) => got.push(f.data.cursor),
  resync: (why) => resyncs.push(why),
});
let s = sockets.at(-1);
eq([param(s, 'after'), param(s, 'epoch')], ['m1.a', 'e1'], 'the first connection resumes from the page');
s.emit('open');
s.send({ type: 'stream.ready', data: { heartbeat_seconds: 20, scope: 'blackboard', epoch: 'e1' } });
s.send({ type: 'blackboard.signals', data: { trigger: 'change', signals: [{}], cursor: 'm2.b' } });
eq(got, ['m2.b'], 'a signals frame is handed over');

// the connection drops: the next one resumes from the last frame's cursor and the epoch
s.close();
advance(1000);
s = sockets.at(-1);
eq(sockets.length, 2, 'a dropped channel reconnects');
eq([param(s, 'after'), param(s, 'epoch'), param(s, 'frames')], ['m2.b', 'e1', 'signals'],
  'from the cursor of the last frame, with the epoch that issued it, asking the same frames');

// another server process answers: its epoch replaces the old one, and its resync reaches the page
s.emit('open');
s.send({ type: 'stream.ready', data: { heartbeat_seconds: 20, scope: 'blackboard', epoch: 'e2' } });
s.send({ type: 'stream.resync', data: { reason: 'the cursor was issued by another server process' } });
if (!resyncs.some((r) => r.includes('another server process'))) fail(`the page was not told: ${resyncs}`);
s.send({ type: 'blackboard.signals', data: { trigger: 'change', signals: [], cursor: 'm9.c' } });
s.close();
advance(1000);
s = sockets.at(-1);
eq([param(s, 'after'), param(s, 'epoch')], ['m9.c', 'e2'], 'a reconnect after a resync resumes with the new epoch');

// a server of another epoch answers and the connection drops before its resync: the cursor
// of the old epoch is never offered with the new one, and the page is told at once
s.emit('open');
s.send({ type: 'stream.ready', data: { heartbeat_seconds: 20, scope: 'blackboard', epoch: 'e4' } });
if (!resyncs.some((r) => r.includes('epoch changed'))) fail(`a new epoch was not reported: ${resyncs}`);
s.close();
advance(1000);
s = sockets.at(-1);
eq([param(s, 'after'), param(s, 'epoch')], [null, null], 'an interrupted handshake offers no cursor of another epoch');

// a resync with no frame after it leaves no cursor: the next connection asks for a fresh start
s.emit('open');
s.send({ type: 'stream.ready', data: { heartbeat_seconds: 20, scope: 'blackboard', epoch: 'e3' } });
s.send({ type: 'stream.resync', data: { reason: 'gone' } });
s.close();
advance(1000);
s = sockets.at(-1);
eq([param(s, 'after'), param(s, 'epoch')], [null, null], 'a refused cursor is not offered again');

// a page rendered before anything happened carries an empty cursor: the start of the stream,
// which every connection resumes from until a frame names a later one
follow('/events?follow=blackboard&frames=signals&after=&epoch=e5', { resync: () => {} });
s = sockets.at(-1);
eq([param(s, 'after'), param(s, 'epoch')], ['', 'e5'], 'an empty cursor is offered as the start');
s.emit('open');
s.send({ type: 'stream.ready', data: { heartbeat_seconds: 20, scope: 'blackboard', epoch: 'e5' } });
s.close();
advance(1000);
s = sockets.at(-1);
eq([param(s, 'after'), param(s, 'epoch')], ['', 'e5'], 'a reconnection keeps the start of the stream');
console.log('socket.js: a blackboard channel resumes by its cursor and epoch and reports a resync');
NODE
grep -q '^socket.js: a blackboard channel resumes' "$CHECK" \
  || { echo "    the client check did not reach its end:"; sed 's/^/    | /' "$CHECK"; exit 1; }
echo "    socket.js resumes a blackboard channel by its cursor and epoch and reports a resync"

# ---------------------------------------------------------------- 2. the page is pushed
# monitor.js against a DOM, a channel and an HTTP client this case plays: the modules are
# copied next to a cockpit.js that records every request.
M="$T/modules"
mkdir -p "$M"
cp "$ROOT/share/cockpit/socket.js" "$ROOT/share/cockpit/monitor.js" "$M/"
cat > "$M/cockpit.js" <<'JS'
export const asked = [];
export async function api(url) {
  asked.push(url);
  return { ok: true, body: { signals: [{ at: 1, source: 'mesh', summary: 'polled' }], cursor: 'm5.p', more: false } };
}
JS
cat > "$M/run.mjs" <<'JS'
const mode = process.argv[2];
let timers = [];
globalThis.setTimeout = (fn, ms) => { timers.push({ ms, fn }); return timers.length; };
// the page's own wait for the channel to open; the socket's watchdog is not this case's
const opening = () => timers.filter((t) => t.ms === 5000).forEach((t) => t.fn());
globalThis.clearTimeout = () => {};
const sockets = [];
class FakeSocket {
  constructor(url) { this.url = url; this.listeners = {}; this.closed = false; sockets.push(this); }
  addEventListener(name, fn) { (this.listeners[name] ||= []).push(fn); }
  emit(name, event) { for (const fn of this.listeners[name] || []) fn(event); }
  send(frame) { this.emit('message', { data: JSON.stringify(frame) }); }
  close() { this.closed = true; }
}
globalThis.WebSocket = FakeSocket;
const element = (tag) => ({
  tag, children: [], textContent: '', dataset: {}, className: '',
  append(...c) { this.children.push(...c); },
  prepend(c) { this.children.unshift(c); },
  replaceWith() {},
  get rows() { return this.children; },
  deleteRow() { this.children.pop(); },
});
const tbody = element('tbody');
const note = element('p');
const frame = element('div');
frame.dataset = {
  mjMonitor: '/api/v1/monitor?limit=200',
  mjMonitorCursor: 'm1.a',
  mjMonitorLive: '/events?follow=blackboard&frames=signals&after=m1.a&epoch=e1',
};
frame.querySelector = (q) => (q === 'tbody' ? tbody : q === '[data-mj-monitor-note]' ? note : null);
globalThis.document = {
  visibilityState: 'visible',
  querySelector: (q) => (q === '[data-mj-monitor]' ? frame : null),
  createElement: element,
  addEventListener() {},
};
globalThis.window = {
  location: { href: 'http://127.0.0.1:8741/cockpit/monitor' },
  setTimeout: globalThis.setTimeout,
  setInterval: () => 1,
  clearInterval() {},
};
const { asked } = await import('./cockpit.js');
await import('./monitor.js');
const fail = (why) => { console.error(why); process.exit(1); };
const s = sockets.at(-1);
if (!s) fail('the page opened no channel');
if (mode === 'pushed') {
  s.emit('open');
  s.send({ type: 'stream.ready', data: { heartbeat_seconds: 20, scope: 'blackboard', epoch: 'e1' } });
  s.send({ type: 'blackboard.signals', data: { trigger: 'change', cursor: 'm2.b',
    signals: [{ at: 7, source: 'mesh', severity: 'action', for_you: true, summary: 'pushed' }] } });
  // the open-timeout passes: a channel that opened is kept
  opening();
  if (asked.length) fail(`the page polled although the channel is live: ${asked}`);
  if (tbody.children.length !== 1) fail(`the pushed signal is not a row: ${tbody.children.length}`);
  const tr = tbody.children[0];
  if (tr.dataset.mjForYou !== 'yes') fail('the row is not marked for the reader');
  if (!/^Live: /.test(note.textContent)) fail(`the note does not say live: ${note.textContent}`);
} else if (mode === 'gap') {
  // the server cannot continue from the page's cursor: what follows is live, and the page keeps
  // saying that something before it may be missing, however many signals arrive after
  s.emit('open');
  s.send({ type: 'stream.ready', data: { heartbeat_seconds: 20, scope: 'blackboard', epoch: 'e1' } });
  s.send({ type: 'stream.resync', data: { reason: 'more than 10 pages of signals' } });
  s.send({ type: 'blackboard.signals', data: { trigger: 'change', cursor: 'm3.c',
    signals: [{ at: 8, source: 'mesh', summary: 'after the gap' }] } });
  if (!/^Live: /.test(note.textContent) || !/may be missing/.test(note.textContent)) {
    fail(`the gap is not kept with the live state: ${note.textContent}`);
  }
} else {
  // the channel never opens
  opening();
  await new Promise((r) => setImmediate(r));
  if (!s.closed) fail('the channel that never opened was not closed');
  if (asked.length !== 1 || !asked[0].includes('after=m1.a')) fail(`the page did not poll from its cursor: ${asked}`);
  if (!/^Polling /.test(note.textContent)) fail(`the note does not say it polls: ${note.textContent}`);
}
console.log(`monitor.js: ${mode}`);
JS
for mode in pushed gap refused; do
  node "$M/run.mjs" "$mode" > "$T/page-$mode.out" 2>&1 \
    || { echo "    monitor.js ($mode):"; sed 's/^/    | /' "$T/page-$mode.out"; exit 1; }
  grep -q "^monitor.js: $mode" "$T/page-$mode.out" || { echo "    monitor.js ($mode) did not finish"; cat "$T/page-$mode.out"; exit 1; }
done
echo "    monitor.js adds a pushed signal without polling, keeps saying a gap it was told of, and polls with a label only when the channel does not open"

# ---------------------------------------------------------------- 3. on a running server
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
  printf 'multicast:\n  enabled: false\ntailscale:\n  enabled: false\ntrust:\n  policy: tofu\n' >> .ai/repo/mesh/majordomus.yaml
  printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 4\n  repository: case-3382\n' >> .ai/repo/mesh/majordomus.yaml
  git add -A && git commit -qm base
)
mkdir -p "$T/state" "$T/bin"
printf '#!/bin/sh\necho "gh: not here in this case" >&2\nexit 4\n' > "$T/bin/gh"
chmod +x "$T/bin/gh"
( cd "$R" && PATH="$T/bin:$PATH" XDG_STATE_HOME="$T/state" exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$T/serve.log" 2>&1 ) &
PIDS="$PIDS $!"
ready() { [ -n "$(jq -r '.url // empty' "$R/.ai/local/state/mcp/server.json" 2>/dev/null)" ]; }
i=0; until ready; do i=$((i + 1)); [ "$i" -lt 200 ] || { cat "$T/serve.log"; exit 1; }; sleep 0.2; done
url="$(jq -r '.url' "$R/.ai/local/state/mcp/server.json")"
cd "$R" || exit

curl -fsS --max-time 60 "$url/cockpit/monitor?for=zq7" > "$T/monitor.html" || { echo "    /cockpit/monitor did not answer"; exit 1; }
LIVE="$(python3 -c 'import html,re,sys; m=re.search(r"data-mj-monitor-live=\"([^\"]+)\"", open(sys.argv[1]).read()); print(html.unescape(m.group(1)) if m else "")' "$T/monitor.html")"
[ -n "$LIVE" ] || { echo "    the page names no live channel"; grep -o 'data-mj-monitor[^>]*' "$T/monitor.html" | head; exit 1; }
for want in follow=blackboard frames=signals for=zq7; do
  case "$LIVE" in *"$want"*) ;; *) echo "    the page's channel lacks $want: $LIVE"; exit 1 ;; esac
done
CURSOR="$(python3 -c 'import html,re,sys; m=re.search(r"data-mj-monitor-cursor=\"([^\"]*)\"", open(sys.argv[1]).read()); print(html.unescape(m.group(1)) if m else "")' "$T/monitor.html")"
python3 - "$LIVE" "$CURSOR" <<'PY' || exit 1
import sys, urllib.parse
live, cursor = sys.argv[1], sys.argv[2]
q = urllib.parse.parse_qs(urllib.parse.urlsplit(live).query, keep_blank_values=True)
# an empty cursor is the start of the stream and is named too: a page rendered before
# anything happened misses nothing that happens before its channel opens
if q.get("after") != [cursor] or not q.get("epoch", [""])[0]:
    sys.exit("    the channel does not continue from the page's cursor %r: %s" % (cursor, live))
PY
echo "    the page names a channel that continues from its own cursor and epoch, for its reader"

cat > "$T/follow.py" <<'PY'
import base64, json, os, socket, sys, time
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
wait_for "$T/live.jsonl" '.type == "stream.ready"' 60 || { echo "    the page's channel did not open:"; cat "$T/live.jsonl" "$T/live.err"; exit 1; }
"$RB" mesh session open --session s1 --worker 3d --intent "the reviewed work" >/dev/null 2>&1 \
  || { echo "    a mesh session could not be opened"; exit 1; }
"$RB" mesh review request "3382: zq7, read this before it lands" --session s1 >/dev/null 2>&1 \
  || { echo "    the review could not be requested"; exit 1; }
wait_for "$T/live.jsonl" '.type == "blackboard.signals" and ([.data.signals[] | select(.kind == "review_requested" and .for_you == true)] | length >= 1)' 25 \
  || { echo "    the review was not pushed marked for the reader:"; jq -c '{type, trigger: .data.trigger}' "$T/live.jsonl" 2>/dev/null | tail -8; exit 1; }
kill "$FOLLOWER" 2>/dev/null || true
grep -q '"blackboard.snapshot"' "$T/live.jsonl" && { echo "    a channel that asked for signals was sent a snapshot"; exit 1; }
echo "    a review that names the reader is pushed to the page marked for it, with no snapshot"

python3 "$T/follow.py" "$url" "/events?follow=blackboard&frames=everything" 1 > "$T/odd.jsonl" 2>/dev/null
head -1 "$T/odd.jsonl" | jq -e '.status | test(" 400 ")' >/dev/null \
  || { echo "    a frames value the channel does not take was not refused"; cat "$T/odd.jsonl"; exit 1; }
echo "    the monitor page is pushed over the live channel, and says when it polls"
