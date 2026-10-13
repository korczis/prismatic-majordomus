// Browser test of the built Czech edition. Build first: python3 handbook/build.py
const ROOT = require('path').resolve(__dirname, '../../..');
const PAGE = process.env.HANDBOOK_PAGE || require('path').join(ROOT, 'target/handbook/cs/prirucka-majordomusu.html');
const OUT = process.env.HANDBOOK_TEST_OUT || require('path').join(ROOT, 'target/handbook/cs/tests');
require('fs').mkdirSync(OUT, { recursive: true });
// Real-click link audit of the handbook. Run: node links.spec.js
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');

const DIR = __dirname;
const page_html = fs.readFileSync(PAGE, 'utf8');
const RESET = '<style>:root{color-scheme:light;box-sizing:border-box}body{margin:0;padding:0;font:14px -apple-system,sans-serif;background:#fff;color:#000}img{max-width:100%}[hidden]:not([hidden=until-found i]){display:none!important}</style>';
fs.writeFileSync(path.join(OUT, 'pw-page.html'),
  '<!doctype html><html><head><meta charset=utf8><meta name=viewport content="width=device-width,initial-scale=1,viewport-fit=cover">' + RESET + '</head><body>' + page_html + '</body></html>');
fs.writeFileSync(path.join(OUT, 'pw-host.html'),
  '<!doctype html><html><body style="margin:0"><iframe id="f" sandbox="allow-scripts allow-popups" src="pw-page.html" style="width:100vw;height:100vh;border:0"></iframe></body></html>');

async function inView(scope, id) {
  return scope.evaluate((id) => {
    if (id.indexOf('doc.') === 0) {
      /* a document link: it must name a document of the corpus, and the reading room must show it */
      const p = id.slice(4).split('::')[0], idx = window.mjDocs && window.mjDocs.index.docs.some((d) => d.p === p);
      const c = document.querySelector('.rd-crumbs code');
      if (!idx) return { ok: false, why: 'not in corpus' };
      return { ok: true, doc: true, shown: !!c && c.textContent === p };
    }
    const t = document.getElementById(id);
    if (!t) return { ok: false, why: 'no element' };
    const r = t.getBoundingClientRect();
    return { ok: r.top > -40 && r.top < window.innerHeight * 0.6, top: Math.round(r.top) };
  }, id);
}

async function auditGroup(scope, selector, label, opts = {}) {
  const loc = scope.locator(selector);
  const n = await loc.count();
  let ok = 0; const bad = [];
  for (let i = 0; i < n; i++) {
    const a = loc.nth(i);
    const href = await a.getAttribute('href');
    if (!href || href[0] !== '#') continue;
    const id = href.slice(1);
    try {
      if (opts.before) await opts.before(a);
      await a.scrollIntoViewIfNeeded({ timeout: 3000 });
      await a.click({ timeout: 3000 });
      const v = await inView(scope, id);
      if (v.ok) ok++; else bad.push(`${id} (${v.why || 'top=' + v.top})`);
    } catch (e) {
      bad.push(`${id} (click failed: ${String(e.message).split('\n')[0].slice(0, 80)})`);
    }
  }
  console.log(`${label}: ${ok}/${n} ok` + (bad.length ? `  BAD: ${bad.slice(0, 8).join('; ')}${bad.length > 8 ? ' …+' + (bad.length - 8) : ''}` : ''));
  return bad.length;
}

(async () => {
  const browser = await chromium.launch();
  let failures = 0;

  // ---------- 1. top-level page, desktop ----------
  {
    const ctx = await browser.newContext({ viewport: { width: 1280, height: 900 }, reducedMotion: 'reduce' });
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(e.message));
    await page.goto('file://' + path.join(OUT, 'pw-page.html'));
    await page.waitForTimeout(800);
    console.log('== desktop, top-level document');
    failures += await auditGroup(page, 'nav.rail ol a', 'sidebar chapters');
    failures += await auditGroup(page, 'nav.rail p.grp a', 'sidebar parts');
    failures += await auditGroup(page, 'header.hero .links a', 'hero links');
    failures += await auditGroup(page, '.part-grid a.pg', 'part overview cards');
    failures += await auditGroup(page, 'main section .see a', 'see-also');
    failures += await auditGroup(page, 'main section .pn a', 'prev/next');
    failures += await auditGroup(page, 'table.hub a', 'problem hub');
    failures += await auditGroup(page, '#gloss dt a', 'glossary');
    failures += await auditGroup(page, '#kudy ol.path a', 'reading paths');
    await page.evaluate(() => document.querySelectorAll('details.backlinks').forEach((d) => { d.open = true; }));
    failures += await auditGroup(page, 'details.backlinks a', 'backlinks');
    failures += await auditGroup(page, 'main section p > a[href^="#"], main section li > a[href^="#"], main section td > a[href^="#"]:not(.cell-go)', 'inline text links');
    failures += await auditGroup(page, 'a.cell-go', 'table row arrows');
    // context bar
    await page.locator('nav.rail ol a[href="#h-finish"]').click();
    await page.waitForTimeout(200);
    const ctxText = await page.locator('.ctx-bar .ctx-c').textContent();
    console.log('context bar after jump:', JSON.stringify(ctxText));
    await page.locator('.ctx-bar .ctx-b[aria-label="Další kapitola"]').click();
    await page.waitForTimeout(200);
    console.log('context bar after next:', JSON.stringify(await page.locator('.ctx-bar .ctx-c').textContent()));
    // palette
    await page.keyboard.press('Control+k');
    await page.locator('#pal-in').fill('handshake');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(200);
    console.log('palette jump to handshake:', JSON.stringify(await inView(page, 'm-handshake')));
    console.log('page errors:', JSON.stringify(errors));
    failures += errors.length;
    // external links
    const ext = await page.evaluate(() => Array.from(new Set(Array.from(document.querySelectorAll('a[href^="http"]')).map((a) => a.getAttribute('href')))));
    let extBad = [];
    for (const u of ext) {
      let status = 0;
      for (let attempt = 0; attempt < 4; attempt++) {
        try { const r = await ctx.request.get(u, { timeout: 25000, maxRedirects: 5 }); status = r.status(); } catch (e) { status = -1; }
        if (status !== 429 && status !== 503) break;
        await page.waitForTimeout(4000 * (attempt + 1));
      }
      if (status !== 200) extBad.push(status + ' ' + u);
      if (u.indexOf('github.com') !== -1) await page.waitForTimeout(350);
    }
    console.log(`external links: ${ext.length - extBad.length}/${ext.length} ok` + (extBad.length ? '  BAD: ' + extBad.join('; ') : ''));
    await ctx.close();
  }

  // ---------- 2. inside a sandboxed iframe (as an artifact is served) ----------
  {
    const ctx = await browser.newContext({ viewport: { width: 1280, height: 900 }, reducedMotion: 'reduce' });
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(e.message));
    page.on('console', (m) => { if (m.type() === 'error') errors.push('console: ' + m.text().slice(0, 140)); });
    await page.goto('file://' + path.join(OUT, 'pw-host.html'));
    await page.waitForTimeout(1200);
    const frame = page.frames().find((f) => f.url().endsWith('pw-page.html'));
    console.log('== desktop, sandboxed iframe (opaque origin)');
    failures += await auditGroup(frame, 'nav.rail ol a', 'sidebar chapters');
    failures += await auditGroup(frame, 'nav.rail p.grp a', 'sidebar parts');
    failures += await auditGroup(frame, 'main section .see a', 'see-also');
    failures += await auditGroup(frame, '.part-grid a.pg', 'part overview cards');
    console.log('frame errors:', JSON.stringify(errors.slice(0, 6)));
    await ctx.close();
  }

  // ---------- 3. phone width: table of contents collapses ----------
  {
    const ctx = await browser.newContext({ viewport: { width: 390, height: 800 }, reducedMotion: 'reduce', hasTouch: true });
    const page = await ctx.newPage();
    await page.goto('file://' + path.join(OUT, 'pw-page.html'));
    await page.waitForTimeout(800);
    console.log('== phone width');
    const loc = page.locator('nav.rail ol a');
    const n = await loc.count();
    let ok = 0; const bad = [];
    for (let i = 0; i < n; i += 6) {
      const open = await page.evaluate(() => document.getElementById('toc').open);
      if (!open) await page.locator('#toc > summary').click();
      const a = loc.nth(i);
      const id = (await a.getAttribute('href')).slice(1);
      await a.scrollIntoViewIfNeeded();
      await a.click();
      await page.waitForTimeout(80);
      const v = await inView(page, id);
      if (v.ok) ok++; else bad.push(id + ' top=' + v.top);
    }
    console.log(`sidebar chapters (every 6th): ${ok}/${Math.ceil(n / 6)} ok` + (bad.length ? '  BAD: ' + bad.join('; ') : ''));
    failures += bad.length;
    await ctx.close();
  }

  await browser.close();
  console.log(failures ? `FAILURES: ${failures}` : 'ALL LINK GROUPS OK');
  process.exit(failures ? 1 : 0);
})();
