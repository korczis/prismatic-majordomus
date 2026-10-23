# Phase 16: Prove cold, churn, load and failure resilience

**Train:** Majordomus P0 CLI + `/ui` + `/cockpit` Performance Rescue  
**Order:** 16 of 18; prerequisites are current repo governance, scope coordination and prior phase evidence.

## Mission and implementation

Design adversarial tests using existing fixture/synthetic architecture: zero prior cached readings, slow never-completing capability, 1/10/100x indexed objects/history, rapid Git changes, 20+ concurrent browsers, long SSE/WS, repeated stop/start, malformed request, huge reply, network unavailable, an executor panic, stale process lease and optional 20-60 min soak. Use stable tests and bounded timeouts, record environment. Distinguish cold no-server (startup) from cold first-reading (running). Verify CPU, RSS, thread and fd growth across steady load.

Determine whether `Ui::warm_once` can swamp 4 readings workers; test starvation and fairness. Verify cache capacity/backpressure, clear cancellation, no work lost by stale generation, correct exact-command behavior, stream reconnect semantics and no secrets in traces. A refused 503 under overload is legitimate if bounded, but should be timely, correctly labeled and not block status/admin operations. No flaky wall-clock timings as unit tests in oversubscribed public runners: use fake clock/synchronization for structural tests and separately profile on controlled host.

## Proof / exit criteria

Maintain a chaos matrix of expected, observed and evidence; record crashes, leaks, blocked conditions and fixed regressions. No indefinite hang in tests; no skipped tests described as green. Scalability analysis should include slope/big-O behavior and a work cap, not just 'passed once'.

### Universal instructions for this phase

- **Follow `MASTER-PROMPT.md` and the actual repo's `AGENTS.md` / `.ai/**` governance.** This file is one executable stage of the same train; do not create an isolated project or new catalog. Refresh HEAD, mesh board, scopes and recent peer work before any mutation.
- **Inspect before patching.** File paths and issue numbers are leads from the 2026-10-23 review, not proof of what is currently merged. Test the actual checkout and name exact SHA.
- **Measure before/after on comparable samples.** Treat cold, warm, browser and hosted separately; do not invent times, regressions or green test verdicts. No destructive changes or bypassed gates.
- **Leave a reviewable change and evidence** in the canonical task/issue/execution graph; wire tests, docs and generated projections once through the existing mechanism. Capture blockers precisely.

## Required phase handoff

At completion record: starting SHA, ending SHA, touched files, active issue/claim, tests run (exact argv and exit status), baseline evidence path, measured cold/warm deltas, failing/blocked cases, next owner/dependency. If evidence is missing, mark the phase NOT_VERIFIED rather than complete.
