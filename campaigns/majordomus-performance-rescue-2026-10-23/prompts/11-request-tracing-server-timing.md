# Phase 11: Implement I3226 request-scoped causal timings

**Train:** Majordomus P0 CLI + `/ui` + `/cockpit` Performance Rescue  
**Order:** 11 of 18; prerequisites are current repo governance, scope coordination and prior phase evidence.

## Mission and implementation

Read canonical I3226 (GitHub #1564), its original scope/dependencies, `src/perf.rs`, `src/http/router.rs`, `src/http/server.rs`, `src/ui/source.rs`, `src/capability/executor.rs`, `src/capability/builtin/perf.rs`. Existing `perf::COUNTERS` is process-global, making route attribution under concurrent requests invalid. Extend one canonical typed perf vocabulary with a *request-scoped* recorder, started at the HTTP boundary and propagated into execution/sources. Provide request-safe immutable results: scheduling/wait, executor/cache, route handler, SSR/nav, rendering, body bytes and response write; time-to-first-byte can't be inferred from handler-only timers. If response writer cannot attach a full stage in a header already sent, record remaining stages in a correlated structured event/trace with no misleading header.

Emit standards-conformant `Server-Timing` on applicable `/ui`, `/cockpit` and `/api` responses; no user secrets, source code or private paths in header values. Define how streaming/upgraded routes and published snapshots differ. Counters and timing must not cause material overhead or hidden allocations on hot path. Pair server request id with Playwright browser navigation trace where useful, observing redaction and CORS/security. Update benchmark runner to associate budget breach with measured stage; no duplicated instrumentation outside perf framework.

## Proof / exit criteria

Prove concurrent overlapping routes produce independent timing, labels safe and bounded, byte counts correct, no header on snapshot routes if excluded, tests for streamed/upgraded handling and negligible instrumentation overhead (< measured tolerable delta). I3226 should close only after it actually lands with full cross-surface tests, never via a standalone prototype.

### Universal instructions for this phase

- **Follow `MASTER-PROMPT.md` and the actual repo's `AGENTS.md` / `.ai/**` governance.** This file is one executable stage of the same train; do not create an isolated project or new catalog. Refresh HEAD, mesh board, scopes and recent peer work before any mutation.
- **Inspect before patching.** File paths and issue numbers are leads from the 2026-10-23 review, not proof of what is currently merged. Test the actual checkout and name exact SHA.
- **Measure before/after on comparable samples.** Treat cold, warm, browser and hosted separately; do not invent times, regressions or green test verdicts. No destructive changes or bypassed gates.
- **Leave a reviewable change and evidence** in the canonical task/issue/execution graph; wire tests, docs and generated projections once through the existing mechanism. Capture blockers precisely.

## Required phase handoff

At completion record: starting SHA, ending SHA, touched files, active issue/claim, tests run (exact argv and exit status), baseline evidence path, measured cold/warm deltas, failing/blocked cases, next owner/dependency. If evidence is missing, mark the phase NOT_VERIFIED rather than complete.
