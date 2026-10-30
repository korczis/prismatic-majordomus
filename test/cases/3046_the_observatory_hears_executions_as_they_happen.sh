# majordomus-covers: none
# claims: observatory-draws-nothing-it-did-not-read
# The Observatory hears executions as they happen: its executions scene is brought forward by
# the live channel every client of this process reads (`/events`), not by reading its page
# again, and it says truthfully whether that channel is live (ADR 0158, amended by #1159).
#
#   the reader   share/ui/observatory-stream.js, run without a browser: the same snapshot and
#                the same events give the same state, a duplicate is dropped, a gap or a
#                `stream.lagged` reads the snapshot again rather than inventing a lane, a channel
#                silent for 2.5 heartbeats is retired, the word goes LIVE → RECONNECTING →
#                STALE, nothing moves while it is down, and the client never writes a frame
#   the page     /ui/observatory carries the executions scene and names the channel, the
#                snapshot and the script; it does not name a path the script made up
#   a server     the same reader against a real `majordomus serve`: an execution started over
#                HTTP arrives over /events in the reader's state with no read of the page or
#                the list after the first; the server is stopped and the word goes STALE;
#                it is started again on the same port and the reader converges on its list
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
command -v node >/dev/null 2>&1 || skip "no node"
command -v jq >/dev/null 2>&1 || skip "no jq"
STREAM="$ROOT/share/ui/observatory-stream.js"

# ---------------------------------------------------------------- the reader, no browser
UNIT="$(mktemp "${TMPDIR:-/tmp}/mj3046-unit.XXXXXX")"
node - "$STREAM" > "$UNIT" 2>&1 <<'NODE' || { echo "    the Observatory's reader does not keep the picture true:"; sed 's/^/    | /' "$UNIT"; rm -f "$UNIT"; exit 1; }
const path = process.argv[2];
const S = require(path);
const fail = (why) => { console.error(why); process.exit(1); };
const eq = (a, b, why) => { if (JSON.stringify(a) !== JSON.stringify(b)) fail(`${why}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`); };
const ev = (id, sequence, type, data) => ({ type: type || 'execution.log', execution_id: id, sequence, data: data || {} });

// ---- the reducer: replay determinism, duplicates, gaps, a new execution
const snap = [{ id: 'x-1', capability: 'health.report', state: 'running', last_sequence: 3, event_count: 3 }];
const events = [ev('x-1', 4), ev('x-1', 5, 'execution.completed'), ev('x-2', 1, 'execution.created', { capability: 'plan.status' }), ev('x-2', 2, 'execution.started')];
const run = (list) => list.reduce((st, f) => S.reduce(st, f).state, S.fromSnapshot(snap));
const a = run(events);
const b = run(events);
eq(a, b, 'the same snapshot and the same events give different states');
eq(S.lanesOf(a).map((l) => [l.id, l.state, l.last_sequence]), [['x-2', 'running', 2], ['x-1', 'succeeded', 5]], 'the lanes after the events');
// every event delivered twice, and again out of order after it was applied: the same state
const doubled = run(events.flatMap((e) => [e, e]).concat([ev('x-1', 4), ev('x-2', 1)]));
eq(doubled, a, 'duplicates changed the state');
eq(S.reduce(a, ev('x-1', 5)).outcome, 'duplicate', 'a repeated sequence is not a duplicate');
// a gap leaves the state untouched and says so
const g = S.reduce(S.fromSnapshot(snap), ev('x-1', 6));
eq(g.outcome, 'gap', 'a skipped sequence is not a gap');
eq(g.state, S.fromSnapshot(snap), 'a gap changed the state');
// the middle of an execution the snapshot did not list is a gap, not an invented lane
eq(S.reduce(S.fromSnapshot([]), ev('x-9', 4)).outcome, 'gap', 'a lane was invented from the middle of a stream');
// out of order arrival: 2 before 1 is a gap, then 1 and 2 converge
let o = S.fromSnapshot([]);
eq(S.reduce(o, ev('x-3', 2)).outcome, 'gap', 'out-of-order arrival was applied');
o = S.reduce(o, ev('x-3', 1, 'execution.created', { capability: 'c' })).state;
o = S.reduce(o, ev('x-3', 2, 'execution.started')).state;
eq(S.lanesOf(o)[0], { id: 'x-3', capability: 'c', state: 'running', last_sequence: 2, events: 2 }, 'out of order did not converge');
// a frame that is not an execution event is ignored
eq(S.reduce(a, { type: 'stream.heartbeat', data: {} }).outcome, 'ignored', 'a heartbeat was folded');

// ---- the channel, against a clock and a socket this script controls
let now = 0; let timers = []; let next = 1;
const setT = (fn, ms) => { const id = next++; timers.push({ id, at: now + Math.max(0, ms || 0), fn }); return id; };
const clearT = (id) => { timers = timers.filter((t) => t.id !== id); };
async function advance(ms) {
  const until = now + ms;
  for (;;) {
    timers.sort((x, y) => x.at - y.at || x.id - y.id);
    const due = timers[0];
    if (!due || due.at > until) break;
    timers.shift(); now = due.at; due.fn();
    await new Promise((r) => setImmediate(r));
  }
  now = until;
  await new Promise((r) => setImmediate(r));
}
Math.random = () => 0.5;
const sockets = [];
class Fake {
  constructor(url) { this.url = url; this.l = {}; this.closed = false; this.sent = []; sockets.push(this); }
  addEventListener(n, f) { (this.l[n] ||= []).push(f); }
  emit(n, e) { for (const f of this.l[n] || []) f(e); }
  say(frame) { this.emit('message', { data: JSON.stringify(frame) }); }
  send(x) { this.sent.push(x); }
  close() { this.closed = true; }
}
let snapshots = 0;
let listing = [{ id: 'x-1', capability: 'health.report', state: 'running', last_sequence: 1, event_count: 1 }];
const fetchFake = async (url) => { snapshots += 1; if (!url.endsWith('/api/v1/executions')) fail(`asked ${url}`); return { ok: true, json: async () => ({ executions: listing }) }; };
const words = []; const changes = [];
const ch = S.connect({ base: 'http://127.0.0.1:9/ui/observatory', path: '/events', snapshot: '/api/v1/executions', WebSocket: Fake, fetch: fetchFake, setTimeout: setT, clearTimeout: clearT, now: () => now, staleAfterMs: 5000, on: { status: (w) => words.push(w), change: (_s, e) => changes.push(e && e.sequence) } });
(async () => {
  eq(sockets.length, 1, 'no socket was opened');
  eq(sockets[0].url, 'ws://127.0.0.1:9/events', 'the channel is not the one the page named');
  sockets[0].say({ type: 'stream.ready', data: { heartbeat_seconds: 20 } });
  await advance(0);
  eq(ch.status(), 'live', 'ready did not make the channel live');
  eq(snapshots, 1, 'the snapshot was not read once on ready');
  sockets[0].say(ev('x-1', 2, 'execution.completed'));
  eq(ch.lanes()[0].state, 'succeeded', 'a pushed event was not folded');
  eq(snapshots, 1, 'an event caused a read: the scene polls');
  // a gap reads the snapshot again, never guesses
  listing = [{ id: 'x-1', capability: 'health.report', state: 'succeeded', last_sequence: 7, event_count: 7 }];
  sockets[0].say(ev('x-1', 9));
  await advance(0);
  eq(snapshots, 2, 'a gap did not read the snapshot again');
  eq(ch.lanes()[0].last_sequence, 7, 'the re-snapshot did not replace the picture');
  // lagged: the same
  sockets[0].say({ type: 'stream.lagged', data: { dropped: 3 } });
  await advance(0);
  eq(snapshots, 3, 'stream.lagged did not read the snapshot again');
  // silence for 2.5 heartbeats: the channel is dead whatever its socket says
  const before = changes.length;
  await advance(50000);
  if (!sockets[0].closed) fail('a silent channel was not retired');
  if (!words.includes('reconnecting')) fail(`a lost channel did not say RECONNECTING: ${words}`);
  // no reconnect succeeds for a while: STALE, and nothing was folded or pulsed meanwhile
  for (const s of sockets.slice(1)) s.emit('close', {});
  await advance(6000);
  for (const s of sockets.slice(1)) s.emit('close', {});
  await advance(6000);
  if (ch.status() !== 'stale') fail(`a channel lost for longer than the bound is '${ch.status()}', not stale`);
  eq(changes.length, before, 'something changed while disconnected');
  // backoff with jitter grows, bounded
  // a reconnect converges: ready again, the snapshot read again
  listing = [{ id: 'x-1', capability: 'health.report', state: 'succeeded', last_sequence: 7, event_count: 7 }, { id: 'x-5', capability: 'plan.status', state: 'running', last_sequence: 2, event_count: 2 }];
  await advance(31000);
  const last = sockets[sockets.length - 1];
  last.say({ type: 'stream.ready', data: { heartbeat_seconds: 20 } });
  await advance(0);
  eq(ch.status(), 'live', 'a reconnect did not make the channel live again');
  eq(ch.lanes().map((l) => l.id), ['x-1', 'x-5'], 'the reconnect did not converge on the snapshot');
  // read-only: no frame was ever written
  if (sockets.some((s) => s.sent.length)) fail('the client wrote a frame to the channel');
  ch.close();
  console.log(`ok: reducer deterministic, duplicates and gaps handled, channel live/reconnecting/stale, ${sockets.length} sockets, ${snapshots} snapshot reads`);
})().catch((e) => fail(String(e && e.stack || e)));
NODE
rm -f "$UNIT"

RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
unset MAJORDOMUS_HTTP_HOST
"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

# ---------------------------------------------------------------- the page names its channel
SRV=""
trap 'serve_down' EXIT
serve_up serve.out serve.err || exit 1
curl -s --max-time 120 -o obs.html "$U/ui/observatory" || { echo "    /ui/observatory did not answer"; exit 1; }
grep -qF 'data-observatory-events="/events"' obs.html || { echo "    the page does not name the live channel /events"; exit 1; }
grep -qF 'data-observatory-executions="/api/v1/executions"' obs.html || { echo "    the page does not name the snapshot the channel brings forward"; exit 1; }
grep -qE 'data-observatory-stream="/ui/assets/observatory-stream\.js' obs.html || { echo "    the page does not name the script that follows the channel"; exit 1; }
code="$(curl -s -o stream.js -w '%{http_code}' "$U/ui/assets/observatory-stream.js")"
[ "$code" = 200 ] && cmp -s stream.js "$STREAM" || { echo "    /ui/assets/observatory-stream.js answered $code, not the script the repository carries"; exit 1; }
grep -q '"id":"executions"' obs.html || grep -q '&quot;id&quot;:&quot;executions&quot;' obs.html \
  || { echo "    the projection carries no executions scene"; exit 1; }
PORT="${U##*:}"
serve_down

# ---------------------------------------------------------------- the reader against a server
LIVE="$(mktemp "${TMPDIR:-/tmp}/mj3046-live.XXXXXX")"
node - "$STREAM" "$RB" "$PWD" "$PORT" > "$LIVE" 2>&1 <<'NODE' || { echo "    the Observatory does not hear a real server's executions:"; sed 's/^/    | /' "$LIVE"; rm -f "$LIVE"; exit 1; }
const [stream, rb, repo, port] = process.argv.slice(2);
const S = require(stream);
const { spawn } = require('node:child_process');
const fail = (why) => { console.error(why); process.exit(1); };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const base = `http://127.0.0.1:${port}`;
let server = null;
function start() {
  return new Promise((resolve, reject) => {
    const env = { ...process.env }; delete env.MAJORDOMUS_HTTP_HOST;
    server = spawn(rb, ['serve', '--repo', repo, '--port', port], { env, stdio: ['ignore', 'ignore', 'pipe'] });
    let err = '';
    const give = setTimeout(() => reject(new Error(`no listening line in 30 s: ${err.slice(-400)}`)), 30000);
    server.stderr.on('data', (d) => { err += d; if (/listening on http:\/\/127\.0\.0\.1:/.test(err)) { clearTimeout(give); resolve(); } });
    server.on('exit', (c) => { clearTimeout(give); reject(new Error(`server exited ${c}: ${err.slice(-400)}`)); });
  });
}
function stop() { return new Promise((r) => { if (!server) return r(); server.removeAllListeners('exit'); server.on('exit', () => r()); server.kill('SIGTERM'); server = null; }); }
async function until(what, test, ms) { const end = Date.now() + ms; while (Date.now() < end) { if (test()) return; await sleep(100); } fail(`not within ${ms} ms: ${what}`); }
(async () => {
  await start();
  let reads = 0;
  const fetchCounted = (url, init) => { reads += 1; return fetch(url, init); };
  const pulses = [];
  const ch = S.connect({ base: `${base}/ui/observatory`, path: '/events', snapshot: '/api/v1/executions', fetch: fetchCounted, staleAfterMs: 2000,
    on: { change: (_s, e) => { if (e) pulses.push(e.execution_id); } } });
  await until('the channel is live', () => ch.status() === 'live', 15000);
  await until('the first snapshot is read', () => reads === 1, 5000);
  const started = await (await fetch(`${base}/api/v1/executions/start`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ capability: 'executions.demonstrate', input: { steps: 2, delay_ms: 150 } }) })).json();
  const id = started.id || (started.execution && started.execution.id);
  if (!id) fail(`no execution id in ${JSON.stringify(started).slice(0, 300)}`);
  await until(`execution ${id} is heard to succeed`, () => ch.lanes().some((l) => l.id === id && l.state === 'succeeded'), 20000);
  if (reads !== 1) fail(`the scene read the list ${reads} times while events arrived: it polls`);
  if (!pulses.includes(id)) fail('no event of the execution arrived over the channel');
  // the server goes away: the word leaves LIVE, then is STALE, and nothing is heard meanwhile
  await stop();
  await until('the word leaves LIVE', () => ch.status() !== 'live', 60000);
  const heard = pulses.length;
  await until('the word is STALE', () => ch.status() === 'stale', 15000);
  if (pulses.length !== heard) fail('an event was heard from a server that is not running');
  // the server comes back on the same port: the reader converges on what that process lists
  await start();
  await until('the channel is live again', () => ch.status() === 'live', 60000);
  const listed = (await (await fetch(`${base}/api/v1/executions`)).json()).executions.map((e) => e.id);
  await until('the picture converges on the list', () => JSON.stringify(ch.lanes().map((l) => l.id)) === JSON.stringify(listed), 10000);
  ch.close();
  await stop();
  console.log(`ok: ${id} heard over /events, ${reads + 0} list read(s) before the restart, stale while down, converged after`);
  process.exit(0);
})().catch(async (e) => { await stop(); fail(String(e && e.stack || e)); });
NODE
rm -f "$LIVE"
