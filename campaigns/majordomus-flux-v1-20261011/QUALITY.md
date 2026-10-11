# Acceptance gates and performance proof

## Establish evidence before thresholds
Collect at least cold + warm samples on representative real routes: Overview, Mesh topology, PR list, PR detail, Roadmap, Docs, Evidence, plus large and empty views. Measure: server TTFB, p50/p95 render/query time, response bytes, client LCP/INP/CLS, graph readiness, cache hit ratio, time to first meaningful item, time to first event after subscribe; separately measure commit-to-published site controlled/external durations. Pin environment and report n.

## Initial engineering targets (not claims of present performance)
- Warm server-rendered HTML p95 < 150ms for non-expensive views with data already local/cached.
- Simple fragment update p95 < 100ms server work.
- Pagination p95 < 150ms over indexed data under representative load; avoid unbounded response size.
- No redundant expensive capability executions within a single snapshot.
- Repeat derive of unchanged inputs produces byte-identical outputs and skips rebuilds.
- Browser critical content remains available with JS off; charts render useful tabular fallback.
- UI freshness/reconnect behavior measured and reported rather than guessed.
- Pages controlled publication time respects existing repository objective where feasible; never misclassify GitHub queue delay as local regression.
Targets require actual baseline and test hardware calibration, and must not be turned into impossible blanket blockers without statistically valid measurement.

## Tests
Unit: escaping/XSS, deterministic component output, stable total ordering, cursor boundaries, invalid/expired cursor, invalidation graph closure, cache key isolation, negative cache, ETag and 304, no stale privilege reuse.
Integration: identical canonical source across CLI/API/UI, added entity appears without manual registrations; first request cold then warm then source mutation then cache invalidation; concurrent duplicate requests execute once; atomic generation failure retains old site.
Realtime: reconnect, replay, gap, reordered/duplicate events, expired cursor, dropped subscriber, auth change/revocation, process restart, slow consumers/backpressure, bounded memory. Verify no fake health when disconnected.
E2E: browser navigation and deep links, route redirects, back/forward, accessible pagination, keyboard/focus, mobile 320px and 768px, desktop, SSR no JS, reduced motion, charts fallback.
Delivery: asset hashes, route availability, internal link graph, correct release/source SHA, negative stale Pages deployment, verified canary/rollback when supported.

## CI and .ai enforcement
Use existing .ai/repo/ci/gates.yaml and actual gate runners. Add or extend policy constraints: `projection-single-source`, `render-is-safe`, `cache-has-provenance`, `window-is-deterministic`, `live-stream-is-honest`, `site-freshness-is-verifiable`. Names are provisional; avoid duplicate rules where existing ones apply. Every blocking rule has a named negative test and CI gate, docs/example and evidence of execution. Do not make unrelated full E2E tests mandatory for every trivial template edit; impact analyzer decides required closure conservatively.
