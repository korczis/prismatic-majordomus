# Claude Opus/Fable — MASTER EXECUTION MISSION: MAJORDOMUS FLUX

You are modifying the current live `korczis/prismatic-majordomus` repository, including its current open PRs, existing .ai rules, gates, ADRs, deploy pipeline and the running UI if locally accessible. **Do not assume this archive describes the latest implementation.**

The operator's intent: pages are too slow, derive and delivery cycles are too slow; implement a simple, performant, derived-friendly unified mechanism for component templating, caching, streaming and pagination. The subsystem name is **Majordomus Flux**. It MUST accelerate delivering correctly linked, real-data, generated/derived/inferred features to Cockpit, `majordomus ui`, and static documentation/Pages.

## Nonnegotiable execution contract
- Read AGENTS.md, actual .ai/** canonical rules, docs/CI.md, docs/GITHUB_PAGES_PERFORMANCE.md, docs/GITHUB_PAGES_ARCHITECTURE.md, docs/COCKPIT.md, ADRs and code. Discover paths and do not assert obsolete paths exist.
- Inspect actual GitHub PRs and worktrees; reconcile related work. Never blindly open duplicate milestones/issues.
- Use Majordomus canonical project records for intent -> milestones -> issues -> acceptance -> evidence. Use existing mesh where supported, within authorized capabilities. Never falsely claim remote execution or autonomous background support.
- No second API, orchestrator, cache authority, registry, navigation database or template catalog. Preserve existing `Context::execute` and capability semantics. UI renders projections, it does not invent data.
- Implement first useful vertical slice immediately, then measure and iterate. Every slice: code -> focused tests -> derive/check -> docs -> gate -> merge where authorized -> version analysis -> release where eligible -> deployment verification -> continue.
- Use typed DTOs and schemas, controlled escaping, strong identity/provenance, deterministic ordering and predictable cache invalidation. SSR and graceful no-JS must remain.
- No mocks/stubs/placeholder telemetry in production. Fixtures are permitted **only in tests** and are explicitly labeled.
- No blanket reduction of required CI correctness. Speed it via dependency impact, artifact reuse and validated result reuse.
- No full-repo rederive for a one-document change if the actual dependency closure proves a smaller derivation suffices.
- No arbitrary positive cache lifetime for authorization-sensitive or mutable state without invalidation semantics.
- Security: no secrets to browser, cross-repo isolation, permission-check before actions and per-user private data, cache keys include authorization scope where required, bounded connections and payload size, no XSS / template injection.
- Continue through milestones without routine confirmation. Stop only at genuine external blockers or unauthorized actions. Report exact checks, PRs, SHAs, release/deploy URLs as evidence, never fabricate them.

## Required work and handoffs
1. Baseline actual slow routes and data queries. Separate dynamic UI latency, static site load, and publication latency.
2. Map existing renderer, per-request capability calls, existing caches, pagination helpers and execution event infrastructure.
3. Introduce smallest useful typed Flux projection/render boundary; adapt existing html builder, don't rewrite it casually.
4. Content-addressed dependency fingerprinting and incremental derivation; atomic publication; reasoned invalidation.
5. Normalized pagination/filter/sort contracts (offset vs cursor decided by data behavior), stable selection and links.
6. Snapshot + live transport strategy: reuse existing WebSocket event channel where applicable, include cursor/replay/resync; do not manufacture real-time events for static facts.
7. Integrate components across `/ui`, Cockpit, HTTP/OpenAPI, docs and Pages as relevant; no duplicate inventories.
8. Performance, failure and cross-surface parity tests, .ai policy/gates, docs, canary, rollout, rollback.

## Report for every issue
Canonical issue ID; status; changed paths; before and after measurements; cache key/invalidation proof; tests executed with results; screenshot/route proof where relevant; evidence links; integrated commit/PR; blocked items and exact reasons.

Read `ARCHITECTURE.md`, `QUALITY.md`, `MIGRATION.md`, and each milestone prompt in dependency order. BEGIN NOW.
