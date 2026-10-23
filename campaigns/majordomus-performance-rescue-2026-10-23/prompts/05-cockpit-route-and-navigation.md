# Phase 05: Remove expensive /cockpit SSR, nav and overview work

**Train:** Majordomus P0 CLI + `/ui` + `/cockpit` Performance Rescue  
**Order:** 05 of 18; prerequisites are current repo governance, scope coordination and prior phase evidence.

## Mission and implementation

Inspect `src/cockpit/{mod.rs,pages.rs,nav.rs,view.rs,assets.rs}` and `src/http/router.rs`, `src/live.rs`. SSR `Cockpit::handle` obtains a Live context then builds page and navigation for each request. Find whether `Live::current()` waits for rebuild, route computations derive expensive graphs, `nav::build` performs redundant `ctx.execute`, overview reads slow providers, `capabilities.describe` runs once per row, or HTML tree grows with whole registries. Measure by route, HTML bytes, per-render allocations, mutex waits, graph sizes and browser DOM long tasks. Compare with the existing Cockpit overview memo PR/issue; inspect and reuse, never cache by a global static key that crosses generations/repositories.

Preserve the 'Cockpit is a projection, not an application' ADR and HTML-complete/no-JS promise. Use canonical stable `Live` generation and memoized typed projection keyed by generation, page identifier, parameters and dependencies; lightweight bounded pagination/virtualization only for proven expensive lists. Do not introduce frontend hardcoded routes/categories; maintain native SSR XSS escaping, CSP and source links. Read existing route-discovery and URL contract tests so no page silently disappears.

## Proof / exit criteria

Test real socket all `STATIC_ROUTES` plus query pages; expensive graphs and long tables; case with large synthetic registry. Compare bytes/CPU/time and verify `src/cockpit/assets.rs` immutable digest caching still valid. Confirm before/after both page TTFB and browser usable time. No SSR blank shell substituted merely to meet a number.

### Universal instructions for this phase

- **Follow `MASTER-PROMPT.md` and the actual repo's `AGENTS.md` / `.ai/**` governance.** This file is one executable stage of the same train; do not create an isolated project or new catalog. Refresh HEAD, mesh board, scopes and recent peer work before any mutation.
- **Inspect before patching.** File paths and issue numbers are leads from the 2026-10-23 review, not proof of what is currently merged. Test the actual checkout and name exact SHA.
- **Measure before/after on comparable samples.** Treat cold, warm, browser and hosted separately; do not invent times, regressions or green test verdicts. No destructive changes or bypassed gates.
- **Leave a reviewable change and evidence** in the canonical task/issue/execution graph; wire tests, docs and generated projections once through the existing mechanism. Capture blockers precisely.

## Required phase handoff

At completion record: starting SHA, ending SHA, touched files, active issue/claim, tests run (exact argv and exit status), baseline evidence path, measured cold/warm deltas, failing/blocked cases, next owner/dependency. If evidence is missing, mark the phase NOT_VERIFIED rather than complete.
