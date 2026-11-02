# 04 — FAST MOBILE-FIRST DERIVED UI, NOT ANOTHER APPLICATION

Expose `majordomus ui` through the actual CLI entrypoint and existing server lifecycle. Reuse Cockpit routes, templating, CSS asset build, UI rules and current JS libraries. Preserve low-level Cockpit for engineers, but supply a **high-level intent-first workspace**. Do not add a new SPA, rewrite the Rust web server, replace existing router, invent a second authoritative frontend store or duplicate registries.

Information architecture, derived from capabilities/object kinds and their relations rather than hardcoded catalog arrays:

- **Overview:** current repo, canonical environment, active intent, blockers, ready work, sessions/peers, PR and deployment health.
- **Intents:** list/search; detail; criterion/evidence matrix; gap/critique; plan; create/edit actions; explain provenance.
- **Work:** milestone/issue DAG, dependency waves, assignment, prioritized queue, worktrees, link to commits and test evidence.
- **Executions:** active/completed runs, timelines, logs, outputs, metrics, cancel/inspect actions, replay and diagnostics.
- **Mesh:** peer graph, node detail, liveness, capabilities, current scopes/sessions, communication and safe typed actions.
- **Delivery:** PR queue, checks, merge readiness, version, release, deploy rollout and independent verification.
- **Capabilities:** discoverable search and inspector, schema-generated safe forms, invocation history and direct links to CLI/MCP/OpenAPI/docs.

Require consistent stable URL and deep-link semantics; browser back/forward; keyboard navigation; breadcrumbs; relationship references; contextual actions; object permalinks; canonical ordering; pagination; search/filter; empty/loading/error/partial/stale states. Where practical derive route links from typed relationship metadata and existing route registry. Every actionable UI control must call a typed server capability, display authorization/preflight diagnostics and show execution outcome; no fake controls.

Mobile-first acceptance: narrow viewport (~320px), tablets, desktop; accessible responsive nav and functioning drawers; no obscured primary actions; touch hit targets; readable code/logs with horizontal scrolling inside panes; responsive graph/table alternatives; focus trap and Escape dismissal; screen-reader labels; keyboard-only execution; reduced-motion; high contrast; semantic error messages. Prefer progressive enhancement over hydration-heavy architecture. No XSS via logs, Markdown or model output. Integrate existing Flowbite/Alpine/Cytoscape/ECharts only if already approved and useful; avoid dependency accretion.

Performance: establish baseline with representative datasets and cold/warm caches. Targets (measure, don't fabricate): warm navigation p95 <100ms, cheap local cached reads p95 <50ms, command acknowledgment p95 <200ms, locally delivered live events p95 <250ms. These are proposed targets, to be tuned only with recorded measurements. Server-side paginate/filter, incremental rendering, indexed retrieval, efficient template/cache invalidation and bounded graphs/logs. Ensure cache keys include repo/HEAD/context/permissions/snapshot revision. Do not cache a privileged action result as public. Validate correct invalidation after mutation, event, HEAD switch and stale evidence.

Derivation acceptance: add a test fixture capability and canonical linked object; after generation it becomes discoverable in capability search, action form, relevant navigation/details, API/MCP/CLI and docs (as supported by actual contracts) without individual frontend registrations. Screenshot and browser E2E tests across three viewport sizes, plus no-JS read behavior where current conventions expect it.
