# MAJORDOMUS UI — BLAZING FAST DERIVED PROJECTION RUNTIME
## Claude Code Opus/Fable autonomous master prompt

You operate inside the real `korczis/prismatic-majordomus` checkout. Execute this mission, do not merely propose it. The goal is a blazing-fast, URL-native, fully derived/inferred, server-rendered, cache-coherent Majordomus UI, tightly integrated with existing CLI/API/OpenAPI/MCP/mesh/intents/tasks/issues/sessions/cockpit and deployed through the repository's actual process.

**Instruction priority:** read `AGENTS.md`, `.ai/**`, applicable schemas/rules/doctrines, ADRs, and project CI/release policy first. Follow real repository conventions. Never claim something is implemented or deployed without evidence. Preserve unrelated worktrees and uncommitted changes. No destructive actions or hidden bypasses. Do not ask for implementation trivia; decide based on repository evidence. Only stop for genuine credential/access blockers or material destructive actions requiring authorization.

## Non-negotiable invariants
- One canonical domain model. UI, CLI, REST/OpenAPI, MCP, docs, and cockpit are derived projections, not independently maintained inventories.
- Every meaningful entity has a stable typed canonical URL, reverse URL generation, breadcrumbs and relationship links; deep links, reload, browser back/forward and query-state work.
- No synchronous full repo traversal, GitHub API request, build, provider query, or unbounded graph construction in common page handlers.
- Templates remain presentation-only; handlers remain thin; projections own batching/indexing and data acquisition.
- Cache correctness is not optional: typed/versioned keys, dependency-aware invalidation, bounds, authorization scoping, stampede protection, live-event invalidation and explainability.
- Progressive enhancement: meaningful HTML without JS; tiny client state for interaction; lazy Cytoscape/ECharts as appropriate; Flowbite/Alpine only where justified.
- Use existing proven libraries and infrastructure before introducing new ones. Prefer Axum + Tokio + Tower/tower-http + Askama + Moka IF an evidence-backed repo audit says adoption is sensible; do not gratuitously replace a strong existing stack.
- Protect mobile UX, accessibility, keyboard navigation, CSP/security, and large-data scaling.
- Delete obsolete duplicate implementations after safe migration, preserve contracts, and enforce standards via executable tests/gates.

## Execution loop
1. Inspect current repo, actual versions and commit, structure, live routes, .ai constraints, tests, deployment, worktrees. Enumerate exact affected modules. Do not assume any feature exists.
2. Capture reproducible baseline: process startup, request p50/p95/p99, route-level TTFB, projection time, render time, HTML/JS/CSS bytes, browser request waterfall, counts of fs/git/DB calls, cache misses, allocations, mesh node/edge counts. Cover /ui, /ui/mesh, main issues/intents/tasks/session pages if present; note unsupported cases.
3. Build request/data flow map and rank measured bottlenecks. Choose minimal architectural intervention. Reject broad rewrites or ceremonial abstractions.
4. Implement narrow vertical slices, always compiling and testing each slice; first make /ui and /ui/mesh measurably fast, then linked entity routes and large collections. Keep existing endpoints working.
5. Establish typed route graph and reverse URLs, request-local memoization, canonical generation-aware snapshot cache, bounded page/fragment/projection caches, single-flight loading, specific invalidation, safe stale-while-revalidate, precompute low-cost summaries.
6. Implement server-side compiled views/streamed optional regions where appropriate; conditional HTTP headers, asset fingerprinting, compression where measured helpful; purposeful route-prefetch on hover/focus/idle with save-data/concurrency caps. Never prefetch secrets or mutations.
7. Implement bounded pagination/cursors and graph neighborhoods. Mesh page initially renders useful semantic summary and limited topology; expand on demand. Lazy load visualization code. Separate events (task logs, peers, sessions) from durable model; use SSE/WS where currently appropriate, resumable replay/backpressure where needed, invalidate only dependencies, avoid full-page refresh.
8. Instrument tracing/metrics and Server-Timing (dev), cache diagnostics, route introspection, slow-query/request tracing, stale/fresh indicators. Keep diagnostic data sensitive-safe.
9. Add deterministic unit/integration/concurrency/property/e2e tests; run in realistic fixtures including changed repo, concurrent requests, auth boundaries, large datasets, reconnection, back/forward, mobile, failure conditions and rapid nav. Add non-flaky performance regression checks.
10. Update canonical `.ai` rules/doctrines, machine enforcement, ADR when policy requires, docs and docs/site projections. Dogfood `majordomus ui` itself.
11. Complete the repository-approved version-bump, release, merge/deploy process, verify actual running service and site, check links and cache headers. If blocked, show precise evidence, do not fake completion. Iterate fix → validate → deliver while safe.

## Performance budgets (targets; measure; do not substitute uncalibrated assertions)
On stated reference hardware with a warmed service and no repository changes:
- p95 cached router/handler response <10 ms; p95 warm API projection <20 ms; p95 common uncached server TTFB <50 ms.
- p95 warm local navigation server response <30 ms, perceived navigation target <100 ms.
- First useful local paint target <300 ms under documented reference environment.
- Repo-enter banner budgets are separate, and no UI optimization may worsen them.
- Cold initial parse/index and large graph paths receive separately published realistic budgets.
Compare baseline to new result, include variance and p50/p95/p99; CI gates must be robust to runner noise, favor deterministic invariants and stable ratio checks.

## Core cache design requirements
- L0 request-local memoization; L1 canonical indexed repository/runtime snapshot; L2 typed page/projection cache; L3 safe render fragment/page cache; L4 conditional browser/static asset caching.
- Version each relevant generation and hash/view schema; compose cache keys from route + normalized parameters + relevant domain generations + projection/template version + visibility/locale context where applicable.
- Source change watcher/event ingestion with debounce, coalescing and bounded refresh work; no O(repo) hashing per request. Race-free invalidation with generation recheck after computation, no stale overwrite. Test git checkout/rebase bursts.
- Single-flight cache fill under 100 competing requests. Bound memory and queue lengths. Cache failure/backoff semantics. Do not cache secrets globally. Explain hits/misses/eviction/invalidation.
- SWR only when semantics permit; fresh execution/progress state must not be misleading. Clearly label stale and avoid stale action authorization.

## Typed router / URL policy
A single canonical router metadata source should power route registration, reverse link generation, breadcrumbs, context navigation, prefetch and route docs. Preserve existing URLs with tested redirects where needed. URL query parameters encode pagination, selected entity, filters, tabs and sorting. Normalize/canonicalize safely, escape IDs, validate access and disallow unintended cross-project routes. Do not create a second SPA router.

## Live mesh / interaction policy
Use repository-supported transports. Show partial meaningful HTML instantly; fetch bounded viewport graph, neighbors or node details on demand; link each node to actual canonical peer/task/session/issue evidence. Stream small typed events or deltas with sequence IDs, ordering, reconnect and backpressure; avoid blasting full topology repeatedly. Expose affordances for actions only from canonical capabilities and existing command/task execution surfaces, never copy business logic into JS.

## Test and proof matrix
- Identical generations produce byte-equivalent deterministic route projection and cache-hit vs miss output.
- Relevant source changes invalidate exactly affected projection families; irrelevant changes remain hot.
- Concurrent cache misses compute once; failure does not poison forever; cache keys segregate authorization contexts.
- ETags/304, static immutable caching, no-cache for private/live endpoints, CSP and template escaping.
- Every canonical deep link works without JS; back/forward/focus/keyboard and mobile drawer work.
- Large issue/task lists bounded; 10k-node fixture does not initialize 10k browser nodes by default.
- Slow/offline providers do not block core page; errors are typed and observable.
- One new canonical entity automatically appears in relevant UI/nav/link surfaces without manual consumer registration.
- No duplicate static route catalogs, unbounded polling, per-request repository scans or N+1 loops remain in migrated paths.
- Before/after table with real p50/p95/p99, peak memory, payload size, count of repo reads, cache hit rate.

## Delivery report (mandatory)
1. inspected commit/branch and actual architecture; 2. baseline evidence and root causes; 3. implemented verticals with files and tests; 4. selected Rust libraries with rationale and rejected alternatives; 5. route/cache/dependency/event design; 6. end-to-end UX evidence; 7. measured before/after results including hardware and commands; 8. enforcement and docs proof; 9. commit/PR/version/release/deploy URLs and verification; 10. honest remaining blockers/debt. Never report a success for a step not independently verified.

**Do the work now. No architecture-only stopping point. Optimize latency and correctness first, then polish.**
