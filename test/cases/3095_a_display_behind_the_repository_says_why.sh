# majordomus-covers: none
# A display that cannot read the repository says it is behind, and says why; it does not say
# LIVE. On 2026-10-16 a kiosk on a machine whose server could not load the repository showed
# "LIVE · READ ..." over a v0.26.0 picture for hours, because the display trusted the age of
# its last reading and the server answered every asking with the generation it last loaded.
# The server now says how it stands (`/ui/pulse`: generation, lag, the reason a load failed),
# and share/ui/observatory.js asks it between readings.
#
# Driven in a real browser over the real script, with a fixture server this case runs and a
# fake clock, so the minutes take none:
#
#   live       a server that is current is LIVE
#   behind     a server whose load fails is BEHIND, with the server's reason and its next attempt
#   again      when the repository moves under a current server, the page is read again at once
#              (the display does not wait for its minute)
#   caught up  when the server catches up, the display is LIVE again and says nothing of it
#   errors     no console error and no uncaught exception
#
# Needs a browser scripts/lib/browser.mjs can launch. Without one it skips locally and fails
# under CI=true, as case 3042 does.
. "$ROOT/test/lib.sh"
command -v node >/dev/null 2>&1 || skip "no node"
[ -d "$ROOT/node_modules/playwright" ] || skip "playwright is not installed (npm ci)"

cat > probe.mjs <<JS
import http from 'node:http';
import { readFileSync } from 'node:fs';
const { launch } = await import('$ROOT/scripts/lib/browser.mjs');
const script = readFileSync('$ROOT/share/ui/observatory.js');

const scene = (id, available, model) => ({
  id, label: id, available, model: available ? model : null, reason: available ? null : 'fixture',
  dwell_seconds: 3600, meaning: 'fixture', requires: ['fixture'],
});
const projection = {
  schema: 'observatory/v1', observed_at: '2026-10-14T20:00:00Z', revision: 'fixture',
  sources: [{ capability: 'fixture', state: 'answered' }],
  scenes: [
    scene('ambient', true, { sources: [{ capability: 'fixture', state: 'answered' }] }),
    scene('activity', true, { events: [], lamport: 5 }),
  ],
};
const attr = JSON.stringify(projection).replace(/&/g, '&amp;').replace(/"/g, '&quot;');
const page = '<!doctype html><html lang="en"><head><title>kiosk</title><link rel="icon" href="data:,"></head><body>'
  + '<div data-observatory="' + attr + '" data-observatory-mode="kiosk"><canvas hidden></canvas></div>'
  + '<script src="/observatory.js"></script>'
  + '<script>window.view = window.mjObservatory.mount(document.querySelector("[data-observatory]"), {});</script>'
  + '</body></html>';

// what the server was asked, and how it stands: the pulse the fixture answers
const asked = { fragment: 0, pulse: 0 };
let beat = { schema: 'ui-pulse/v1', generation: 1, head: 'abc', lag: 'current', error: null, retry_in_s: null };
const server = http.createServer((req, res) => {
  const url = new URL(req.url, 'http://fixture');
  if (url.pathname === '/observatory.js') {
    res.writeHead(200, { 'content-type': 'text/javascript; charset=utf-8' }); res.end(script); return;
  }
  if (url.pathname === '/ui/pulse') {
    asked.pulse += 1;
    res.writeHead(200, { 'content-type': 'application/json' }); res.end(JSON.stringify(beat)); return;
  }
  if (url.pathname === '/api/v1/observatory/events') {
    res.writeHead(200, { 'content-type': 'application/json' }); res.end(JSON.stringify({ events: [], cursor: 5 })); return;
  }
  if (url.pathname === '/ui/kiosk/') {
    if (url.searchParams.has('fragment')) asked.fragment += 1;
    res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' }); res.end(page);
    return;
  }
  res.writeHead(404); res.end();
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const origin = 'http://127.0.0.1:' + server.address().port;

const browser = await launch();
if (!browser) { console.log('NO-BROWSER'); server.close(); process.exit(0); }

const status = (p) => p.evaluate(() => document.querySelector('.mj-obs-status')?.textContent || '');
// real time only for the fixture's answers; the page's clock is driven
const until = async (p, predicate, what) => {
  for (let i = 0; i < 120; i += 1) {
    if (await predicate()) return true;
    await p.clock.runFor(500);
    await new Promise((r) => setTimeout(r, 25));
  }
  console.log('TIMEOUT ' + what + ' ' + JSON.stringify(asked));
  return false;
};

const errors = [];
const p = await browser.newPage();
p.on('console', (m) => { if (m.type() === 'error') errors.push('console: ' + m.text()); });
p.on('pageerror', (e) => errors.push('uncaught: ' + e.message));
await p.clock.install();
await p.goto(origin + '/ui/kiosk/');
await until(p, async () => asked.pulse >= 1, 'first pulse');
await until(p, async () => (await status(p)).startsWith('LIVE'), 'live');
console.log('live status ' + (await status(p)));

beat = { ...beat, lag: 'failed', error: 'manifest: duplicate key', retry_in_s: 3 };
await until(p, async () => (await status(p)).startsWith('BEHIND'), 'behind');
console.log('behind status ' + (await status(p)));

// the repository moved under a current server: the page is read again before its minute
beat = { ...beat, generation: 2, lag: 'current', error: null, retry_in_s: null };
asked.fragment = 0;
await until(p, async () => asked.fragment >= 1, 'read again');
console.log('again fragment reads ' + (asked.fragment >= 1 ? 'at least 1' : '0'));
await until(p, async () => (await status(p)).startsWith('LIVE'), 'caught up');
console.log('caught up status ' + (await status(p)));
console.log('errors ' + errors.length + (errors.length ? ' ' + errors.join(' | ') : ''));
await browser.close(); server.close();
JS
node probe.mjs > probe.out 2>&1 || { sed 's/^/    /' probe.out; echo "    the probe did not run"; exit 1; }
if grep -q '^NO-BROWSER$' probe.out; then
  [ "${CI:-}" = true ] && { echo "    no browser could be started under CI"; exit 1; }
  skip "no browser could be started (install Chrome, or: npx playwright install chromium)"
fi
sed 's/^/    | /' probe.out
expect_grep '^live status LIVE · ' probe.out
expect_grep '^behind status BEHIND · .*the server could not read the repository: manifest: duplicate key \(next attempt in 3 s\)' probe.out
expect_grep '^again fragment reads at least 1$' probe.out
expect_grep '^caught up status LIVE · ' probe.out
expect_grep '^errors 0$' probe.out
