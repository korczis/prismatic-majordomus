# Data / action mapping and runtime contract

1. Inventory existing relevant CLI, `capability!`, HTTP, MCP, OpenAPI and Cockpit implementation, discover producer per field rather than infer schema from a screenshot. Do not automatically create endpoints because a card exists.
2. Build/reuse typed view projections: `ProjectSummary`, `MetricObservation`, `EntityCollection`, `EventPage`, `GraphView`, `TimeSeriesView`, `ExecutionView`, `VerificationView` are conceptual terms, **not demanded Rust type names**. Prefer existing types and serialization shape.
3. Every observation includes: source kind, source identifier, source revision/version where meaningful, observed_at if genuinely observed, availability/error and authorization scope. Runtime-only state cannot be published into static GitHub Pages as live.
4. Counts: explicit population, scope and last refresh. Percent: denominator, numerator and window; refuse dividing by zero. Delta: compare commensurate windows. Durations: distinguish pipeline elapsed, queue wait, computation and deploy verification.
5. Events: monotonic/cursor sequence if supported, dedupe, retry, bounded buffering, no fabricated replay. On reconnect expose last successfully observed revision.
6. Graphs: canonical node/edge IDs, typed relationship, no guessed links. Clearly explain undiscoverable topology; no random sample graph in live page.
7. Mutations: capability discovered → schema-derived form → authorize → typed validate → canonical executor → execution ID → evidence/event stream → terminal verified state. UI receives no unrestricted shell invocation.
8. Missing feature: if no canonical signal, render unsupported/not measured with docs/source pointer. Create separate scope-tracked backend issue only if required for accepted intent; don't create duplicate state in frontend.
9. Tests: same semantic operation via CLI/HTTP/MCP/SSR consistent; negative auth and malformed payload tests; refresh stale and concurrent updates; pagination/filter stable; no secret leakage.

Mock data may exist ONLY inside isolated unit tests/fixtures with clear naming and never be shipped or used as screenshots of active production features.
