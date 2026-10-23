# Phase 06: Stop repeated multi-second plan and Git derivations

**Train:** Majordomus P0 CLI + `/ui` + `/cockpit` Performance Rescue  
**Order:** 06 of 18; prerequisites are current repo governance, scope coordination and prior phase evidence.

## Mission and implementation

Review open I3442 and any in-flight PR or mesh claim. Inspect `src/capability/builtin/plan.rs` for `plan.flight`, `plan.delivery`, `src/plan/flight.rs`, `src/plan/delivery.rs`, existing `CachePolicy::Generation` and `CachePolicy::Disabled`, `readings::REVALIDATE=1s`. Reproduce Git-history cost with genuine full repository and synthetic scaled fixtures; measure subprocess count, bytes read, history walk length, CPU, memory, provenance and how many times a page/tab causes it.

Implement I3442's intended *dependency-keyed* caching: immutable delivery inference is cached on the actual trunk commit, refs, relevant plan/release/forge/deployment observations and source versions, with live mesh claims overlaid fresh on each read. Do NOT memoize only by wall-clock TTL or return stale plan transitions as current; ensure commands and mutation operations validate authoritative state and get correct refusal when plans change. Replace repetitive whole-history traversal with a derived compact/indexed projection only when the key and correctness proof support it; reuse existing typed domain components. Deduplicate concurrent identical misses with bounded single-flight, and invalidate efficiently.

Investigate similarly expensive `health.report`, integration.queue, roadmap, issue lists, delivery/gates, `distribution.releases`, `release.version`; do not optimize ones without an observed contribution to user-facing latency. Surface slow expensive work as lazy panels when necessary, preserving provenance and error semantics.

## Proof / exit criteria

Produce before/after benchmark on identical trunk states, 1x/10x/100x history records and rapid commit churn. Assert a warm view does NOT re-run Git history every second; a changed plan/Git ref invalidates precisely; mesh overlay changes don't force full recalculation. Include correctness/property tests against original exact computation and real UI first paint.

### Universal instructions for this phase

- **Follow `MASTER-PROMPT.md` and the actual repo's `AGENTS.md` / `.ai/**` governance.** This file is one executable stage of the same train; do not create an isolated project or new catalog. Refresh HEAD, mesh board, scopes and recent peer work before any mutation.
- **Inspect before patching.** File paths and issue numbers are leads from the 2026-10-23 review, not proof of what is currently merged. Test the actual checkout and name exact SHA.
- **Measure before/after on comparable samples.** Treat cold, warm, browser and hosted separately; do not invent times, regressions or green test verdicts. No destructive changes or bypassed gates.
- **Leave a reviewable change and evidence** in the canonical task/issue/execution graph; wire tests, docs and generated projections once through the existing mechanism. Capture blockers precisely.

## Required phase handoff

At completion record: starting SHA, ending SHA, touched files, active issue/claim, tests run (exact argv and exit status), baseline evidence path, measured cold/warm deltas, failing/blocked cases, next owner/dependency. If evidence is missing, mark the phase NOT_VERIFIED rather than complete.
