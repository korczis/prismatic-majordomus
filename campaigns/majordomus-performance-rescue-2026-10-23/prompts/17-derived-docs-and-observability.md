# Phase 17: Project verified performance truth everywhere it belongs

**Train:** Majordomus P0 CLI + `/ui` + `/cockpit` Performance Rescue  
**Order:** 17 of 18; prerequisites are current repo governance, scope coordination and prior phase evidence.

## Mission and implementation

Update `docs/PERFORMANCE.md`, `docs/MAJORDOMUS_UI.md`, `docs/COCKPIT.md`, relevant `.ai/repo/features`, rules/doctrines and the existing observability surface from actual measured facts. Keep authored goals and observed measurements distinct; an old benchmark in source comments is historical evidence, not today's SLO. Where route-specific perf details are exposed, derive them from canonical request-scoped perf data and existing metrics; do not construct another telemetry domain, handcode route lists, or expose sensitive local paths. Show cold vs warm, stale/pending, p50/p95, sample counts, trace links and slowest stage where supported.

Regenerate docs/openapi/MCP/landing/Pages using canonical repository generator, preserving source and provenance, localizations if present and metadata contracts. Surface actionable 'this route is slow because stage X costs Y' findings with evidence and navigation to existing issue/trace, not vanity charts. Ensure GitHub Pages static snapshots indicate last observed revision and timestamp and never pretend to be a live control-plane session.

## Proof / exit criteria

All changed claims have reproducing tests; generated artifact checks pass, no new duplicated catalog/route definition exists, documentation links resolve and browser view displays measured information with correct freshness and permissions. Record source files, authoritativeness and where numerical baselines live.

### Universal instructions for this phase

- **Follow `MASTER-PROMPT.md` and the actual repo's `AGENTS.md` / `.ai/**` governance.** This file is one executable stage of the same train; do not create an isolated project or new catalog. Refresh HEAD, mesh board, scopes and recent peer work before any mutation.
- **Inspect before patching.** File paths and issue numbers are leads from the 2026-10-23 review, not proof of what is currently merged. Test the actual checkout and name exact SHA.
- **Measure before/after on comparable samples.** Treat cold, warm, browser and hosted separately; do not invent times, regressions or green test verdicts. No destructive changes or bypassed gates.
- **Leave a reviewable change and evidence** in the canonical task/issue/execution graph; wire tests, docs and generated projections once through the existing mechanism. Capture blockers precisely.

## Required phase handoff

At completion record: starting SHA, ending SHA, touched files, active issue/claim, tests run (exact argv and exit status), baseline evidence path, measured cold/warm deltas, failing/blocked cases, next owner/dependency. If evidence is missing, mark the phase NOT_VERIFIED rather than complete.
