// Browser test of the built Czech edition. Build first: python3 handbook/build.py
const ROOT = require('path').resolve(__dirname, '../../..');
const PAGE = process.env.HANDBOOK_PAGE || require('path').join(ROOT, 'target/handbook/cs/prirucka-majordomusu.html');
const OUT = process.env.HANDBOOK_TEST_OUT || require('path').join(ROOT, 'target/handbook/cs/tests');
require('fs').mkdirSync(OUT, { recursive: true });
const { chromium } = require('playwright');
const fs = require('fs'), path = require('path');
(async () => {
  const html = fs.readFileSync(PAGE, 'utf8');
  fs.writeFileSync(path.join(OUT, 'pw-g.html'), '<!doctype html><html><head><meta charset=utf8><meta name=viewport content="width=device-width,initial-scale=1"><style>[hidden]{display:none!important}body{margin:0}</style></head><body>' + html + '</body></html>');
  const b = await chromium.launch();
  const p = await (await b.newContext({ viewport: { width: 1400, height: 950 }, reducedMotion: 'reduce' })).newPage();
  const errs = []; p.on('pageerror', (e) => errs.push(e.message));
  await p.goto('file://' + path.join(OUT, 'pw-g.html') + '#stealth'); await p.waitForTimeout(1500);
  console.log('old #stealth lands on osobni: top', await p.evaluate(() => Math.round(document.getElementById('osobni').getBoundingClientRect().top)));
  console.log('h2:', await p.locator('#osobni > h2').textContent());
  console.log('nav label:', await p.locator('nav.rail a[href="#osobni"]').textContent());
  const txt = await p.evaluate(() => document.getElementById('osobni').innerText);
  console.log('wording left:', JSON.stringify((txt.match(/[^\n.]{0,30}(tajn|stealth|skryt|prozrad|schov)[^\n.]{0,30}/gi) || [])));
  console.log('whole page "Tajný režim":', await p.evaluate(() => (document.body.innerText.match(/tajn[ýé][a-z]* režim/gi) || []).length));
  for (let i = 0; i < 5; i++) { await p.locator('#gr-steps .flow-step-b').nth(i).click(); }
  console.log('grad:', await p.locator('#gr-ctl .flow-pos').textContent(), '|', (await p.locator('#gr-out').innerText()).split('\n').slice(-1)[0].slice(0, 90));
  const who = await p.locator('#say-seg .seg-b').allTextContents();
  for (let i = 0; i < who.length; i++) { await p.locator('#say-seg .seg-b').nth(i).click(); console.log('say', who[i], '→', (await p.locator('#say-card .say-q').textContent()).slice(0, 70), '| links', await p.locator('#say-card a').count()); }
  // every link of the personal chapter resolves
  const bad = await p.evaluate(() => Array.from(document.querySelectorAll('#osobni a[href^="#"]')).map((a) => a.getAttribute('href')).filter((h) => !h.startsWith('#doc.') && !h.startsWith('#g.') && !h.startsWith('#ex.') && !h.startsWith('#arch.') && !document.getElementById(h.slice(1))));
  console.log('dead internal links in chapter:', JSON.stringify(bad), '| internal links', await p.locator('#osobni a[href^="#"]').count());
  await p.locator('#say-card a.md-in').first().click().catch(() => {}); await p.waitForTimeout(1500);
  console.log('doc from say card:', await p.locator('.rd-crumbs code').textContent().catch(() => 'none'));
  await p.locator('#lab-say').screenshot({ path: path.join(OUT, 'pm-say.png') });
  await p.locator('#lab-grad').screenshot({ path: path.join(OUT, 'pm-grad.png') });
  console.log('overflow', await p.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth), 'errors', JSON.stringify(errs));
  const m = await (await b.newContext({ viewport: { width: 390, height: 800 } })).newPage();
  await m.goto('file://' + path.join(OUT, 'pw-g.html') + '#osobni'); await m.waitForTimeout(1200);
  console.log('phone overflow', await m.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth));
  await b.close();
})();
