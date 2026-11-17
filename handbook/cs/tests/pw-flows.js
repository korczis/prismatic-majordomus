// Browser test of the built Czech edition. Build first: python3 handbook/build.py
const ROOT = require('path').resolve(__dirname, '../../..');
const PAGE = process.env.HANDBOOK_PAGE || require('path').join(ROOT, 'target/handbook/cs/prirucka-majordomusu.html');
const OUT = process.env.HANDBOOK_TEST_OUT || require('path').join(ROOT, 'target/handbook/cs/tests');
require('fs').mkdirSync(OUT, { recursive: true });
const { chromium } = require('playwright');
const fs = require('fs'), path = require('path');
(async () => {
  const html = fs.readFileSync(PAGE, 'utf8');
  fs.writeFileSync(path.join(OUT, 'pw-rd.html'), '<!doctype html><html><head><meta charset=utf8><meta name=viewport content="width=device-width,initial-scale=1"><style>[hidden]{display:none!important}body{margin:0}</style></head><body>' + html + '</body></html>');
  const b = await chromium.launch();
  for (const theme of ['light', 'dark']) {
    const ctx = await b.newContext({ viewport: { width: 1400, height: 950 }, colorScheme: theme });
    const p = await ctx.newPage(); const errs = []; p.on('pageerror', (e) => errs.push(e.message));
    await p.goto('file://' + path.join(OUT, 'pw-rd.html')); await p.waitForTimeout(1000);
    await p.locator('#stavba .flow-bar .seg-b').nth(1).click(); await p.waitForTimeout(1200);
    await p.locator('#stavba .flow-ctl button', { hasText: 'krok ▶' }).click(); await p.waitForTimeout(200);
    console.log(theme, 'stavba steps', await p.locator('#stavba .flow-step').count(), '| pos', await p.locator('#stavba .flow-pos').textContent(), '| step 1:', (await p.locator('#stavba .flow-step').first().textContent()).slice(0, 110));
    await p.locator('#stavba .flow-step-b').nth(4).click(); await p.waitForTimeout(150);
    console.log(theme, 'seek ->', await p.locator('#stavba .flow-pos').textContent(), 'on', await p.locator('#stavba svg .flow-on').count());
    await p.locator('#stavba').screenshot({ path: path.join(OUT, `fl-stavba-${theme}.png`) });
    await p.locator('#h-mapa .flow-bar .seg-b').nth(1).click(); await p.waitForTimeout(2500);
    console.log(theme, 'h-mapa steps', await p.locator('#h-mapa .flow-step').count(), (await p.locator('#h-mapa .flow-step').last().textContent()).slice(0, 80));
    await p.evaluate(() => { document.getElementById('toky').scrollIntoView(); }); await p.waitForTimeout(3500);
    console.log(theme, 'toky:', await p.locator('#fr-total').textContent(), '|', await p.locator('#fg-total').textContent(), '| canvases', await p.locator('#toky canvas').count(), '| table rows', await p.locator('#fr-table tbody tr').count(), await p.locator('#fg-table tbody tr').count());
    await p.locator('#lab-flow-rules').screenshot({ path: path.join(OUT, `fl-rules-${theme}.png`) });
    await p.locator('#lab-flow-gates').screenshot({ path: path.join(OUT, `fl-gates-${theme}.png`) });
    // click a node: left column of the rule sankey
    const box = await p.locator('#fr-chart').boundingBox();
    await p.mouse.move(box.x + 16, box.y + 120); await p.waitForTimeout(200); await p.mouse.click(box.x + 16, box.y + 120); await p.waitForTimeout(300);
    console.log(theme, 'rule node click:', (await p.locator('#fr-note').textContent()).slice(0, 60), 'rows', await p.locator('#fr-list li').count());
    const first = p.locator('#fr-list li a').first();
    if (await first.count()) { await first.click(); await p.waitForSelector('#rd-body .md-done', { state: 'attached', timeout: 20000 }); console.log(theme, 'rule opened:', (await p.locator('.rd-title').textContent()).slice(0, 60), await p.evaluate(() => location.hash.slice(0, 40))); }
    console.log(theme, 'overflow', await p.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth), 'errors', JSON.stringify(errs));
    await ctx.close();
  }
  await b.close();
})();
