# majordomus-covers: none
# /ui/peers shows the blackboard and is read again when it changes (ADR 0176, I2983).
#
# Who works on what was on the blackboard and on no UI screen. What is decided here:
#
#   1. share/ui/live.js reads the screen again on every snapshot, a connection's first one
#      included (the blackboard may have changed between the page and the channel), and its
#      state line says live or reconnecting; ui.js reads one screen at a time, so an earlier
#      answer that arrives last never replaces a later one;
#   2. on a running server, /ui/peers lists a worker announced over MCP with its intent and
#      its claim, names the channel it follows and the module that follows it, and that
#      module and the one socket client are served beside ui.js — the client byte for byte
#      the Cockpit's;
#   3. the screen read again after a second announcement shows it at once: it reads the
#      blackboard the live channel reads, not a reading from before the change.
. "$ROOT/test/lib.sh"
command -v node >/dev/null 2>&1 || skip "no node"
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v curl >/dev/null 2>&1 || skip "curl not installed"

# ---------------------------------------------------------------- 1. the module
M="$T/modules"
mkdir -p "$M"
cp "$ROOT/share/ui/live.js" "$ROOT/share/ui/socket.js" "$ROOT/share/ui/ui.js" "$M/"
cat > "$M/run.mjs" <<'JS'
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
globalThis.window = { location: { href: 'http://127.0.0.1:8741/ui/peers' } };
const { followScreen, SAID } = await import('./live.js');
const fail = (why) => { console.error(why); process.exit(1); };
let reads = 0;
let line = '';
const followed = followScreen('/events?follow=blackboard&frames=snapshot', {
  refresh: () => { reads += 1; },
  say: (text) => { line = text; },
});
if (line !== SAID.connecting) fail(`the line does not say it is connecting: ${line}`);
let s = sockets.at(-1);
if (new URL(s.url).searchParams.get('frames') !== 'snapshot') fail(`not the screen's channel: ${s.url}`);
s.emit('open');
s.send({ type: 'stream.ready', data: { heartbeat_seconds: 20, scope: 'blackboard', epoch: 'e1' } });
s.send({ type: 'blackboard.snapshot', data: { trigger: 'connect', blackboard: {} } });
if (reads !== 1) fail(`the connection's snapshot did not read the screen again: ${reads}`);
if (line !== SAID.live || !followed.live()) fail(`the line does not say live: ${line}`);
s.send({ type: 'blackboard.snapshot', data: { trigger: 'change', blackboard: {} } });
if (reads !== 2) fail(`a change did not read the screen again: ${reads}`);
s.send({ type: 'blackboard.snapshot', data: { trigger: 'refresh', blackboard: {} } });
if (reads !== 3) fail(`a refresh that found a change did not read the screen again: ${reads}`);
s.close();
if (line !== SAID.reconnecting || followed.live()) fail(`a dropped channel does not say reconnecting: ${line}`);
advance(1000);
s = sockets.at(-1);
if (sockets.length !== 2) fail('a dropped channel did not reconnect');
s.emit('open');
s.send({ type: 'stream.ready', data: { heartbeat_seconds: 20, scope: 'blackboard', epoch: 'e1' } });
if (reads !== 3) fail(`a reconnection read the screen before its snapshot: ${reads}`);
s.send({ type: 'blackboard.snapshot', data: { trigger: 'connect', blackboard: {} } });
if (reads !== 4) fail(`a reconnection did not read the screen once: ${reads}`);

// ui.js: one reading at a time. Two changes in quick succession: the second waits for the
// first and is read after it, so the later screen is the one that stays
const ui = (await import('node:fs')).readFileSync(new URL('./ui.js', import.meta.url), 'utf8');
const from = ui.indexOf('const REFRESH_MS');
const to = ui.indexOf('// A snapshot (ADR 0141)', from);
if (from < 0 || to < 0 || !ui.slice(from, to).includes('// One reading at a time.')) {
  fail('ui.js has no one-reading-at-a-time refresh');
}
const requests = [];
let shown = 'G0';
const screenOf = (revision) => ({ revision, replaceWith(next) { main.current = next; shown = next.revision; } });
const main = { current: screenOf('G0'), querySelector: () => null, get lastElementChild() { return this.current; } };
const { runInNewContext } = await import('node:vm');
const read = runInNewContext(ui.slice(from, to) + '\nrefresh;', {
  document: {
    visibilityState: 'visible', getElementById: () => main,
    // a screen whose revision says so carries a source line that is still catching up
    querySelector: (selector) => (selector.includes('data-catching-up') && shown.endsWith('~') ? {} : null),
    createElement: () => ({ set innerHTML(v) { this.firstElementChild = screenOf(v); } }),
  },
  window: { location: { pathname: '/ui/peers', search: '' } },
  AbortController, inFlight: new Set(), PANEL_TIMEOUT_MS: 15000,
  setTimeout, clearTimeout, enhance: () => {},
  fetch: (url) => new Promise((resolve) => requests.push({ url, resolve })),
});
const first = read();
const second = read();
await Promise.resolve();
if (requests.length !== 1) fail(`two readings were in flight at once: ${requests.length}`);
requests[0].resolve({ ok: true, text: async () => 'G1' });
for (let i = 0; i < 20 && requests.length < 2; i++) await new Promise((r) => setImmediate(r));
if (requests.length !== 2) fail('the reading asked for during the first was not made after it');
requests[1].resolve({ ok: true, text: async () => 'G2' });
await first; await second;
if (shown !== 'G2') fail(`the screen shows ${shown}, not the later reading`);
if (!/[?&]fragment(&|$)/.test(requests[1].url)) fail(`the reading is not the fragment: ${requests[1].url}`);

// A reading that comes back with a value the server is still reading again is read again by
// itself, whoever asked for it: a change frame's reading on a loaded server can come back
// before the blackboard was read at that change, and no other frame may follow it
const settle = async () => { for (let i = 0; i < 20; i++) await new Promise((r) => setImmediate(r)); };
requests.length = 0;
const third = read();
await Promise.resolve();
requests[0].resolve({ ok: true, text: async () => 'G3~' });
await third;
if (shown !== 'G3~') fail(`the catching-up screen is not shown: ${shown}`);
advance(400);
await settle();
if (requests.length !== 2) fail('a screen that came back catching up was not read again');
requests[1].resolve({ ok: true, text: async () => 'G4' });
await settle();
advance(10000);
await settle();
if (shown !== 'G4') fail(`the screen shows ${shown}, not the reading that caught up`);
if (requests.length !== 2) fail(`a screen that caught up was read again: ${requests.length} readings`);
console.log('live.js: a change and a reconnection read the screen again');
JS
node "$M/run.mjs" > "$T/live.out" 2>&1 || { echo "    live.js:"; sed 's/^/    | /' "$T/live.out"; exit 1; }
grep -q '^live.js: a change and a reconnection' "$T/live.out" || { echo "    live.js did not finish"; cat "$T/live.out"; exit 1; }
echo "    live.js reads the screen again on every snapshot; ui.js reads one screen at a time and reads a catching-up screen again"

# ---------------------------------------------------------------- 2. the screen
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"
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
PIDS="$PIDS $!"
ready() { [ -n "$(jq -r '.url // empty' "$R/.ai/local/state/mcp/server.json" 2>/dev/null)" ]; }
i=0; until ready; do i=$((i + 1)); [ "$i" -lt 200 ] || { cat "$T/serve.log"; exit 1; }; sleep 0.2; done
url="$(jq -r '.url' "$R/.ai/local/state/mcp/server.json")"

mcp() { # <session or ""> <json>
  if [ -n "$1" ]; then
    curl -fsS --max-time 30 -X POST "$url/mcp" -H "mcp-session-id: $1" \
      -H 'content-type: application/json' -H 'accept: application/json, text/event-stream' -d "$2"
  else
    curl -fsS --max-time 30 -D "$T/headers" -X POST "$url/mcp" \
      -H 'content-type: application/json' -H 'accept: application/json, text/event-stream' -d "$2"
  fi
}
mcp "" '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case-3383","version":"1"}}}' >/dev/null \
  || { echo "    the MCP client could not attach"; exit 1; }
SID="$(tr -d '\r' < "$T/headers" | awk 'tolower($1)=="mcp-session-id:" {print $2}')"
announce() {
  mcp "$SID" "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"majordomus_announce\",\"arguments\":{\"intent\":\"$1\",\"scope\":[\"$2\"]}}}" >/dev/null \
    || { echo "    the MCP client could not announce"; exit 1; }
}
announce "the first reading of 3383" "docs"

curl -fsS --max-time 120 "$url/ui/peers" > "$T/peers.html" || { echo "    /ui/peers did not answer"; exit 1; }
for want in '<title>Peers' 'the first reading of 3383' '>docs<' 'case-3383' \
  'data-follow="/events?follow=blackboard&amp;frames=snapshot"' 'data-follow-state' 'data-follow-lib="/ui/assets/live.js'; do
  grep -qF "$want" "$T/peers.html" || { echo "    /ui/peers lacks $want"; exit 1; }
done
curl -fsS --max-time 30 "$url/ui" | grep -qF 'href="/ui/peers"' || { echo "    the navigation does not lead to /ui/peers"; exit 1; }
echo "    /ui/peers lists the announced worker with its intent and claim, and names its channel"

LIB="$(grep -o 'data-follow-lib="[^"]*"' "$T/peers.html" | head -1 | sed 's/^data-follow-lib="//; s/"$//; s/&amp;/\&/g')"
curl -fsS --max-time 30 "$url$LIB" > "$T/live.js" || { echo "    $LIB is not served"; exit 1; }
grep -q 'export function followScreen' "$T/live.js" || { echo "    $LIB is not live.js"; exit 1; }
curl -fsS --max-time 30 "$url/ui/assets/socket.js" > "$T/socket.js" || { echo "    the socket client is not served beside ui.js"; exit 1; }
cmp -s "$T/socket.js" "$ROOT/share/cockpit/socket.js" \
  || { echo "    the UI's socket client is not the Cockpit's byte for byte"; exit 1; }
echo "    live.js and the one socket client are served beside ui.js"

# ---------------------------------------------------------------- 3. read again, at once
announce "the second reading of 3383" "test"
curl -fsS --max-time 120 "$url/ui/peers?fragment" > "$T/fragment.html" || { echo "    the screen's fragment did not answer"; exit 1; }
grep -qF 'the second reading of 3383' "$T/fragment.html" \
  || { echo "    the screen read after a change does not show it:"; grep -o 'reading of 3383' "$T/fragment.html" | head; exit 1; }
echo "    the screen read again after a change shows it at once"
echo "    /ui/peers shows the blackboard and is read again when it changes"
