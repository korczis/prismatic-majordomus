// The audit runner: the plan, a browser, a running server, and one results document.
//
// It holds no route, no width and no threshold. The pages and the widths come from
// ui-discover.mjs, the invariants from ui-audit.mjs, and the verdict is arithmetic over
// what those two produced. What this file owns is the shape of the evidence — `ui-audit/v1`
// — because that document is what the report renders and what CI compares, and a shape
// invented at each call site is a shape that drifts.

import { planSurfaces } from './ui-discover.mjs';
import { audit, jobs } from './ui-audit.mjs';
import { RESULTS_SCHEMA, write } from './ui-results.mjs';
import { shardPages, visitId, visitsOf } from './ui-shard.mjs';

export { RESULTS_SCHEMA, write };

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
 * `shard` is `{ index, of }` (ui-shard.mjs): this run drives shard K of N of the visits that
 * remain after `select` and `limit`, and the document says which shard it is, how many visits
 * the plan had and which of them it drove, so that the union of the N documents can prove it
 * is the whole audit, each visit once.
 */
export async function run(origin, surfaces, cssPath, { select, limit, shard, onVisit } = {}) {
  const target = planSurfaces(surfaces, cssPath);
  let pages = target.pages;
  if (select) pages = pages.filter((page) => page.route.includes(select));
  if (limit) pages = pages.slice(0, limit);
  const narrowed = pages.length !== target.pages.length;
  const planned = visitsOf(pages).length;
  if (shard) pages = shardPages(pages, shard);

  const started = Date.now();
  const concurrency = jobs();
  const visits = await audit(origin, pages, { onVisit, concurrency });
  const findings = [];
  for (const visit of visits) {
    for (const finding of visit.findings) {
      findings.push({ route: visit.route, width: visit.width, tier: visit.tier, ...finding });
    }
  }
  const rules = {};
  for (const finding of findings) rules[finding.rule] = (rules[finding.rule] ?? 0) + 1;

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
    // a shard is a part of the audit however much of it the plan held; the union says whether
    // the parts are the whole
    complete: !narrowed && (!shard || shard.of === 1),
    source: target.source,
    breakpoints: target.breakpoints,
    viewports: target.viewports,
    surfaces: target.surfaces,
    pages: pages.length,
    visits: visits.length,
    seconds: Math.round((Date.now() - started) / 1000),
    concurrency,
    ...(shard ? {
      shard: {
        index: shard.index,
        of: shard.of,
        planned,
        narrowed,
        driven: visits.map((visit) => visitId(visit.route, visit.width)),
      },
    } : {}),
    // `<` and `>` compare code units, the way every Rust and shell comparator here does.
    // `localeCompare` would order this report by whoever's machine rendered it.
    rules: Object.fromEntries(Object.entries(rules).sort((a, b) => b[1] - a[1] || (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0))),
    foreign,
    findings,
  };
}
