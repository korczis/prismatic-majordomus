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
  for (const theme of ['light', 'dark']) {
    const ctx = await b.newContext({ viewport: { width: 1400, height: 950 }, colorScheme: theme, reducedMotion: 'reduce' });
    const p = await ctx.newPage(); const errs = []; p.on('pageerror', (e) => errs.push(e.message));
    await p.goto('file://' + path.join(OUT, 'pw-g.html')); await p.waitForTimeout(800);
    await p.evaluate(() => document.getElementById('graf').scrollIntoView()); await p.waitForTimeout(3000);
    console.log(theme, await p.locator('#gk-total').textContent(), '| canvas', await p.locator('#gk-chart canvas').count(), '| table rows', await p.locator('#gk-table tbody tr').count());
    await p.locator('#lab-kinds details summary').click();
    await p.locator('#gk-table tbody tr .chip', { hasText: /^claim$/ }).click(); await p.waitForTimeout(200);
    console.log(theme, 'kind panel:', (await p.locator('#gk-panel .gk-h').textContent()).slice(0, 60), '| tiles', await p.locator('#gk-panel .g-tile').count(), '| relation chips', await p.locator('#gk-panel .chips-btn .chip').count());
    await p.locator('#gk-panel .chips-btn .chip').first().click(); await p.waitForTimeout(200);
    console.log(theme, 'pair panel:', (await p.locator('#gk-panel .gk-h').textContent()).slice(0, 70), '| pairs', await p.locator('#gk-panel .g-pair').count());
    const tabs = await p.locator('#gc-seg .seg-b').allTextContents(); console.log(theme, 'flows:', tabs.join(' | '));
    for (let i = 0; i < tabs.length; i++) { await p.locator('#gc-seg .seg-b').nth(i).click(); await p.waitForTimeout(80); if (!(await p.locator('#gc-chain .gc-node').count())) console.log(theme, 'EMPTY FLOW', tabs[i]); }
    await p.locator('#gc-seg .seg-b').first().click();
    await p.locator('#gc-ctl button', { hasText: 'další příklad' }).click(); await p.waitForTimeout(100);
    console.log(theme, 'chain:', (await p.locator('#gc-chain .gc-node .g-tile-h b').allTextContents()).join('  >  ').slice(0, 300), '|', await p.locator('#gc-ctl .flow-pos').textContent());
    await p.locator('#lab-chains').screenshot({ path: path.join(OUT, `g-chain-${theme}.png`) });
    await p.locator('#lab-kinds').screenshot({ path: path.join(OUT, `g-kinds-${theme}.png`) });
    // trace: click a chain tile -> ego view recentres, address is bookmarkable
    await p.locator('#gc-chain .gc-node .g-tile-h').nth(2).click(); await p.waitForTimeout(500);
    console.log(theme, 'trace centre:', await p.locator('#gt-grid .g-tile.center b').textContent(), '| hash', (await p.evaluate(() => location.hash)).slice(0, 70), '| in groups', await p.locator('#gt-grid .gt-col').first().locator('.gt-group').count(), '| out groups', await p.locator('#gt-grid .gt-col').last().locator('.gt-group').count());
    await p.locator('#gt-q').fill('scope'); await p.waitForTimeout(150);
    console.log(theme, 'search hits', await p.locator('#gt-hits li').count());
    await p.locator('#gt-hits li a').first().click(); await p.waitForTimeout(400);
    console.log(theme, 'after hit:', await p.locator('#gt-grid .g-tile.center b').textContent(), '| trail', (await p.locator('#gt-trail').textContent()).slice(0, 120));
    await p.locator('#lab-trace').screenshot({ path: path.join(OUT, `g-trace-${theme}.png`) });
    const rd = p.locator('#gt-grid a.md-in').first();
    if (await rd.count()) { await rd.click(); await p.waitForSelector('#rd-body .md-done', { state: 'attached', timeout: 20000 }); console.log(theme, 'tile -> reader:', (await p.locator('.rd-title').textContent()).slice(0, 60)); }
    console.log(theme, 'overflow', await p.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth), 'errors', JSON.stringify(errs));
    if (theme === 'light') {
      const r = await p.evaluate(async () => { const d = await window.mjGraphData(); let bad = 0; for (const k of d.keys) { if (!(await window.mjGraph('#g.' + encodeURIComponent(k)))) bad++; } return { nodes: d.nodes, unresolved: bad, tiles: document.querySelectorAll('#gt-grid .g-tile').length }; });
      console.log('SWEEP every object opens:', JSON.stringify(r), 'errors', JSON.stringify(errs));
    }
    await ctx.close();
  }
  const m = await b.newContext({ viewport: { width: 390, height: 800 }, reducedMotion: 'reduce' }); const mp = await m.newPage();
  await mp.goto('file://' + path.join(OUT, 'pw-g.html') + '#g.' + encodeURIComponent('projection:mcp')); await mp.waitForTimeout(2500);
  console.log('phone deep link centre:', await mp.locator('#gt-grid .g-tile.center b').textContent().catch(() => 'NONE'), '| overflow', await mp.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth));
  await b.close();
})();
