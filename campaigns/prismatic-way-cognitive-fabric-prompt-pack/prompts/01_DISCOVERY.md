# WAVE 01 — Discovery, reuse, baseline and integration boundary

Read MASTER and all contracts. This is not implementation-free analysis: conclude with an actionable, smallest-patch plan and begin the first passing vertical test if no blocker exists.

Audit actual implementations of: episode start/stop and briefing, Claude/Codex/Gemini integrations, hooks, event/monitor stream, task check/finish, intents/gaps/critiques, knowledge observation/derive/candidates/curated, evolve skill, reasoning assessment/plan/advisors, mesh journal/fold/claims/reviews, entity addressing, graph dependencies, generated capabilities, REST/MCP/OpenAPI, Cockpit, site publication and telemetry.

For each capability write: exists? canonical source? runtime implementation? code path? tests? known bug? potential reuser? proposed minimal change? Risk? Performance?

Check exact entity schemas and IDs. Evaluate whether a current correlation/event mechanism already covers candidate events; enhance it rather than adding a competing bus. Identify preexisting PRs/branches that may already implement the feature. Do not infer completed runtime behavior from documentation.

Measure baseline: events/second, p50/p95 synchronous hook duration, derive time and cache behavior, intent duplicate ratio in fixture, inference calls per event, size of graph delta, stale projection rate, current test pass count, and baseline usage/cost if instrumentation exists. If unavailable label 'not measured'.

Output `INTEGRATION-MAP` with a minimal vertical slice: one observed prompt → canonical intent match → task link → explicit provenance → same ID exposed in CLI/API/MCP/UI. State exact files to change and corresponding tests. Follow existing worktree/claim/PR process.
