// site-i18n-probe — every link of every page, in every language, followed in a real browser
// (ADR 0218, project.every-link-and-control-is-tested).
//
//   node scripts/lib/site-i18n-probe.mjs <public-dir> <base-url> [--jobs N] [--only /path/ | /prefix/* ...]
//
// <base-url> is the address the build was made for (its config base_url); the probe answers it
// from <public-dir> itself. Every page
// of the site's own (a page that carries the language switch) is loaded in Chromium with its
// scripts running, so a link a script writes is a link like any other, and for each one:
//
//   lang        the document declares the language of the copy it is in
//   resolves    every same-origin link the page carries, after scripts ran, lands on a file the
//               build wrote (a page, an asset or a fragment's page)
//   stays       on a copy in another language, a link to a page that has a copy in that language
//               leads to that copy, not to the English original; only the switch and the
//               alternates, which exist to change language, are exempt
//   switch      the switch names every declared language, its links land on built pages, and the
//               link of the language being read is marked aria-current
//   alternates  the hreflang alternates name every declared language, each lands on a built page,
//               and each of those pages names this one back
//   layout      at every width the design declares, the bar of a copy has the height and the rows
//               of the English page's bar, and nothing in it overflows
//
// Exit 0 clean, 10 findings, 12 nothing to probe or no browser.

import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

const args = process.argv.slice(2);
const [publicDir, rawBase] = args;
if (!publicDir || !rawBase) {
  process.stderr.write('usage: site-i18n-probe.mjs <public-dir> <base-url> [--jobs N] [--only /path/ ...]\n');
  process.exit(2);
}
const base = rawBase.replace(/\/$/, '');
let jobs = 8;
const only = [];
for (let i = 2; i < args.length; i += 1) {
  if (args[i] === '--jobs') jobs = Number(args[++i]);
  else if (args[i] === '--only') only.push(args[++i]);
}

const languagesFile = join(publicDir, '..', 'i18n', 'languages.json');
const config = JSON.parse(readFileSync(existsSync(languagesFile) ? languagesFile
  : new URL('../../site/i18n/languages.json', import.meta.url), 'utf8'));
const others = config.languages.filter((l) => l !== config.source);

// The build, as the set of paths a link may land on.
const files = new Set();
(function walk(dir) {
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) walk(p);
    else files.add(`/${relative(publicDir, p)}`);
  }
})(publicDir);
// The mounts the publication holds a snapshot at, which the deploy places after this build and
// scripts/ci/link-check decides by the same declaration (ADR 0141).
const buildJson = join(publicDir, 'build.json');
const snapshots = existsSync(buildJson)
  ? (JSON.parse(readFileSync(buildJson, 'utf8')).snapshots ?? []).map((s) => `${s.mount}/`) : [];
const lands = (path) => {
  const p = decodeURI(path);
  if (snapshots.includes(p)) return true;
  if (files.has(p)) return true;
  if (p.endsWith('/')) return files.has(`${p}index.html`);
  return files.has(`${p}/index.html`) || (statSafe(join(publicDir, p)) && files.has(`${p}/index.html`));
};
function statSafe(p) { try { return statSync(p); } catch { return null; } }

const langOf = (path) => {
  const first = path.split('/')[1];
  return others.includes(first) ? first : config.source;
};
const counterpart = (path, lang) => {
  const l = langOf(path);
  const english = l === config.source ? path : path.slice(l.length + 1);
  return lang === config.source ? english : `/${lang}${english}`;
};

// The pages: every built page that carries the switch, which is every page of the site's own.
const pages = [...files].filter((f) => f.endsWith('/index.html'))
  .filter((f) => readFileSync(join(publicDir, f), 'utf8').includes('data-i18n-switch'))
  .map((f) => f.slice(0, -'index.html'.length))
  .filter((p) => !only.length || only.some((o) => p === o || (o.endsWith('*') && p.startsWith(o.slice(0, -1)))))
  .sort();
if (!pages.length) {
  process.stderr.write('site-i18n-probe: no page carries the language switch\n');
  process.exit(12);
}

let chromium;
try {
  ({ chromium } = await import('playwright'));
} catch {
  process.stderr.write('site-i18n-probe: playwright is not installed (npm ci)\n');
  process.exit(12);
}
const browser = await chromium.launch();
const findings = [];
let links = 0;
const finding = (page, kind, detail) => findings.push(`${kind.padEnd(10)} ${page}  ${detail}`);

async function probe(context, path) {
  const page = await context.newPage();
  try {
    const response = await page.goto(`${base}${path}`, { waitUntil: 'domcontentloaded', timeout: 60000 });
    if (!response || !response.ok()) { finding(path, 'resolves', `the page answered ${response?.status()}`); return; }
    if (new URL(page.url()).pathname !== path) {
      finding(path, 'lang', `the page sent the reader on to ${new URL(page.url()).pathname}`);
      return;
    }
    const seen = await page.evaluate(() => ({
      lang: document.documentElement.lang,
      canonical: document.querySelector('link[rel=canonical]')?.href ?? '',
      // an <a> inside an SVG diagram carries its href as an SVGAnimatedString
      anchors: [...document.querySelectorAll('a[href]')].map((a) => ({
        href: new URL(a.getAttribute('href'), location.href).href,
        keep: a.hasAttribute('data-i18n-keep') || a.hasAttribute('hreflang'),
        switchLang: a.getAttribute('data-i18n-lang'),
        current: a.getAttribute('aria-current'),
        inSwitch: !!a.closest('[data-i18n-switch]'),
      })),
      alternates: [...document.querySelectorAll('link[rel=alternate][hreflang]')]
        .map((l) => ({ lang: l.hreflang, href: l.href })),
    }));
    const lang = langOf(path);
    if (seen.lang !== lang && !(lang === config.source && seen.lang !== config.source)) {
      finding(path, 'lang', `declares ${seen.lang}, is the ${lang} copy`);
    }
    for (const a of seen.anchors) {
      const url = new URL(a.href);
      if (url.origin !== new URL(base).origin) continue;
      links += 1;
      const target = url.pathname;
      if (!lands(target)) { finding(path, 'resolves', `${target} is not built`); continue; }
      if (lang !== config.source && !a.keep && langOf(target) === config.source) {
        const there = counterpart(target, lang);
        if (lands(there)) finding(path, 'stays', `${target} leaves the ${lang} copy; ${there} exists`);
      }
    }
    const switchLinks = seen.anchors.filter((a) => a.inSwitch && a.switchLang);
    for (const l of config.languages) {
      const s = switchLinks.find((a) => a.switchLang === l);
      if (!s) { finding(path, 'switch', `names no ${l} link`); continue; }
      const target = new URL(s.href).pathname;
      if (target !== counterpart(path, l)) finding(path, 'switch', `${l} leads to ${target}, not ${counterpart(path, l)}`);
      if ((l === lang) !== (s.current === 'true')) finding(path, 'switch', `${l} is${s.current ? '' : ' not'} marked current`);
    }
    // an authored page written in another language at an English address is that language's
    const docLang = lang === config.source ? seen.lang : lang;
    // a duplicate — a copy whose canonical is another page — is held only to naming that page
    const canonicalPath = new URL(seen.canonical || `${base}${path}`).pathname;
    const duplicate = canonicalPath !== path;
    // an authored page in another language, and its copies, owe only their own language: there
    // is no English original to name
    const authored = (lang === config.source && seen.lang !== config.source)
      || (duplicate && !seen.alternates.some((a) => a.lang === config.source));
    for (const l of authored ? [docLang] : config.languages) {
      const alt = seen.alternates.find((a) => a.lang === l);
      if (!alt) { finding(path, 'alternate', `names no ${l} alternate`); continue; }
      const target = new URL(alt.href).pathname;
      if (!lands(target)) { finding(path, 'alternate', `${l} alternate ${target} is not built`); continue; }
      // a copy whose own language is answered by an authored page elsewhere is that page's
      // duplicate: it names the authored page canonical, and the pair is held to each other
      if (duplicate) {
        if (l === docLang && target !== canonicalPath) {
          finding(path, 'alternate', `its canonical is ${canonicalPath} and its ${l} alternate ${target}`);
        }
        continue;
      }
      if (l === docLang && target !== path) {
        if (new URL(seen.canonical || base).pathname !== target) {
          finding(path, 'alternate', `its ${l} alternate is ${target} and its canonical is not`);
        }
        continue;
      }
      const back = readFileSync(join(publicDir, target.endsWith('/') ? `${target}index.html` : target), 'utf8');
      const named = [...back.matchAll(/<link rel="alternate" hreflang="([^"]+)" href="([^"]+)">/g)]
        .find((m) => m[1] === docLang);
      if (!named || new URL(named[2], base).pathname !== path) {
        finding(path, 'alternate', `${target} does not name ${path} back as ${docLang}`);
      }
    }
  } catch (error) {
    finding(path, 'resolves', `the browser failed: ${error.message.split('\n')[0]}`);
  } finally {
    await page.close();
  }
}

const context = await browser.newContext();
// Hermetic: the site's origin is answered from the build itself and nothing else is asked, so the
// probe measures the tree being published at the address it is published at, with no server and
// no network. Only documents and scripts are served: the links are what is measured.
const TYPES = { html: 'text/html', js: 'text/javascript', mjs: 'text/javascript', json: 'application/json',
  xml: 'application/xml', txt: 'text/plain', svg: 'image/svg+xml' };
await context.route('**/*', (route) => {
  const request = route.request();
  if (['image', 'font', 'media', 'stylesheet'].includes(request.resourceType())) return route.abort();
  const url = new URL(request.url());
  if (url.origin !== new URL(base).origin) return route.abort();
  let file = decodeURI(url.pathname);
  if (file.endsWith('/')) file += 'index.html';
  else if (!files.has(file) && files.has(`${file}/index.html`)) file += '/index.html';
  if (!files.has(file)) return route.fulfill({ status: 404, body: 'not built' });
  const ext = file.split('.').pop();
  return route.fulfill({ status: 200, contentType: TYPES[ext] ?? 'application/octet-stream',
    body: readFileSync(join(publicDir, file)) });
});
// A stored choice would send the reader on; the probe reads every copy as it is linked.
await context.addInitScript(() => { try { localStorage.setItem('mj-lang', 'en'); } catch {} });
let next = 0;
let done = 0;
await Promise.all(Array.from({ length: Math.max(1, jobs) }, async () => {
  while (next < pages.length) {
    const path = pages[next];
    next += 1;
    await probe(context, path);
    done += 1;
    if (done % 500 === 0) process.stderr.write(`  ${done}/${pages.length} page(s)\n`);
  }
}));
// layout: the bar of every copy is the bar of the English page, at every width the design
// declares — a translation that is longer than its English must not wrap the bar onto a second
// line, push a control off it or change its height (the owner's report of 2026-10-19: the Czech
// bar wrapped at 1280 px)
const tokens = readFileSync(new URL('../../share/design/tokens.yaml', import.meta.url), 'utf8');
// the widths the design declares, and the tablet width between them where a bar most often wraps
const WIDTHS = [...new Set([...(/^viewports:\s*\[([^\]]*)\]/m.exec(tokens)?.[1] ?? '320, 1280')
  .split(',').map(Number), 768])].sort((a, b) => a - b);
const SAMPLE = ['/', '/getting-started/', '/docs/'].filter((p) => pages.includes(p));
for (const width of WIDTHS) {
  const layout = await browser.newContext({ viewport: { width, height: 900 } });
  await layout.route('**/*', (route) => {
    const url = new URL(route.request().url());
    if (url.origin !== new URL(base).origin) return route.abort();
    let file = decodeURI(url.pathname);
    if (file.endsWith('/')) file += 'index.html';
    if (!files.has(file)) return route.fulfill({ status: 404, body: 'not built' });
    const ext = file.split('.').pop();
    const type = { css: 'text/css', woff2: 'font/woff2', png: 'image/png', svg: 'image/svg+xml' }[ext]
      ?? TYPES[ext] ?? 'application/octet-stream';
    return route.fulfill({ status: 200, contentType: type, body: readFileSync(join(publicDir, file)) });
  });
  await layout.addInitScript(() => { try { localStorage.setItem('mj-lang', 'en'); } catch {} });
  const page = await layout.newPage();
  const bar = async (path) => {
    await page.goto(`${base}${path}`, { waitUntil: 'load' });
    return page.evaluate(() => {
      const nav = document.querySelector('body nav');
      const r = nav.getBoundingClientRect();
      const tops = new Set([...nav.querySelectorAll('a, button')].filter((e) => e.offsetParent)
        .map((e) => Math.round(e.getBoundingClientRect().top)));
      return { height: Math.round(r.height), rows: tops.size, overflow: nav.scrollWidth > nav.clientWidth };
    });
  };
  for (const path of SAMPLE) {
    const english = await bar(path);
    for (const l of others) {
      const copy = `/${l}${path}`;
      if (!pages.includes(copy)) continue;
      const seen = await bar(copy);
      if (seen.height !== english.height || seen.rows !== english.rows || seen.overflow) {
        finding(copy, 'layout', `at ${width}px the bar is ${seen.height}px in ${seen.rows} row(s)${seen.overflow ? ', overflowing' : ''}; the English is ${english.height}px in ${english.rows}`);
      }
    }
  }
  await layout.close();
}
await browser.close();

for (const f of findings.sort()) console.log(f);
console.log(`site-i18n-probe: ${pages.length} page(s) in ${config.languages.length} language(s), ${links} link(s) followed, ${findings.length} finding(s)`);
process.exit(findings.length ? 10 : 0);
