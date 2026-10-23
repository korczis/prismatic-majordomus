# Phase 03: Bound shared runtime startup, readiness and lease convergence

**Train:** Majordomus P0 CLI + `/ui` + `/cockpit` Performance Rescue  
**Order:** 03 of 18; prerequisites are current repo governance, scope coordination and prior phase evidence.

## Mission and implementation

Trace `src/commands/serve.rs`, lease timing and probe functions, `src/http/server.rs`, `src/app.rs`, startup `Live`, `src/web/discover.rs` and UI warm policy. Derive startup timeline: executable launch, repo discovery/index, registry/schema, graph, served surfaces, bind, lease publish, first accepted request, first usable response. A health probe must not be a proxy for UI readiness unless the contract explicitly says so. Track stale server replacement, lock contention, shared checkout and linked worktree separation.

Distinguish background initialization from a CLI entry that synchronously waits. Test `converge(..., wait=0)` against no server, running healthy server, server midway through startup, another owner with valid lease, outdated binary, crash and restart. Audit unnecessary busy-grace, repeat polling, lock wait, failure loops, and `watch_file .ai/local/state/mcp/server.json` interactions.

Preserve single owner per checkout, correct graceful shutdown, no orphan work processes, correct lease/version identity, never attach to wrong checkout or accept stale metadata as healthy. Prefer publishing a minimal usable server before optional expensive derived reads, when architecture permits.

## Proof / exit criteria

Add timed lifecycle test with port=0 and reproducible cold startup. Record `started`/`ready`/`serving` with honest timestamps. A fast readiness response must not hide a nonfunctional UI; verify a full navigation under the same start. Report startup stage p50/p95 and exact changed counters.

### Universal instructions for this phase

- **Follow `MASTER-PROMPT.md` and the actual repo's `AGENTS.md` / `.ai/**` governance.** This file is one executable stage of the same train; do not create an isolated project or new catalog. Refresh HEAD, mesh board, scopes and recent peer work before any mutation.
- **Inspect before patching.** File paths and issue numbers are leads from the 2026-10-23 review, not proof of what is currently merged. Test the actual checkout and name exact SHA.
- **Measure before/after on comparable samples.** Treat cold, warm, browser and hosted separately; do not invent times, regressions or green test verdicts. No destructive changes or bypassed gates.
- **Leave a reviewable change and evidence** in the canonical task/issue/execution graph; wire tests, docs and generated projections once through the existing mechanism. Capture blockers precisely.

## Required phase handoff

At completion record: starting SHA, ending SHA, touched files, active issue/claim, tests run (exact argv and exit status), baseline evidence path, measured cold/warm deltas, failing/blocked cases, next owner/dependency. If evidence is missing, mark the phase NOT_VERIFIED rather than complete.
