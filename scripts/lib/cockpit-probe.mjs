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

import { readFileSync } from 'node:fs';

import { chromium } from 'playwright';

import { crawl, documentFetcher, FAMILY_SAMPLE, sample as sampleFamilies, spread } from './ui-routes.mjs';

const BASE = process.argv[2];
const MODE = process.argv[3] || 'full'; // full | quick | drawer | phone
// The widths come from the design declaration through `/api/v1/design`, read once the
// server answers; nothing here holds a width. Until then the sweep has none.
let WIDTHS = [];
/** The design as the executable carries it: the fingerprint every stylesheet must match. */
let DESIGN = null;
/**
 * The least of the frame, in each direction, that a drawing must occupy before it is a
 * drawing at all. Not a quality bar — a floor. `breadthfirst` over the composed graph put
 * 1442 nodes on one BFS level, so `fit()` zoomed out until that row fitted the frame's
 * width and the result was about 1 pixel tall: 0.16% down, over three healthy canvas
 * layers. Anything at or under a tenth of the frame in either direction is that failure.
 */
const MIN_DRAWN = 0.1;
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
          // Not `!first || ...`: an empty list satisfied that immediately, so the wait
          // returned at once whenever the registry had not arrived, and the assertion
          // below then read nothing and blamed the ordering. Wait for a result to exist
          // AND to be the one asked for; an empty list is a timeout, which is the truth.
          const first = document.querySelector('.mj-palette-results li[data-href]');
          return !!first && first.dataset.href.includes('objects.search');
        },
        null,
        { timeout: 5000 },
      )
      .catch(() => {});
    // Three outcomes, and they are not one verdict: no result at all, a result that is
    // the wrong one, and the right one. Reporting the first as `put "" first` described
    // an ordering that never happened and sent a reader looking for a sort bug.
    const { count, first } = await page.evaluate(() => {
      const rows = document.querySelectorAll('.mj-palette-results li[data-href]');
      return { count: rows.length, first: rows[0]?.dataset.href || '' };
    });
    if (count === 0) {
      fail(
        'palette',
        'filtering for objects.search produced no entries at all — the registry did not ' +
          'arrive or the filter matched nothing; this is not an ordering failure',
      );
    } else if (!first.includes('objects.search')) {
      fail('palette', `filtering for objects.search put "${first}" first of ${count}`);
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
async function runner(context, width = 1600) {
  const page = await context.newPage();
  const at = width < 1024 ? `@${width}px ` : '';
  watch(page, `runner ${at}`.trim());
  await page.setViewportSize({ width, height: width < 1024 ? 740 : 1200 });
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
    fail('runner', `${at}running objects.search answered "${result.status}"`);
  } else if (!result.body.includes('"hits"')) {
    fail('runner', `${at}the answer carries no hits`);
  } else {
    ok('runner', `${at}a form generated from the schema called the capability's own route and rendered its answer`);
  }

  // A long answer is read where it is, not by widening the page: the block scrolls inside
  // itself, and the form and its button stay inside the viewport. At a phone's width the
  // JSON is wider than the screen, so this is the case that matters there.
  const fits = await page.evaluate(() => {
    const d = document.documentElement;
    const pre = document.querySelector('[data-mj-result] .mj-pre');
    const button = document.querySelector('.mj-runner button[type=submit]').getBoundingClientRect();
    const wider = [...document.querySelectorAll('.mj-runner input, .mj-runner select, .mj-runner textarea')]
      .filter((c) => c.getBoundingClientRect().right > d.clientWidth + 1).length;
    return {
      page: d.scrollWidth <= d.clientWidth + 1,
      preScrolls: !pre || pre.scrollWidth <= pre.clientWidth + 1 || ['auto', 'scroll'].includes(getComputedStyle(pre).overflowX),
      button: button.left >= 0 && button.right <= d.clientWidth + 1 && button.height >= 24,
      wider,
    };
  });
  if (!fits.page) fail('runner', `${at}the answer widened the page`);
  if (!fits.preScrolls) fail('runner', `${at}the answer block neither fits nor scrolls inside itself`);
  if (!fits.button) fail('runner', `${at}the run button is outside the viewport or under 24px`);
  if (fits.wider) fail('runner', `${at}${fits.wider} form control(s) reach past the viewport`);

  // and it is the real route: what the preview promised is what the browser asked for
  const asked = [];
  page.on('request', (r) => asked.push(r.url()));
  await page.click('.mj-runner button[type=submit]');
  await page.waitForTimeout(600);
  if (!asked.some((u) => u.includes('/api/v1/search?query=majordomus'))) {
    fail('runner', `${at}the request went somewhere else: ${asked.filter((u) => u.includes('/api/')).join(' ')}`);
  }
  await page.close();
}

/** The graph drawing, when the library is vendored; the tables, always. */
async function graph(context, width = 1600) {
  const page = await context.newPage();
  const at = width < 1024 ? `@${width}px ` : '';
  watch(page, `graph ${at}`.trim());
  await page.setViewportSize({ width, height: width < 1024 ? 740 : 1200 });
  await page.goto(BASE + '/cockpit/graphs/registry', { waitUntil: 'networkidle' });

  const tables = await page.evaluate(() => ({
    nodes: document.querySelectorAll('.mj-table tbody tr').length,
    frame: !!document.querySelector('[data-mj-graph]'),
  }));
  if (tables.nodes < 10) fail('graph', `${at}the page lists ${tables.nodes} rows; the graph has more than that`);

  const vendored = (await fetch(BASE + '/cockpit/assets/vendor/cytoscape.min.js')).ok;
  if (!vendored) {
    const said = await page.textContent('[data-mj-graph]');
    if (!/not available|not in share/.test(said || '')) {
      fail('graph', `${at}the library is absent and the frame does not say so`);
    } else {
      ok('graph', `${at}without the library the frame says so and every node and edge is still listed`);
    }
  } else {
    // A canvas element is not a drawing. Counting `[data-mj-graph] canvas` reported three
    // healthy layers over a drawing that was a horizontal line about one pixel tall — every
    // large graph, for as long as this page existed — because Cytoscape always makes three
    // layers and `breadthfirst` put 1400 nodes on one row. So the question asked here is
    // how much of the frame the rendered elements actually occupy, answered by the renderer
    // through `frame.mjGraph.extent()`: a drawing that fills a tenth of the frame in either
    // direction is not a drawing, and this fails.
    await page.waitForFunction(() => !!document.querySelector('[data-mj-graph]')?.mjGraph, null, {
      timeout: 20000,
    }).catch(() => {});
    const drew = await page.evaluate(() => {
      const frame = document.querySelector('[data-mj-graph]');
      return {
        layers: document.querySelectorAll('[data-mj-graph] canvas').length,
        extent: frame && frame.mjGraph ? frame.mjGraph.extent() : null,
        layout: frame && frame.mjGraph ? frame.mjGraph.layout : null,
      };
    });
    if (!drew.layers) fail('graph', `${at}the drawing library loaded and drew nothing`);
    else if (!drew.extent) {
      fail('graph', `${at}${drew.layers} canvas layer(s) and no extent: the viewer never reported what it drew`);
    } else {
      const e = drew.extent;
      const across = e.width / Math.max(e.frameWidth, 1);
      const down = e.height / Math.max(e.frameHeight, 1);
      const said = `${e.nodes} nodes by "${drew.layout}" occupy ${Math.round(e.width)}x${Math.round(e.height)}px of a ${Math.round(e.frameWidth)}x${Math.round(e.frameHeight)}px frame`;
      if (across < MIN_DRAWN || down < MIN_DRAWN) {
        fail('graph', `${at}the drawing is not a drawing: ${said} (${(across * 100).toFixed(1)}% across, ${(down * 100).toFixed(1)}% down; ${MIN_DRAWN * 100}% of each is the least that conveys anything)`);
      } else {
        // the search narrows the drawing without touching the page
        await page.fill('[data-mj-graph-search]', 'objects');
        await page.waitForTimeout(400);
        ok('graph', `${at}${said} over the same nodes the page lists`);
      }
    }
  }
  if (width < 1024) {
    const fits = await page.evaluate(() => {
      const d = document.documentElement;
      const frame = document.querySelector('[data-mj-graph]')?.getBoundingClientRect();
      return { page: d.scrollWidth <= d.clientWidth + 1, frame: !frame || frame.right <= d.clientWidth + 1 };
    });
    if (!fits.page) fail('graph', `${at}the graph page is wider than the screen`);
    if (!fits.frame) fail('graph', `${at}the drawing's frame reaches past the screen`);
  }
  await page.close();
}

/**
 * Which graph is the biggest. Asked, never named: the set of graphs is derived and a probe
 * that hard-codes `composed` stops looking at the largest one the day a larger is derived.
 * `graph.list` describes without deriving, so each is asked for its own metadata.
 */
async function largestGraph() {
  const list = await api('/api/v1/graphs');
  let largest = null;
  let most = -1;
  for (const info of list.graphs || []) {
    const g = await api(`/api/v1/graph?id=${encodeURIComponent(info.id)}`);
    const nodes = g && g.metadata ? g.metadata.nodes : 0;
    if (nodes > most) {
      most = nodes;
      largest = info.id;
    }
  }
  return largest;
}

/**
 * The same question of the biggest graph there is. `registry` above is 162 nodes and was
 * legible even when every graph used one layout; `composed` is 1442 nodes and 34 kinds, and
 * it is the one that rendered as a hairline. A probe that only ever looks at the small
 * graph cannot see the defect that only the large one has.
 */
async function graphAtScale(context, largest) {
  if (!largest) return;
  const page = await context.newPage();
  watch(page, 'graph-scale');
  await page.setViewportSize({ width: 1600, height: 1200 });
  await page.goto(`${BASE}/cockpit/graphs/${largest}`, { waitUntil: 'networkidle' });
  await page.waitForFunction(() => !!document.querySelector('[data-mj-graph]')?.mjGraph, null, {
    timeout: 40000,
  }).catch(() => {});
  const drew = await page.evaluate(() => {
    const frame = document.querySelector('[data-mj-graph]');
    return frame && frame.mjGraph
      ? { extent: frame.mjGraph.extent(), layout: frame.mjGraph.layout, kinds: frame.mjGraph.shape.kinds }
      : null;
  });
  if (!drew) {
    fail('graph-scale', `${largest}: the viewer never reported what it drew`);
  } else {
    const e = drew.extent;
    const across = e.width / Math.max(e.frameWidth, 1);
    const down = e.height / Math.max(e.frameHeight, 1);
    const said = `${largest}: ${e.nodes} nodes, ${drew.kinds} kinds, laid out by "${drew.layout}", occupying ${Math.round(e.width)}x${Math.round(e.height)}px of ${Math.round(e.frameWidth)}x${Math.round(e.frameHeight)}px`;
    if (across < MIN_DRAWN || down < MIN_DRAWN) {
      fail('graph-scale', `${said} — ${(across * 100).toFixed(1)}% across and ${(down * 100).toFixed(1)}% down is a line, not a graph`);
    } else {
      const legend = await page.evaluate(
        () => document.querySelectorAll('[data-mj-graph-legend] .mj-graph-legend-entry').length,
      );
      if (legend < drew.kinds) {
        fail('graph-scale', `${said}, and the legend names ${legend} of its ${drew.kinds} kinds`);
      } else {
        ok('graph-scale', `${said}, with a legend naming every kind`);
      }
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

/**
 * The sidebar as a drawer, below the width where it sits beside the page. Until the drawer
 * existed the sidebar was `hidden` there with no way to show it: a phone had no section
 * navigation at all, and the overflow sweep passed, because a page with nothing on it does
 * not overflow. So this asserts what a person does, on a touch screen, at every width the
 * design declares below `lg` and in landscape: the trigger is there and large enough to
 * tap, the drawer opens over the page as a modal dialog carrying the same catalogue as the
 * sidebar, and every way out (the close control, the backdrop, Escape, an entry followed,
 * the back button, widening the window) leaves no lock, no inert page and no layer behind.
 */
async function drawer(browser) {
  const narrow = WIDTHS.filter((w) => w < 1024);
  const sizes = [...narrow.map((w) => ({ width: w, height: 740 })), { width: 667, height: 375 }];
  // what a page must look like whenever the drawer is closed, whatever closed it
  const closedState = (page) =>
    page.evaluate(() => {
      const nav = document.getElementById('mj-nav');
      const vw = document.documentElement.clientWidth;
      const vh = window.innerHeight;
      const top = document.elementFromPoint(vw / 2, vh / 2);
      return {
        open: nav.hasAttribute('data-open'),
        role: nav.getAttribute('role'),
        locked: document.documentElement.classList.contains('mj-nav-locked'),
        inert: document.querySelectorAll('[inert]').length,
        covered: !!(top && top.closest('.mj-nav-backdrop, #mj-nav')),
        expanded: document.querySelector('.mj-nav-open').getAttribute('aria-expanded'),
      };
    });
  const leftClosed = (s) =>
    !s.open && !s.role && !s.locked && s.inert === 0 && !s.covered && s.expanded === 'false';
  const describe = (s) => JSON.stringify(s);

  const context = await browser.newContext({ hasTouch: true, isMobile: true });
  try {
    for (const size of sizes) {
      const at = `@${size.width}x${size.height}`;
      const page = await context.newPage();
      watch(page, `drawer ${at}`);
      await page.setViewportSize(size);
      await page.goto(BASE + '/cockpit', { waitUntil: 'networkidle' });
      await page.waitForFunction(() => !!window.Alpine);

      // --- closed at first, and the trigger is there to be tapped
      const trigger = (await page.locator('.mj-nav-open').count())
        ? await page.locator('.mj-nav-open').boundingBox()
        : null;
      if (!trigger) {
        fail('drawer', `${at}: no visible trigger; the sections are unreachable`);
        await page.close();
        continue;
      }
      if (trigger.width < 44 || trigger.height < 44) {
        fail('drawer', `${at}: the trigger is ${trigger.width}x${trigger.height}, under 44x44`);
      }
      if (!leftClosed(await closedState(page))) {
        fail('drawer', `${at}: not closed on arrival: ${describe(await closedState(page))}`);
      }

      // --- a tap opens it as a modal dialog over the page, with the sidebar's catalogue
      await page.tap('.mj-nav-open');
      await page.waitForSelector('#mj-nav[data-open]', { state: 'visible', timeout: 3000 });
      const opened = await page.evaluate(() => {
        const nav = document.getElementById('mj-nav');
        const box = nav.getBoundingClientRect();
        const vw = document.documentElement.clientWidth;
        return {
          role: nav.getAttribute('role'),
          modal: nav.getAttribute('aria-modal'),
          name: nav.getAttribute('aria-label'),
          focus: document.activeElement && document.activeElement.className,
          locked: document.documentElement.classList.contains('mj-nav-locked'),
          // the effect, not the class: a class whose rule went missing locks nothing
          lockedStyle: getComputedStyle(document.documentElement).overflow === 'hidden',
          contained: getComputedStyle(nav).overscrollBehaviorY === 'contain',
          mainInert: document.getElementById('main').inert,
          topbarInert: document.querySelector('.mj-topbar').inert,
          entries: nav.querySelectorAll('a.mj-nav-link').length,
          fits: box.left >= 0 && box.right <= vw + 1 && box.top >= 0,
          scrolls: nav.scrollHeight <= nav.clientHeight || getComputedStyle(nav).overflowY === 'auto',
          small: [...nav.querySelectorAll('a.mj-nav-link, .mj-nav-close')]
            .filter((a) => a.getClientRects().length)
            .filter((a) => a.getBoundingClientRect().height < 44).length,
          sw: document.documentElement.scrollWidth,
          vw,
        };
      });
      const problems = [];
      if (opened.role !== 'dialog' || opened.modal !== 'true' || !opened.name) problems.push('not a named modal dialog');
      if (!String(opened.focus).includes('mj-nav-close')) problems.push(`focus on "${opened.focus}", not the close control`);
      if (!opened.locked || !opened.lockedStyle) problems.push('the page behind still scrolls');
      if (!opened.contained) problems.push("scrolling past the drawer's end scrolls the page");
      if (!opened.mainInert || !opened.topbarInert) problems.push('the page behind is still reachable');
      if (!opened.entries) problems.push('it carries no entries');
      if (!opened.fits) problems.push('it does not fit the viewport');
      if (!opened.scrolls) problems.push('its content cannot scroll');
      if (opened.small) problems.push(`${opened.small} control(s) under 44px tall`);
      if (opened.sw > opened.vw + 1) problems.push(`the page overflows: ${opened.sw} > ${opened.vw}`);
      if (problems.length) fail('drawer', `${at}: open, ${problems.join('; ')}`);

      // --- Tab stays inside: backwards from the first control lands on the last
      await page.keyboard.press('Shift+Tab');
      const wrapped = await page.evaluate(() => !!document.activeElement.closest('#mj-nav'));
      if (!wrapped) fail('drawer', `${at}: Shift+Tab left the open drawer`);

      // --- Escape closes, and focus goes back to the trigger
      await page.keyboard.press('Escape');
      let s = await closedState(page);
      const back = await page.evaluate(() => document.activeElement.classList.contains('mj-nav-open'));
      if (!leftClosed(s) || !back) fail('drawer', `${at}: Escape left ${describe(s)}, focus back: ${back}`);

      // --- the backdrop closes it, tapped beside the drawer
      await page.tap('.mj-nav-open');
      await page.waitForSelector('#mj-nav[data-open]', { timeout: 3000 });
      const box = await page.locator('#mj-nav').boundingBox();
      await page.touchscreen.tap(Math.min(size.width - 8, box.x + box.width + 20), size.height / 2);
      s = await closedState(page);
      if (!leftClosed(s)) fail('drawer', `${at}: the backdrop left ${describe(s)}`);

      // --- the close control closes it
      await page.tap('.mj-nav-open');
      await page.waitForSelector('#mj-nav[data-open]', { timeout: 3000 });
      await page.tap('.mj-nav-close');
      s = await closedState(page);
      if (!leftClosed(s)) fail('drawer', `${at}: the close control left ${describe(s)}`);

      // --- Ctrl/Cmd+K over the open drawer: the palette must not open under a modal drawer
      //     whose inert page would hold it, so the drawer closes first and the palette's
      //     field has the focus before anything else is pressed
      await page.tap('.mj-nav-open');
      await page.waitForSelector('#mj-nav[data-open]', { timeout: 3000 });
      await page.keyboard.press('Control+k');
      await page.waitForSelector('.mj-palette-panel', { state: 'visible', timeout: 4000 });
      const palette = await page.evaluate(() => ({
        drawer: document.getElementById('mj-nav').hasAttribute('data-open'),
        focus: document.activeElement && document.activeElement.classList.contains('mj-palette-input'),
        inert: !!document.querySelector('.mj-palette').closest('[inert]'),
      }));
      if (palette.drawer || !palette.focus || palette.inert) {
        fail('drawer', `${at}: Ctrl+K over the open drawer left ${JSON.stringify(palette)}`);
      }
      await page.keyboard.press('Escape');

      // --- tapped fast and often, it ends in one state and not a mixture of two
      for (let i = 0; i < 5; i++) {
        await page.locator('.mj-nav-open').dispatchEvent('click').catch(() => {});
      }
      const settled = await page.evaluate(() => {
        const nav = document.getElementById('mj-nav');
        const open = nav.hasAttribute('data-open');
        const lock = document.documentElement.classList.contains('mj-nav-locked');
        return { open, consistent: open === lock && open === (nav.getAttribute('role') === 'dialog'), backdrops: document.querySelectorAll('.mj-nav-backdrop').length };
      });
      if (!settled.consistent || settled.backdrops !== 1) fail('drawer', `${at}: repeated taps left ${JSON.stringify(settled)}`);
      if (settled.open) await page.keyboard.press('Escape');

      // --- an entry followed leaves the page with the drawer closed, and back restores a
      //     page that is closed too, not the cached one with the drawer still over it
      await page.tap('.mj-nav-open');
      await page.waitForSelector('#mj-nav[data-open]', { timeout: 3000 });
      const from = page.url();
      const entry = page.locator('#mj-nav a.mj-nav-link:not(.mj-nav-link--current)').first();
      await entry.scrollIntoViewIfNeeded();
      await Promise.all([page.waitForURL((u) => u.href !== from, { timeout: 10000 }), entry.tap()]);
      await page.waitForLoadState('networkidle');
      s = await closedState(page);
      if (!leftClosed(s)) fail('drawer', `${at}: after following an entry, ${describe(s)}`);
      await page.goBack({ waitUntil: 'networkidle' });
      s = await closedState(page);
      if (!leftClosed(s)) fail('drawer', `${at}: after going back, ${describe(s)}`);

      // --- open, then forward and back (a phone's back gesture with the drawer open): the
      //     page back restores, from the back-forward cache when the browser keeps one,
      //     must not come back with the drawer over it
      await page.tap('.mj-nav-open');
      await page.waitForSelector('#mj-nav[data-open]', { timeout: 3000 });
      await page.goForward({ waitUntil: 'networkidle' });
      await page.goBack({ waitUntil: 'networkidle' });
      s = await closedState(page);
      const cached = await page.evaluate(() => performance.getEntriesByType('navigation')[0]?.type);
      if (!leftClosed(s)) fail('drawer', `${at}: forward then back with the drawer open (${cached}), ${describe(s)}`);
      // A browser driven by Playwright reloads on back rather than restoring from the
      // back-forward cache (measured: no page state survives, notRestoredReasons is null), so
      // the restore a phone performs is delivered here as the event it fires: a `pageshow`
      // whose `persisted` is true, over a page with the drawer open.
      await page.tap('.mj-nav-open');
      await page.waitForSelector('#mj-nav[data-open]', { timeout: 3000 });
      await page.evaluate(() => window.dispatchEvent(new PageTransitionEvent('pageshow', { persisted: true })));
      s = await closedState(page);
      if (!leftClosed(s)) fail('drawer', `${at}: a page restored from the back-forward cache kept the drawer, ${describe(s)}`);

      // --- widened past `lg` while open, it becomes the sidebar again: beside the page,
      //     not modal, and the page neither locked nor inert
      await page.tap('.mj-nav-open');
      await page.waitForSelector('#mj-nav[data-open]', { timeout: 3000 });
      await page.setViewportSize({ width: 1280, height: 800 });
      await page.waitForTimeout(100);
      const wide = await page.evaluate(() => {
        const nav = document.getElementById('mj-nav');
        return {
          open: nav.hasAttribute('data-open'),
          role: nav.getAttribute('role'),
          locked: document.documentElement.classList.contains('mj-nav-locked'),
          inert: document.querySelectorAll('[inert]').length,
          position: getComputedStyle(nav).position,
          visible: nav.getClientRects().length > 0,
          trigger: document.querySelector('.mj-nav-open').getClientRects().length > 0,
        };
      });
      if (wide.open || wide.role || wide.locked || wide.inert || wide.position !== 'sticky' || !wide.visible || wide.trigger) {
        fail('drawer', `${at}: widened to 1280 while open, ${JSON.stringify(wide)}`);
      }

      // --- a link to the sections at desktop width is the sidebar, never a modal one
      await page.goto(BASE + '/cockpit#mj-nav', { waitUntil: 'networkidle' });
      await page.waitForFunction(() => !!window.Alpine);
      const deep = await page.evaluate(() => ({
        role: document.getElementById('mj-nav').getAttribute('role'),
        locked: document.documentElement.classList.contains('mj-nav-locked'),
        inert: document.querySelectorAll('[inert]').length,
      }));
      if (deep.role || deep.locked || deep.inert) fail('drawer', `${at}: /cockpit#mj-nav at 1280 made the sidebar modal, ${JSON.stringify(deep)}`);
      await page.setViewportSize(size);

      // --- a link to the sections at a phone's width opens the drawer as the component's,
      //     not as a :target the component does not know about, and Escape closes it
      await page.goto(BASE + '/cockpit#mj-nav', { waitUntil: 'networkidle' });
      await page.waitForFunction(() => !!window.Alpine);
      const linked = await page.evaluate(() => ({
        open: document.getElementById('mj-nav').hasAttribute('data-open'),
        role: document.getElementById('mj-nav').getAttribute('role'),
        hash: location.hash,
      }));
      await page.keyboard.press('Escape');
      const linkedClosed = !(await page.locator('#mj-nav').isVisible());
      if (!linked.open || linked.role !== 'dialog' || linked.hash || !linkedClosed) {
        fail('drawer', `${at}: /cockpit#mj-nav opened ${JSON.stringify(linked)}, Escape closed it: ${linkedClosed}`);
      }

      // --- the same link loaded fresh, in a page of its own: the goto above stays in one
      //     document (a fragment change, so `hashchange`); this is a load with the fragment
      //     already in the address, which only the takeover at start-up can answer
      const fresh = await context.newPage();
      await fresh.setViewportSize(size);
      await fresh.goto(BASE + '/cockpit#mj-nav', { waitUntil: 'networkidle' });
      await fresh.waitForFunction(() => !!window.Alpine);
      const loaded = await fresh.evaluate(() => ({
        open: document.getElementById('mj-nav').hasAttribute('data-open'),
        role: document.getElementById('mj-nav').getAttribute('role'),
        hash: location.hash,
      }));
      await fresh.keyboard.press('Escape');
      const loadedClosed = !(await fresh.locator('#mj-nav').isVisible());
      if (!loaded.open || loaded.role !== 'dialog' || loaded.hash || !loadedClosed) {
        fail('drawer', `${at}: /cockpit#mj-nav loaded fresh opened ${JSON.stringify(loaded)}, Escape closed it: ${loadedClosed}`);
      }
      await fresh.close();

      // --- an entry followed closes the drawer at once, not when the next page arrives:
      //     a page that takes seconds must not leave the drawer over a page that is leaving.
      //     Read in the same turn as the click, before the navigation can commit.
      await page.tap('.mj-nav-open');
      await page.waitForSelector('#mj-nav[data-open]', { timeout: 3000 });
      const leaving = await page.evaluate(() => {
        const nav = document.getElementById('mj-nav');
        nav.querySelector('a.mj-nav-link:not(.mj-nav-link--current)').click();
        return nav.hasAttribute('data-open');
      });
      if (leaving) fail('drawer', `${at}: an entry followed left the drawer open while the next page loaded`);
      await page.waitForLoadState('networkidle');

      // --- the focused skip link does not sit on the trigger (the walk's seed 1017 found it)
      await page.goto(BASE + '/cockpit', { waitUntil: 'networkidle' });
      await page.waitForFunction(() => !!window.Alpine);
      await page.keyboard.press('Tab');
      const hit = await page.evaluate(() => {
        const r = document.querySelector('.mj-nav-open').getBoundingClientRect();
        const top = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2);
        return { skip: document.activeElement.classList.contains('mj-skip'), on: top && top.closest('.mj-nav-open') ? 'trigger' : top && top.className };
      });
      if (hit.skip && hit.on !== 'trigger') fail('drawer', `${at}: the focused skip link covers the trigger (a tap lands on "${hit.on}")`);
      await page.close();
    }
    if (!findings.some((f) => f.includes(' drawer '))) {
      ok(
        'drawer',
        `on touch at ${sizes.map((s) => `${s.width}x${s.height}`).join(', ')}: opens as a modal dialog with the sidebar's catalogue; the close control, the backdrop, Escape, an entry, back and widening each leave the page unlocked and reachable`,
      );
    }
  } finally {
    await context.close();
  }

  // --- a random walk against a model of the drawer, and the script failing to arrive
  await drawerWalks(browser, narrow[0] || 320);
  await drawerAccessibility(browser, narrow[0] || 320);
  await drawerWithoutAlpine(browser, narrow[0] || 320);

  // --- without the script the trigger still opens it: the stylesheet shows the `:target`
  const nojs = await browser.newContext({ javaScriptEnabled: false, viewport: { width: narrow[0] || 320, height: 740 } });
  try {
    const page = await nojs.newPage();
    await page.goto(BASE + '/cockpit', { waitUntil: 'load' });
    await page.click('.mj-nav-open');
    const shown = await page.locator('#mj-nav').isVisible();
    const entries = await page.locator('#mj-nav a.mj-nav-link').count();
    await page.click('.mj-nav-close');
    const hidden = !(await page.locator('#mj-nav').isVisible());
    if (!shown || !entries || !hidden) {
      fail('drawer', `without JavaScript: shown ${shown}, ${entries} entries, closed again ${hidden}`);
    } else {
      ok('drawer', `without JavaScript the trigger opens the sections through :target (${entries} entries) and the close link shuts them`);
    }
  } finally {
    await nojs.close();
  }
}

const AXE = new URL('../../node_modules/axe-core/axe.min.js', import.meta.url);

/**
 * The accessibility engine over the drawer and the top bar, closed and open, at a phone's
 * width: the subtree this change added and the one it changed. The page-wide audit is
 * scripts/ui-audit's; this asks only about the controls a phone now navigates with.
 * Evaluated through the debugger, as ui-audit does, so the Cockpit's policy stays on.
 */
async function drawerAccessibility(browser, width) {
  const context = await browser.newContext({ hasTouch: true, isMobile: true, viewport: { width, height: 740 } });
  try {
    const page = await context.newPage();
    await page.goto(BASE + '/cockpit', { waitUntil: 'networkidle' });
    await page.waitForFunction(() => !!window.Alpine);
    await page.evaluate(readFileSync(AXE, 'utf8'));
    const audit = (state) =>
      page.evaluate(async (state) => {
        // eslint-disable-next-line no-undef
        const r = await axe.run(
          { include: [['#mj-nav'], ['.mj-topbar']] },
          { resultTypes: ['violations'], runOnly: { type: 'tag', values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'] } },
        );
        return r.violations.map((v) => `${state}: ${v.id} (${v.impact}) on ${v.nodes.slice(0, 2).map((n) => n.target.join(' ')).join(', ')}`);
      }, state);
    const found = [...(await audit('closed'))];
    await page.tap('.mj-nav-open');
    await page.waitForSelector('#mj-nav[data-open]', { timeout: 3000 });
    found.push(...(await audit('open')));
    for (const v of found) fail('drawer', `@${width}px accessibility, ${v}`);
    if (!found.length) ok('drawer', `@${width}px the accessibility engine (WCAG 2.0-2.2 A/AA) finds nothing on the drawer or the top bar, closed or open`);
  } finally {
    await context.close();
  }
}

/**
 * Walks that found a defect, kept so the defect stays found: 1017 put the focused skip link
 * over the trigger after an entry was followed and Tab pressed.
 */
const REGRESSION_SEEDS = [1017];

/**
 * The surfaces a person works in, at the narrowest width the design declares and on a touch
 * screen: the capability runner (a form, a request, a long answer) and the graph (a drawing
 * and its tables). The same blocks the sweep runs at 1600, asked again where a phone is.
 */
async function phoneSurfaces(browser) {
  const width = Math.min(...WIDTHS);
  const phone = await browser.newContext({ hasTouch: true, isMobile: true });
  try {
    for (const [name, check] of [
      ['runner', () => runner(phone, width)],
      ['graph', () => graph(phone, width)],
    ]) {
      try {
        await check();
      } catch (e) {
        const lines = String(e.message || e).split('\n').filter((l) => l.trim());
        fail(name, `@${width}px ` + lines.slice(0, 3).join(' — ').slice(0, 240));
      }
    }
  } finally {
    await phone.close();
  }
}

/** A small seeded generator (mulberry32): the same seed walks the same path. */
function seeded(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * The drawer driven by a seeded random walk and checked, after every step, against a model
 * of what it should be. The scripted sequence above covers the transitions a person is
 * expected to make; this covers the orders nobody wrote down: Escape on a closed drawer, a
 * resize between two opens, Ctrl+K over an open drawer, back after a resize, Tab after a
 * close. The invariants are the controller's own (share/cockpit/cockpit.js): one state,
 * written into the page in one place, so every one of its marks must agree with the model
 * and with each other at all times.
 *
 *   COCKPIT_PROBE_SEED=N     walk that one seed (what a failure prints, to replay it)
 *   COCKPIT_PROBE_WALKS=N    how many seeds (default 4), COCKPIT_PROBE_STEPS=N per walk (40)
 */
async function drawerWalks(browser, width) {
  const steps = Number(process.env.COCKPIT_PROBE_STEPS || 40);
  const seeds = process.env.COCKPIT_PROBE_SEED
    ? [Number(process.env.COCKPIT_PROBE_SEED)]
    : [
        // seeds that once found a defect walk every run, whatever the day
        ...REGRESSION_SEEDS,
        ...Array.from({ length: Number(process.env.COCKPIT_PROBE_WALKS || 4) }, (_, i) => 1009 * (i + 1) + new Date().getUTCDate()),
      ];
  const size = { width, height: 740 };
  let walked = 0;
  for (const seed of seeds) {
    const rand = seeded(seed);
    const context = await browser.newContext({ hasTouch: true, isMobile: true, viewport: size });
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(e.message));
    await page.goto(BASE + '/cockpit', { waitUntil: 'networkidle' });
    await page.waitForFunction(() => !!window.Alpine);
    const model = { open: false, wide: false, navigated: 0 };
    const trace = [];
    let broken = null;
    for (let step = 0; step < steps && !broken; step++) {
      // only actions a person could take in the model's state
      const actions = model.wide
        ? ['narrow', 'escape', 'ctrl-k', 'tab']
        : model.open
          ? ['backdrop', 'close', 'escape', 'tab', 'shift-tab', 'wide', 'ctrl-k', 'entry', 'escape-twice']
          : ['open', 'open', 'escape', 'tab', 'wide', 'ctrl-k', ...(model.navigated ? ['back'] : [])];
      const action = actions[Math.floor(rand() * actions.length)];
      trace.push(action);
      try {
      switch (action) {
        case 'open':
          await page.tap('.mj-nav-open', { timeout: 5000 });
          model.open = true;
          break;
        case 'backdrop': {
          const box = await page.locator('#mj-nav').boundingBox();
          await page.touchscreen.tap(Math.min(size.width - 6, box.x + box.width + 12), size.height / 2);
          model.open = false;
          break;
        }
        case 'close':
          await page.tap('.mj-nav-close', { timeout: 5000 });
          model.open = false;
          break;
        case 'escape':
          await page.keyboard.press('Escape');
          model.open = false;
          break;
        case 'escape-twice':
          await page.keyboard.press('Escape');
          await page.keyboard.press('Escape');
          model.open = false;
          break;
        case 'tab':
          await page.keyboard.press('Tab');
          break;
        case 'shift-tab':
          await page.keyboard.press('Shift+Tab');
          break;
        case 'wide':
          await page.setViewportSize({ width: 1280, height: 800 });
          model.wide = true;
          model.open = false;
          break;
        case 'narrow':
          await page.setViewportSize(size);
          model.wide = false;
          break;
        case 'ctrl-k': {
          await page.keyboard.press('Control+k');
          await page.waitForSelector('.mj-palette-panel', { state: 'visible', timeout: 4000 });
          const under = await page.evaluate(() => document.getElementById('mj-nav').hasAttribute('data-open'));
          if (under) throw new Error('the palette opened under the open drawer');
          await page.keyboard.press('Escape');
          model.open = false;
          break;
        }
        case 'entry': {
          const from = page.url();
          const entry = page.locator('#mj-nav a.mj-nav-link:not(.mj-nav-link--current)').nth(Math.floor(rand() * 5));
          await entry.scrollIntoViewIfNeeded();
          await Promise.all([page.waitForURL((u) => u.href !== from, { timeout: 15000 }), entry.tap()]);
          await page.waitForLoadState('networkidle');
          await page.waitForFunction(() => !!window.Alpine);
          model.open = false;
          model.navigated++;
          break;
        }
        case 'back':
          await page.goBack({ waitUntil: 'networkidle' });
          model.navigated--;
          model.open = false;
          break;
      }
      } catch (e) {
        // an action a person could take and the page did not let them: that is the finding
        broken = `step ${step + 1} (${action}) could not be done: ${String(e.message || e).split('\n')[0].slice(0, 100)}`;
        break;
      }
      await page.waitForTimeout(30);
      const seen = await page.evaluate(() => {
        const nav = document.getElementById('mj-nav');
        const trigger = document.querySelector('.mj-nav-open');
        const backdrops = document.querySelectorAll('.mj-nav-backdrop');
        const vw = document.documentElement.clientWidth;
        const top = document.elementFromPoint(vw - 4, window.innerHeight / 2);
        const active = document.activeElement;
        return {
          open: nav.hasAttribute('data-open'),
          locked: document.documentElement.classList.contains('mj-nav-locked'),
          dialog: nav.getAttribute('role') === 'dialog' && nav.getAttribute('aria-modal') === 'true',
          expanded: trigger.getAttribute('aria-expanded') === 'true',
          inert: document.querySelectorAll('[inert]').length,
          mainInert: document.getElementById('main').inert,
          backdrops: backdrops.length,
          backdropShown: backdrops[0] && getComputedStyle(backdrops[0]).display !== 'none',
          edgeCovered: !!(top && top.closest('.mj-nav-backdrop')),
          focusInside: !!(active && active.closest('#mj-nav')),
          navShown: nav.getClientRects().length > 0,
          sticky: getComputedStyle(nav).position === 'sticky',
          triggerShown: trigger.getClientRects().length > 0,
          overflow: document.documentElement.scrollWidth > vw + 1,
          palette: !!document.querySelector('.mj-palette-panel') && document.querySelector('.mj-palette-panel').getClientRects().length > 0,
        };
      });
      const broke = [];
      if (seen.open !== model.open) broke.push(`open is ${seen.open}, the model says ${model.open}`);
      if (new Set([seen.open, seen.locked, seen.dialog, seen.expanded, !!seen.backdropShown]).size !== 1) {
        broke.push('open, lock, dialog role, aria-expanded and backdrop disagree');
      }
      if (!seen.open && seen.inert) broke.push(`${seen.inert} element(s) inert while closed`);
      if (seen.open && !seen.mainInert) broke.push('main reachable while open');
      if (seen.backdrops !== 1) broke.push(`${seen.backdrops} backdrops`);
      if (!seen.open && seen.edgeCovered) broke.push('a layer covers the page while closed');
      if (seen.open && !seen.focusInside) broke.push('focus outside the open drawer');
      if (model.wide && (!seen.navShown || !seen.sticky || seen.triggerShown || seen.dialog)) broke.push('not a plain sidebar at desktop width');
      if (!model.wide && !seen.triggerShown) broke.push('no trigger at phone width');
      if (seen.overflow) broke.push('horizontal overflow');
      if (seen.palette) broke.push('the palette stayed open');
      if (errors.length) broke.push(`page error: ${errors[0].slice(0, 80)}`);
      if (broke.length) broken = `step ${step + 1} (${action}): ${broke.join('; ')}`;
      walked++;
    }
    if (broken) {
      fail('drawer', `walk seed ${seed}: ${broken}; path ${trace.join(' > ')} — replay: COCKPIT_PROBE_SEED=${seed} scripts/cockpit-probe --drawer`);
    }
    await context.close();
  }
  if (!findings.some((f) => f.includes('walk seed'))) {
    ok('drawer', `${seeds.length} seeded walk(s) (${seeds.join(', ')}), ${walked} step(s): the drawer agreed with its model and with itself after every one`);
  }
}

/**
 * The script arrives but Alpine does not (blocked, failed, a stale vendor directory): the
 * component never binds, so the trigger's click is never prevented and it is the link it
 * is. The sections must still open, through `:target`, rather than the page having a
 * control that does nothing.
 */
async function drawerWithoutAlpine(browser, width) {
  const context = await browser.newContext({ hasTouch: true, isMobile: true, viewport: { width, height: 740 } });
  try {
    await context.route('**/vendor/alpine.csp.min.js*', (route) => route.abort());
    const page = await context.newPage();
    await page.goto(BASE + '/cockpit', { waitUntil: 'networkidle' });
    const alpine = await page.evaluate(() => !!window.Alpine);
    await page.tap('.mj-nav-open');
    const shown = await page.locator('#mj-nav').isVisible();
    await page.tap('.mj-nav-close');
    const hidden = !(await page.locator('#mj-nav').isVisible());
    if (alpine) fail('drawer', 'Alpine started although its file was blocked; the chaos case proved nothing');
    else if (!shown || !hidden) fail('drawer', `Alpine blocked: the trigger opened ${shown}, the close link closed ${hidden}`);
    else ok('drawer', 'with Alpine blocked the trigger still opens the sections and the close link shuts them');
  } finally {
    await context.close();
  }
}

const browser = await chromium.launch({ channel: 'chrome' });
// the drawer alone: the fast answer to "can a phone reach the sections", for a change that
// touches the shell and should not wait for the whole sweep
if (MODE === 'drawer' || MODE === 'phone') {
  try {
    DESIGN = await api('/api/v1/design');
    WIDTHS = DESIGN.viewports;
    await drawer(browser);
    if (MODE === 'phone') await phoneSurfaces(browser);
  } catch (e) {
    fail('drawer', String(e.message || e).split('\n').filter((l) => l.trim()).slice(0, 3).join(' — ').slice(0, 260));
  } finally {
    await browser.close();
  }
  for (const line of notes) console.log(line);
  for (const line of findings) console.log(line);
  console.log(`cockpit-probe: the ${MODE} check found ${findings.length}`);
  process.exit(findings.length ? 10 : 0);
}
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
    ['drawer', () => drawer(browser)],
    ['phone', () => phoneSurfaces(browser)],
    ['runner', () => runner(context)],
    ['graph', () => graph(context)],
    ['graph-scale', async () => graphAtScale(context, await largestGraph())],
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
