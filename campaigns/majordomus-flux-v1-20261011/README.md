# Majordomus Flux V1 — Derived Delivery Engine

**Mission:** dramatically reduce route time-to-first-byte, total interactive readiness, generation time, and time from merged source change to correctly linked public surface. Preserve Majordomus canonical data, security, provenance and cross-surface consistency.

**Execution order:** `MASTER.md`, then numbered prompts. Each issue is independently trackable and must be reconciled with existing canonical project model, never manually duplicated. Execute in small verified vertical slices, release/deploy whenever authorized and eligible; do not wait for the entire program.

**Important:** Sources from GitHub observed on 2026-10-11: `apps/majordomus-cli/src/cockpit/html.rs` has a typed safe HTML builder; `cockpit/pages.rs` invokes `Context::execute`; `web/serve.rs` caches static surface content; `docs/GITHUB_PAGES_PERFORMANCE.md` already defines publication latency and performance evidence. This pack is a proposal, not a statement that Flux has already been implemented.

## Principles
1. One canonical source and typed projection contract; no second data model.
2. Preserve server-rendered content and no-JS functionality.
3. Cache only validated canonical outputs; no unproven cross-identity or cross-repo reuse.
4. Incremental derivation and conditional HTTP responses, not inaccurate stale freshness claims.
5. Streaming is an enhancement, not mandatory for first render.
6. Honest `unknown`, `unavailable`, `stale`, `unauthorized` states.
7. Explicit measurable outcome, timings split by CPU/render/query/queue/network and external platform scheduling.
8. Reuse existing modules and ADRs; rename or migrate old implementation cleanly rather than maintaining parallel engines.

## Pack
- MASTER.md: autonomous implementation contract
- ARCHITECTURE.md: typed boundaries
- QUALITY.md: performance, safety and acceptance
- MIGRATION.md: .ai, docs, rollout and compatibility
- TRACKING.md and manifest.json: milestone/issue tracking
- prompts/00...07: granular milestones
