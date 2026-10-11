#!/usr/bin/env node
// search-check: the site's one search answers for every page, offers nothing it cannot open, and
// can be opened from every layout. Run by scripts/site-check (check 20) over site/public after
// scripts/site-build, and by test/cases/1071_the_site_searches_from_everywhere.sh over a build
// of its own.
//
// The search (site/search.js) reads /docs/index.json, which the build writes from the pages it
// published (scripts/lib/docs-index.mjs). This holds the two to each other in both directions,
// and holds the templates to the trigger:
//
//   index      the built index exists, parses, and lists pages
//   entries    every entry's route is a page this build wrote, and not a redirect stub
//   complete   every page this build wrote is an entry: an index that silently lost a section
//              would make the search say "no page matches" about a page that exists
//   scope      nothing is offered outside the publication: an entry carrying a kind is of a
//              kind site/data/publication.toml publishes (through the catalogue of
//              site/data/generated/entities.json) and lies under that kind's route, and an
//              entry naming its source names a file git tracks, so nothing of the local half
//              of the layer, or of any other untracked file, can become a result
//   trigger    every page of the site's own carries the trigger once, the dialog it controls
//              once, and the script that drives both once. A composed surface's pages
//              (ADR 0086) are another producer's and carry their own navigation; their mounts
//              are the ones the build recorded in /build.json, so nothing here names one
//   layouts    every template that renders a document reaches the trigger and the dialog,
//              through what it extends and includes: a layout family added tomorrow without
//              the navbar is refused at its source, before a page of it is built
//
//   scripts/lib/search-check.mjs [--public DIR] [--templates DIR] [--entities FILE]
//
// Defaults are the checkout's site/public, site/templates and site/data/generated/entities.json,
// under MJ_ROOT when it is set. Exit 0 clean, 10 findings, 12 the build is not there to judge.

import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { basename, dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { readPage, routeOf } from './docs-index.mjs';

const ROOT = process.env.MJ_ROOT || resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');
const args = process.argv.slice(2);
const opt = (name, fallback) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? resolve(args[i + 1]) : fallback;
};
const PUB = opt('--public', join(ROOT, 'site', 'public'));
const TEMPLATES = opt('--templates', join(ROOT, 'site', 'templates'));
const ENTITIES = opt('--entities', join(ROOT, 'site', 'data', 'generated', 'entities.json'));
const SHOWN = 12;
// the name a summary line carries: scripts/site-check relays `<name>: ...` as the run's summary
const NAME = basename(process.argv[1] || 'search-check.mjs');

if (!existsSync(PUB)) {
  console.error(`${NAME}: ${PUB} does not exist; run scripts/site-build`);
  process.exit(12);
}

let fails = 0;
const ok = (msg) => console.log(`OK   search       ${msg}`);
const fail = (msg) => { fails++; console.log(`FAIL search       ${msg}`); };
// one finding per offender up to a bound, then one line for the rest, so a template that broke
// every page reads as one cause rather than as two thousand lines
const report = (what, offenders) => {
  offenders.slice(0, SHOWN).forEach((o) => fail(`${o}: ${what}`));
  if (offenders.length > SHOWN) fail(`and ${offenders.length - SHOWN} more page(s): ${what}`);
};

const walk = (dir, out = []) => {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, entry.name);
    if (entry.isDirectory()) walk(p, out);
    else out.push(p);
  }
  return out;
};

// ------------------------------------------------------------------------------------ index
const IX = join(PUB, 'docs', 'index.json');
let index = null;
try {
  index = JSON.parse(readFileSync(IX, 'utf8'));
} catch (e) {
  fail(`/docs/index.json is missing or does not parse (${e.code || e.message}); the search has nothing to read`);
}
if (index && (index.schema !== 1 || !Array.isArray(index.pages) || index.pages.length === 0)) {
  fail('/docs/index.json is not a documentation index of schema 1 with pages');
  index = null;
}

// the composed surfaces' mounts, as the build recorded them beside the pages
let mounts = [];
try {
  const b = JSON.parse(readFileSync(join(PUB, 'build.json'), 'utf8'));
  mounts = (b.surfaces || []).map((s) => s.mount).filter((m) => typeof m === 'string' && m.startsWith('/'));
} catch { /* a build with no build.json composed nothing */ }
const composed = (route) => mounts.some((m) => route === `${m}/` || route.startsWith(`${m}/`));

// every document of the build: route -> html
const built = new Map();
for (const file of walk(PUB)) {
  if (!file.endsWith(`${sep}index.html`) && file !== join(PUB, 'index.html')) continue;
  const html = readFileSync(file, 'utf8');
  if (!readPage(html)) continue;
  built.set(routeOf(PUB, file), html);
}

if (index) {
  // ---------------------------------------------------------------------------------- entries
  const missing = [];
  const seen = new Set();
  for (const p of index.pages) {
    const route = p && p.route;
    if (typeof route !== 'string' || !route.startsWith('/') || route.includes('..')) {
      missing.push(JSON.stringify(route));
      continue;
    }
    seen.add(route);
    if (!built.has(route)) missing.push(route);
    else if (!p.title) missing.push(`${route} (no title)`);
  }
  if (missing.length) report('an index entry the build did not write as a page, so a result would 404', missing);
  else ok(`every one of ${index.pages.length} index entries is a page this build wrote`);

  // --------------------------------------------------------------------------------- complete
  const absent = [...built.keys()].filter((r) => !seen.has(r)).sort();
  if (absent.length) report('a page this build wrote that the search cannot find', absent);
  else ok(`every one of ${built.size} built page(s) is an entry`);

  // ------------------------------------------------------------------------------------ scope
  let entities = null;
  try { entities = JSON.parse(readFileSync(ENTITIES, 'utf8')); } catch { /* reported below */ }
  if (!entities || !Array.isArray(entities.catalogue)) {
    fail(`${relative(ROOT, ENTITIES)} has no catalogue; the publication scope cannot be read`);
  } else {
    const catalogue = new Map(entities.catalogue.map((k) => [k.kind, k]));
    let tracked = null;
    try {
      tracked = new Set(execFileSync('git', ['-C', ROOT, 'ls-files', '-z'], { encoding: 'utf8', maxBuffer: 1 << 28 })
        .split('\0').filter(Boolean));
    } catch (e) {
      fail(`git could not list the tracked files of ${ROOT}; an entry's source cannot be judged (${e.message.split('\n')[0]})`);
    }
    const outside = [];
    for (const p of index.pages) {
      if (p.kind !== undefined) {
        const k = catalogue.get(p.kind);
        if (!k || !k.route || k.projection === 'none') outside.push(`${p.route} (kind ${p.kind}, which the publication declares unpublished)`);
        else if (!p.route.startsWith(k.route)) outside.push(`${p.route} (kind ${p.kind}, published at ${k.route})`);
      }
      if (p.source !== undefined && tracked && !tracked.has(p.source)) {
        outside.push(`${p.route} (source ${p.source}, which git does not track)`);
      }
    }
    if (outside.length) report('an index entry outside the publication scope', outside);
    else ok(`no entry lies outside the publication: every kind is published at its route, every source is tracked`);
  }
}

// ---------------------------------------------------------------------------------- trigger
const count = (html, rx) => (html.match(rx) || []).length;
const own = [...built.entries()].filter(([route]) => !composed(route));
const noTrigger = [];
for (const [route, html] of own) {
  const triggers = count(html, /<button\b[^>]*\bdata-site-search-open\b[^>]*>/g);
  const wired = count(html, /<button\b[^>]*\bdata-site-search-open\b[^>]*\baria-controls="site-search"[^>]*>/g);
  const dialogs = count(html, /<dialog\b[^>]*\bid="site-search"[^>]*\bdata-index="[^"]*\/docs\/index\.json"[^>]*>/g);
  const scripts = count(html, /<script\b[^>]*\bsrc="[^"]*\/js\/search\.js"/g);
  if (triggers !== 1 || wired !== 1 || dialogs !== 1 || scripts !== 1) {
    noTrigger.push(`${route} (trigger ${triggers}, wired ${wired}, dialog ${dialogs}, script ${scripts})`);
  }
}
if (own.length === 0) fail('the build holds no page of the site\'s own; a check over nothing has not passed');
else if (noTrigger.length) report('does not carry the search trigger, its dialog over /docs/index.json and js/search.js exactly once', noTrigger);
else ok(`all ${own.length} page(s) of the site's own carry the trigger, the dialog and the script once${mounts.length ? ` (composed, left to their producers: ${mounts.join(' ')})` : ''}`);

// ---------------------------------------------------------------------------------- layouts
if (!existsSync(TEMPLATES) || !statSync(TEMPLATES).isDirectory()) {
  fail(`${TEMPLATES} is not a template directory`);
} else {
  const source = new Map();
  for (const f of walk(TEMPLATES)) {
    if (!f.endsWith('.html')) continue;
    // a mention in a template comment is not markup
    source.set(relative(TEMPLATES, f).split(sep).join('/'), readFileSync(f, 'utf8').replace(/\{#[\s\S]*?#\}/g, ''));
  }
  const extendsOf = (t) => ((source.get(t) || '').match(/\{%-?\s*extends\s+"([^"]+)"/) || [])[1];
  const reaches = new Map();
  // what a template's markup carries, itself and through every template it includes
  const carries = (t, marker, stack = new Set()) => {
    const key = `${t}\0${marker}`;
    if (reaches.has(key)) return reaches.get(key);
    if (stack.has(t) || !source.has(t)) return false;
    stack.add(t);
    const text = source.get(t);
    let found = text.includes(marker);
    for (const m of text.matchAll(/\{%-?\s*include\s+"([^"]+)"/g)) {
      if (found) break;
      found = carries(m[1], marker, stack);
    }
    stack.delete(t);
    reaches.set(key, found);
    return found;
  };
  const roots = [...source.keys()].filter((t) => !extendsOf(t) && /<html[\s>]/i.test(source.get(t))).sort();
  const lacking = [];
  for (const t of roots) {
    for (const marker of ['data-site-search-open', 'id="site-search"', 'js/search.js']) {
      if (!carries(t, marker)) lacking.push(`site/templates/${t} (no ${marker})`);
    }
  }
  // every template that extends another ends at a root that rendered the document
  const orphans = [];
  for (const t of source.keys()) {
    let at = t; const seen = new Set();
    while (extendsOf(at) && !seen.has(at)) { seen.add(at); at = extendsOf(at); }
    if (at !== t && !roots.includes(at)) orphans.push(`site/templates/${t} (extends down to ${at}, which renders no document)`);
  }
  if (roots.length === 0) fail('no template renders a document (<html>); the layouts cannot be judged');
  else if (lacking.length || orphans.length) {
    report('a layout that does not reach the site\'s search', lacking);
    report('a template outside every layout family', orphans);
  } else {
    const families = [...source.keys()].filter((t) => extendsOf(t)).length;
    ok(`every layout reaches the trigger, the dialog and the script: ${roots.join(', ')}, and the ${families} template(s) that extend it`);
  }
}

if (fails) {
  console.log(`${NAME}: ${fails} finding(s)`);
  process.exit(10);
}
