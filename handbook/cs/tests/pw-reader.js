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
  const sweep = process.argv.includes('--sweep');
  for (const theme of ['light', 'dark']) {
    const ctx = await b.newContext({ viewport: { width: 1400, height: 950 }, reducedMotion: 'reduce', colorScheme: theme });
    const p = await ctx.newPage(); const errs = []; p.on('pageerror', (e) => errs.push(e.message));
    await p.goto('file://' + path.join(OUT, 'pw-rd.html')); await p.waitForTimeout(1200);
    console.log(theme, 'list rows', await p.locator('#rd-list li').count(), 'decorated chapter links', await p.locator('main a.md-open').count(), 'gh twins', await p.locator('main a.gh-x').count());
    // a chapter link opens the reader in place
    await p.locator('main section:not(#citarna) a.md-open').first().click();
    await p.waitForSelector('#rd-body .md-done', { state: 'attached', timeout: 20000 });
    console.log(theme, 'opened from chapter:', await p.locator('.rd-title').textContent(), '| hash', await p.evaluate(() => location.hash));
    for (const doc of ['docs/HOW_IT_WORKS.md', 'docs/MESH.md', '.ai/repo/adrs/0044-the-peer-board-belongs-to-the-repository.md', '.ai/repo/rules/project/the-server-sees-the-current-tree.v1.md', 'docs/STEALTH.md']) {
      const ok = await p.evaluate((d) => window.mjDoc(d), doc);
      if (!ok) { console.log(theme, 'NOT IN CORPUS', doc); continue; }
      await p.waitForFunction((d) => document.querySelector('.rd-crumbs code') && document.querySelector('.rd-crumbs code').textContent === d, doc, { timeout: 20000 });
      await p.waitForTimeout(doc.includes('HOW_IT') || doc.includes('MESH') ? 4000 : 300);
      const r = await p.evaluate(() => ({
        title: document.querySelector('.rd-title').textContent.slice(0, 50),
        blocks: document.querySelectorAll('#rd-body > *').length, inl: document.querySelectorAll('#rd-body a.md-in').length,
        gh: document.querySelectorAll('#rd-body a.md-gh').length, ext: document.querySelectorAll('#rd-body a.md-ext').length,
        mer: document.querySelectorAll('#rd-body .md-mermaid svg').length, merSrc: document.querySelectorAll('#rd-body pre.mermaid-src').length,
        hl: document.querySelectorAll('#rd-body .hljs span').length, boxes: Array.from(document.querySelectorAll('.rd-box-h')).map((x) => x.textContent).join(' | '),
        meta: !!document.querySelector('.rd-meta'), tables: document.querySelectorAll('#rd-body .table-wrap').length }));
      console.log(theme, doc.split('/').pop(), JSON.stringify(r));
      if (doc.includes('HOW_IT') || doc.includes('0044')) await p.locator('#rd-doc').screenshot({ path: path.join(OUT, `rd-${doc.split('/').pop().slice(0, 8)}-${theme}.png`) }).catch(() => {});
    }
    // an in-document link navigates within the reader, back works, a chapter backlink leaves it
    const inl = p.locator('#rd-side a.md-in').first();
    if (await inl.count()) { const t = await inl.getAttribute('href'); await inl.click(); await p.waitForTimeout(600); console.log(theme, 'side link', t, '->', await p.locator('.rd-crumbs code').textContent()); }
    for (const v of ['graph', 'tree']) {
      await p.locator(`#rd-views [data-v="${v}"]`).click(); await p.waitForTimeout(v === 'graph' ? 3500 : 1500);
      console.log(theme, v, 'canvas', await p.locator('#rd-chart canvas').count(), (await p.locator('#rd-chart-note').textContent()).slice(0, 50));
      await p.locator('#rd-chart-wrap').screenshot({ path: path.join(OUT, `rd-${v}-${theme}.png`) });
    }
    await p.locator('#rd-q').fill('worktree'); await p.waitForTimeout(200);
    console.log(theme, 'filter worktree:', await p.locator('#rd-count').textContent());
    console.log(theme, 'overflow', await p.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth), 'errors', JSON.stringify(errs));
    if (sweep && theme === 'light') {
      const res = await p.evaluate(async () => {
        const docs = window.mjDocs.index.docs, by = {}; docs.forEach((d) => { by[d.p] = 1; });
        let bad = [], links = 0, empty = [];
        for (const d of docs) {
          window.mjDoc(d.p, { quiet: true });
          for (let i = 0; i < 200; i++) { const c = document.querySelector('.rd-crumbs code'); if (c && c.textContent === d.p) break; await new Promise((r) => setTimeout(r, 10)); }
          const body = document.getElementById('rd-body');
          if (body.children.length < 2) empty.push(d.p);
          body.querySelectorAll('a[href^="#doc."]').forEach((a) => { links++; const t = a.getAttribute('href').slice(5).split('::')[0]; if (!by[t]) bad.push(d.p + ' -> ' + t); });
        }
        return { docs: docs.length, links, bad: bad.slice(0, 10), badCount: bad.length, empty };
      });
      console.log('SWEEP', JSON.stringify(res), 'errors', JSON.stringify(errs.slice(0, 5)));
    }
    await ctx.close();
  }
  const m = await b.newContext({ viewport: { width: 390, height: 800 }, reducedMotion: 'reduce' }); const mp = await m.newPage();
  await mp.goto('file://' + path.join(OUT, 'pw-rd.html') + '#doc.docs/MESH.md'); await mp.waitForSelector('#rd-body .md-done', { state: 'attached', timeout: 20000 }); await mp.waitForTimeout(2500);
  console.log('phone deep link:', await mp.locator('.rd-title').textContent(), 'overflow', await mp.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth), 'scrollY', await mp.evaluate(() => Math.round(scrollY)));
  await mp.screenshot({ path: path.join(OUT, 'rd-phone.png') });
  await b.close();
})();
