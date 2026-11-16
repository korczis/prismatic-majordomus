// The audit as N shards (I3940): which visits a shard drives, and the proof that the shards
// together drove every visit exactly once.
//
// One serial audit took 57-80 minutes over about eight thousand visits (runs 39762797718 and
// 39780445745) and was the longest pole of a full validate run. A visit is independent of
// every other, so the audit splits the way the two probes do (I2841, I3602): the visits of
// the plan, in the plan's own deterministic order, are numbered from 1, and shard K of N
// drives those whose number is K modulo N — scripts/site-probe's `NR % n == k % n`, the same
// stride. A stride rather than N blocks, because the plan clusters the expensive pages (the
// Cockpit's, the documentation's) and a block would hand one shard all of them.
//
// This file holds no route and no width: it cuts whatever `planSurfaces` produced.

import { RESULTS_SCHEMA } from './ui-results.mjs';

/** The id of one visit, as a shard reports it and the union counts it. */
export const visitId = (route, width) => `${route}@${width}`;

/**
 * Read `K/N`: two positive integers with K at most N. Anything else throws, because a
 * malformed shard used to be the one thing that could make a sweep quietly drive nothing.
 */
export function parseShard(text) {
  const match = /^([0-9]+)\/([0-9]+)$/.exec(String(text ?? ''));
  const index = match ? Number(match[1]) : 0;
  const of = match ? Number(match[2]) : 0;
  if (!match || index < 1 || of < 1 || index > of) {
    throw new Error(`MJ_UI_SHARD must be K/N with 1 <= K <= N, got '${text ?? ''}'`);
  }
  return { index, of };
}

/** Every visit of a page list, in the order the audit drives them: page by page, width by width. */
export function visitsOf(pages) {
  const visits = [];
  for (const page of pages) for (const width of page.widths) visits.push({ route: page.route, width });
  return visits;
}

/**
 * The pages of shard K of N, each narrowed to the widths that fall to it; a page none of whose
 * visits fall to the shard is dropped. The Nth shard of N is `NR % N == 0`, as in the probes.
 */
export function shardPages(pages, { index, of }) {
  let number = 0;
  const kept = [];
  for (const page of pages) {
    const widths = page.widths.filter(() => {
      number += 1;
      return number % of === index % of;
    });
    if (widths.length) kept.push({ ...page, widths });
  }
  return kept;
}

const compare = (a, b) => (a < b ? -1 : a > b ? 1 : 0);

/**
 * Join the shards' results documents into the one document a single audit would have written,
 * and say what is wrong with the set. `problems` is empty only when the documents are shards
 * 1..N of one plan, each once, and the visits they drove are the plan's visits, each once.
 * A `results` is returned either way so that a person can read what the shards did find.
 */
export function unionResults(docs) {
  const problems = [];
  if (!docs.length) return { problems: ['no shard left a results document: no shard said what it drove'], results: null };

  const shards = docs.map((doc) => doc.shard).filter(Boolean);
  if (shards.length !== docs.length) problems.push('a results document does not say which shard it is');
  if (docs.some((doc) => doc.schema !== RESULTS_SCHEMA)) problems.push(`a results document is not ${RESULTS_SCHEMA}`);

  const of = [...new Set(shards.map((s) => s.of))];
  const planned = [...new Set(shards.map((s) => s.planned))];
  if (of.length > 1) problems.push(`the shards disagree on how many there are: ${of.join(' ')}`);
  if (planned.length > 1) problems.push(`the shards disagree on how many visits the plan has: ${planned.join(' ')}`);
  const same = (key) => new Set(docs.map((doc) => JSON.stringify(doc[key]))).size <= 1;
  if (!same('viewports') || !same('breakpoints')) problems.push('the shards were not planned alike: their widths differ');

  if (of.length === 1) {
    const got = shards.map((s) => s.index).sort((a, b) => a - b);
    const want = Array.from({ length: of[0] }, (_, i) => i + 1);
    if (got.join(' ') !== want.join(' ')) problems.push(`shards [${got.join(' ')}] reported, not each of 1..${of[0]} once`);
  }

  const driven = shards.flatMap((s) => s.driven ?? []);
  const seen = new Set();
  const twice = new Set();
  for (const id of driven) (seen.has(id) ? twice : seen).add(id);
  if (twice.size) problems.push(`driven by more than one shard: ${[...twice].slice(0, 3).join(' ')}`);
  if (planned.length === 1 && driven.length !== planned[0]) {
    problems.push(`the shards drove ${driven.length} visit(s) of the ${planned[0]} the plan has`);
  }
  if (docs.some((doc) => doc.shard && (doc.shard.driven ?? []).length !== doc.visits)) {
    problems.push('a shard drove a different number of visits than it lists');
  }

  const findings = docs.flatMap((doc) => doc.findings ?? [])
    .sort((a, b) => compare(a.route, b.route) || a.width - b.width || compare(a.rule, b.rule));
  const rules = {};
  for (const finding of findings) rules[finding.rule] = (rules[finding.rule] ?? 0) + 1;
  const foreign = [];
  const declared = new Set();
  for (const doc of docs) {
    for (const subtree of doc.foreign ?? []) {
      if (declared.has(subtree.declares)) continue;
      declared.add(subtree.declares);
      foreign.push(subtree);
    }
  }
  const first = docs[0];
  const routes = new Set(driven.map((id) => id.slice(0, id.lastIndexOf('@'))));

  const results = {
    schema: RESULTS_SCHEMA,
    origin: first.origin,
    complete: shards.length === docs.length && shards.every((s) => !s.narrowed),
    source: first.source,
    breakpoints: first.breakpoints,
    viewports: first.viewports,
    surfaces: first.surfaces,
    pages: routes.size,
    visits: docs.reduce((n, doc) => n + doc.visits, 0),
    // the shards ran side by side: the audit took as long as the slowest of them
    seconds: Math.max(...docs.map((doc) => doc.seconds)),
    concurrency: first.concurrency,
    rules: Object.fromEntries(Object.entries(rules).sort((a, b) => b[1] - a[1] || compare(a[0], b[0]))),
    foreign,
    findings,
  };
  return { problems, results };
}
