# Phase 15: Validate exact deployed bits on hosted dynamic and static surfaces

**Train:** Majordomus P0 CLI + `/ui` + `/cockpit` Performance Rescue  
**Order:** 15 of 18; prerequisites are current repo governance, scope coordination and prior phase evidence.

## Mission and implementation

Discover actual deployment topology from `.ai/repo/deployments/**`, web surface registry, GigaLixir config and GH Pages workflows; do not invent dynamic routes on GitHub Pages or imply Pages hosts WebSockets. Identify actual live origin(s) and expected URLs from canonical deployment records. Use repository-approved CI/release/deploy workflows only with existing authorization. Verify commit/release identity and signed or documented artifact provenance where available, proper stale/active indicators, headers and asset digest, redirect behavior and portable base paths.

Run the same browser session matrix against local runtime, GigaLixir/hosted live endpoint and built GH Pages snapshot. Verify `/ui`, `/cockpit`, key navigation, JS/CSS, CSP, cache headers, console errors, HTTP APIs/OpenAPI/MCP only where declared, websocket/event channels only where served live, and responsive viewports. Static Pages must truthfully say snapshot/revision/freshness and link to live control plane where authorized. Verify real user-perceived navigation and update time, not just HTTP 200. A 200 from an old revision is not a deployed current result.

## Proof / exit criteria

Deliver served SHA/revision, deployed URL provenance, observed header/response/body/browser evidence, p50/p95 per surface where enough samples, deployment pipeline link and clear distinction local vs hosted vs static. If credentials are absent/unauthorized, report BLOCKED precisely with reproducible local evidence; never claim release finished.

### Universal instructions for this phase

- **Follow `MASTER-PROMPT.md` and the actual repo's `AGENTS.md` / `.ai/**` governance.** This file is one executable stage of the same train; do not create an isolated project or new catalog. Refresh HEAD, mesh board, scopes and recent peer work before any mutation.
- **Inspect before patching.** File paths and issue numbers are leads from the 2026-10-23 review, not proof of what is currently merged. Test the actual checkout and name exact SHA.
- **Measure before/after on comparable samples.** Treat cold, warm, browser and hosted separately; do not invent times, regressions or green test verdicts. No destructive changes or bypassed gates.
- **Leave a reviewable change and evidence** in the canonical task/issue/execution graph; wire tests, docs and generated projections once through the existing mechanism. Capture blockers precisely.

## Required phase handoff

At completion record: starting SHA, ending SHA, touched files, active issue/claim, tests run (exact argv and exit status), baseline evidence path, measured cold/warm deltas, failing/blocked cases, next owner/dependency. If evidence is missing, mark the phase NOT_VERIFIED rather than complete.
