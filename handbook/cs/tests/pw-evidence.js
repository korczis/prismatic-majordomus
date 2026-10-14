// Browser test of the built Czech edition. Build first: python3 handbook/build.py
const ROOT = require('path').resolve(__dirname, '../../..');
const PAGE = process.env.HANDBOOK_PAGE || require('path').join(ROOT, 'target/handbook/cs/prirucka-majordomusu.html');
const OUT = process.env.HANDBOOK_TEST_OUT || require('path').join(ROOT, 'target/handbook/cs/tests');
require('fs').mkdirSync(OUT, { recursive: true });
const { chromium } = require('playwright');
const fs = require('fs'), path = require('path');
(async () => {
  const html = fs.readFileSync(PAGE, 'utf8');
  fs.writeFileSync(path.join(OUT, 'pw-e.html'), '<!doctype html><html><head><meta charset=utf8><meta name=viewport content="width=device-width,initial-scale=1"><style>[hidden]{display:none!important}body{margin:0}</style></head><body>' + html + '</body></html>');
  const b = await chromium.launch();
  const p = await (await b.newContext({ viewport: { width: 1400, height: 950 }, reducedMotion: 'reduce' })).newPage();
  const errs = []; p.on('pageerror', (e) => errs.push(e.message));
  await p.goto('file://' + path.join(OUT, 'pw-e.html')); await p.waitForTimeout(1200);
  const ALLOW = ['kudy', 'mapa-prirucky', 'hodnoceni', 'slovnik', 'pruzkumnik', 'citarna', 'graf', 'toky'];
  const r = await p.evaluate((ALLOW) => {
    const chapters = Array.from(document.querySelectorAll('nav.rail ol a')).map((a) => a.getAttribute('href').slice(1));
    const without = chapters.filter((c) => !document.getElementById('ev-' + c));
    return { chapters: chapters.length, panels: document.querySelectorAll('.ev-panel').length, without, unexpected: without.filter((c) => !ALLOW.includes(c)) };
  }, ALLOW);
  console.log('chapters', r.chapters, 'panels', r.panels, 'without a panel', JSON.stringify(r.without), 'UNEXPECTED', JSON.stringify(r.unexpected));
  // every panel opens and renders exactly the objects the build computed
  const sweep = await p.evaluate(async () => {
    await window.mjGraphUI.ready();
    const bad = []; let tiles = 0, anchors = 0;
    for (const sid of Object.keys(window.mjEvidence)) {
      const d = document.getElementById('ev-' + sid); d.open = true; d.dispatchEvent(new Event('toggle'));
      for (let i = 0; i < 100 && d.querySelectorAll('.ev-body > .ev-kind .g-tile').length === 0; i++) await new Promise((r) => setTimeout(r, 10));
      const n = d.querySelectorAll('.ev-body > .ev-kind .g-tile').length;
      if (n !== window.mjEvidence[sid].s.length) bad.push(sid + ': ' + n + ' vs ' + window.mjEvidence[sid].s.length);
      tiles += n; anchors += d.querySelectorAll('.ev-decl').length;
      d.open = false;
    }
    return { panels: Object.keys(window.mjEvidence).length, tiles, anchors, bad };
  });
  console.log('sweep', JSON.stringify(sweep));
  await p.evaluate(() => { const d = document.getElementById('ev-h-finish'); d.open = true; d.scrollIntoView(); }); await p.waitForTimeout(400);
  console.log('h-finish summary:', (await p.locator('#ev-h-finish > summary').textContent()).slice(0, 160));
  await p.locator('#ev-h-finish').screenshot({ path: path.join(OUT, 'ev-finish.png') });
  await p.locator('#ev-h-finish .g-tile-h').first().click(); await p.waitForTimeout(500);
  console.log('tile -> trace centre:', await p.locator('#gt-grid .g-tile.center b').textContent(), '| hash', (await p.evaluate(() => location.hash)).slice(0, 60));
  for (let i = 0; i < 5; i++) await p.locator('#sw-steps .flow-step-b').nth(i).click();
  console.log('switch stepper:', await p.locator('#sw-ctl .flow-pos').textContent(), '|', (await p.locator('#sw-out').innerText()).split('\n').slice(-1)[0].slice(0, 80));
  console.log('grad stepper steps:', await p.locator('#gr-steps .flow-step').count());
  console.log('overflow', await p.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth), 'errors', JSON.stringify(errs));
  await b.close();
})();
