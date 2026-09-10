// The Cockpit measured in a real browser. Driven by scripts/cockpit-probe, which starts the
// server and hands this script its URL; every route it visits is derived from that server,
// never listed here.
//
// The derivation is not this file's. `scripts/lib/ui-routes.mjs` crawls a served surface out
// of its own anchors and files what it finds into families, and both browser instruments
// over this repository — this probe and the UI audit — import it, so the two cannot disagree
// about what a Cockpit route is. A capability, a kind or a graph added to the backend is
// probed the first time it exists, and a page added to the Cockpit is probed the first time
// something links to it.
//
// What this probe still owns is what it asserts, which is not what the audit asserts: the
// shell, the security headers, the design fingerprint, the interactions and the claim that
// the browser layer is optional. The audit owns the contrast, the accessibility engine, the
// landmarks, the components and the width sweep. Two instruments, one page set.
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

import { crawl, documentFetcher, FAMILY_SAMPLE, sample as sampleFamilies, spread } from './ui-routes.mjs';

const BASE = process.argv[2];
const MODE = process.argv[3] || 'full'; // full | quick
// The widths come from the design declaration through `/api/v1/design`, read once the
// server answers; nothing here holds a width. Until then the sweep has none.
let WIDTHS = [];
/** The design as the executable carries it: the fingerprint every stylesheet must match. */
let DESIGN = null;
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
 * Every Cockpit route this server serves, crawled out of the Cockpit itself.
 *
 * The mount is a proper prefix of every route the surface owns, so no other surface's
 * mount has to be named for the crawl to stay inside it.
 */
async function routes() {
  return crawl({ mount: '/cockpit', fetchText: documentFetcher(BASE) });
}

/**
 * The routes the browser visits: everything the shell puts in front of a reader, because
 * each of those is its own page function, and a sample of each generated family, because a
 * thousand capability pages share one renderer and what is worth visiting is a spread
 * across them. Quick mode narrows the families to one member each and leaves the
 * navigation whole.
 */
function sample(derived) {
  return sampleFamilies(derived, MODE === 'quick' ? 1 : FAMILY_SAMPLE);
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

/**
 * The contract between the page and the declaration: the stylesheet the page loaded
 * carries the fingerprint the executable was built with (`--mj-design` against
 * `data-design`), the body renders in the declared type stack, and every badge word on the
 * page is collected for the vocabulary check. What makes this a cross-surface test is that
 * the site probe asks the same question of the same fingerprint.
 */
async function designContract(page, route, badgeWords) {
  const seen = await page.evaluate(() => {
    const root = getComputedStyle(document.documentElement);
    const body = getComputedStyle(document.body);
    return {
      served: root.getPropertyValue('--mj-design').trim().replace(/^"|"$/g, ''),
      built: document.body.dataset.design || '',
      font: body.fontFamily.split(',')[0].trim(),
      words: [...document.querySelectorAll('[class*="mj-badge--"],[class*="mj-status--"]')].flatMap((e) =>
        [...e.classList].filter((c) => /^mj-(badge|status)--/.test(c)).map((c) => c.replace(/^mj-(badge|status)--/, '')),
      ),
    };
  });
  for (const w of seen.words) badgeWords.add(w);
  if (!seen.served) fail('design', `${route}: the stylesheet carries no --mj-design`);
  else if (seen.served !== DESIGN.design)
    fail('design', `${route}: the stylesheet carries design ${seen.served}; the executable answers ${DESIGN.design}`);
  if (seen.built !== DESIGN.design)
    fail('design', `${route}: the page carries data-design "${seen.built}"; the executable answers ${DESIGN.design}`);
  if (!/^(ui-sans-serif|system-ui|-apple-system|")/.test(seen.font))
    fail('design', `${route}: body renders in '${seen.font}', not the declared stack`);
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
    // and the filtering is rendered on the query event the component dispatches after the
    // input, not on the input itself: when the registry was in before the fill, the ready
    // render still shows every page, and the filtered render lands a beat later. Wait for
    // the list to reflect the query rather than for a fixed number of milliseconds.
    await page
      .waitForFunction(
        () => {
          const first = document.querySelector('.mj-palette-results li[data-href]');
          return !first || first.dataset.href.includes('objects.search');
        },
        null,
        { timeout: 5000 },
      )
      .catch(() => {});
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
  const before = await page.evaluate(() => document.documentElement.classList.contains(document.body.dataset.themeClass));
  await page.click('.mj-theme-toggle');
  const after = await page.evaluate(() => document.documentElement.classList.contains(document.body.dataset.themeClass));
  if (before === after) {
    fail('theme', 'the toggle did not change the theme');
  } else {
    await page.reload({ waitUntil: 'domcontentloaded' });
    const kept = await page.evaluate(() => document.documentElement.classList.contains(document.body.dataset.themeClass));
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
async function withoutJavaScript(browser, derived) {
  const context = await browser.newContext({ javaScriptEnabled: false });
  const page = await context.newPage();
  // one member of each family, spread: the claim is about the renderers, and there is one
  // renderer per family. Nothing is named here either.
  const perFamily = Object.values(derived.families).map((routes) => routes[0]);
  const sampled = spread([derived.entry, ...perFamily.filter((r) => r !== derived.entry)], MODE === 'quick' ? 3 : 8);
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
  const derived = await routes();
  const total = derived.routes.length;
  // the design contract: the widths, the fingerprint the stylesheet must carry, and the
  // vocabulary every badge word must be filed under
  DESIGN = await api('/api/v1/design');
  WIDTHS = DESIGN.viewports;
  const vocabulary = new Set(
    (await api('/api/v1/design/tokens')).tokens
      .filter((t) => t.kind === 'status' || t.kind === 'state')
      .map((t) => t.name),
  );
  const badgeWords = new Set();
  const visiting = sample(derived);
  ok(
    'routes',
    `${total} route(s) crawled out of the Cockpit in ${derived.fetched} fetch(es), in ${Object.keys(derived.families).length} family(ies), ${derived.navigation.length} of them advertised by the shell; ${visiting.length} visited in a browser at ${WIDTHS.length} widths`,
  );

  const context = await browser.newContext();
  for (const route of visiting) {
    const page = await context.newPage();
    watch(page, route);
    let first = true;
    for (const width of WIDTHS) {
      await page.setViewportSize({ width, height: 1000 });
      // a route that never goes quiet — a page that polls a slow API on a machine with
      // many worktrees — is a finding about that route, not the end of the sweep
      let response;
      try {
        response = await page.goto(BASE + route, { waitUntil: 'networkidle' });
      } catch (e) {
        fail('load', `${route} @${width}px: ${String(e.message || e).split('\n')[0].slice(0, 120)}`);
        break;
      }
      if (first) {
        if (!(await shell(page, route, response))) break;
        first = false;
        await designContract(page, route, badgeWords);
      }
      await overflow(page, route, width);
    }
    await page.close();
  }
  for (const word of [...badgeWords].sort()) {
    if (!vocabulary.has(word)) {
      fail('design', `badge word '${word}' is filed under no status in share/design/tokens.yaml`);
    }
  }
  if (!findings.length) ok('pages', `every visited route carried the shell, the policy, the design ${DESIGN.design} and no overflow`);

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
    await withoutJavaScript(browser, derived);
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
