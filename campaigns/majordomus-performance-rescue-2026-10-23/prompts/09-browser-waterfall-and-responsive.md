# Phase 09: Measure and repair real browser experience

**Train:** Majordomus P0 CLI + `/ui` + `/cockpit` Performance Rescue  
**Order:** 09 of 18; prerequisites are current repo governance, scope coordination and prior phase evidence.

## Mission and implementation

Use canonical `scripts/ui audit`, `scripts/lib/browser.mjs`, `scripts/lib/ui-routes.mjs`, `scripts/lib/ui-observatory.mjs`, `scripts/lib/cockpit-probe.mjs`, actual existing test scripts and `share/ui`/`share/cockpit` assets. Discover page URLs by the built surface, not a second handwritten inventory. Instrument Navigation Timing, Paint Timing, LCP, long tasks, total blocking, DOM count, response sizes, CSS/JS download and layout/paint, service-worker/cache behavior if present, WS status, console warnings and unresolved Promise rejection. Prefer real browser traces, screenshots only as secondary evidence.

Test real cold browser context versus warm same-context navigation; desktop narrow/mobile widths, normal/reduced-motion, keyboard and no-JS. Look at asset cache version/digests, multiple style/script loads, unnecessary 1.2 MB JSON/HTML, layout thrashing, chart/graph load only on pages that need it, text/graph fallback, navigation prefetch I3227 only if measured and browser data permits. Avoid frontend abstraction churn; keep CSP and progressive enhancement. The goal is useful time-to-interaction, not a rapidly returned empty document while scripts hang.

## Proof / exit criteria

Record per-route browser waterfall/trace artifacts, before/after navigation and input responsiveness, console/network errors, empty/loaded/stale semantics, keyboard operation and 2+ viewport screenshots. Test a cold user session and navigation sequence through `/ui`, `/cockpit`, `/ui/roadmap`, `/cockpit/graphs/composed` if discoverable. CI browser unavailable => explicit BLOCKED/SKIP, not PASS.

### Universal instructions for this phase

- **Follow `MASTER-PROMPT.md` and the actual repo's `AGENTS.md` / `.ai/**` governance.** This file is one executable stage of the same train; do not create an isolated project or new catalog. Refresh HEAD, mesh board, scopes and recent peer work before any mutation.
- **Inspect before patching.** File paths and issue numbers are leads from the 2026-10-23 review, not proof of what is currently merged. Test the actual checkout and name exact SHA.
- **Measure before/after on comparable samples.** Treat cold, warm, browser and hosted separately; do not invent times, regressions or green test verdicts. No destructive changes or bypassed gates.
- **Leave a reviewable change and evidence** in the canonical task/issue/execution graph; wire tests, docs and generated projections once through the existing mechanism. Capture blockers precisely.

## Required phase handoff

At completion record: starting SHA, ending SHA, touched files, active issue/claim, tests run (exact argv and exit status), baseline evidence path, measured cold/warm deltas, failing/blocked cases, next owner/dependency. If evidence is missing, mark the phase NOT_VERIFIED rather than complete.
