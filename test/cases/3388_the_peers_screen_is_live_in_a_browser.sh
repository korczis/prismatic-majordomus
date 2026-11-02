# majordomus-covers: none
# /ui/peers is live in a real browser (I2983): the screen a person opens shows a worker the
# moment it announces, without a reload, at desktop and at phone width.
#
# Case 3383 holds live.js and ui.js in node, against fakes. This case is the browser half: the
# real server, the real socket, the real scripts and a real browser. The run is the one that
# found ui.js left a screen stale. A change frame's reading came back catching up on a loaded
# machine, and the page waited for a frame that never came (2026-10-19).
#
#   live       the state line says live once the channel is open
#   pushed     an MCP announcement reaches the open page, and the page was loaded once
#   phone      the same at 390 px, with no horizontal scroll
#   errors     no console error and no uncaught exception
#
# Needs a browser scripts/lib/browser.mjs can launch. Without one it skips locally and fails
# under CI=true, as case 3042 does.
. "$ROOT/test/lib.sh"
mj_runs node || skip "$(mj_runs_why node)"
command -v curl >/dev/null 2>&1 || skip "curl not installed"
command -v jq >/dev/null 2>&1 || skip "jq not installed"
[ -d "$ROOT/node_modules/playwright" ] || skip "playwright is not installed (npm ci)"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"

T="$(mktemp -d "${TMPDIR:-/tmp}/mj-3388.XXXXXX")"
PIDS=""
trap 'for p in $PIDS; do kill "$p" 2>/dev/null || true; done; rm -rf "$T"' EXIT
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

cat > "$T/probe.mjs" <<JS
const { launch } = await import('$ROOT/scripts/lib/browser.mjs');
const url = process.argv[2];
async function mcp(session, body) {
  const headers = { 'content-type': 'application/json', accept: 'application/json, text/event-stream' };
  if (session) headers['mcp-session-id'] = session;
  const reply = await fetch(url + '/mcp', { method: 'POST', headers, body: JSON.stringify(body) });
  if (!reply.ok) throw new Error('mcp ' + body.method + ' answered ' + reply.status);
  return { session: reply.headers.get('mcp-session-id'), text: await reply.text() };
}
const attached = await mcp(null, { jsonrpc: '2.0', id: 1, method: 'initialize',
  params: { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'case-3388', version: '1' } } });
const announce = (intent) => mcp(attached.session, { jsonrpc: '2.0', id: 2, method: 'tools/call',
  params: { name: 'majordomus_announce', arguments: { intent, scope: ['docs'] } } });
const browser = await launch();
if (!browser) { console.log('NOBROWSER'); process.exit(0); }
let failed = false;
const say = (ok, line) => { console.log((ok ? 'ok ' : 'FAIL ') + line); if (!ok) failed = true; };
for (const [name, viewport] of [['desktop', { width: 1280, height: 900 }], ['phone', { width: 390, height: 844 }]]) {
  const context = await browser.newContext({ viewport, reducedMotion: 'reduce' });
  const page = await context.newPage();
  const errors = [];
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  page.on('pageerror', (e) => errors.push('uncaught: ' + e.message));
  let loads = 0;
  page.on('load', () => { loads += 1; });
  await page.goto(url + '/ui/peers', { waitUntil: 'load' });
  const live = await page.waitForFunction(
    () => /^Live:/.test(document.querySelector('[data-follow-state]')?.textContent?.trim() || ''),
    null, { timeout: 30000 }).then(() => true, () => false);
  say(live, name + ' live');
  const intent = 'case 3388 ' + name + ' ' + Date.now();
  await announce(intent);
  const pushed = await page.waitForFunction((t) => document.body.innerText.includes(t), intent, { timeout: 20000 })
    .then(() => true, () => false);
  say(pushed && loads === 1, name + ' pushed (' + loads + ' load(s))');
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
  say(overflow <= 1, name + ' no horizontal scroll (' + overflow + ' px)');
  say(errors.length === 0, name + ' no error' + (errors.length ? ': ' + errors.slice(0, 3).join(' | ') : ''));
  await context.close();
}
await browser.close();
process.exit(failed ? 1 : 0);
JS
node "$T/probe.mjs" "$url" > "$T/probe.out" 2>&1 || { echo "    the browser run failed:"; sed 's/^/    | /' "$T/probe.out"; exit 1; }
if grep -q '^NOBROWSER' "$T/probe.out"; then skip "no browser scripts/lib/browser.mjs can launch"; fi
for line in 'desktop live' 'desktop pushed' 'desktop no horizontal scroll' 'desktop no error' \
            'phone live' 'phone pushed' 'phone no horizontal scroll' 'phone no error'; do
  grep -q "^ok $line" "$T/probe.out" || { echo "    not shown: $line"; sed 's/^/    | /' "$T/probe.out"; exit 1; }
done
echo "    /ui/peers says live, shows an announcement without a reload, at desktop and phone width"
