# Phase 13: Keep mesh, forge and model/provider outages off read hot paths

**Train:** Majordomus P0 CLI + `/ui` + `/cockpit` Performance Rescue  
**Order:** 13 of 18; prerequisites are current repo governance, scope coordination and prior phase evidence.

## Mission and implementation

Audit discovery/mesh peer reads, provider probes, GitHub/forge integration, cache freshness and fallback served snapshots in CLI, `health.report`, UI, Cockpit and server startup. Measure DNS, stale rendezvous seed, NAT/WebRTC attempts and GitHub rate limiting separately only when real paths call them. Do not add remote connections to `env enter`/banner. A local UI must respond with truthful `offline/unknown/stale` information even if every remote peer/provider is down, rather than waiting for remote timeouts. Prioritize incremental event-driven updates over burst polling.

Ensure no upstream failure corrupts local authoritative data or blocks a safe local operation; maintain authentication/signature/protocol validation for mesh commands, no unsigned trust or newly exposed remote endpoints. Bound retries/backoff with jitter without inventing a new orchestrator. Use existing mesh supervisor/lease infrastructure if available. Preserve a fresh overlay of peer claims while caching expensive historical plan computations.

## Proof / exit criteria

Failure-injection tests: DNS failure, TCP connection refusal, stalled remote, GitHub 429/5xx, peer disconnect/reconnect, partial mesh, many worktrees, clock skew; verify `/ui`, `/cockpit` and CLI stay responsive and correctly label availability. Record which external resource caused which latency; no unverified claims of mesh health.

### Universal instructions for this phase

- **Follow `MASTER-PROMPT.md` and the actual repo's `AGENTS.md` / `.ai/**` governance.** This file is one executable stage of the same train; do not create an isolated project or new catalog. Refresh HEAD, mesh board, scopes and recent peer work before any mutation.
- **Inspect before patching.** File paths and issue numbers are leads from the 2026-10-23 review, not proof of what is currently merged. Test the actual checkout and name exact SHA.
- **Measure before/after on comparable samples.** Treat cold, warm, browser and hosted separately; do not invent times, regressions or green test verdicts. No destructive changes or bypassed gates.
- **Leave a reviewable change and evidence** in the canonical task/issue/execution graph; wire tests, docs and generated projections once through the existing mechanism. Capture blockers precisely.

## Required phase handoff

At completion record: starting SHA, ending SHA, touched files, active issue/claim, tests run (exact argv and exit status), baseline evidence path, measured cold/warm deltas, failing/blocked cases, next owner/dependency. If evidence is missing, mark the phase NOT_VERIFIED rather than complete.
