# majordomus-covers: none
# /ui/board is live in a real browser (I4236): the screen a person opens pulls its board in,
# follows the blackboard and is drawn again when a worker arrives, without a reload, and a
# card leads to its issue; at desktop and at phone width.
#
# The unit tests of ui::board hold what the screen draws from given answers. This case is the
# browser half: the real server over a fixture plan, the real socket, the real scripts.
#
#   board      the deferred board panel arrives and shows the fixture's ready issue as a card
#   live       the state line says live once the channel is open
#   redrawn    an MCP session that announces moves the Workers count, and the page was loaded once
#   card       the card's id leads to the issue's inspector, which answers and names it
#   phone      no horizontal scroll at 390 px: the board scrolls inside itself
#   errors     no console error and no uncaught exception
#
# Needs a browser scripts/lib/browser.mjs can launch. Without one it skips locally and fails
# under CI=true, as case 3388 does.
. "$ROOT/test/lib.sh"
command -v node >/dev/null 2>&1 || skip "no node"
command -v curl >/dev/null 2>&1 || skip "curl not installed"
[ -d "$ROOT/node_modules/playwright" ] || skip "playwright is not installed (npm ci)"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
unset MAJORDOMUS_HTTP_HOST

"$MJ" init >/dev/null
pj_init
pj_milestone M1 1
pj_issue I0001 M1
pj_issue I0002 M1 I0001
git add -A >/dev/null && git commit -qm "a plan of two issues"

SRV=""
trap 'serve_down' EXIT
serve_up serve.out serve.err || exit 1

cat > probe.mjs <<JS
const { launch } = await import('$ROOT/scripts/lib/browser.mjs');
const url = process.argv[2];
async function mcp(session, body) {
  const headers = { 'content-type': 'application/json', accept: 'application/json, text/event-stream' };
  if (session) headers['mcp-session-id'] = session;
  const reply = await fetch(url + '/mcp', { method: 'POST', headers, body: JSON.stringify(body) });
  if (!reply.ok) throw new Error('mcp ' + body.method + ' answered ' + reply.status);
  return { session: reply.headers.get('mcp-session-id'), text: await reply.text() };
}
async function arrive(name) {
  const s = await mcp(null, { jsonrpc: '2.0', id: 1, method: 'initialize',
    params: { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name, version: '1' } } });
  await mcp(s.session, { jsonrpc: '2.0', id: 2, method: 'tools/call',
    params: { name: 'majordomus_announce', arguments: { intent: name, scope: ['docs'] } } });
}
const workers = () => {
  const k = [...document.querySelectorAll('.mj-ui-kpi')]
    .find((x) => x.querySelector('.mj-ui-kpi-label')?.textContent.trim() === 'Workers');
  return k?.querySelector('.mj-ui-kpi-value')?.textContent.trim() ?? '';
};
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
  await page.goto(url + '/ui/board', { waitUntil: 'load' });
  const board = await page.waitForSelector('.mj-ui-board #card-I0001', { timeout: 30000 }).then(() => true, () => false);
  say(board, name + ' board');
  const live = await page.waitForFunction(
    () => /^Live:/.test(document.querySelector('[data-follow-state]')?.textContent?.trim() || ''),
    null, { timeout: 30000 }).then(() => true, () => false);
  say(live, name + ' live');
  const before = await page.evaluate(workers);
  await arrive('case-4236-' + name + '-' + Date.now());
  const redrawn = await page.waitForFunction((b) => {
    const k = [...document.querySelectorAll('.mj-ui-kpi')]
      .find((x) => x.querySelector('.mj-ui-kpi-label')?.textContent.trim() === 'Workers');
    const now = k?.querySelector('.mj-ui-kpi-value')?.textContent.trim() ?? '';
    return now !== '' && now !== b;
  }, before, { timeout: 20000 }).then(() => true, () => false);
  say(redrawn && loads === 1, name + ' redrawn (Workers ' + before + ' -> ' + (await page.evaluate(workers)) + ', ' + loads + ' load(s))');
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
  say(overflow <= 1, name + ' no horizontal scroll (' + overflow + ' px)');
  const link = page.locator('#card-I0001 a.mj-ui-board-id');
  await page.waitForSelector('#card-I0001 a.mj-ui-board-id', { timeout: 20000 }).catch(() => {});
  const [reply] = await Promise.all([
    page.waitForNavigation({ timeout: 30000 }).catch(() => null),
    link.click().catch(() => null),
  ]);
  const landed = !!reply && reply.status() === 200 && /[?&]issue=I0001\b/.test(page.url())
    && (await page.content()).includes('I0001');
  say(landed, name + ' card leads to its issue (' + (reply ? reply.status() + ' ' + page.url().replace(url, '') : 'no navigation') + ')');
  say(errors.length === 0, name + ' no error' + (errors.length ? ': ' + errors.slice(0, 3).join(' | ') : ''));
  await context.close();
}
await browser.close();
process.exit(failed ? 1 : 0);
JS
node probe.mjs "$U" > probe.out 2>&1 || { echo "    the browser run failed:"; sed 's/^/    | /' probe.out; exit 1; }
if grep -q '^NOBROWSER' probe.out; then skip "no browser scripts/lib/browser.mjs can launch"; fi
for v in desktop phone; do
  for line in board live redrawn 'no horizontal scroll' 'card leads to its issue' 'no error'; do
    grep -q "^ok $v $line" probe.out || { echo "    not shown: $v $line"; sed 's/^/    | /' probe.out; exit 1; }
  done
done
echo "    /ui/board pulls its board in, redraws when a worker arrives, and a card leads to its issue, at desktop and phone width"
