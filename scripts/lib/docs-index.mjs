// The machine-readable documentation index, /docs/index.json.
//
// It is a serialisation of what the build published, not a list anybody keeps: every page is
// found by walking the built site for index.html files, its title and description are the
// ones the page itself carries, and nothing here names a route. The kinds and their counts are
// site/data/generated/entities.json's catalogue (a projection of site/data/publication.toml
// and the index), the interface counts are the registry's own summary, and the build identity
// is the /build.json the same build serves. A reader of this file — the documentation hub's
// search, the Cockpit, an agent — therefore sees exactly the site a visitor sees, and a page
// that was not built cannot appear in it.
//
// Deterministic: pages are sorted by route, keys are written in a fixed order, and nothing is
// timestamped; two builds of one tree write the same bytes.
//
//   node scripts/lib/docs-index.mjs <public-dir> <entities.json> <registry.json>
//
// Exit 0 written, 2 usage, 10 a published route was not built, 12 an input is missing.

import { existsSync, readdirSync, readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { join, relative, sep } from 'node:path';

export function decode(text) {
  return text
    .replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"')
    .replace(/&#x27;|&#39;/g, "'").replace(/&mdash;/g, '—').replace(/&ndash;/g, '–')
    .replace(/&rarr;/g, '→').replace(/&hellip;/g, '…').replace(/&nbsp;/g, ' ')
    .replace(/&#(\d+);/g, (_, n) => String.fromCodePoint(Number(n)))
    .replace(/&amp;/g, '&');
}

// The title a page gives itself in its <head> (an inline SVG carries <title> elements of its
// own further down), without the " — <site name>" suffix, where the site name is the one the
// page states in og:site_name; and the description it states in its meta tag. A page with no
// head title, or one that only redirects, is not a document and is left out.
export function readPage(html) {
  const head = (html.match(/<head[\s>][\s\S]*?<\/head>/i) || [''])[0];
  const t = head.match(/<title>([\s\S]*?)<\/title>/i);
  if (!t) return null;
  if (/<meta[^>]+http-equiv=["']?refresh/i.test(head)) return null;
  const site = head.match(/<meta\s+property=["']og:site_name["']\s+content=["']([^"']*)["']/i);
  let title = decode(t[1].trim());
  const suffix = site ? ` — ${decode(site[1])}` : null;
  if (suffix && title.endsWith(suffix) && title.length > suffix.length) {
    title = title.slice(0, -suffix.length).trim();
  }
  const d = head.match(/<meta\s+name=["']description["']\s+content=["']([\s\S]*?)["']\s*\/?>/i);
  return { title, description: d ? decode(d[1].trim()) : '' };
}

// The route of a built file: public/adrs/adr-0001/index.html is /adrs/adr-0001/.
export function routeOf(publicDir, file) {
  const rel = relative(publicDir, file).split(sep).join('/');
  return '/' + rel.replace(/(^|\/)index\.html$/, '$1');
}

function walk(dir, out) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, entry.name);
    if (entry.isDirectory()) walk(p, out);
    else if (entry.name === 'index.html') out.push(p);
  }
  return out;
}

export function build({ publicDir, entities, registry, buildInfo }) {
  const byRoute = new Map();
  const published = new Map(entities.kinds.map((k) => [k.kind, k.route]));
  for (const e of entities.entities) {
    const base = published.get(e.kind);
    if (base) byRoute.set(`${base}${e.slug}/`, e);
  }
  const pages = [];
  for (const file of walk(publicDir, []).sort()) {
    const page = readPage(readFileSync(file, 'utf8'));
    if (!page) continue;
    const route = routeOf(publicDir, file);
    const e = byRoute.get(route);
    pages.push(e
      ? { route, title: page.title, description: page.description, kind: e.kind, id: e.id, source: e.source }
      : { route, title: page.title, description: page.description });
  }
  pages.sort((a, b) => (a.route < b.route ? -1 : a.route > b.route ? 1 : 0));
  const s = registry.registry.summary;
  return {
    schema: 1,
    generated_by: 'scripts/lib/docs-index.mjs',
    build: {
      commit: buildInfo.commit ?? null,
      source_version: buildInfo.source_version ?? null,
      registry_fingerprint: buildInfo.registry_fingerprint ?? null,
    },
    counts: {
      pages: pages.length,
      capabilities: s.total,
      cli_commands: s.cli_commands,
      http_routes: s.http_routes,
      mcp_tools: s.mcp_tools,
      mcp_resources: s.mcp_resources,
      modules: s.modules,
    },
    kinds: entities.catalogue,
    pages,
  };
}

// What the build owes the index: every route the catalogue publishes a kind at, and every
// entity page, is a page that was built. A missing one is a documentation link that 404s on
// the public site, so it refuses the build rather than being left for a visitor to find.
export function missing(index, entities) {
  const have = new Set(index.pages.map((p) => p.route));
  const want = [];
  for (const k of entities.catalogue) if (k.route) want.push(k.route);
  const published = new Map(entities.kinds.map((k) => [k.kind, k.route]));
  for (const e of entities.entities) {
    const base = published.get(e.kind);
    if (base) want.push(`${base}${e.slug}/`);
  }
  return [...new Set(want)].filter((r) => !have.has(r)).sort();
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const [publicDir, entitiesPath, registryPath] = process.argv.slice(2);
  if (!publicDir || !entitiesPath || !registryPath) {
    console.error('usage: docs-index.mjs <public-dir> <entities.json> <registry.json>');
    process.exit(2);
  }
  for (const p of [publicDir, entitiesPath, registryPath]) {
    if (!existsSync(p)) { console.error(`docs-index: ${p} does not exist`); process.exit(12); }
  }
  const entities = JSON.parse(readFileSync(entitiesPath, 'utf8'));
  if (!Array.isArray(entities.catalogue)) {
    console.error(`docs-index: ${entitiesPath} has no catalogue; run scripts/generate-site-data`);
    process.exit(12);
  }
  const buildPath = join(publicDir, 'build.json');
  const buildInfo = existsSync(buildPath) ? JSON.parse(readFileSync(buildPath, 'utf8')) : {};
  const index = build({
    publicDir, entities, registry: JSON.parse(readFileSync(registryPath, 'utf8')), buildInfo,
  });
  const gone = missing(index, entities);
  if (gone.length > 0) {
    for (const r of gone) console.error(`docs-index: ${r} is published by the catalogue but was not built`);
    process.exit(10);
  }
  mkdirSync(join(publicDir, 'docs'), { recursive: true });
  writeFileSync(join(publicDir, 'docs', 'index.json'), JSON.stringify(index, null, 1) + '\n');
  console.log(`  docs/index.json: ${index.pages.length} page(s), ${index.kinds.length} kind(s)`);
}
