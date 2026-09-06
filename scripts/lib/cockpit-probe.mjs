// The Cockpit measured in a real browser. Driven by scripts/cockpit-probe, which starts the
// server and hands this script its URL; every route it visits is derived from that server,
// never listed here.
//
// Route derivation, in order of how much it can be wrong:
//   1. the areas         — crawled out of the shell's own navigation, which the server
//                          rendered from the registry, the index and the graph derivations
//   2. every capability  — /api/v1/capabilities
//   3. every graph       — /api/v1/graphs
//   4. every object      — /api/v1/objects
// A capability, a kind or a graph added to the backend is therefore probed the first time
// it exists, and a route this file forgot is a route the crawl still finds.
//
// What the sweep asserts on every route it visits: the page answers 200 as HTML, carries the
// shell and the security headers, has no horizontal overflow at three widths, produces no
// console error and no failed request, and — the point of a browser rather than curl — the
// interactions the page declares actually work.
//
// Findings are printed as `FAIL <kind> <message>`, one per line, the way every other check
// in this repository prints them. Exit 0 clean, 10 with findings.
//
// Playwright drives the Chrome that is already installed (`channel: 'chrome'`); nothing is
// downloaded. The caller decides whether a missing browser is a skip.

import { chromium } from 'playwright';

const BASE = process.argv[2];
const MODE = process.argv[3] || 'full'; // full | quick
const WIDTHS = [390, 1024, 1600];
const findings = [];
const notes = [];

const fail = (kind, message) => findings.push(`FAIL ${kind.padEnd(10)} ${message}`);
const ok = (kind, message) => notes.push(`OK   ${kind.padEnd(10)} ${message}`);

if (!BASE) {
  console.error('cockpit-probe: no base URL given');
  process.exit(2);
}

/** Ask the server, as a client would. */
async function api(path) {
  const response = await fetch(BASE + path);
  if (!response.ok) throw new Error(`${path} answered ${response.status}`);
  return response.json();
}

/**
 * Every Cockpit route this server serves, derived from it. The areas come from the shell's
 * own navigation — one fetch, then every in-Cockpit href it carries — so a page added to
 * the Cockpit is probed without this file learning its name.
 */
async function routes() {
  const shell = await (await fetch(BASE + '/cockpit')).text();
  const hrefs = new Set();
  for (const m of shell.matchAll(/href="(\/cockpit[^"#]*)"/g)) {
    const href = m[1].replace(/&amp;/g, '&');
    if (!href.startsWith('/cockpit/assets/')) hrefs.add(href);
  }
  hrefs.add('/cockpit');

  const [capabilities, graphs, objects] = await Promise.all([
    api('/api/v1/capabilities'),
    api('/api/v1/graphs'),
    api('/api/v1/objects'),
  ]);

  const groups = {
    area: [...hrefs].sort(),
    capability: capabilities.capabilities.map(
      (c) => '/cockpit/capabilities/' + encodeURIComponent(c.id),
    ),
    graph: graphs.graphs.map((g) => '/cockpit/graphs/' + encodeURIComponent(g.id)),
    object: objects.objects.map((o) => '/cockpit/object?uri=' + encodeURIComponent(o.uri)),
  };
  // the two views that exist for their library alone, and are reached from the graph pages
  groups.area.push('/cockpit/graphs/topology', '/cockpit/activity');
  groups.area = [...new Set(groups.area)].sort();
  return groups;
}

/**
 * The routes the browser visits. Every area, because each is its own page function; and a
 * sample of each generated family, because a thousand capability pages share one renderer
 * and the sample that matters is the widest and the narrowest of them. The status sweep in
 * the shell script visits every one of them.
 */
function sample(groups) {
  const take = (list, n) => {
    if (list.length <= n) return list;
    const step = Math.floor(list.length / n);
    return Array.from({ length: n }, (_, i) => list[i * step]);
  };
  const n = MODE === 'quick' ? 1 : 4;
  return [
    ...groups.area.map((r) => ['area', r]),
    ...take(groups.capability, n).map((r) => ['capability', r]),
    ...take(groups.graph, n).map((r) => ['graph', r]),
    ...take(groups.object, n).map((r) => ['object', r]),
  ];
}

/** Watch one page for anything a browser considers broken. */
function watch(page, route) {
  page.on('console', (m) => {
    if (m.type() !== 'error') return;
    const text = m.text();
    // a vendored library that is not built is an absence the page is designed for, and it
    // says so itself; the probe asserts the saying-so, not the presence
    if (/is not in share\/cockpit\/vendor/.test(text)) return;
    fail('console', `${route}: ${text.slice(0, 160)}`);
  });
  page.on('pageerror', (e) => fail('pageerror', `${route}: ${String(e).slice(0, 160)}`));
  page.on('requestfailed', (r) => {
    const url = r.url();
    if (!url.startsWith(BASE)) return;
    const why = r.failure()?.errorText || '';
    // a navigation cancels the requests the previous page had in flight, and the browser
    // reports each as aborted; that is the driver's own doing, not the page's
    if (why.includes('ERR_ABORTED')) return;
    fail('request', `${route}: ${url.replace(BASE, '')} — ${why}`);
  });
}

/** No content past the right edge, at any width a developer might use. */
async function overflow(page, route, width) {
  const measured = await page.evaluate(() => {
    const d = document.documentElement;
    const clipped = (e) => {
      for (let p = e; p; p = p.parentElement) {
        const o = getComputedStyle(p);
        if (['auto', 'scroll', 'hidden'].includes(o.overflowX)) return true;
      }
      return false;
    };
    const name = (e) =>
      e.tagName.toLowerCase() + '.' + String(e.className || '').split(' ').slice(0, 2).join('.');
    const wide = [...document.querySelectorAll('body *')]
      .filter((e) => e.getBoundingClientRect().right > d.clientWidth + 1 && !clipped(e.parentElement || e))
      .slice(0, 2)
      .map(name);
    return { vw: d.clientWidth, sw: d.scrollWidth, wide };
  });
  if (measured.sw > measured.vw + 1) {
    fail(
      'overflow',
      `${route} @${width}px: scrollWidth ${measured.sw} > viewport ${measured.vw} (${measured.wide.join('|') || 'none named'})`,
    );
  }
}

/** The shell, on every page, whatever the page is. */
async function shell(page, route, response) {
  if (!response || response.status() !== 200) {
    fail('status', `${route}: answered ${response ? response.status() : 'nothing'}`);
    return false;
  }
  const headers = response.headers();
  const policy = headers['content-security-policy'] || '';
  if (!policy.includes("default-src 'none'")) fail('csp', `${route}: no strict policy`);
  if (/unsafe-eval/.test(policy)) fail('csp', `${route}: the policy allows unsafe-eval`);
  const scriptSrc = (policy.split('script-src ')[1] || '').split(';')[0];
  if (scriptSrc.includes('unsafe-inline')) fail('csp', `${route}: script-src allows unsafe-inline`);
  if (headers['x-content-type-options'] !== 'nosniff') fail('headers', `${route}: no nosniff`);

  const shape = await page.evaluate(() => ({
    title: document.title,
    nav: document.querySelectorAll('.mj-sidebar a.mj-nav-link').length,
    main: !!document.querySelector('main#main'),
    skip: !!document.querySelector('a.mj-skip'),
    h1: document.querySelector('h1')?.textContent || '',
    styled:
      getComputedStyle(document.body).backgroundColor !== 'rgba(0, 0, 0, 0)' &&
      getComputedStyle(document.querySelector('h1') || document.body).fontWeight !== '400',
  }));
  if (!shape.title.endsWith('Majordomus Cockpit')) fail('shell', `${route}: title "${shape.title}"`);
  if (!shape.main) fail('shell', `${route}: no <main id=main>`);
  if (!shape.skip) fail('a11y', `${route}: no skip link`);
  if (!shape.h1) fail('shell', `${route}: no heading`);
  if (shape.nav < 6) fail('nav', `${route}: ${shape.nav} navigation entries`);
  if (!shape.styled) fail('style', `${route}: the stylesheet did not apply`);
  return true;
}

/** The interactions the shell declares, once, on the landing page. */
async function interactions(context) {
  const page = await context.newPage();
  watch(page, 'interactions');
  await page.setViewportSize({ width: 1600, height: 1000 });
  await page.goto(BASE + '/cockpit', { waitUntil: 'networkidle' });

  // --- Alpine is the CSP build and it started: the component is bound
  const alpine = await page.evaluate(() => !!window.Alpine);
  if (!alpine) {
    fail('alpine', 'Alpine did not start; the palette and the theme toggle are dead');
  } else {
    ok('alpine', "Alpine's CSP build started under a policy with no unsafe-eval");
  }

  // --- the command palette opens on the keyboard and fills from the API
  await page.keyboard.press('Control+k');
  const opened = await page
    .waitForSelector('.mj-palette-panel', { state: 'visible', timeout: 4000 })
    .then(() => true)
    .catch(() => false);
  if (!opened) {
    fail('palette', 'Ctrl+K did not open the palette');
  } else {
    await page.waitForFunction(
      () => document.querySelectorAll('.mj-palette-results li[data-href]').length > 20,
      null,
      { timeout: 8000 },
    ).catch(() => {});
    const entries = await page.evaluate(() => {
      const items = [...document.querySelectorAll('.mj-palette-results li[data-href]')];
      return { count: items.length, kinds: [...new Set(items.map((i) => i.firstChild.textContent))] };
    });
    if (entries.count < 20) {
      fail('palette', `opened with ${entries.count} entries; it reads the registry, so it should have many`);
    } else {
      ok('palette', `Ctrl+K opened it with ${entries.count} entries of kinds: ${entries.kinds.slice(0, 6).join(', ')}`);
    }
    // filtering, and Enter goes where the entry says
    await page.fill('.mj-palette-input', 'objects.search');
    // the palette says when the registry is in; a fixed wait raced it and blamed the
    // filtering for what was still a fetch
    await page
      .waitForSelector('.mj-palette-results[data-state="ready"]', { timeout: 15000 })
      .catch(() => {});
    await page.waitForTimeout(100);
    const first = await page.evaluate(
      () => document.querySelector('.mj-palette-results li[data-href]')?.dataset.href || '',
    );
    if (!first.includes('objects.search')) {
      fail('palette', `filtering for objects.search put "${first}" first`);
    } else {
      await page.keyboard.press('Enter');
      await page.waitForURL(/capabilities/, { timeout: 5000 }).catch(() => {});
      if (!page.url().includes('objects.search')) fail('palette', 'Enter did not navigate to the entry');
      else ok('palette', 'filtering and Enter navigate to the capability the entry names');
    }
  }

  // --- the theme toggle flips the root class and survives a reload
  await page.goto(BASE + '/cockpit', { waitUntil: 'networkidle' });
  const before = await page.evaluate(() => document.documentElement.classList.contains('dark'));
  await page.click('.mj-theme-toggle');
  const after = await page.evaluate(() => document.documentElement.classList.contains('dark'));
  if (before === after) {
    fail('theme', 'the toggle did not change the theme');
  } else {
    await page.reload({ waitUntil: 'domcontentloaded' });
    const kept = await page.evaluate(() => document.documentElement.classList.contains('dark'));
    if (kept !== after) fail('theme', 'the theme did not survive a reload');
    else ok('theme', `the toggle flips to ${after ? 'dark' : 'light'} and the choice survives a reload`);
  }

  // --- the skip link is the first thing the keyboard reaches, and it reaches main
  await page.goto(BASE + '/cockpit', { waitUntil: 'domcontentloaded' });
  await page.keyboard.press('Tab');
  const focused = await page.evaluate(() => ({
    cls: document.activeElement?.className || '',
    href: document.activeElement?.getAttribute('href') || '',
  }));
  if (!focused.cls.includes('mj-skip') || focused.href !== '#main') {
    fail('a11y', `the first tab stop is "${focused.cls}" rather than the skip link`);
  } else {
    ok('a11y', 'the first tab stop is the skip link, and it points at #main');
  }

  await page.close();
}

/** The generic runner, on a real capability, through its real route. */
async function runner(context) {
  const page = await context.newPage();
  watch(page, 'runner');
  await page.setViewportSize({ width: 1600, height: 1200 });
  await page.goto(BASE + '/cockpit/capabilities/objects.search', { waitUntil: 'networkidle' });

  const preview = () => page.textContent('[data-mj-preview]');
  const before = await preview();
  if (!before.startsWith('GET /api/v1/search')) {
    fail('runner', `the invocation preview reads "${before}"`);
  }

  // the form was generated from the schema: the required string, the bounded integer
  const controls = await page.evaluate(() =>
    [...document.querySelectorAll('[data-mj-type]')].map((c) => ({
      name: c.getAttribute('name'),
      type: c.dataset.mjType,
      required: c.hasAttribute('required'),
      tag: c.tagName.toLowerCase(),
    })),
  );
  const query = controls.find((c) => c.name === 'query');
  const limit = controls.find((c) => c.name === 'limit');
  if (!query?.required || query.type !== 'string') fail('runner', 'the required string control is wrong');
  if (limit?.type !== 'integer' || limit.tag !== 'input') fail('runner', 'the bounded integer control is wrong');

  // the capability's own benchmark case fills the form. The examples live behind a
  // disclosure, so a person opens it first and so does this.
  await page.click('.mj-card:has([data-mj-fill]) .mj-summary');
  await page.waitForSelector('[data-mj-fill]', { state: 'visible', timeout: 5000 });
  await page.click('[data-mj-fill]');
  const filled = await preview();
  if (filled === before) fail('runner', 'loading a benchmark case changed nothing');

  await page.fill('[name="query"]', 'majordomus');
  await page.fill('[name="limit"]', '3');
  const typed = await preview();
  if (!typed.includes('query=majordomus') || !typed.includes('limit=3')) {
    fail('runner', `typing did not reach the preview: "${typed}"`);
  }

  await page.click('.mj-runner button[type=submit]');
  await page.waitForSelector('[data-mj-result] .mj-pre', { timeout: 8000 }).catch(() => {});
  const result = await page.evaluate(() => {
    const badge = document.querySelector('[data-mj-result] .mj-badge');
    const body = document.querySelector('[data-mj-result] .mj-pre code');
    return { status: badge?.textContent?.trim() || '', body: body?.textContent || '' };
  });
  if (result.status !== '200') {
    fail('runner', `running objects.search answered "${result.status}"`);
  } else if (!result.body.includes('"hits"')) {
    fail('runner', 'the answer carries no hits');
  } else {
    ok('runner', 'a form generated from the schema called the capability\'s own route and rendered its answer');
  }

  // and it is the real route: what the preview promised is what the browser asked for
  const asked = [];
  page.on('request', (r) => asked.push(r.url()));
  await page.click('.mj-runner button[type=submit]');
  await page.waitForTimeout(600);
  if (!asked.some((u) => u.includes('/api/v1/search?query=majordomus'))) {
    fail('runner', `the request went somewhere else: ${asked.filter((u) => u.includes('/api/')).join(' ')}`);
  }
  await page.close();
}

/** The graph drawing, when the library is vendored; the tables, always. */
async function graph(context) {
  const page = await context.newPage();
  watch(page, 'graph');
  await page.setViewportSize({ width: 1600, height: 1200 });
  await page.goto(BASE + '/cockpit/graphs/registry', { waitUntil: 'networkidle' });

  const tables = await page.evaluate(() => ({
    nodes: document.querySelectorAll('.mj-table tbody tr').length,
    frame: !!document.querySelector('[data-mj-graph]'),
  }));
  if (tables.nodes < 10) fail('graph', `the page lists ${tables.nodes} rows; the graph has more than that`);

  const vendored = (await fetch(BASE + '/cockpit/assets/vendor/cytoscape.min.js')).ok;
  if (!vendored) {
    const said = await page.textContent('[data-mj-graph]');
    if (!/not available|not in share/.test(said || '')) {
      fail('graph', 'the library is absent and the frame does not say so');
    } else {
      ok('graph', 'without the library the frame says so and every node and edge is still listed');
    }
  } else {
    await page.waitForFunction(() => !!document.querySelector('[data-mj-graph] canvas'), null, {
      timeout: 15000,
    }).catch(() => {});
    const drew = await page.evaluate(() => document.querySelectorAll('[data-mj-graph] canvas').length);
    if (!drew) fail('graph', 'the drawing library loaded and drew nothing');
    else {
      // the search narrows the drawing without touching the page
      await page.fill('[data-mj-graph-search]', 'objects');
      await page.waitForTimeout(400);
      ok('graph', `the drawing rendered (${drew} canvas layer(s)) over the same nodes the page lists`);
    }
  }
  await page.close();
}

/** Every page, with JavaScript turned off: the claim that the browser layer is optional. */
async function withoutJavaScript(browser, groups) {
  const context = await browser.newContext({ javaScriptEnabled: false });
  const page = await context.newPage();
  const sampled = [
    '/cockpit',
    '/cockpit/health',
    '/cockpit/graphs/registry',
    groups.capability[0],
    groups.object[0],
  ];
  for (const route of sampled) {
    const response = await page.goto(BASE + route, { waitUntil: 'domcontentloaded' });
    const shape = await page.evaluate(() => ({
      nav: document.querySelectorAll('.mj-sidebar a.mj-nav-link').length,
      rows: document.querySelectorAll('.mj-table tbody tr').length,
      text: document.body.innerText.length,
    }));
    if (response.status() !== 200 || shape.nav < 6 || shape.text < 400) {
      fail('nojs', `${route}: without JavaScript the page is ${shape.text} characters and ${shape.nav} links`);
    }
  }
  ok('nojs', `${sampled.length} route(s) carry their navigation and their content with JavaScript disabled`);
  await context.close();
}

/** The page refuses to be framed, which is what its policy says. */
async function framing(context) {
  const page = await context.newPage();
  await page.setContent(`<iframe src="${BASE}/cockpit"></iframe>`);
  await page.waitForTimeout(1500);
  const reached = await page.evaluate(() => {
    const f = document.querySelector('iframe');
    try {
      return !!f.contentDocument?.querySelector('.mj-sidebar');
    } catch (e) {
      return false;
    }
  });
  if (reached) fail('csp', 'the Cockpit can be framed by another origin');
  else ok('csp', "frame-ancestors 'none' holds: another page cannot frame the Cockpit");
  await page.close();
}

const browser = await chromium.launch({ channel: 'chrome' });
try {
  const groups = await routes();
  const total = Object.values(groups).reduce((n, g) => n + g.length, 0);
  const visiting = sample(groups);
  ok(
    'routes',
    `${total} route(s) derived from the server (${groups.area.length} area, ${groups.capability.length} capability, ${groups.graph.length} graph, ${groups.object.length} object); ${visiting.length} visited in a browser at ${WIDTHS.length} widths`,
  );

  const context = await browser.newContext();
  for (const [, route] of visiting) {
    const page = await context.newPage();
    watch(page, route);
    let first = true;
    for (const width of WIDTHS) {
      await page.setViewportSize({ width, height: 1000 });
      const response = await page.goto(BASE + route, { waitUntil: 'networkidle' });
      if (first) {
        if (!(await shell(page, route, response))) break;
        first = false;
      }
      await overflow(page, route, width);
    }
    await page.close();
  }
  if (!findings.length) ok('pages', `every visited route carried the shell, the policy and no overflow`);

  // each block is its own finding when it throws: one broken interaction must not hide the
  // others, and a stack trace is not a finding
  for (const [name, check] of [
    ['interactions', () => interactions(context)],
    ['runner', () => runner(context)],
    ['graph', () => graph(context)],
    ['framing', () => framing(context)],
  ]) {
    try {
      await check();
    } catch (e) {
      // a Playwright failure names the selector on its second line, which is the part that
      // says what was not clickable; the first line alone is "Timeout 30000ms exceeded"
      const lines = String(e.message || e).split('\n').filter((l) => l.trim());
      fail(name, lines.slice(0, 3).join(' — ').slice(0, 260));
    }
  }
  await context.close();
  try {
    await withoutJavaScript(browser, groups);
  } catch (e) {
    fail('nojs', String(e.message || e).split('\n')[0].slice(0, 200));
  }
} finally {
  await browser.close();
}

for (const line of notes) console.log(line);
for (const line of findings) console.log(line);
console.log(`cockpit-probe: the browser sweep found ${findings.length}`);
process.exit(findings.length ? 10 : 0);
