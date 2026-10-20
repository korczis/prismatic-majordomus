// The audit runner: the plan, a browser, a running server, and one results document.
//
// It holds no route, no width and no threshold. The pages and the widths come from
// ui-discover.mjs, the invariants from ui-audit.mjs, and the verdict is arithmetic over
// what those two produced. What this file owns is the shape of the evidence — `ui-audit/v1`
// — because that document is what the report renders and what CI compares, and a shape
// invented at each call site is a shape that drifts.

import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { planSurfaces } from './ui-discover.mjs';

/** The contract of the results document, read back by `majordomus web report ui`. */
export const RESULTS_SCHEMA = 'ui-audit/v1';

/**
 * Run the plan against a running origin and return the results document.
 *
 * `surfaces` is what the topology says the executable serves, as `ui-routes.mjs` resolved
 * it: a built surface carries the directory it was rendered into, and a surface the
 * executable renders carries the routes crawled out of it. The mount matters — a built
 * directory does not know where it is served from, and a plan that assumed the root would
 * visit paths nobody answers.
 *
 * `select` narrows the pages for local iteration; a narrowed run says so in the document,
 * so a partial run can never be read as a clean full one.
 *
 * `shard` ("K/N") keeps the stride of the sorted routes that `scripts/site-probe --partition`
 * gives, the one rule every browser gate is split by (I2841, I3602, I3604), and the document
 * then names the routes it audited; `report` names a file that receives them too, for
 * `site-probe --verify-report`. A shard's document is never complete on its own: only the
 * `merge` of every shard, verified, is.
 */
export async function run(origin, surfaces, cssPath, { select, limit, onVisit, shard, report } = {}) {
  const target = planSurfaces(surfaces, cssPath);
  let pages = target.pages;
  if (select) pages = pages.filter((page) => page.route.includes(select));
  if (limit) pages = pages.slice(0, limit);
  const all = routesOf(pages);
  const sharded = Boolean(shard) && shard !== '1/1';
  if (sharded) pages = shardPages(pages, shard);
  const routes = routesOf(pages);
  if (report) {
    mkdirSync(dirname(report), { recursive: true });
    writeFileSync(report, `# shard ${shard || '1/1'} of ${all.length} routes\n${routes.map((r) => `${r}\n`).join('')}`);
  }

  // the browser engine is the audit's alone: `merge`, which a fan-in with no browser and no
  // node_modules runs, must not load it
  const { audit, jobs } = await import('./ui-audit.mjs');
  const started = Date.now();
  const concurrency = jobs();
  const visits = await audit(origin, pages, { onVisit, concurrency });
  const findings = [];
  for (const visit of visits) {
    for (const finding of visit.findings) {
      findings.push({ route: visit.route, width: visit.width, tier: visit.tier, ...finding });
    }
  }

  // What the run did not measure, and what declared it. A subtree a page marks
  // `data-mj-foreign` is a third-party component tree the repository has decided not to
  // restyle, and the engine was told to skip it; saying so is the difference between a clean
  // report and a report that quietly narrowed its own question. Deduplicated by what was
  // declared, because one widget on forty pages is one decision.
  const foreign = [];
  const declared = new Set();
  for (const visit of visits) {
    for (const subtree of visit.foreign ?? []) {
      if (declared.has(subtree.declares)) continue;
      declared.add(subtree.declares);
      foreign.push({ route: visit.route, ...subtree });
    }
  }

  return {
    schema: RESULTS_SCHEMA,
    origin,
    complete: pages.length === target.pages.length,
    source: target.source,
    breakpoints: target.breakpoints,
    viewports: target.viewports,
    surfaces: target.surfaces,
    pages: pages.length,
    visits: visits.length,
    seconds: Math.round((Date.now() - started) / 1000),
    concurrency,
    rules: ruleCounts(findings),
    foreign,
    findings,
    ...(sharded ? { shard, routes } : {}),
  };
}

/** Write a results document where the report renderer expects to read it. */
export function write(path, results) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `${JSON.stringify(results, null, 2)}\n`);
}

/** The distinct routes of a page list, in code-unit order. */
function routesOf(pages) {
  return [...new Set(pages.map((page) => page.route))].sort();
}

/**
 * How many findings each rule produced, most first and then by name. `<` and `>` compare code
 * units, the way every Rust and shell comparator here does; `localeCompare` would order this
 * report by whoever's machine rendered it.
 */
function ruleCounts(findings) {
  const rules = {};
  for (const finding of findings) rules[finding.rule] = (rules[finding.rule] ?? 0) + 1;
  return Object.fromEntries(Object.entries(rules).sort((a, b) => b[1] - a[1] || (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0)));
}

const SITE_PROBE = join(dirname(fileURLToPath(import.meta.url)), '..', 'site-probe');

/** The pages of shard "K/N": those whose route `site-probe --partition K/N` keeps. */
export function shardPages(pages, shard) {
  const kept = new Set(
    execFileSync(SITE_PROBE, ['--partition', shard], { input: `${routesOf(pages).join('\n')}\n` })
      .toString().split('\n').filter(Boolean),
  );
  return pages.filter((page) => kept.has(page.route));
}

/**
 * One results document out of the documents of every shard of one sweep. It refuses parts
 * that were not planned alike (another source, other widths, other surfaces) or that
 * audited a route twice, because a merge that quietly double-counts or mixes two plans is a
 * report that overstates what it saw. Whether the parts are every shard is the route
 * reports' question (`site-probe --verify-report`); `complete` is what the caller says that
 * check answered. The merged document has the shape a single runner writes, so the report
 * and every reader of `ui-audit/v1` take it unchanged.
 */
export function merge(parts, { complete = false } = {}) {
  if (!parts.length) throw new Error('no shard documents to merge');
  const [first] = parts;
  for (const part of parts) {
    if (part.schema !== RESULTS_SCHEMA) throw new Error(`a part is ${part.schema}, not ${RESULTS_SCHEMA}`);
    for (const key of ['source', 'breakpoints', 'viewports', 'surfaces']) {
      if (JSON.stringify(part[key]) !== JSON.stringify(first[key])) throw new Error(`the parts disagree on ${key}: they were not planned alike`);
    }
    // a part that does not say which routes it audited cannot be checked for overlap, so the
    // merge refuses it on its own rather than trusting site-probe --verify-report to have run
    if (!Array.isArray(part.routes)) throw new Error(`shard ${part.shard ?? '(unnamed)'} does not list the routes it audited`);
  }
  const seen = new Map();
  for (const part of parts) {
    for (const route of part.routes) {
      if (seen.has(route)) throw new Error(`${route} was audited by shard ${seen.get(route)} and by shard ${part.shard}`);
      seen.set(route, part.shard);
    }
  }
  const order = (a, b) => (a < b ? -1 : a > b ? 1 : 0);
  const findings = parts.flatMap((part) => part.findings)
    .sort((a, b) => order(a.route, b.route) || (a.width ?? 0) - (b.width ?? 0) || order(a.rule, b.rule));
  const foreign = [];
  const declared = new Set();
  for (const subtree of parts.flatMap((part) => part.foreign ?? [])) {
    if (declared.has(subtree.declares)) continue;
    declared.add(subtree.declares);
    foreign.push(subtree);
  }
  return {
    schema: RESULTS_SCHEMA,
    origin: first.origin,
    complete,
    source: first.source,
    breakpoints: first.breakpoints,
    viewports: first.viewports,
    surfaces: first.surfaces,
    pages: parts.reduce((n, part) => n + part.pages, 0),
    visits: parts.reduce((n, part) => n + part.visits, 0),
    seconds: Math.max(...parts.map((part) => part.seconds ?? 0)),
    concurrency: parts.reduce((n, part) => n + (part.concurrency ?? 0), 0),
    rules: ruleCounts(findings),
    foreign,
    findings,
  };
}
