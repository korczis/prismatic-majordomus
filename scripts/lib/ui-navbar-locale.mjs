// ui-navbar-locale.mjs: the main navigation measured the same in every language the build publishes.
//
// Two checks already press and follow everything on the site: scripts/interaction-probe drives
// every control on every page through the behaviour spec that claims it, and the site's
// language probe (site-i18n-probe.mjs) follows every link of every language copy. Neither
// measures what a translation does to the bar a reader meets on every page. On 2026-10-19 the
// Czech bar took two rows at 1280px and 768px (73px and 121px tall in English, 121px and 157px
// in Czech): every link still worked, so every check was green, and every Czech page was broken.
//
// So this file measures the bar on every page and at every width the audit visits. The bar is
// one partial on every page of a language, so the whole set — both themes, closed and, where
// the width shows a menu button, opened — is measured on each language's front page, and the
// closed bar in the page's own theme everywhere else:
//
//   navbar.wraps         the bar's top-level items, laid out in a row, take more than one row
//   navbar.clipped       an item of the bar cuts its own text off
//   navbar.locale-drift  the bar, closed or opened, in either theme, is not the same height in a
//                        language as in the language the site is written in, on the same page
//                        at the same width (localeDrift, over the whole run)
//
// No language is named here. The audit's pages come from the build's filesystem and sitemap
// (ui-discover.mjs), so a `/cs/` tree the build writes is visited like any other, and the
// source language is the one the site's front page declares. "Pixel-perfect" across languages
// means the bar's boxes are identical; the words are not, so a screenshot comparison would
// only ever measure the translation.

/** How many pixels two heights may differ by before they are two heights. */
export const HEIGHT_TOLERANCE = 1;

/** How long an opened menu is given to finish appearing. */
export const SETTLE_MS = 250;

/** The themes the bar is measured in, by the class the site's theme switch sets on <html>. */
export const THEMES = [['light', false], ['dark', true]];

/**
 * The language prefix of a route, when the page it renders is in a language other than the
 * source's: `/cs/docs/` in Czech is `{ lang: 'cs', base: '/docs/' }`. A route whose first
 * segment is not its page's language is the source language's own route.
 */
export function localeOf(route, lang, source) {
  if (!lang || lang === source) return { lang: source, base: route };
  const prefix = `/${lang}/`;
  if (route === `/${lang}` || route === prefix) return { lang, base: '/' };
  if (route.startsWith(prefix)) return { lang, base: route.slice(prefix.length - 1) };
  return { lang, base: route };
}

/**
 * Every translated bar against the source language's bar on the same page, at the same width,
 * in the same theme and state. Returns the findings to add, each naming the visit it belongs to.
 *
 * The source language is the language of the site's front page, as that page declares it; a
 * run that did not visit the front page compares nothing rather than guessing.
 */
export function localeDrift(visits) {
  const front = visits.find((v) => v.route === '/' && v.navbar?.lang);
  if (!front) return [];
  const source = front.navbar.lang;
  const key = (base, width, m) => `${base}@${width}@${m.theme}@${m.state}`;
  const expected = new Map();
  for (const v of visits) {
    if (!v.navbar?.measures) continue;
    const { lang, base } = localeOf(v.route, v.navbar.lang, source);
    if (lang !== source) continue;
    for (const m of v.navbar.measures) expected.set(key(base, v.width, m), m.height);
  }
  const findings = [];
  for (const v of visits) {
    if (!v.navbar?.measures) continue;
    const { lang, base } = localeOf(v.route, v.navbar.lang, source);
    if (lang === source) continue;
    for (const m of v.navbar.measures) {
      const want = expected.get(key(base, v.width, m));
      if (want === undefined || Math.abs(m.height - want) <= HEIGHT_TOLERANCE) continue;
      findings.push({
        route: v.route,
        width: v.width,
        finding: {
          rule: 'navbar.locale-drift',
          detail: `the main navigation (${m.state}, ${m.theme}) is ${m.height}px tall in ${lang} and ` +
            `${want}px in ${source} on ${base} at ${v.width}px`,
        },
      });
    }
  }
  return findings;
}

/** Add localeDrift's findings to the visits they name; returns how many were added. */
export function applyLocaleDrift(visits) {
  const added = localeDrift(visits);
  for (const { route, width, finding } of added) {
    const visit = visits.find((v) => v.route === route && v.width === width);
    if (visit) visit.findings.push(finding);
  }
  return added.length;
}

/** Whether a route is a language's front page: `/`, or `/<lang>/` for the page's language. */
export function isFront(route, lang) {
  return route === '/' || (Boolean(lang) && (route === `/${lang}/` || route === `/${lang}`));
}

/** The bar of one visit: its findings, and its heights for localeDrift. */
export async function measureNavbar(page, route) {
  const findings = [];

  const read = () => page.evaluate(() => {
    const visible = (el) => {
      const box = el.getBoundingClientRect();
      if (box.width === 0 || box.height === 0) return false;
      const style = getComputedStyle(el);
      return style.visibility !== 'hidden' && style.display !== 'none';
    };
    const nav = document.querySelector('nav[aria-label="Main"]');
    if (!nav || !visible(nav)) return null;
    const rows = new Set();
    let horizontal = false;
    for (const list of nav.querySelectorAll('ul')) {
      if (!visible(list) || list.parentElement?.closest('ul')) continue; // the top level only
      if (!getComputedStyle(list).flexDirection.startsWith('row')) continue;
      horizontal = true;
      for (const item of list.children) {
        if (visible(item)) rows.add(Math.round(item.getBoundingClientRect().top));
      }
    }
    const clipped = [];
    for (const el of nav.querySelectorAll('a, button')) {
      if (!visible(el)) continue;
      if (el.scrollWidth > el.clientWidth + 1 && getComputedStyle(el).overflowX !== 'visible') {
        clipped.push((el.textContent || '').trim().slice(0, 40));
      }
    }
    // the bar and whatever it opened: the tallest bottom edge of anything visible inside it
    const top = nav.getBoundingClientRect().top;
    let bottom = nav.getBoundingClientRect().bottom;
    for (const el of nav.querySelectorAll('*')) {
      if (visible(el)) bottom = Math.max(bottom, el.getBoundingClientRect().bottom);
    }
    const toggle = [...nav.querySelectorAll('[data-collapse-toggle]')].find(visible);
    return {
      height: Math.round(bottom - top),
      rows: horizontal ? rows.size : 1,
      clipped,
      toggle: toggle ? toggle.getAttribute('data-collapse-toggle') : null,
    };
  });
  const theme = (dark) => page.evaluate((dark) => {
    document.documentElement.classList.toggle('dark', dark);
  }, dark);

  const lang = await page.evaluate(() => document.documentElement.lang || null);
  const wasDark = await page.evaluate(() => document.documentElement.classList.contains('dark'));
  const measures = [];
  const full = isFront(route, lang);
  const themes = full ? THEMES : [[wasDark ? 'dark' : 'light', wasDark]];
  for (const [name, dark] of themes) {
    await theme(dark);
    const closed = await read();
    if (!closed) { await theme(wasDark); return { findings, navbar: { lang, measures: null } }; }
    measures.push({ theme: name, state: 'closed', height: closed.height });
    if (name === themes[0][0]) {
      if (closed.rows > 1) {
        findings.push({ rule: 'navbar.wraps', detail: `the main navigation's items take ${closed.rows} rows` });
      }
      for (const text of closed.clipped) {
        findings.push({ rule: 'navbar.clipped', detail: `"${text}" is cut off inside the main navigation` });
      }
    }
    if (full && closed.toggle) {
      const button = page.locator(`nav[aria-label="Main"] [data-collapse-toggle="${closed.toggle}"]`).first();
      if (await button.click({ timeout: 2000 }).then(() => true, () => false)) {
        await page.waitForTimeout(SETTLE_MS);
        const opened = await read();
        if (opened) {
          measures.push({ theme: name, state: 'opened', height: opened.height });
          for (const text of opened.clipped) {
            if (name === themes[0][0]) findings.push({ rule: 'navbar.clipped', detail: `"${text}" is cut off inside the opened main navigation` });
          }
        }
        await button.click({ timeout: 2000 }).catch(() => {});
        await page.waitForTimeout(SETTLE_MS);
      }
    }
  }
  await theme(wasDark);
  return { findings, navbar: { lang, measures } };
}
