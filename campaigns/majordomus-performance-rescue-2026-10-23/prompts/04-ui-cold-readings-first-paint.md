# Phase 04: Fix cold /ui first reading without lying about freshness

**Train:** Majordomus P0 CLI + `/ui` + `/cockpit` Performance Rescue  
**Order:** 04 of 18; prerequisites are current repo governance, scope coordination and prior phase evidence.

## Mission and implementation

This is a high-priority architectural path. Read `src/capability/readings.rs`, `src/ui/source.rs`, `src/ui/dashboard.rs`, `src/ui/mod.rs`, `src/ui/shell.rs`, `src/live.rs`, `share/ui/ui.js`, `share/cockpit/socket.js`, ADR 0142 (discover exact file path), closed I2486. `Patience::PAGE.cold` was 60 s at the reviewed commit. Verify current value, actual cold sources, 4-worker queue saturation and `ask_all` joins.

Prove whether SSR blocks on first slow source, and which path does it: first request after server start, source expired, workspace change, background `Ui::warm_once` contention, or source queue priority inversion. A cold page must promptly show useful SSR shell with truthful `pending/unknown/stale/failed`, source identity, age and accessible fallback; asynchronous readings should update via existing live transport without polling storms. Preserve no-JS useful content and post-load accessibility; never display missing facts as zero or healthy. Study `MAJORDOMUS_READING_PATIENCE_MS` parsing: numeric overrides currently cannot shorten cold waits, so test that semantics are intentional after repair.

Repair in the single canonical readings/view path with bounded default cold patience and correct pending/updating behavior, not individual panels full of timeouts. Prove an arbitrarily slow or never-resolving capability does not block document response. Control number of warm-up routes, queues, worker starvation, lock holding, response late arrivals, and cancellation. Ensure a pending first reading transitions to current without requiring a browser hard reload, or document the explicit accessible refresh path and its limitations.

## Proof / exit criteria

Add synthetic slow source (e.g. 30-90 s delay in isolated fixture) and fast source; measure document p95, response state, eventual update, no JS fallback, and source attribution. Explicit tests: slow source never answers; source panics; multiple simultaneous cold browsers; server restarts; revalidation after git mutation. Existing `readings` unit tests and UI tests still pass. Avoid just changing 60 to 1 and declaring victory.

### Universal instructions for this phase

- **Follow `MASTER-PROMPT.md` and the actual repo's `AGENTS.md` / `.ai/**` governance.** This file is one executable stage of the same train; do not create an isolated project or new catalog. Refresh HEAD, mesh board, scopes and recent peer work before any mutation.
- **Inspect before patching.** File paths and issue numbers are leads from the 2026-10-23 review, not proof of what is currently merged. Test the actual checkout and name exact SHA.
- **Measure before/after on comparable samples.** Treat cold, warm, browser and hosted separately; do not invent times, regressions or green test verdicts. No destructive changes or bypassed gates.
- **Leave a reviewable change and evidence** in the canonical task/issue/execution graph; wire tests, docs and generated projections once through the existing mechanism. Capture blockers precisely.

## Required phase handoff

At completion record: starting SHA, ending SHA, touched files, active issue/claim, tests run (exact argv and exit status), baseline evidence path, measured cold/warm deltas, failing/blocked cases, next owner/dependency. If evidence is missing, mark the phase NOT_VERIFIED rather than complete.
