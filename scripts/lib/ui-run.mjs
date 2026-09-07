// The audit runner: the plan, a browser, a running server, and one results document.
//
// It holds no route, no width and no threshold. The pages and the widths come from
// ui-discover.mjs, the invariants from ui-audit.mjs, and the verdict is arithmetic over
// what those two produced. What this file owns is the shape of the evidence — `ui-audit/v1`
// — because that document is what the report renders and what CI compares, and a shape
// invented at each call site is a shape that drifts.

import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';

import { plan } from './ui-discover.mjs';
import { audit } from './ui-audit.mjs';

/** The contract of the results document, read back by `majordomus web report ui`. */
export const RESULTS_SCHEMA = 'ui-audit/v1';

/**
 * Run the plan against a running origin and return the results document.
 *
 * `select` narrows the pages for local iteration; a narrowed run says so in the document,
 * so a partial run can never be read as a clean full one.
 */
export async function run(origin, publicDir, cssPath, { select, limit, onVisit } = {}) {
  const target = plan(publicDir, cssPath);
  let pages = target.pages;
  if (select) pages = pages.filter((page) => page.route.includes(select));
  if (limit) pages = pages.slice(0, limit);

  const started = Date.now();
  const visits = await audit(origin, pages, { onVisit });
  const findings = [];
  for (const visit of visits) {
    for (const finding of visit.findings) {
      findings.push({ route: visit.route, width: visit.width, tier: visit.tier, ...finding });
    }
  }
  const rules = {};
  for (const finding of findings) rules[finding.rule] = (rules[finding.rule] ?? 0) + 1;

  return {
    schema: RESULTS_SCHEMA,
    origin,
    complete: pages.length === target.pages.length,
    source: target.source,
    breakpoints: target.breakpoints,
    viewports: target.viewports,
    pages: pages.length,
    visits: visits.length,
    seconds: Math.round((Date.now() - started) / 1000),
    rules: Object.fromEntries(Object.entries(rules).sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))),
    findings,
  };
}

/** Write a results document where the report renderer expects to read it. */
export function write(path, results) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `${JSON.stringify(results, null, 2)}\n`);
}
