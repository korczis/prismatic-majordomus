# Phase 10: Make caching correct, typed, bounded and observable

**Train:** Majordomus P0 CLI + `/ui` + `/cockpit` Performance Rescue  
**Order:** 10 of 18; prerequisites are current repo governance, scope coordination and prior phase evidence.

## Mission and implementation

Inventory existing `CachePolicy::{Process,Generation,Disabled}`, executor cache and `readings` slots (`REVALIDATE`, `REMEMBERED`, `MAX_SOURCES`), UI/Cockpit memo (`Live::Memo`), static asset digest cache, environment snapshots and bridge cache. For every proposed cache or optimization, identify a canonical dependency set, stable key, version/schema, age, max entries/bytes, eviction, concurrency single-flight, invalidation and what an exact read means. If cache policies differ by capability side effects, do not conflate; cached mutation is forbidden.

Prove request results against the uncached authoritative handler with randomized inputs, rapid git mutation, branch switch and worktree identities; stale and pending must remain explicit to humans, but commands/mutations may refuse. Keep local/remote trust boundaries, avoid storing secrets/headers, don't serialize ephemeral socket/proc state as permanently valid. Prefer performance improvements from avoiding computation (constant-time projection lookup, incremental derivation) over unbounded memory.

## Proof / exit criteria

Add cache-hit/miss/eviction and correctness tests, no cross-repo/worktree leakage, no memory growth in 20/60 minute soak, no duplicate CPU storm on invalidation, no accidental 200/healthy with out-of-date data. Provide cache inventory with observed benefit and exact baseline before/after. Delete ineffective redundant caches rather than layering more.

### Universal instructions for this phase

- **Follow `MASTER-PROMPT.md` and the actual repo's `AGENTS.md` / `.ai/**` governance.** This file is one executable stage of the same train; do not create an isolated project or new catalog. Refresh HEAD, mesh board, scopes and recent peer work before any mutation.
- **Inspect before patching.** File paths and issue numbers are leads from the 2026-10-23 review, not proof of what is currently merged. Test the actual checkout and name exact SHA.
- **Measure before/after on comparable samples.** Treat cold, warm, browser and hosted separately; do not invent times, regressions or green test verdicts. No destructive changes or bypassed gates.
- **Leave a reviewable change and evidence** in the canonical task/issue/execution graph; wire tests, docs and generated projections once through the existing mechanism. Capture blockers precisely.

## Required phase handoff

At completion record: starting SHA, ending SHA, touched files, active issue/claim, tests run (exact argv and exit status), baseline evidence path, measured cold/warm deltas, failing/blocked cases, next owner/dependency. If evidence is missing, mark the phase NOT_VERIFIED rather than complete.
