// The browser audit: every discovered page, at every width its tier calls for, in a real
// browser, against the invariants a rendered page must satisfy.
//
// The pages and the widths come from ui-discover.mjs, so this file names no route and no
// breakpoint. What it adds is what only a browser can answer: whether the document overflows
// its viewport and which element does it, what an accessibility engine finds, whether the
// landmarks and headings make a document, whether every id is unique, whether a component's
// trigger names a target that exists, and whether the page logged an error while loading.
//
// The server it drives is the repository's own — `majordomus serve` — so the audit does not
// hold a second opinion about how the site is served or on which port.

import { readFileSync } from 'node:fs';
import { availableParallelism } from 'node:os';
import { chromium } from 'playwright';

const AXE = new URL('../../node_modules/axe-core/axe.min.js', import.meta.url);

/** How much horizontal overflow is a browser rounding artefact rather than a defect. */
export const OVERFLOW_TOLERANCE = 1;

/**
 * How long one page-and-width visit may take before it is abandoned.
 *
 * Navigation has its own timeout; `page.evaluate` has none, and neither does the
 * accessibility engine running inside it. A single large document can therefore hold a
 * sixteen-hundred-visit sweep open indefinitely, which looks exactly like a slow machine
 * until somebody checks after twenty minutes. The whole visit gets a deadline, and a visit
 * that hits it is a finding about that page.
 */
export const VISIT_DEADLINE_MS = 60000;

/** Run a promise against a deadline, rejecting with a message a report can print. */
function withDeadline(work, ms, what) {
  let timer;
  return Promise.race([
    work,
    new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error(`${what} did not finish within ${ms}ms`)), ms);
    }),
  ]).finally(() => clearTimeout(timer));
}

/**
 * Visit one page at one width and return every finding, each already carrying the route, the
 * width, the rule it broke and enough of the DOM to find it again.
 */
export async function auditPage(page, origin, route, width) {
  const findings = [];
  const console_errors = [];
  const failed_requests = [];
  page.removeAllListeners('console');
  page.removeAllListeners('requestfailed');
  page.on('console', (message) => {
    if (message.type() === 'error') console_errors.push(message.text().slice(0, 300));
  });
  page.on('requestfailed', (request) => {
    // only the site's own assets: an outside host failing is the network's business. The
    // browser's own reason is carried through, because "an asset failed" and "the browser
    // cancelled a request it no longer needed" are different findings and look identical
    // without it.
    if (!request.url().startsWith(origin)) return;
    const reason = request.failure()?.errorText ?? 'unknown';
    // A cancelled request is not a failed one. `net::ERR_ABORTED` is the browser saying it
    // no longer needs the response — a navigation replaced the page, or a script dropped
    // the element that asked for it — and a lazily loaded asset in flight when the audit
    // moves to the next visit produces exactly that. It says nothing about the site.
    if (reason.includes('ERR_ABORTED')) return;
    failed_requests.push(`${request.method()} ${request.url()} (${reason})`);
  });

  await page.setViewportSize({ width, height: 900 });
  // A page that never loads is a finding about that page, not the end of the run. An audit
  // that aborts on the first slow navigation reports nothing about the 1600 visits after
  // it, which is how a whole sweep is lost to one asset that hung.
  let response;
  try {
    response = await page.goto(`${origin}${route}`, { waitUntil: 'load', timeout: 20000 });
  } catch (error) {
    findings.push({
      rule: 'page.unreachable',
      detail: `the page did not load: ${String(error?.message ?? error).split('\n')[0]}`,
    });
    return { route, width, status: 0, findings, console_errors, failed_requests };
  }
  const status = response?.status() ?? 0;
  if (status >= 400) {
    findings.push({ rule: 'page.status', detail: `the page answered ${status}` });
    return { route, width, status, findings, console_errors, failed_requests };
  }

  const observed = await page.evaluate((tolerance) => {
    const out = { overflow: null, landmarks: {}, headings: [], duplicate_ids: [], triggers: [] };

    // horizontal overflow, with the elements that cause it rather than the fact alone.
    // The body's scroll width, not the document element's: content inside its own
    // `overflow-x:auto` box scrolls within the page — the pattern this site uses for wide
    // tables — and Chromium still counts that box's unclipped content in
    // `documentElement.scrollWidth`, which reports overflow on a page that cannot be panned
    // sideways at all. What a reader can drag is the body.
    const doc = document.documentElement;
    if (document.body.scrollWidth > doc.clientWidth + tolerance) {
      const offenders = [];
      for (const el of document.querySelectorAll('body *')) {
        const box = el.getBoundingClientRect();
        if (box.width === 0 && box.height === 0) continue;
        if (box.right <= doc.clientWidth + tolerance) continue;
        const style = getComputedStyle(el);
        // an element inside its own scrolling box is allowed to be wider than the viewport
        let scrollable = false;
        for (let p = el.parentElement; p; p = p.parentElement) {
          const overflowX = getComputedStyle(p).overflowX;
          if (overflowX === 'auto' || overflowX === 'scroll') { scrollable = true; break; }
        }
        if (scrollable) continue;
        offenders.push({
          selector: el.tagName.toLowerCase() +
            (el.id ? `#${el.id}` : '') +
            (el.className && typeof el.className === 'string' ? `.${el.className.trim().split(/\s+/).slice(0, 3).join('.')}` : ''),
          right: Math.round(box.right),
          width: Math.round(box.width),
          position: style.position,
        });
        if (offenders.length >= 5) break;
      }
      out.overflow = { scrollWidth: document.body.scrollWidth, clientWidth: doc.clientWidth, offenders };
    }

    // landmarks and heading order: a document, not a pile of divs
    out.landmarks = {
      main: document.querySelectorAll('main, [role=main]').length,
      nav: document.querySelectorAll('nav, [role=navigation]').length,
      h1: document.querySelectorAll('h1').length,
    };
    let previous = 0;
    for (const heading of document.querySelectorAll('h1,h2,h3,h4,h5,h6')) {
      const level = Number(heading.tagName[1]);
      if (previous && level > previous + 1) {
        out.headings.push({ from: previous, to: level, text: (heading.textContent || '').trim().slice(0, 60) });
      }
      previous = level;
    }

    // every id is unique: a duplicate breaks every reference to it, including a component's
    const seen = new Set();
    for (const el of document.querySelectorAll('[id]')) {
      if (seen.has(el.id)) out.duplicate_ids.push(el.id);
      seen.add(el.id);
    }

    // a component's trigger names a target that exists, and says what it controls
    const attributes = [
      ['data-collapse-toggle', 'collapse'],
      ['data-dropdown-toggle', 'dropdown'],
      ['data-modal-target', 'modal'],
      ['data-modal-toggle', 'modal'],
      ['data-drawer-target', 'drawer'],
      ['data-drawer-toggle', 'drawer'],
      ['data-accordion-target', 'accordion'],
      ['data-tabs-toggle', 'tabs'],
      ['data-tooltip-target', 'tooltip'],
      ['data-popover-target', 'popover'],
    ];
    for (const [attribute, component] of attributes) {
      for (const el of document.querySelectorAll(`[${attribute}]`)) {
        const target = el.getAttribute(attribute);
        const resolved = target ? document.getElementById(target.replace(/^#/, '')) : null;
        out.triggers.push({
          component,
          attribute,
          target,
          resolves: Boolean(resolved),
          controls: el.getAttribute('aria-controls'),
          expanded: el.getAttribute('aria-expanded'),
          accessible_name: (el.textContent || '').trim().slice(0, 40) || el.getAttribute('aria-label') || '',
        });
      }
    }
    return out;
  }, OVERFLOW_TOLERANCE);

  if (observed.overflow) {
    findings.push({
      rule: 'responsive.horizontal-overflow',
      detail: `the document is ${observed.overflow.scrollWidth}px wide in a ${observed.overflow.clientWidth}px viewport`,
      elements: observed.overflow.offenders,
    });
  }
  if (observed.landmarks.main !== 1) {
    findings.push({ rule: 'semantics.main', detail: `${observed.landmarks.main} main landmark(s); a page has one` });
  }
  if (observed.landmarks.h1 !== 1) {
    findings.push({ rule: 'semantics.h1', detail: `${observed.landmarks.h1} level-one heading(s); a page has one` });
  }
  for (const skip of observed.headings) {
    findings.push({ rule: 'semantics.heading-order', detail: `h${skip.from} is followed by h${skip.to}: "${skip.text}"` });
  }
  for (const id of [...new Set(observed.duplicate_ids)]) {
    findings.push({ rule: 'semantics.duplicate-id', detail: `id "${id}" appears more than once` });
  }
  for (const trigger of observed.triggers) {
    if (!trigger.resolves) {
      findings.push({
        rule: 'component.target-missing',
        detail: `a ${trigger.component} trigger names "${trigger.target}", which no element has`,
      });
    }
    if (!trigger.accessible_name) {
      findings.push({ rule: 'component.trigger-unnamed', detail: `a ${trigger.component} trigger has no accessible name` });
    }
  }
  for (const error of console_errors) {
    findings.push({ rule: 'runtime.console-error', detail: error });
  }
  for (const request of failed_requests) {
    findings.push({ rule: 'runtime.asset-failed', detail: request });
  }

  // the accessibility engine, last, so its findings sit beside the repository's own
  await page.addScriptTag({ content: readFileSync(AXE, 'utf8') });
  const axe = await page.evaluate(async () => {
    // eslint-disable-next-line no-undef
    const results = await axe.run(document, { resultTypes: ['violations'], runOnly: { type: 'tag', values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'] } });
    return results.violations.map((v) => ({
      id: v.id,
      impact: v.impact,
      help: v.help,
      nodes: v.nodes.slice(0, 3).map((n) => n.target.join(' ')),
    }));
  });
  for (const violation of axe) {
    findings.push({
      rule: `accessibility.${violation.id}`,
      detail: `${violation.help} (${violation.impact})`,
      elements: violation.nodes.map((selector) => ({ selector })),
    });
  }

  return { route, width, status, findings, components: observed.triggers.map((t) => t.component) };
}

/** Audit every page of a plan against a running origin. */
/**
 * How many pages the audit drives at once.
 *
 * One visit is mostly waiting — for a navigation, for the accessibility engine inside the
 * page — so a serial audit leaves a machine idle for most of a run that takes a quarter of
 * an hour. The default follows the machine rather than a number chosen here, and stays
 * modest because each worker is a browser tab with a page and an engine in it.
 */
export function jobs() {
  const asked = Number.parseInt(process.env.MJ_UI_JOBS ?? '', 10);
  if (Number.isFinite(asked) && asked > 0) return Math.min(asked, 16);
  return Math.min(Math.max(availableParallelism(), 2), 4);
}

/**
 * Audit every page of a plan against a running origin.
 *
 * The visits are spread over a pool of tabs, and the results are put back in plan order
 * whatever order they finished in: a results document that changed with the scheduler would
 * make every diff of a run unreadable.
 */
export async function audit(origin, pages, { onVisit, concurrency = jobs() } = {}) {
  // the browser the repository already drives for the cockpit probe: the system Chrome,
  // so a CI runner and a laptop use one browser and neither downloads another
  const browser = await chromium.launch({ channel: 'chrome' });
  const context = await browser.newContext();

  // the whole target set, flattened, so the workers share one queue rather than a page each:
  // a page with twelve widths would otherwise hold a worker while the others idled
  const targets = [];
  for (const target of pages) {
    for (const width of target.widths) targets.push({ route: target.route, width, tier: target.tier });
  }
  const visits = new Array(targets.length);
  let next = 0;
  let done = 0;
  let stopped = null;

  const worker = async () => {
    let page = await context.newPage();
    try {
      while (stopped === null) {
        const index = next;
        if (index >= targets.length) break;
        next += 1;
        const target = targets[index];
        let visit;
        try {
          visit = await withDeadline(
            auditPage(page, origin, target.route, target.width),
            VISIT_DEADLINE_MS,
            `${target.route} at ${target.width}px`,
          );
        } catch (error) {
          // A refused connection is not a fact about this page. The server the audit drives
          // is gone, and every remaining visit would be recorded as an unreachable page — a
          // whole tail of the site marked broken by one process exiting. Stop, and say what
          // actually happened.
          if (String(error?.message ?? error).includes('ERR_CONNECTION_REFUSED')) {
            stopped = new Error(
              `the server at ${origin} stopped answering during the run, at ${target.route} ` +
                `(${done} visit(s) measured); the audit cannot speak for the rest`,
            );
            break;
          }
          // Whatever else went wrong on this page, the next page is still worth measuring —
          // but a deadline only stops *waiting*; the work it abandoned is still running in
          // that tab, and every later operation would queue behind it. So the tab goes.
          visit = {
            route: target.route,
            width: target.width,
            status: 0,
            findings: [{
              rule: 'page.audit-failed',
              detail: String(error?.message ?? error).split('\n')[0],
            }],
          };
          await page.close().catch(() => {});
          page = await context.newPage();
        }
        visits[index] = { ...visit, tier: target.tier };
        done += 1;
        onVisit?.(visit, done);
      }
    } finally {
      await page.close().catch(() => {});
    }
  };

  try {
    // One throwaway visit first. The server answers its readiness probe before it has built
    // anything, and the first real page pays for the index, the registry and every asset at
    // once — long enough, on a loaded machine, to exceed a per-visit timeout and report the
    // first page of the run as unreachable. Warming it costs one page load and removes a
    // whole class of finding that says more about the machine than about the site.
    const warm = await context.newPage();
    await warm.goto(origin, { waitUntil: 'networkidle', timeout: 120000 }).catch(() => {});
    await warm.close().catch(() => {});

    await Promise.all(Array.from({ length: Math.max(1, concurrency) }, () => worker()));
    if (stopped) throw stopped;
  } finally {
    await browser.close();
  }
  return visits.filter(Boolean);
}
