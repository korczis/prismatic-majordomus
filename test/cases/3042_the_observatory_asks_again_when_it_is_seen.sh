# majordomus-covers: none
# The Observatory asks again when it is seen, and blames the server only for an asking that
# failed. A display that is hidden reads nothing (it does not spend the server on a screen
# nobody sees), so after a hidden spell its reading is old through no fault of the server;
# share/ui/observatory.js used to leave it old until the next timer and say "the server is not
# answering" meanwhile, which sent a reader hunting a dead server that was answering in 6 ms
# (2026-10-14, the kiosk at /ui/kiosk).
#
# Driven in a real browser over the real script, with a fixture server this case runs and a
# fake clock, so five hidden minutes take none:
#
#   hidden     five minutes hidden ask nothing: no page reading, no journal reading
#   seen       the moment the page is seen, the page and the journal are each asked once
#   asking     while that asking is under way the status says it is asking, not that the
#              server is silent
#   answered   once the server answered, the display is LIVE
#   down       when the server answers 503, the status says the server is not answering
#   errors     no console error and no uncaught exception on either run
#
# Needs a browser scripts/lib/browser.mjs can launch. Without one it skips locally and fails
# under CI=true, as case 334 does.
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

// what the server was asked, and how it answers: 'ok', or 'down' (503 to every reading)
const asked = { fragment: 0, events: 0 };
let mode = 'ok';
let hold = 0; // milliseconds a reading (page or journal) is held before it is answered
const server = http.createServer((req, res) => {
  const url = new URL(req.url, 'http://fixture');
  if (url.pathname === '/observatory.js') {
    res.writeHead(200, { 'content-type': 'text/javascript; charset=utf-8' }); res.end(script); return;
  }
  if (url.pathname === '/api/v1/observatory/events') {
    asked.events += 1;
    if (mode === 'down') { res.writeHead(503); res.end(); return; }
    setTimeout(() => {
      res.writeHead(200, { 'content-type': 'application/json' });
      res.end(JSON.stringify({ events: [], cursor: 5 }));
    }, hold);
    return;
  }
  if (url.pathname === '/ui/kiosk/') {
    const reading = url.searchParams.has('fragment');
    if (reading) asked.fragment += 1;
    const answer = () => {
      if (reading && mode === 'down') { res.writeHead(503); res.end(); return; }
      res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' }); res.end(page);
    };
    if (reading && hold) setTimeout(answer, hold); else answer();
    return;
  }
  res.writeHead(404); res.end();
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const origin = 'http://127.0.0.1:' + server.address().port;

const browser = await launch();
if (!browser) { console.log('NO-BROWSER'); server.close(); process.exit(0); }

const setSeen = (p, seen) => p.evaluate((v) => {
  Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => (v ? 'visible' : 'hidden') });
  Object.defineProperty(document, 'hidden', { configurable: true, get: () => !v });
  document.dispatchEvent(new Event('visibilitychange'));
}, seen);
const status = (p) => p.evaluate(() => document.querySelector('.mj-obs-status')?.textContent || '');
// waits in real time only: the page's clock stands still, so no timer of the display fires and
// whatever it asks here it asked because it was seen
const settle = async (p, predicate, what) => {
  for (let i = 0; i < 100; i += 1) {
    if (predicate()) return true;
    await new Promise((r) => setTimeout(r, 30));
  }
  console.log('TIMEOUT ' + what + ' ' + JSON.stringify(asked));
  return false;
};

for (const run of ['ok', 'down']) {
  const p = await browser.newPage();
  const errors = [];
  // the browser reports the fixture's own 503s as failed loads: that is the server this run
  // takes down, not the script; anything else on the console is an error of the script
  p.on('console', (m) => {
    if (m.type() === 'error' && !(run === 'down' && /Failed to load resource.*503/.test(m.text()))) {
      errors.push('console: ' + m.text());
    }
  });
  p.on('pageerror', (e) => errors.push('uncaught: ' + e.message));
  await p.clock.install();
  mode = 'ok'; hold = 0;
  await p.goto(origin + '/ui/kiosk/');
  await p.clock.runFor(1000);
  asked.fragment = 0; asked.events = 0;

  await setSeen(p, false);
  await p.clock.runFor(5 * 60 * 1000);
  console.log(run + ' hidden asked fragment ' + asked.fragment + ' events ' + asked.events);

  mode = run; hold = run === 'ok' ? 1500 : 0;
  await setSeen(p, true);
  await settle(p, () => asked.fragment >= 1 && asked.events >= 1, run + ' seen');
  console.log(run + ' seen asked fragment ' + asked.fragment + ' events ' + asked.events);

  if (run === 'ok') {
    // the reading is held, so a frame painted now shows the display while it is asking
    await p.clock.runFor(200);
    console.log('ok asking status ' + (await status(p)));
    let now = '';
    for (let i = 0; i < 60 && !(now = await status(p)).startsWith('LIVE'); i += 1) {
      await p.clock.runFor(100); await new Promise((r) => setTimeout(r, 50));
    }
    console.log('ok answered status ' + now);
  } else {
    let now = '';
    for (let i = 0; i < 60 && !(now = await status(p)).includes('not answering'); i += 1) {
      await p.clock.runFor(100); await new Promise((r) => setTimeout(r, 50));
    }
    console.log('down status ' + now);
  }
  console.log(run + ' errors ' + errors.length + (errors.length ? ' ' + errors.join(' | ') : ''));
  await p.close();
}
await browser.close(); server.close();
JS
node probe.mjs > probe.out 2>&1 || { sed 's/^/    /' probe.out; echo "    the probe did not run"; exit 1; }
if grep -q '^NO-BROWSER$' probe.out; then
  [ "${CI:-}" = true ] && { echo "    no browser could be started under CI"; exit 1; }
  skip "no browser could be started (install Chrome, or: npx playwright install chromium)"
fi
# what the browser saw is printed before any verdict, so a failure is read, not guessed at
sed 's/^/    | /' probe.out
expect_grep '^ok hidden asked fragment 0 events 0$' probe.out
expect_grep '^down hidden asked fragment 0 events 0$' probe.out
expect_grep '^ok seen asked fragment 1 events 1$' probe.out
expect_grep '^down seen asked fragment 1 events 1$' probe.out
expect_grep '^ok asking status STALE · read .* s ago, asking again$' probe.out
expect_grep '^ok answered status LIVE · ' probe.out
expect_grep '^down status STALE · read .* s ago, the server is not answering$' probe.out
expect_grep '^ok errors 0$' probe.out
expect_grep '^down errors 0$' probe.out
